//! The bounded workspace-transaction control registry.
//!
//! [`BoundedSandboxWorkspaceTransactionControl`] implements the control-plane
//! slice REVIEW-20260730 authorized for `REQ-2026-0021`: the transaction
//! state machine, the fixed 21-stage orchestration order, the sealed
//! checkpoint candidate with Agents-only compare-and-swap promotion, and the
//! fencing/idempotency discipline, over a bounded in-process registry. It is
//! **not** the PostgreSQL transaction authority
//! (`persistenceConcurrencyAndRecovery` stays behind the contract's
//! `x-sdkwork-no-database-implementation` gate); composition and tests use
//! this registry while the durable authority re-implements the same
//! transitions over `REQ-2026-0018` persistence with the database clock.
//!
//! Every operation is fail-closed: stale fencing is rejected before any
//! mutation, the same operation with the same fingerprint replays
//! idempotently, a different fingerprint conflicts, only one transaction may
//! be active per runtime binding, the first terminal outcome is
//! compare-and-swap, and a read-write release without a durable checkpoint
//! candidate is refused
//! (`checkpoint.sandbox_read_write_release_without_durable_candidate_allowed`
//! is false).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::bounds::{
    SANDBOX_RECONCILIATION_BATCH_SIZE_MAX, SANDBOX_REFERENCE_MAX_LENGTH, SANDBOX_RETRY_AFTER_MS_MAX,
};
use crate::checkpoint::{SandboxCandidatePromotion, SandboxWorkspaceCheckpointCandidate};
use crate::compensation::SandboxCompensationWindow;
use crate::error::{SandboxWorkspaceRuntimeError, SandboxWorkspaceRuntimeResult};
use crate::stage::{SandboxOrchestrationLedger, SandboxOrchestrationStage, SandboxStageEvidence};
use crate::state::SandboxWorkspaceRuntimeTransactionState;

/// Whole-seconds clock for transaction timestamps.
pub type SandboxTransactionClock = Arc<dyn Fn() -> u64 + Send + Sync>;

/// The workspace mount mode the request declares.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxWorkspaceMountMode {
    /// Read-only projection; release records a read-only no-checkpoint
    /// outcome (`checkpoint.readOnlyPolicy`).
    ReadOnly,
    /// Read-write projection; release requires a durable candidate and
    /// handoff (`checkpoint.readWritePolicy`).
    ReadWrite,
}

impl SandboxWorkspaceMountMode {
    /// The contract policy name.
    #[must_use]
    pub const fn sandbox_checkpoint_policy(self) -> &'static str {
        match self {
            Self::ReadOnly => "ReadOnlyNoCheckpoint",
            Self::ReadWrite => "DurableCheckpointRequired",
        }
    }
}

/// A validated transaction request
/// (`contract`: `request.requiredFields`, all `sandbox_`-prefixed; unknown
/// fields cannot exist on a closed struct).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxWorkspaceRuntimeTransactionRequest {
    /// Transaction identity.
    pub sandbox_workspace_runtime_transaction_id: String,
    /// Tenant scope hash (never the raw tenant id).
    pub sandbox_tenant_scope_hash: String,
    /// The workspace identity.
    pub sandbox_workspace_id: String,
    /// The Agents-owned source workspace revision reference.
    pub sandbox_workspace_revision_ref: String,
    /// The short-lived authorization grant reference.
    pub sandbox_workspace_authorization_grant_ref: String,
    /// The session identity.
    pub sandbox_session_id: String,
    /// The opaque kernel execution placement reference.
    pub sandbox_kernel_execution_placement_ref: String,
    /// The kernel execution placement generation.
    pub sandbox_kernel_execution_placement_generation: u64,
    /// The caller operation identity (the idempotency key).
    pub sandbox_operation_id: String,
    /// The execution profile reference.
    pub sandbox_execution_profile_id: String,
    /// The required capabilities (canonical, sorted).
    pub sandbox_required_capabilities: Vec<String>,
    /// The minimum assurance name.
    pub sandbox_minimum_assurance: String,
    /// The workload class reference; Sandbox policy resolves it to an
    /// entitled resource ceiling.
    pub sandbox_workload_class_ref: String,
    /// The workspace mount mode.
    pub sandbox_workspace_mount_mode: SandboxWorkspaceMountMode,
    /// The immutable request fingerprint (lowercase hex SHA-256).
    pub sandbox_request_fingerprint: String,
    /// The caller-observed registry fencing token.
    pub sandbox_fencing_token: i64,
    /// The transaction deadline, in whole seconds from the injected clock.
    pub sandbox_deadline_at: u64,
    /// The trace identity.
    pub sandbox_trace_id: String,
}

fn sandbox_validated_reference(value: &str) -> SandboxWorkspaceRuntimeResult<()> {
    let valid = !value.is_empty()
        && value.len() <= SANDBOX_REFERENCE_MAX_LENGTH
        && value
            .chars()
            .all(|c| c.is_ascii_graphic() && !c.is_whitespace())
        && !value.starts_with('/')
        && !value.contains("://");
    if valid {
        Ok(())
    } else {
        Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest)
    }
}

const SANDBOX_TRANSACTION_FINGERPRINT_LENGTH: usize = 64;

impl SandboxWorkspaceRuntimeTransactionRequest {
    /// Validates every field shape fail-closed.
    pub fn sandbox_validated(&self) -> SandboxWorkspaceRuntimeResult<()> {
        for reference in [
            self.sandbox_workspace_runtime_transaction_id.as_str(),
            self.sandbox_tenant_scope_hash.as_str(),
            self.sandbox_workspace_id.as_str(),
            self.sandbox_workspace_revision_ref.as_str(),
            self.sandbox_workspace_authorization_grant_ref.as_str(),
            self.sandbox_session_id.as_str(),
            self.sandbox_kernel_execution_placement_ref.as_str(),
            self.sandbox_operation_id.as_str(),
            self.sandbox_execution_profile_id.as_str(),
            self.sandbox_workload_class_ref.as_str(),
            self.sandbox_trace_id.as_str(),
        ] {
            sandbox_validated_reference(reference)?;
        }
        let fingerprint_valid = self.sandbox_request_fingerprint.len()
            == SANDBOX_TRANSACTION_FINGERPRINT_LENGTH
            && self
                .sandbox_request_fingerprint
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if !fingerprint_valid {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        if self.sandbox_fencing_token < 0 {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        for capability in &self.sandbox_required_capabilities {
            sandbox_validated_reference(capability)?;
        }
        Ok(())
    }
}

/// A terminal transaction outcome recorded exactly once
/// (`fencingAndIdempotency.sandbox_first_terminal_outcome_cas_required`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SandboxTerminalOutcome {
    /// The transaction released after every ordered stage completed.
    Released,
    /// The transaction quarantined; capacity stays consumed.
    Quarantined,
}

struct SandboxTransactionRecord {
    sandbox_state: SandboxWorkspaceRuntimeTransactionState,
    sandbox_version: u64,
    sandbox_fencing_token: i64,
    sandbox_request: SandboxWorkspaceRuntimeTransactionRequest,
    sandbox_ledger: SandboxOrchestrationLedger,
    sandbox_candidate: Option<SandboxWorkspaceCheckpointCandidate>,
    sandbox_checkpoint_handoff_durable: bool,
}

#[derive(Default)]
struct SandboxRegistryState {
    sandbox_transactions: BTreeMap<String, SandboxTransactionRecord>,
    sandbox_active_by_binding: BTreeMap<String, String>,
    sandbox_outcomes_by_operation: BTreeMap<String, SandboxTerminalOutcome>,
    sandbox_fingerprints_by_operation: BTreeMap<String, String>,
    sandbox_highest_fencing_token: i64,
}

/// The bounded transaction control registry.
#[derive(Clone)]
pub struct BoundedSandboxWorkspaceTransactionControl {
    sandbox_clock: SandboxTransactionClock,
    sandbox_state: Arc<Mutex<SandboxRegistryState>>,
}

/// The handle returned when a transaction begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxTransactionHandle {
    /// The transaction identity.
    pub sandbox_workspace_runtime_transaction_id: String,
    /// The initial state (always `requested`).
    pub sandbox_state: SandboxWorkspaceRuntimeTransactionState,
    /// The fencing token the transaction was bound under.
    pub sandbox_fencing_token: i64,
}

impl BoundedSandboxWorkspaceTransactionControl {
    /// Builds the registry over an injected clock.
    #[must_use]
    pub fn sandbox_with_clock(sandbox_clock: SandboxTransactionClock) -> Self {
        Self {
            sandbox_clock,
            sandbox_state: Arc::new(Mutex::new(SandboxRegistryState::default())),
        }
    }

    /// The current whole-seconds reading of the injected clock.
    #[must_use]
    pub fn sandbox_now(&self) -> u64 {
        (self.sandbox_clock)()
    }

    fn sandbox_lock(&self) -> SandboxWorkspaceRuntimeResult<MutexGuard<'_, SandboxRegistryState>> {
        self.sandbox_state
            .lock()
            .map_err(|_| SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInternalFailure)
    }

    fn sandbox_record<'a>(
        state: &'a mut SandboxRegistryState,
        sandbox_workspace_runtime_transaction_id: &str,
    ) -> SandboxWorkspaceRuntimeResult<&'a mut SandboxTransactionRecord> {
        state
            .sandbox_transactions
            .get_mut(sandbox_workspace_runtime_transaction_id)
            .ok_or(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest)
    }

    fn sandbox_check_fencing(
        record: &SandboxTransactionRecord,
        sandbox_expected_fencing_token: i64,
    ) -> SandboxWorkspaceRuntimeResult<()> {
        if sandbox_expected_fencing_token != record.sandbox_fencing_token {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeStaleFencing);
        }
        Ok(())
    }

    /// Begins one transaction.
    ///
    /// Fail-closed in contract order: request shape, idempotent replay
    /// (same operation + same fingerprint returns the recorded handle),
    /// fingerprint conflict, single active transaction per runtime binding,
    /// stale fencing. The transaction binds a freshly issued highest fencing
    /// token.
    pub fn sandbox_begin(
        &self,
        request: SandboxWorkspaceRuntimeTransactionRequest,
    ) -> SandboxWorkspaceRuntimeResult<SandboxTransactionHandle> {
        request.sandbox_validated()?;
        let mut state = self.sandbox_lock()?;
        if let Some(recorded) = state
            .sandbox_fingerprints_by_operation
            .get(&request.sandbox_operation_id)
        {
            return if *recorded == request.sandbox_request_fingerprint {
                let record = state
                    .sandbox_transactions
                    .get(&request.sandbox_workspace_runtime_transaction_id)
                    .ok_or(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest)?;
                Ok(SandboxTransactionHandle {
                    sandbox_workspace_runtime_transaction_id: record
                        .sandbox_request
                        .sandbox_workspace_runtime_transaction_id
                        .clone(),
                    sandbox_state: record.sandbox_state,
                    sandbox_fencing_token: record.sandbox_fencing_token,
                })
            } else {
                Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest)
            };
        }
        // A transaction identity is bound exactly once: only the exact
        // operation replay above may observe it again. Re-binding it under a
        // fresh operation would let that operation's replay later resolve to
        // a different transaction, so it is refused before any mutation (the
        // durable authority enforces the same uniqueness as a primary key).
        if state
            .sandbox_transactions
            .contains_key(&request.sandbox_workspace_runtime_transaction_id)
        {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        if state
            .sandbox_active_by_binding
            .contains_key(&request.sandbox_session_id)
        {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeWriterConflict);
        }
        if request.sandbox_fencing_token < state.sandbox_highest_fencing_token {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeStaleFencing);
        }
        if state.sandbox_transactions.len() >= SANDBOX_RECONCILIATION_BATCH_SIZE_MAX {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeCapacityUnavailable);
        }
        // The deadline policy carries a hard max by contract; a deadline at
        // or before the current clock reading is refused.
        if request.sandbox_deadline_at <= (self.sandbox_clock)() {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        let sandbox_fencing_token = state
            .sandbox_highest_fencing_token
            .checked_add(1)
            .ok_or(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest)?;
        state.sandbox_highest_fencing_token = sandbox_fencing_token;
        let record = SandboxTransactionRecord {
            sandbox_state: SandboxWorkspaceRuntimeTransactionState::Requested,
            sandbox_version: 0,
            sandbox_fencing_token,
            sandbox_ledger: SandboxOrchestrationLedger::sandbox_new(),
            sandbox_candidate: None,
            sandbox_checkpoint_handoff_durable: false,
            sandbox_request: request.clone(),
        };
        state.sandbox_fingerprints_by_operation.insert(
            request.sandbox_operation_id.clone(),
            request.sandbox_request_fingerprint.clone(),
        );
        state.sandbox_active_by_binding.insert(
            request.sandbox_session_id.clone(),
            request.sandbox_workspace_runtime_transaction_id.clone(),
        );
        state.sandbox_transactions.insert(
            request.sandbox_workspace_runtime_transaction_id.clone(),
            record,
        );
        Ok(SandboxTransactionHandle {
            sandbox_workspace_runtime_transaction_id: request
                .sandbox_workspace_runtime_transaction_id,
            sandbox_state: SandboxWorkspaceRuntimeTransactionState::Requested,
            sandbox_fencing_token,
        })
    }

    /// Records one ordered stage completion and advances the transaction
    /// state per the fixed stage-to-state mapping.
    pub fn sandbox_advance_stage(
        &self,
        sandbox_workspace_runtime_transaction_id: &str,
        stage: SandboxOrchestrationStage,
        evidence: SandboxStageEvidence,
        sandbox_expected_fencing_token: i64,
    ) -> SandboxWorkspaceRuntimeResult<SandboxWorkspaceRuntimeTransactionState> {
        let mut state = self.sandbox_lock()?;
        let record = Self::sandbox_record(&mut state, sandbox_workspace_runtime_transaction_id)?;
        Self::sandbox_check_fencing(record, sandbox_expected_fencing_token)?;
        if matches!(
            record.sandbox_state,
            SandboxWorkspaceRuntimeTransactionState::Released
                | SandboxWorkspaceRuntimeTransactionState::Quarantined
                | SandboxWorkspaceRuntimeTransactionState::Compensating
        ) {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        // Compensation is the failure window: the closed table offers it no
        // transition back to a running state, so stages may not complete
        // inside it — recording them here would let a compensating
        // transaction walk into `released` without ever passing through the
        // states those stages map to.
        record.sandbox_ledger.sandbox_record(stage, evidence)?;
        let next_state = Self::sandbox_state_for_stage(record, stage)?;
        if next_state != record.sandbox_state {
            if !record.sandbox_state.sandbox_can_transition_to(next_state) {
                return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
            }
            record.sandbox_state = next_state;
        }
        record.sandbox_version = record.sandbox_version.saturating_add(1);
        Ok(next_state)
    }

    fn sandbox_state_for_stage(
        record: &SandboxTransactionRecord,
        stage: SandboxOrchestrationStage,
    ) -> SandboxWorkspaceRuntimeResult<SandboxWorkspaceRuntimeTransactionState> {
        Ok(match stage {
            SandboxOrchestrationStage::AdmissionConfirmed => {
                SandboxWorkspaceRuntimeTransactionState::Admitted
            }
            SandboxOrchestrationStage::CapacityReservationConfirmed => {
                SandboxWorkspaceRuntimeTransactionState::CapacityReserved
            }
            SandboxOrchestrationStage::RuntimeBindingFenced => {
                SandboxWorkspaceRuntimeTransactionState::RuntimeBound
            }
            SandboxOrchestrationStage::ProjectionAttached => {
                SandboxWorkspaceRuntimeTransactionState::WorkspaceAttaching
            }
            SandboxOrchestrationStage::EnvironmentReady => {
                SandboxWorkspaceRuntimeTransactionState::Ready
            }
            SandboxOrchestrationStage::CommandAdmissionOpened => {
                SandboxWorkspaceRuntimeTransactionState::Executing
            }
            SandboxOrchestrationStage::CommandAdmissionFrozenAndDrained => {
                SandboxWorkspaceRuntimeTransactionState::Checkpointing
            }
            // All other stages complete inside the current state.
            _ => record.sandbox_state,
        })
    }

    /// Seals and binds the durable checkpoint candidate to the transaction.
    /// A read-only transaction has no candidate; a read-write release without
    /// one is refused later.
    pub fn sandbox_attach_checkpoint_candidate(
        &self,
        sandbox_workspace_runtime_transaction_id: &str,
        candidate: SandboxWorkspaceCheckpointCandidate,
        sandbox_expected_fencing_token: i64,
    ) -> SandboxWorkspaceRuntimeResult<()> {
        let mut state = self.sandbox_lock()?;
        let record = Self::sandbox_record(&mut state, sandbox_workspace_runtime_transaction_id)?;
        Self::sandbox_check_fencing(record, sandbox_expected_fencing_token)?;
        if record.sandbox_candidate.is_some() {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        if record.sandbox_request.sandbox_workspace_mount_mode
            != SandboxWorkspaceMountMode::ReadWrite
        {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        record.sandbox_candidate = Some(candidate);
        Ok(())
    }

    /// Records the durable checkpoint handoff
    /// (`checkpoint.sandbox_handoff_persisted_before_runtime_release`).
    pub fn sandbox_record_checkpoint_handoff(
        &self,
        sandbox_workspace_runtime_transaction_id: &str,
        sandbox_expected_fencing_token: i64,
    ) -> SandboxWorkspaceRuntimeResult<()> {
        let mut state = self.sandbox_lock()?;
        let record = Self::sandbox_record(&mut state, sandbox_workspace_runtime_transaction_id)?;
        Self::sandbox_check_fencing(record, sandbox_expected_fencing_token)?;
        if record.sandbox_request.sandbox_workspace_mount_mode
            == SandboxWorkspaceMountMode::ReadWrite
        {
            let candidate = record
                .sandbox_candidate
                .as_ref()
                .ok_or(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeCheckpointFailed)?;
            if !candidate.sandbox_is_sealed() {
                return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeCheckpointFailed);
            }
        }
        record.sandbox_checkpoint_handoff_durable = true;
        record.sandbox_version = record.sandbox_version.saturating_add(1);
        Ok(())
    }

    /// Agents-only promotion of the bound candidate
    /// (`checkpoint.sandbox_agents_alone_promotes_candidate_to_workspace_revision`).
    pub fn sandbox_promote_checkpoint(
        &self,
        sandbox_workspace_runtime_transaction_id: &str,
        sandbox_expected_source_workspace_revision_ref: &str,
    ) -> SandboxWorkspaceRuntimeResult<SandboxCandidatePromotion> {
        let mut state = self.sandbox_lock()?;
        let record = Self::sandbox_record(&mut state, sandbox_workspace_runtime_transaction_id)?;
        let candidate = record
            .sandbox_candidate
            .as_mut()
            .ok_or(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeCheckpointFailed)?;
        candidate.sandbox_promote(sandbox_expected_source_workspace_revision_ref)
    }

    /// Releases the transaction: every ordered stage must carry evidence,
    /// commands must be frozen and drained, and a read-write transaction must
    /// have its durable checkpoint candidate and handoff recorded — a
    /// read-write release without them is refused
    /// (`sandbox_read_write_release_without_durable_candidate_allowed` is
    /// false). The capacity/admission stage completes last; the terminal
    /// outcome is compare-and-swap and never re-opened.
    pub fn sandbox_release(
        &self,
        sandbox_workspace_runtime_transaction_id: &str,
        sandbox_expected_fencing_token: i64,
    ) -> SandboxWorkspaceRuntimeResult<SandboxTerminalOutcome> {
        let mut state = self.sandbox_lock()?;
        if let Some(recorded) = state
            .sandbox_outcomes_by_operation
            .get(sandbox_workspace_runtime_transaction_id)
        {
            return Ok(recorded.clone());
        }
        let (sandbox_request, ledger, handoff_durable) = {
            let record =
                Self::sandbox_record(&mut state, sandbox_workspace_runtime_transaction_id)?;
            Self::sandbox_check_fencing(record, sandbox_expected_fencing_token)?;
            (
                record.sandbox_request.clone(),
                record.sandbox_ledger.clone(),
                record.sandbox_checkpoint_handoff_durable,
            )
        };
        let sandbox_stages = crate::stage::sandbox_orchestration_stages();
        for stage in sandbox_stages {
            let read_only_noop_allowed = sandbox_request.sandbox_workspace_mount_mode
                == SandboxWorkspaceMountMode::ReadOnly
                && matches!(
                    stage,
                    SandboxOrchestrationStage::CheckpointCandidateDurable
                        | SandboxOrchestrationStage::CheckpointHandoffDurable
                );
            let Some(evidence) = ledger.sandbox_completed_ref(*stage) else {
                return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeCleanupIncomplete);
            };
            if read_only_noop_allowed {
                continue;
            }
            if matches!(evidence, SandboxStageEvidence::NotApplicable { .. })
                && matches!(
                    stage,
                    SandboxOrchestrationStage::CheckpointCandidateDurable
                        | SandboxOrchestrationStage::CheckpointHandoffDurable
                )
            {
                return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeCheckpointFailed);
            }
        }
        if sandbox_request.sandbox_workspace_mount_mode == SandboxWorkspaceMountMode::ReadWrite
            && !handoff_durable
        {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeCheckpointFailed);
        }
        let sandbox_session_id = {
            let record =
                Self::sandbox_record(&mut state, sandbox_workspace_runtime_transaction_id)?;
            if !record
                .sandbox_state
                .sandbox_can_transition_to(SandboxWorkspaceRuntimeTransactionState::Released)
            {
                return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
            }
            record.sandbox_state = SandboxWorkspaceRuntimeTransactionState::Released;
            record.sandbox_version = record.sandbox_version.saturating_add(1);
            record.sandbox_request.sandbox_session_id.clone()
        };
        state.sandbox_active_by_binding.remove(&sandbox_session_id);
        let outcome = SandboxTerminalOutcome::Released;
        state.sandbox_outcomes_by_operation.insert(
            sandbox_workspace_runtime_transaction_id.to_owned(),
            outcome.clone(),
        );
        Ok(outcome)
    }

    /// Enters compensation for the window matching the completed stage
    /// count, returning the window and its fixed required-action list.
    pub fn sandbox_enter_compensation(
        &self,
        sandbox_workspace_runtime_transaction_id: &str,
        sandbox_expected_fencing_token: i64,
    ) -> SandboxWorkspaceRuntimeResult<(
        SandboxCompensationWindow,
        &'static [crate::compensation::SandboxCompensationAction],
    )> {
        let mut state = self.sandbox_lock()?;
        let record = Self::sandbox_record(&mut state, sandbox_workspace_runtime_transaction_id)?;
        Self::sandbox_check_fencing(record, sandbox_expected_fencing_token)?;
        if record.sandbox_state == SandboxWorkspaceRuntimeTransactionState::Released {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        let window = SandboxCompensationWindow::sandbox_window_for_completed_stage(
            record.sandbox_ledger.sandbox_highest_completed_position(),
        );
        if !record
            .sandbox_state
            .sandbox_can_transition_to(SandboxWorkspaceRuntimeTransactionState::Compensating)
        {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        record.sandbox_state = SandboxWorkspaceRuntimeTransactionState::Compensating;
        record.sandbox_version = record.sandbox_version.saturating_add(1);
        let actions = window.sandbox_required_actions();
        Ok((window, actions))
    }

    /// Completes compensation into the security-failure terminal. Capacity
    /// stays consumed; the transaction never re-opens.
    pub fn sandbox_quarantine(
        &self,
        sandbox_workspace_runtime_transaction_id: &str,
        sandbox_expected_fencing_token: i64,
    ) -> SandboxWorkspaceRuntimeResult<SandboxTerminalOutcome> {
        let mut state = self.sandbox_lock()?;
        if let Some(recorded) = state
            .sandbox_outcomes_by_operation
            .get(sandbox_workspace_runtime_transaction_id)
        {
            return Ok(recorded.clone());
        }
        let (sandbox_session_id,) = {
            let record =
                Self::sandbox_record(&mut state, sandbox_workspace_runtime_transaction_id)?;
            Self::sandbox_check_fencing(record, sandbox_expected_fencing_token)?;
            if !record
                .sandbox_state
                .sandbox_can_transition_to(SandboxWorkspaceRuntimeTransactionState::Quarantined)
            {
                return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
            }
            (record.sandbox_request.sandbox_session_id.clone(),)
        };
        let record = Self::sandbox_record(&mut state, sandbox_workspace_runtime_transaction_id)?;
        record.sandbox_state = SandboxWorkspaceRuntimeTransactionState::Quarantined;
        record.sandbox_version = record.sandbox_version.saturating_add(1);
        state.sandbox_active_by_binding.remove(&sandbox_session_id);
        let outcome = SandboxTerminalOutcome::Quarantined;
        state.sandbox_outcomes_by_operation.insert(
            sandbox_workspace_runtime_transaction_id.to_owned(),
            outcome.clone(),
        );
        Ok(outcome)
    }

    /// The retry guidance ceiling (`bounds.sandbox_retry_after_ms_max`).
    #[must_use]
    pub const fn sandbox_retry_after_ms_max() -> u64 {
        SANDBOX_RETRY_AFTER_MS_MAX
    }
}
