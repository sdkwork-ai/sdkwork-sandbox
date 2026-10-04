//! The bounded single-controller pool registry.
//!
//! [`BoundedSandboxPoolControl`] implements the eight contract operations of
//! `specs/sandbox-runtime-pool.contract.json` (`operations`) over bounded
//! in-process registries. It is the control-plane slice authorized by
//! `REQ-2026-0019`: the provider-neutral state machine, fenced idempotent
//! claims and bounded registries. It is **not** the cloud claim authority —
//! that is PostgreSQL
//! (`persistenceConcurrencyAndRecovery.sandbox_cloud_authoritativeStore`),
//! and process-local memory must never act as that authority
//! (`.sandbox_processLocalMemoryAuthorityAllowed` is false). Composition and
//! tests use this registry; the durable authority will re-implement the same
//! transitions over `REQ-2026-0018` persistence with the database clock.
//!
//! Every operation is fail-closed: validation happens before mutation, stale
//! fencing tokens are rejected before any side effect
//! (`claim.staleFencingRejectedBeforeSideEffect`), capacity is never
//! overcommitted, and uncertainty lands in `quarantined` — a time-to-live
//! alone can never return a slot to `ready`
//! (`releaseAndSanitization.sandbox_ttl_aloneMayReturnSlotToReady` is false).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bounds::{
    SANDBOX_POOL_CANDIDATE_SLOT_COUNT_MAX, SANDBOX_POOL_CLAIM_ATTEMPT_COUNT_MAX,
    SANDBOX_POOL_CLEANUP_DEADLINE_SECONDS_MAX, SANDBOX_POOL_EXHAUSTED_RETRY_AFTER_SECONDS,
    SANDBOX_POOL_PER_PROFILE_TARGET_MAX, SANDBOX_POOL_RECONCILIATION_BATCH_SIZE_MAX,
    SANDBOX_POOL_REFILL_OPERATIONS_PER_NODE_MAX,
};
use crate::claim::{SandboxPoolClaim, SandboxPoolClaimRequest};
use crate::error::{
    SandboxPoolClaimConflictKind, SandboxPoolDependency, SandboxPoolQuarantineReason,
    SandboxPoolRetryAfter, SandboxRuntimePoolError, SandboxRuntimePoolResult,
};
use crate::fencing::SandboxPoolFencingToken;
use crate::identity::{
    SandboxPoolClaimId, SandboxPoolOperationId, SandboxPoolSlotId, SandboxResourceProfileId,
};
use crate::port::{
    SandboxPoolCapacityTarget, SandboxPoolPreparationRequest, SandboxPoolQuarantineRequest,
    SandboxPoolReadyRequest, SandboxPoolRefillAction, SandboxPoolReleaseOutcome,
    SandboxPoolReleaseRequest, SandboxPoolRetireRequest, SandboxPoolSlotReconciliationReport,
    SandboxPoolSlotReconciliationRequest, SandboxPoolTargetReconciliationReport,
    SandboxRuntimePoolPort,
};
use crate::slot::{SandboxPoolClass, SandboxPoolSlot};
use crate::state::{SandboxPoolClaimState, SandboxPoolSlotState};

/// Whole-seconds clock the control plane reads for every timestamp.
///
/// The default is the host system clock for local composition and tests; the
/// cloud authority must inject the database clock
/// (`persistenceConcurrencyAndRecovery.sandbox_databaseClockRequired`) at the
/// durable slice.
pub type SandboxPoolClock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Returns the host system clock as whole seconds since the Unix epoch.
#[must_use]
pub fn sandbox_system_clock() -> SandboxPoolClock {
    Arc::new(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs())
    })
}

#[derive(Default)]
struct SandboxPoolRegistryState {
    sandbox_slots: BTreeMap<SandboxPoolSlotId, SandboxPoolSlot>,
    sandbox_claims: BTreeMap<SandboxPoolClaimId, SandboxPoolClaim>,
    sandbox_claims_by_operation: BTreeMap<SandboxPoolOperationId, SandboxPoolClaimId>,
    sandbox_active_claims_by_slot: BTreeMap<SandboxPoolSlotId, SandboxPoolClaimId>,
    sandbox_active_claims_by_binding: BTreeMap<String, SandboxPoolClaimId>,
    sandbox_failed_claim_attempts: BTreeMap<SandboxPoolOperationId, u32>,
    sandbox_claim_sequence: u64,
    sandbox_highest_fencing_token: SandboxPoolFencingToken,
}

fn sandbox_is_active_claim_state(state: SandboxPoolClaimState) -> bool {
    matches!(
        state,
        SandboxPoolClaimState::Claiming
            | SandboxPoolClaimState::Bound
            | SandboxPoolClaimState::Releasing
    )
}

fn sandbox_note_failed_claim(
    state: &mut SandboxPoolRegistryState,
    sandbox_operation_id: &SandboxPoolOperationId,
) {
    let attempts = state
        .sandbox_failed_claim_attempts
        .entry(sandbox_operation_id.clone())
        .or_insert(0);
    *attempts = attempts.saturating_add(1);
}

/// The bounded pool registry implementing [`SandboxRuntimePoolPort`].
#[derive(Clone)]
pub struct BoundedSandboxPoolControl {
    sandbox_clock: SandboxPoolClock,
    sandbox_state: Arc<Mutex<SandboxPoolRegistryState>>,
}

impl BoundedSandboxPoolControl {
    /// Builds the registry over an injected clock.
    #[must_use]
    pub fn sandbox_with_clock(sandbox_clock: SandboxPoolClock) -> Self {
        Self {
            sandbox_clock,
            sandbox_state: Arc::new(Mutex::new(SandboxPoolRegistryState::default())),
        }
    }

    /// Builds the registry over the host system clock. For local composition
    /// and tests only; see [`SandboxPoolClock`].
    #[must_use]
    pub fn sandbox_with_system_clock() -> Self {
        Self::sandbox_with_clock(sandbox_system_clock())
    }

    /// The current whole-seconds reading of the injected clock.
    #[must_use]
    pub fn sandbox_now(&self) -> u64 {
        (self.sandbox_clock)()
    }

    /// A snapshot of one slot.
    #[must_use]
    pub fn sandbox_slot(
        &self,
        sandbox_pool_slot_id: &SandboxPoolSlotId,
    ) -> Option<SandboxPoolSlot> {
        let state = self.sandbox_lock().ok()?;
        state.sandbox_slots.get(sandbox_pool_slot_id).cloned()
    }

    /// A snapshot of one claim.
    #[must_use]
    pub fn sandbox_claim(
        &self,
        sandbox_pool_claim_id: &SandboxPoolClaimId,
    ) -> Option<SandboxPoolClaim> {
        let state = self.sandbox_lock().ok()?;
        state.sandbox_claims.get(sandbox_pool_claim_id).cloned()
    }

    /// The claim bound to one operation, when recorded.
    #[must_use]
    pub fn sandbox_claim_for_operation(
        &self,
        sandbox_operation_id: &SandboxPoolOperationId,
    ) -> Option<SandboxPoolClaim> {
        let state = self.sandbox_lock().ok()?;
        let sandbox_pool_claim_id = state
            .sandbox_claims_by_operation
            .get(sandbox_operation_id)?;
        state.sandbox_claims.get(sandbox_pool_claim_id).cloned()
    }

    /// How many `ready` slots a profile currently offers. Quarantined slots
    /// are excluded by construction
    /// (`scaling.sandbox_quarantinedCapacityExcludedFromReady`).
    #[must_use]
    pub fn sandbox_ready_slot_count(
        &self,
        sandbox_resource_profile_id: &SandboxResourceProfileId,
    ) -> usize {
        self.sandbox_lock().map_or(0, |state| {
            state
                .sandbox_slots
                .values()
                .filter(|slot| {
                    slot.sandbox_resource_profile_id == *sandbox_resource_profile_id
                        && slot.sandbox_slot_state == SandboxPoolSlotState::Ready
                })
                .count()
        })
    }

    /// How many slots a profile holds out of service in `quarantined`.
    /// Their capacity stays consumed
    /// (`releaseAndSanitization.sandbox_uncertain_cleanup_keeps_capacity_consumed`)
    /// and stays excluded from available capacity
    /// (`scaling.sandbox_quarantinedCapacityExcludedFromAvailableCapacity`).
    #[must_use]
    pub fn sandbox_quarantined_slot_count(
        &self,
        sandbox_resource_profile_id: &SandboxResourceProfileId,
    ) -> usize {
        self.sandbox_lock().map_or(0, |state| {
            state
                .sandbox_slots
                .values()
                .filter(|slot| {
                    slot.sandbox_resource_profile_id == *sandbox_resource_profile_id
                        && slot.sandbox_slot_state == SandboxPoolSlotState::Quarantined
                })
                .count()
        })
    }

    fn sandbox_lock(&self) -> SandboxRuntimePoolResult<MutexGuard<'_, SandboxPoolRegistryState>> {
        self.sandbox_state
            .lock()
            .map_err(|_| SandboxRuntimePoolError::SandboxPoolInternal)
    }

    /// Moves one claim to a terminal or next state, maintaining the active
    /// indexes. Fails closed on a transition outside the contract table.
    fn sandbox_finalize_claim_state(
        state: &mut SandboxPoolRegistryState,
        sandbox_pool_claim_id: &SandboxPoolClaimId,
        sandbox_next_claim_state: SandboxPoolClaimState,
    ) -> SandboxRuntimePoolResult<SandboxPoolClaim> {
        let claim = state
            .sandbox_claims
            .get_mut(sandbox_pool_claim_id)
            .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
        if !claim
            .sandbox_claim_state
            .sandbox_can_transition_to(sandbox_next_claim_state)
        {
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        claim.sandbox_claim_state = sandbox_next_claim_state;
        claim.sandbox_version = claim.sandbox_version.saturating_add(1);
        if !sandbox_is_active_claim_state(sandbox_next_claim_state) {
            state
                .sandbox_active_claims_by_slot
                .remove(&claim.sandbox_pool_slot_id);
            state
                .sandbox_active_claims_by_binding
                .remove(claim.sandbox_runtime_binding_id.as_str());
        }
        Ok(claim.clone())
    }

    /// Quarantines one slot and its active claim. Idempotent for an already
    /// quarantined slot; capacity stays consumed either way.
    fn sandbox_quarantine_locked(
        state: &mut SandboxPoolRegistryState,
        sandbox_pool_slot_id: &SandboxPoolSlotId,
        sandbox_now: u64,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlot> {
        if state
            .sandbox_active_claims_by_slot
            .contains_key(sandbox_pool_slot_id)
        {
            let sandbox_pool_claim_id = state
                .sandbox_active_claims_by_slot
                .get(sandbox_pool_slot_id)
                .cloned()
                .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
            Self::sandbox_finalize_claim_state(
                state,
                &sandbox_pool_claim_id,
                SandboxPoolClaimState::Quarantined,
            )?;
        }
        let slot = state
            .sandbox_slots
            .get_mut(sandbox_pool_slot_id)
            .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
        if slot.sandbox_slot_state == SandboxPoolSlotState::Retired {
            return Err(SandboxRuntimePoolError::SandboxPoolNotReady {
                sandbox_expected_state: "any serviceable state".to_owned(),
                sandbox_actual_state: Some(SandboxPoolSlotState::Retired.as_str().to_owned()),
            });
        }
        if slot.sandbox_slot_state == SandboxPoolSlotState::Quarantined {
            return Ok(slot.clone());
        }
        if !slot
            .sandbox_slot_state
            .sandbox_can_transition_to(SandboxPoolSlotState::Quarantined)
        {
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        slot.sandbox_slot_state = SandboxPoolSlotState::Quarantined;
        slot.sandbox_version = slot.sandbox_version.saturating_add(1);
        slot.sandbox_updated_at = sandbox_now;
        Ok(slot.clone())
    }
}

impl SandboxRuntimePoolPort for BoundedSandboxPoolControl {
    fn sandbox_prepare_pool_slot(
        &self,
        request: SandboxPoolPreparationRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlot> {
        match request.sandbox_pool_class {
            SandboxPoolClass::WarmMicroVmSlot => {
                let sandbox_evidence_present = request
                    .sandbox_warm_kvm_evidence_ref
                    .as_ref()
                    .is_some_and(|evidence| !evidence.as_str().is_empty());
                if !sandbox_evidence_present {
                    return Err(SandboxRuntimePoolError::SandboxPoolDependencyUnavailable {
                        sandbox_dependency: SandboxPoolDependency::WarmKvmEvidence,
                    });
                }
            }
            _ => {
                if request.sandbox_warm_kvm_evidence_ref.is_some() {
                    return Err(SandboxRuntimePoolError::SandboxPoolInternal);
                }
            }
        }
        let mut state = self.sandbox_lock()?;
        let sandbox_now = self.sandbox_now();
        if state
            .sandbox_slots
            .contains_key(&request.sandbox_pool_slot_id)
        {
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        if state.sandbox_slots.len() >= SANDBOX_POOL_CANDIDATE_SLOT_COUNT_MAX {
            return Err(SandboxRuntimePoolError::SandboxPoolExhausted {
                sandbox_retry_after: SandboxPoolRetryAfter::new(
                    SANDBOX_POOL_EXHAUSTED_RETRY_AFTER_SECONDS,
                )
                .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?,
            });
        }
        let slot = SandboxPoolSlot::sandbox_new(
            request.sandbox_pool_slot_id,
            request.sandbox_pool_class,
            request.sandbox_resource_profile_id,
            request.sandbox_node_reference,
            request.sandbox_provider_id,
            request.sandbox_provider_kind,
            request.sandbox_isolation_assurance,
            request.sandbox_artifact_manifest_revision,
            request.sandbox_capacity_revision,
            sandbox_now,
        )?;
        state
            .sandbox_slots
            .insert(slot.sandbox_pool_slot_id.clone(), slot.clone());
        Ok(slot)
    }

    fn sandbox_claim_pool_slot(
        &self,
        request: SandboxPoolClaimRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolClaim> {
        let request = request.sandbox_validated()?;
        let mut state = self.sandbox_lock()?;

        if state
            .sandbox_failed_claim_attempts
            .get(&request.sandbox_operation_id)
            .is_some_and(|attempts| *attempts >= SANDBOX_POOL_CLAIM_ATTEMPT_COUNT_MAX)
        {
            return Err(SandboxRuntimePoolError::SandboxPoolClaimConflict {
                sandbox_conflict_kind: SandboxPoolClaimConflictKind::ClaimAttemptBudgetExhausted,
            });
        }

        if let Some(sandbox_pool_claim_id) = state
            .sandbox_claims_by_operation
            .get(&request.sandbox_operation_id)
            .cloned()
        {
            let recorded = state
                .sandbox_claims
                .get(&sandbox_pool_claim_id)
                .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
            if recorded.sandbox_request_fingerprint == request.sandbox_request_fingerprint {
                return Ok(recorded.clone());
            }
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolClaimConflict {
                sandbox_conflict_kind: SandboxPoolClaimConflictKind::FingerprintMismatch,
            });
        }

        if state
            .sandbox_active_claims_by_binding
            .contains_key(request.sandbox_runtime_binding_id.as_str())
        {
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolClaimConflict {
                sandbox_conflict_kind: SandboxPoolClaimConflictKind::RuntimeBindingAlreadyClaimed,
            });
        }

        // The fixed allocation order: confirmed inputs of steps one to three
        // must accompany the claim (`allocationOrdering`).
        let Some(sandbox_admission_grant_id) = request.sandbox_admission_grant_id else {
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolDependencyUnavailable {
                sandbox_dependency: SandboxPoolDependency::AdmissionReservation,
            });
        };
        let Some(sandbox_capacity_reservation_id) = request.sandbox_capacity_reservation_id else {
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolDependencyUnavailable {
                sandbox_dependency: SandboxPoolDependency::CapacityReservation,
            });
        };

        let sandbox_now = self.sandbox_now();
        let Some(slot) = state
            .sandbox_slots
            .get(&request.sandbox_pool_slot_id)
            .cloned()
        else {
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolNotReady {
                sandbox_expected_state: SandboxPoolSlotState::Ready.as_str().to_owned(),
                sandbox_actual_state: None,
            });
        };
        if slot.sandbox_resource_profile_id != request.sandbox_resource_profile_id {
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolNotReady {
                sandbox_expected_state: SandboxPoolSlotState::Ready.as_str().to_owned(),
                sandbox_actual_state: Some(slot.sandbox_slot_state.as_str().to_owned()),
            });
        }
        if slot.sandbox_slot_state == SandboxPoolSlotState::Quarantined {
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolSlotQuarantined);
        }
        if slot.sandbox_slot_state != SandboxPoolSlotState::Ready {
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolNotReady {
                sandbox_expected_state: SandboxPoolSlotState::Ready.as_str().to_owned(),
                sandbox_actual_state: Some(slot.sandbox_slot_state.as_str().to_owned()),
            });
        }
        if slot.sandbox_capacity_revision != request.sandbox_capacity_revision {
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolClaimConflict {
                sandbox_conflict_kind: SandboxPoolClaimConflictKind::CapacityRevisionMismatch,
            });
        }
        if request.sandbox_expected_fencing_token != slot.sandbox_fencing_token {
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolStaleFencing);
        }
        if state
            .sandbox_active_claims_by_slot
            .contains_key(&request.sandbox_pool_slot_id)
        {
            sandbox_note_failed_claim(&mut state, &request.sandbox_operation_id);
            return Err(SandboxRuntimePoolError::SandboxPoolClaimConflict {
                sandbox_conflict_kind: SandboxPoolClaimConflictKind::ClaimAlreadyActive,
            });
        }

        let sandbox_fencing_token = state
            .sandbox_highest_fencing_token
            .sandbox_next()
            .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
        let sandbox_sequence = state
            .sandbox_claim_sequence
            .checked_add(1)
            .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
        state.sandbox_claim_sequence = sandbox_sequence;
        let sandbox_pool_claim_id =
            SandboxPoolClaimId::new(format!("sandbox_pool_claim_{sandbox_sequence}"))
                .map_err(|_| SandboxRuntimePoolError::SandboxPoolInternal)?;

        let claim = SandboxPoolClaim {
            sandbox_tenant_id: request.sandbox_tenant_id,
            sandbox_pool_claim_id: sandbox_pool_claim_id.clone(),
            sandbox_pool_slot_id: request.sandbox_pool_slot_id.clone(),
            sandbox_admission_grant_id,
            sandbox_capacity_reservation_id,
            sandbox_capacity_revision: request.sandbox_capacity_revision,
            sandbox_session_id: request.sandbox_session_id,
            sandbox_runtime_binding_id: request.sandbox_runtime_binding_id,
            sandbox_operation_id: request.sandbox_operation_id,
            sandbox_request_fingerprint: request.sandbox_request_fingerprint,
            sandbox_fencing_token,
            sandbox_claim_state: SandboxPoolClaimState::Bound,
            sandbox_version: 0,
            sandbox_claimed_at: sandbox_now,
            sandbox_expires_at: sandbox_now.saturating_add(request.sandbox_ttl_seconds),
        };
        let slot = state
            .sandbox_slots
            .get_mut(&request.sandbox_pool_slot_id)
            .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
        // The registry collapses `ready -> claiming -> claimed` into one
        // critical section, performing both contracted transitions in order;
        // the durable authority commits the same pair as two compare-and-swap
        // steps.
        if !slot
            .sandbox_slot_state
            .sandbox_can_transition_to(SandboxPoolSlotState::Claiming)
            || !SandboxPoolSlotState::Claiming
                .sandbox_can_transition_to(SandboxPoolSlotState::Claimed)
        {
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        slot.sandbox_slot_state = SandboxPoolSlotState::Claiming;
        slot.sandbox_slot_state = SandboxPoolSlotState::Claimed;
        slot.sandbox_fencing_token = sandbox_fencing_token;
        slot.sandbox_version = slot.sandbox_version.saturating_add(1);
        slot.sandbox_updated_at = sandbox_now;
        state.sandbox_highest_fencing_token = sandbox_fencing_token;
        state
            .sandbox_claims
            .insert(sandbox_pool_claim_id.clone(), claim.clone());
        state.sandbox_claims_by_operation.insert(
            claim.sandbox_operation_id.clone(),
            sandbox_pool_claim_id.clone(),
        );
        state.sandbox_active_claims_by_slot.insert(
            claim.sandbox_pool_slot_id.clone(),
            sandbox_pool_claim_id.clone(),
        );
        state.sandbox_active_claims_by_binding.insert(
            claim.sandbox_runtime_binding_id.as_str().to_owned(),
            sandbox_pool_claim_id,
        );
        Ok(claim)
    }

    fn sandbox_mark_pool_slot_ready(
        &self,
        request: SandboxPoolReadyRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlot> {
        crate::claim::sandbox_validate_fingerprint(
            &request.sandbox_preparation_evidence_fingerprint,
        )?;
        let mut state = self.sandbox_lock()?;
        let sandbox_now = self.sandbox_now();
        let Some(slot) = state.sandbox_slots.get_mut(&request.sandbox_pool_slot_id) else {
            return Err(SandboxRuntimePoolError::SandboxPoolNotReady {
                sandbox_expected_state: "preparing or sanitizing".to_owned(),
                sandbox_actual_state: None,
            });
        };
        if request.sandbox_expected_fencing_token != slot.sandbox_fencing_token {
            return Err(SandboxRuntimePoolError::SandboxPoolStaleFencing);
        }
        if slot.sandbox_slot_state == SandboxPoolSlotState::Quarantined {
            return Err(SandboxRuntimePoolError::SandboxPoolSlotQuarantined);
        }
        if !matches!(
            slot.sandbox_slot_state,
            SandboxPoolSlotState::Preparing | SandboxPoolSlotState::Sanitizing
        ) {
            return Err(SandboxRuntimePoolError::SandboxPoolNotReady {
                sandbox_expected_state: "preparing or sanitizing".to_owned(),
                sandbox_actual_state: Some(slot.sandbox_slot_state.as_str().to_owned()),
            });
        }
        if slot.sandbox_preparation_evidence_fingerprint.as_deref()
            == Some(request.sandbox_preparation_evidence_fingerprint.as_str())
        {
            // Re-presenting the same evidence is not fresh evidence.
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        if slot.sandbox_slot_state == SandboxPoolSlotState::Sanitizing {
            let sandbox_pool_claim_id = state
                .sandbox_active_claims_by_slot
                .get(&request.sandbox_pool_slot_id)
                .cloned()
                .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
            Self::sandbox_finalize_claim_state(
                &mut state,
                &sandbox_pool_claim_id,
                SandboxPoolClaimState::Released,
            )?;
        }
        let slot = state
            .sandbox_slots
            .get_mut(&request.sandbox_pool_slot_id)
            .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
        if !slot
            .sandbox_slot_state
            .sandbox_can_transition_to(SandboxPoolSlotState::Ready)
        {
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        slot.sandbox_slot_state = SandboxPoolSlotState::Ready;
        slot.sandbox_preparation_evidence_fingerprint =
            Some(request.sandbox_preparation_evidence_fingerprint);
        slot.sandbox_version = slot.sandbox_version.saturating_add(1);
        slot.sandbox_updated_at = sandbox_now;
        Ok(slot.clone())
    }

    fn sandbox_release_pool_claim(
        &self,
        request: SandboxPoolReleaseRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolReleaseOutcome> {
        let mut state = self.sandbox_lock()?;
        let sandbox_now = self.sandbox_now();
        let Some(claim) = state
            .sandbox_claims
            .get(&request.sandbox_pool_claim_id)
            .cloned()
        else {
            return Err(SandboxRuntimePoolError::SandboxPoolNotReady {
                sandbox_expected_state: SandboxPoolClaimState::Bound.as_str().to_owned(),
                sandbox_actual_state: None,
            });
        };
        if request.sandbox_expected_fencing_token != claim.sandbox_fencing_token {
            return Err(SandboxRuntimePoolError::SandboxPoolStaleFencing);
        }
        let sandbox_pool_slot_id = claim.sandbox_pool_slot_id.clone();
        match claim.sandbox_claim_state {
            SandboxPoolClaimState::Bound => {
                Self::sandbox_finalize_claim_state(
                    &mut state,
                    &claim.sandbox_pool_claim_id,
                    SandboxPoolClaimState::Releasing,
                )?;
                let slot = state
                    .sandbox_slots
                    .get_mut(&sandbox_pool_slot_id)
                    .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
                if !slot
                    .sandbox_slot_state
                    .sandbox_can_transition_to(SandboxPoolSlotState::Sanitizing)
                {
                    return Err(SandboxRuntimePoolError::SandboxPoolInternal);
                }
                slot.sandbox_slot_state = SandboxPoolSlotState::Sanitizing;
                slot.sandbox_version = slot.sandbox_version.saturating_add(1);
                slot.sandbox_updated_at = sandbox_now;
            }
            // Idempotent release: replaying a release in flight or already
            // finished reports the current outcome without further mutation
            // (`releaseAndSanitization.sandbox_release_idempotent`).
            SandboxPoolClaimState::Releasing | SandboxPoolClaimState::Released => {}
            SandboxPoolClaimState::Quarantined => {
                return Err(SandboxRuntimePoolError::SandboxPoolSlotQuarantined);
            }
            SandboxPoolClaimState::Claiming => {
                return Err(SandboxRuntimePoolError::SandboxPoolInternal);
            }
        }
        let sandbox_pool_claim = state
            .sandbox_claims
            .get(&request.sandbox_pool_claim_id)
            .cloned()
            .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
        let sandbox_pool_slot = state
            .sandbox_slots
            .get(&sandbox_pool_slot_id)
            .cloned()
            .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
        Ok(SandboxPoolReleaseOutcome {
            sandbox_pool_claim,
            sandbox_pool_slot,
        })
    }

    fn sandbox_quarantine_pool_slot(
        &self,
        request: SandboxPoolQuarantineRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlot> {
        let mut state = self.sandbox_lock()?;
        let sandbox_now = self.sandbox_now();
        let Some(slot) = state.sandbox_slots.get(&request.sandbox_pool_slot_id) else {
            return Err(SandboxRuntimePoolError::SandboxPoolNotReady {
                sandbox_expected_state: "any serviceable state".to_owned(),
                sandbox_actual_state: None,
            });
        };
        if request.sandbox_expected_fencing_token != slot.sandbox_fencing_token {
            return Err(SandboxRuntimePoolError::SandboxPoolStaleFencing);
        }
        Self::sandbox_quarantine_locked(&mut state, &request.sandbox_pool_slot_id, sandbox_now)
    }

    fn sandbox_retire_pool_slot(
        &self,
        request: SandboxPoolRetireRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlot> {
        let mut state = self.sandbox_lock()?;
        let sandbox_now = self.sandbox_now();
        let Some(slot) = state.sandbox_slots.get(&request.sandbox_pool_slot_id) else {
            return Err(SandboxRuntimePoolError::SandboxPoolNotReady {
                sandbox_expected_state: "any state".to_owned(),
                sandbox_actual_state: None,
            });
        };
        if request.sandbox_expected_fencing_token != slot.sandbox_fencing_token {
            return Err(SandboxRuntimePoolError::SandboxPoolStaleFencing);
        }
        if slot.sandbox_slot_state == SandboxPoolSlotState::Retired {
            return Ok(slot.clone());
        }
        if state
            .sandbox_active_claims_by_slot
            .contains_key(&request.sandbox_pool_slot_id)
        {
            let sandbox_pool_claim_id = state
                .sandbox_active_claims_by_slot
                .get(&request.sandbox_pool_slot_id)
                .cloned()
                .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
            Self::sandbox_finalize_claim_state(
                &mut state,
                &sandbox_pool_claim_id,
                SandboxPoolClaimState::Quarantined,
            )?;
        }
        let slot = state
            .sandbox_slots
            .get_mut(&request.sandbox_pool_slot_id)
            .ok_or(SandboxRuntimePoolError::SandboxPoolInternal)?;
        if !slot
            .sandbox_slot_state
            .sandbox_can_transition_to(SandboxPoolSlotState::Retired)
        {
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        slot.sandbox_slot_state = SandboxPoolSlotState::Retired;
        slot.sandbox_version = slot.sandbox_version.saturating_add(1);
        slot.sandbox_updated_at = sandbox_now;
        Ok(slot.clone())
    }

    fn sandbox_reconcile_pool_slots(
        &self,
        request: SandboxPoolSlotReconciliationRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlotReconciliationReport> {
        if request.sandbox_batch_size == 0
            || request.sandbox_batch_size > SANDBOX_POOL_RECONCILIATION_BATCH_SIZE_MAX
        {
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        let mut state = self.sandbox_lock()?;
        let sandbox_now = self.sandbox_now();

        let mut sandbox_expired_by_tenant: BTreeMap<String, Vec<SandboxPoolSlotId>> =
            BTreeMap::new();
        for claim in state.sandbox_claims.values() {
            if sandbox_is_active_claim_state(claim.sandbox_claim_state)
                && claim.sandbox_is_expired_at(sandbox_now)
            {
                sandbox_expired_by_tenant
                    .entry(claim.sandbox_tenant_id.as_str().to_owned())
                    .or_default()
                    .push(claim.sandbox_pool_slot_id.clone());
            }
        }
        // Tenant-aware fairness: tenants are visited in round-robin order so
        // one tenant's expired claims cannot consume the whole bounded batch
        // (`persistenceConcurrencyAndRecovery`
        // `.sandbox_reconciliationTenantAwareAndBounded`).
        let mut sandbox_candidates: Vec<(SandboxPoolSlotId, SandboxPoolQuarantineReason)> =
            Vec::new();
        let mut sandbox_rounds: Vec<Vec<SandboxPoolSlotId>> =
            sandbox_expired_by_tenant.into_values().collect();
        let mut sandbox_round = 0usize;
        loop {
            let mut sandbox_progressed = false;
            for sandbox_group in &mut sandbox_rounds {
                if sandbox_round < sandbox_group.len() {
                    sandbox_candidates.push((
                        sandbox_group[sandbox_round].clone(),
                        SandboxPoolQuarantineReason::ClaimExpired,
                    ));
                    sandbox_progressed = true;
                }
            }
            if !sandbox_progressed {
                break;
            }
            sandbox_round = sandbox_round.saturating_add(1);
        }
        for slot in state.sandbox_slots.values() {
            if matches!(
                slot.sandbox_slot_state,
                SandboxPoolSlotState::Claiming | SandboxPoolSlotState::Sanitizing
            ) && sandbox_now.saturating_sub(slot.sandbox_updated_at)
                >= SANDBOX_POOL_CLEANUP_DEADLINE_SECONDS_MAX
            {
                sandbox_candidates.push((
                    slot.sandbox_pool_slot_id.clone(),
                    SandboxPoolQuarantineReason::TransitionalStuck,
                ));
            }
        }

        let mut sandbox_report = SandboxPoolSlotReconciliationReport::default();
        let mut sandbox_already_visited: std::collections::BTreeSet<SandboxPoolSlotId> =
            std::collections::BTreeSet::new();
        for (sandbox_pool_slot_id, sandbox_quarantine_reason) in sandbox_candidates {
            if sandbox_report.sandbox_examined >= request.sandbox_batch_size {
                break;
            }
            sandbox_report.sandbox_examined = sandbox_report.sandbox_examined.saturating_add(1);
            if !sandbox_already_visited.insert(sandbox_pool_slot_id.clone()) {
                continue;
            }
            if Self::sandbox_quarantine_locked(&mut state, &sandbox_pool_slot_id, sandbox_now)
                .is_ok()
            {
                sandbox_report
                    .sandbox_quarantined
                    .push((sandbox_pool_slot_id, sandbox_quarantine_reason));
            }
        }
        Ok(sandbox_report)
    }

    fn sandbox_reconcile_pool_targets(
        &self,
        request: Vec<SandboxPoolCapacityTarget>,
    ) -> SandboxRuntimePoolResult<SandboxPoolTargetReconciliationReport> {
        if request.len() > SANDBOX_POOL_RECONCILIATION_BATCH_SIZE_MAX {
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        for target in &request {
            if target.sandbox_ready_target > SANDBOX_POOL_PER_PROFILE_TARGET_MAX {
                return Err(SandboxRuntimePoolError::SandboxPoolInternal);
            }
        }
        let sandbox_examined_profiles = request.len();
        let state = self.sandbox_lock()?;
        let mut sandbox_refill_actions: Vec<SandboxPoolRefillAction> = Vec::new();
        let mut sandbox_refill_per_node: BTreeMap<String, usize> = BTreeMap::new();
        for target in request {
            let sandbox_ready = state
                .sandbox_slots
                .values()
                .filter(|slot| {
                    slot.sandbox_resource_profile_id == target.sandbox_resource_profile_id
                        && slot.sandbox_slot_state == SandboxPoolSlotState::Ready
                })
                .count();
            let sandbox_deficit = target
                .sandbox_ready_target
                .saturating_sub(u32::try_from(sandbox_ready).unwrap_or(u32::MAX));
            if sandbox_deficit == 0 {
                continue;
            }
            let sandbox_node_budget = sandbox_refill_per_node
                .entry(target.sandbox_node_reference.as_str().to_owned())
                .or_insert(0);
            let sandbox_room =
                SANDBOX_POOL_REFILL_OPERATIONS_PER_NODE_MAX.saturating_sub(*sandbox_node_budget);
            if sandbox_room == 0 {
                continue;
            }
            let sandbox_refill_count = usize::try_from(sandbox_deficit)
                .unwrap_or(usize::MAX)
                .min(sandbox_room);
            *sandbox_node_budget = sandbox_node_budget.saturating_add(sandbox_refill_count);
            sandbox_refill_actions.push(SandboxPoolRefillAction {
                sandbox_resource_profile_id: target.sandbox_resource_profile_id,
                sandbox_node_reference: target.sandbox_node_reference,
                sandbox_refill_count,
            });
        }
        Ok(SandboxPoolTargetReconciliationReport {
            sandbox_refill_actions,
            sandbox_examined_profiles,
        })
    }
}
