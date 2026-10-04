//! Control-plane slice tests: the state machine, fenced idempotent claims,
//! bounded registries and quarantine discipline of
//! `specs/sandbox-runtime-pool.contract.json`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::bounds::{
    SANDBOX_POOL_CANDIDATE_SLOT_COUNT_MAX, SANDBOX_POOL_CLAIM_ATTEMPT_COUNT_MAX,
    SANDBOX_POOL_CLAIM_TTL_SECONDS_MAX, SANDBOX_POOL_RECONCILIATION_BATCH_SIZE_MAX,
};
use crate::claim::SandboxPoolClaimRequest;
use crate::error::{
    SandboxPoolClaimConflictKind, SandboxPoolDependency, SandboxPoolQuarantineReason,
    SandboxRuntimePoolError,
};
use crate::fencing::SandboxPoolFencingToken;
use crate::identity::{
    SandboxPoolOpaqueRef, SandboxPoolOperationId, SandboxPoolProviderKind, SandboxPoolSlotId,
    SandboxPoolTenantId, SandboxResourceProfileId,
};
use crate::port::{
    SandboxPoolCapacityTarget, SandboxPoolPreparationRequest, SandboxPoolQuarantineRequest,
    SandboxPoolReadyRequest, SandboxPoolReleaseRequest, SandboxPoolRetireRequest,
    SandboxPoolSlotReconciliationRequest, SandboxRuntimePoolPort,
};
use crate::registry::BoundedSandboxPoolControl;
use crate::slot::{SandboxIsolationAssurance, SandboxPoolClass};
use crate::state::{SandboxPoolClaimState, SandboxPoolSlotState};

fn sandbox_test_clock(start: u64) -> (crate::registry::SandboxPoolClock, Arc<AtomicU64>) {
    let now = Arc::new(AtomicU64::new(start));
    let reader = Arc::clone(&now);
    (Arc::new(move || now.load(Ordering::SeqCst)), reader)
}

fn sandbox_registry(start: u64) -> (BoundedSandboxPoolControl, Arc<AtomicU64>) {
    let (clock, reader) = sandbox_test_clock(start);
    (BoundedSandboxPoolControl::sandbox_with_clock(clock), reader)
}

fn sandbox_preparation_request(
    slot_name: &str,
    profile: &str,
    node: &str,
) -> SandboxPoolPreparationRequest {
    SandboxPoolPreparationRequest {
        sandbox_pool_slot_id: SandboxPoolSlotId::new(slot_name).expect("valid slot id"),
        sandbox_pool_class: SandboxPoolClass::PreparedSlot,
        sandbox_resource_profile_id: SandboxResourceProfileId::new(profile).expect("valid profile"),
        sandbox_node_reference: SandboxPoolOpaqueRef::new(node).expect("valid node ref"),
        sandbox_provider_id: SandboxPoolOpaqueRef::new("provider").expect("valid provider"),
        sandbox_provider_kind: SandboxPoolProviderKind::new("firecracker").expect("valid kind"),
        sandbox_isolation_assurance: SandboxIsolationAssurance::MicroVm,
        sandbox_artifact_manifest_revision: SandboxPoolOpaqueRef::new("manifest-r1")
            .expect("valid revision"),
        sandbox_capacity_revision: 7,
        sandbox_warm_kvm_evidence_ref: None,
    }
}

fn sandbox_claim_request(slot: &str, token: i64, operation: &str) -> SandboxPoolClaimRequest {
    SandboxPoolClaimRequest {
        sandbox_tenant_id: SandboxPoolTenantId::new("tenant-a").expect("valid tenant"),
        sandbox_pool_slot_id: SandboxPoolSlotId::new(slot).expect("valid slot id"),
        sandbox_session_id: SandboxPoolOpaqueRef::new("session-1").expect("valid session"),
        sandbox_runtime_binding_id: SandboxPoolOpaqueRef::new("binding-1").expect("valid binding"),
        sandbox_operation_id: SandboxPoolOperationId::new(operation).expect("valid operation"),
        sandbox_request_fingerprint: "a".repeat(crate::claim::SANDBOX_POOL_FINGERPRINT_LENGTH),
        sandbox_admission_grant_id: Some(
            SandboxPoolOpaqueRef::new("grant-1").expect("valid grant"),
        ),
        sandbox_capacity_reservation_id: Some(
            SandboxPoolOpaqueRef::new("reservation-1").expect("valid reservation"),
        ),
        sandbox_capacity_revision: 7,
        sandbox_expected_fencing_token: SandboxPoolFencingToken::try_from(token)
            .expect("valid token"),
        sandbox_ttl_seconds: 30,
        sandbox_resource_profile_id: SandboxResourceProfileId::new("profile")
            .expect("valid profile"),
    }
}

fn sandbox_ready_request(slot: &str, token: i64, evidence: &str) -> SandboxPoolReadyRequest {
    SandboxPoolReadyRequest {
        sandbox_pool_slot_id: SandboxPoolSlotId::new(slot).expect("valid slot id"),
        sandbox_expected_fencing_token: SandboxPoolFencingToken::try_from(token)
            .expect("valid token"),
        sandbox_preparation_evidence_fingerprint: evidence.to_owned(),
    }
}

fn sandbox_make_ready(registry: &BoundedSandboxPoolControl, slot: &str, evidence: &str) -> i64 {
    registry
        .sandbox_prepare_pool_slot(sandbox_preparation_request(slot, "profile", "node-a"))
        .expect("prepare");
    let slot_record = registry
        .sandbox_mark_pool_slot_ready(sandbox_ready_request(slot, 0, evidence))
        .expect("ready");
    slot_record.sandbox_fencing_token.sandbox_as_i64()
}

/// Raises a ready slot's fencing token to a positive value through one full
/// claim-release-ready cycle, so a stale (lower) token is representable.
fn sandbox_bump_slot_token(
    registry: &BoundedSandboxPoolControl,
    slot: &str,
    operation: &str,
) -> i64 {
    let current = registry
        .sandbox_slot(&SandboxPoolSlotId::new(slot).expect("valid slot id"))
        .expect("slot")
        .sandbox_fencing_token
        .sandbox_as_i64();
    let claim = registry
        .sandbox_claim_pool_slot(sandbox_claim_request(slot, current, operation))
        .expect("bump claim");
    let claim_token = claim.sandbox_fencing_token;
    registry
        .sandbox_release_pool_claim(SandboxPoolReleaseRequest {
            sandbox_pool_claim_id: claim.sandbox_pool_claim_id.clone(),
            sandbox_expected_fencing_token: claim_token,
        })
        .expect("bump release");
    let ready = registry
        .sandbox_mark_pool_slot_ready(sandbox_ready_request(
            slot,
            claim_token.sandbox_as_i64(),
            &"f".repeat(64),
        ))
        .expect("bump ready");
    ready.sandbox_fencing_token.sandbox_as_i64()
}

#[test]
fn prepare_then_ready_requires_fresh_tenant_neutral_evidence() {
    let (registry, _clock) = sandbox_registry(1_000);
    registry
        .sandbox_prepare_pool_slot(sandbox_preparation_request("slot-1", "profile", "node-a"))
        .expect("prepare");

    let missing_evidence = registry.sandbox_mark_pool_slot_ready(sandbox_ready_request(
        "slot-1",
        0,
        "not-a-fingerprint",
    ));
    assert!(
        matches!(
            missing_evidence,
            Err(SandboxRuntimePoolError::SandboxPoolInternal)
        ),
        "a malformed evidence fingerprint must fail closed",
    );

    let slot = registry
        .sandbox_mark_pool_slot_ready(sandbox_ready_request(
            "slot-1",
            0,
            &"b".repeat(crate::claim::SANDBOX_POOL_FINGERPRINT_LENGTH),
        ))
        .expect("ready");
    assert_eq!(slot.sandbox_slot_state, SandboxPoolSlotState::Ready);
    assert!(slot.sandbox_is_tenant_neutral());
    assert_eq!(slot.sandbox_version, 1);

    // A ready slot is no longer a mark-ready target.
    let already_ready = registry.sandbox_mark_pool_slot_ready(sandbox_ready_request(
        "slot-1",
        0,
        &"c".repeat(crate::claim::SANDBOX_POOL_FINGERPRINT_LENGTH),
    ));
    assert!(matches!(
        already_ready,
        Err(SandboxRuntimePoolError::SandboxPoolNotReady { .. }),
    ));
}

#[test]
fn warm_slots_stay_behind_the_kvm_evidence_gate() {
    let (registry, _clock) = sandbox_registry(1_000);
    let mut request = sandbox_preparation_request("warm-1", "profile", "node-a");
    request.sandbox_pool_class = SandboxPoolClass::WarmMicroVmSlot;
    let refused = registry.sandbox_prepare_pool_slot(request.clone());
    assert!(matches!(
        refused,
        Err(SandboxRuntimePoolError::SandboxPoolDependencyUnavailable {
            sandbox_dependency: SandboxPoolDependency::WarmKvmEvidence,
        }),
    ));
    request.sandbox_warm_kvm_evidence_ref =
        Some(SandboxPoolOpaqueRef::new("kvm-evidence-r1").expect("valid evidence ref"));
    let admitted = registry.sandbox_prepare_pool_slot(request);
    assert!(admitted.is_ok(), "the evidence reference opens the gate");

    let prepared = sandbox_preparation_request("prepared-1", "profile", "node-a");
    assert!(
        registry.sandbox_prepare_pool_slot(prepared).is_ok(),
        "PreparedSlot needs no warm evidence",
    );
}

#[test]
fn claim_binds_one_fenced_owner_and_bumps_the_persisted_highest_token() {
    let (registry, _clock) = sandbox_registry(1_000);
    let token = sandbox_make_ready(&registry, "slot-1", &"b".repeat(64));
    let claim = registry
        .sandbox_claim_pool_slot(sandbox_claim_request("slot-1", token, "operation-1"))
        .expect("claim");
    assert_eq!(claim.sandbox_claim_state, SandboxPoolClaimState::Bound);
    assert!(claim.sandbox_fencing_token.sandbox_as_i64() > token);
    let slot = registry
        .sandbox_slot(&SandboxPoolSlotId::new("slot-1").expect("valid slot id"))
        .expect("slot");
    assert_eq!(slot.sandbox_slot_state, SandboxPoolSlotState::Claimed);
    assert_eq!(
        slot.sandbox_fencing_token, claim.sandbox_fencing_token,
        "the slot persists the highest fencing token",
    );
}

#[test]
fn claim_requires_the_confirmed_ordering_inputs() {
    let (registry, _clock) = sandbox_registry(1_000);
    let token = sandbox_make_ready(&registry, "slot-1", &"b".repeat(64));
    let mut unconfirmed = sandbox_claim_request("slot-1", token, "operation-1");
    unconfirmed.sandbox_admission_grant_id = None;
    let refused = registry.sandbox_claim_pool_slot(unconfirmed);
    assert!(
        matches!(
            refused,
            Err(SandboxRuntimePoolError::SandboxPoolDependencyUnavailable {
                sandbox_dependency: SandboxPoolDependency::AdmissionReservation,
            }),
        ),
        "a claim without a confirmed admission reservation is refused",
    );

    let mut no_capacity = sandbox_claim_request("slot-1", token, "operation-1");
    no_capacity.sandbox_capacity_reservation_id = None;
    let refused = registry.sandbox_claim_pool_slot(no_capacity);
    assert!(matches!(
        refused,
        Err(SandboxRuntimePoolError::SandboxPoolDependencyUnavailable {
            sandbox_dependency: SandboxPoolDependency::CapacityReservation,
        }),
    ));

    let mut mismatched = sandbox_claim_request("slot-1", token, "operation-1");
    mismatched.sandbox_capacity_revision = 8;
    assert!(matches!(
        registry.sandbox_claim_pool_slot(mismatched),
        Err(SandboxRuntimePoolError::SandboxPoolClaimConflict {
            sandbox_conflict_kind: SandboxPoolClaimConflictKind::CapacityRevisionMismatch,
        }),
    ));
}

#[test]
fn same_operation_same_fingerprint_replays_the_same_claim() {
    let (registry, _clock) = sandbox_registry(1_000);
    let token = sandbox_make_ready(&registry, "slot-1", &"b".repeat(64));
    let first = registry
        .sandbox_claim_pool_slot(sandbox_claim_request("slot-1", token, "operation-1"))
        .expect("claim");
    let replay = registry
        .sandbox_claim_pool_slot(sandbox_claim_request("slot-1", token, "operation-1"))
        .expect("replay");
    assert_eq!(first, replay, "the replay returns the identical claim");
    assert_eq!(
        first.sandbox_fencing_token, replay.sandbox_fencing_token,
        "a replay must not consume another fencing token",
    );
}

#[test]
fn same_operation_different_fingerprint_conflicts() {
    let (registry, _clock) = sandbox_registry(1_000);
    let token = sandbox_make_ready(&registry, "slot-1", &"b".repeat(64));
    registry
        .sandbox_claim_pool_slot(sandbox_claim_request("slot-1", token, "operation-1"))
        .expect("claim");
    let mut conflicting = sandbox_claim_request("slot-1", token, "operation-1");
    conflicting.sandbox_request_fingerprint = "c".repeat(64);
    let conflict = registry.sandbox_claim_pool_slot(conflicting);
    assert!(matches!(
        conflict,
        Err(SandboxRuntimePoolError::SandboxPoolClaimConflict {
            sandbox_conflict_kind: SandboxPoolClaimConflictKind::FingerprintMismatch,
        }),
    ));
}

#[test]
fn stale_fencing_is_rejected_before_any_mutation() {
    let (registry, _clock) = sandbox_registry(1_000);
    let token = sandbox_make_ready(&registry, "slot-1", &"b".repeat(64));
    let bumped = sandbox_bump_slot_token(&registry, "slot-1", "operation-bump");
    assert!(bumped > token, "the bump cycle persists a higher token");
    let slot_before = registry
        .sandbox_slot(&SandboxPoolSlotId::new("slot-1").expect("valid slot id"))
        .expect("slot");
    let stale =
        registry.sandbox_claim_pool_slot(sandbox_claim_request("slot-1", token, "operation-1"));
    assert!(matches!(
        stale,
        Err(SandboxRuntimePoolError::SandboxPoolStaleFencing)
    ));
    let slot = registry
        .sandbox_slot(&SandboxPoolSlotId::new("slot-1").expect("valid slot id"))
        .expect("slot");
    assert_eq!(slot.sandbox_slot_state, SandboxPoolSlotState::Ready);
    assert_eq!(
        slot.sandbox_version, slot_before.sandbox_version,
        "nothing was mutated",
    );
}

#[test]
fn one_active_claim_per_slot_and_per_runtime_binding() {
    let (registry, _clock) = sandbox_registry(1_000);
    let token_a = sandbox_make_ready(&registry, "slot-a", &"b".repeat(64));
    let token_b = sandbox_make_ready(&registry, "slot-b", &"c".repeat(64));
    registry
        .sandbox_claim_pool_slot(sandbox_claim_request("slot-a", token_a, "operation-1"))
        .expect("first claim");

    let second_on_another_slot = sandbox_claim_request("slot-b", token_b, "operation-2");
    let refused = registry.sandbox_claim_pool_slot(second_on_another_slot);
    assert!(matches!(
        refused,
        Err(SandboxRuntimePoolError::SandboxPoolClaimConflict {
            sandbox_conflict_kind: SandboxPoolClaimConflictKind::RuntimeBindingAlreadyClaimed,
        }),
    ));

    let mut other_binding = sandbox_claim_request("slot-b", token_b, "operation-2");
    other_binding.sandbox_runtime_binding_id =
        SandboxPoolOpaqueRef::new("binding-2").expect("valid binding");
    registry
        .sandbox_claim_pool_slot(other_binding)
        .expect("another binding may claim another slot");

    let same_slot_again = sandbox_claim_request("slot-a", token_a, "operation-3");
    let mut different_tenant = same_slot_again;
    different_tenant.sandbox_runtime_binding_id =
        SandboxPoolOpaqueRef::new("binding-3").expect("valid binding");
    different_tenant.sandbox_tenant_id = SandboxPoolTenantId::new("tenant-c").expect("valid");
    let refused = registry.sandbox_claim_pool_slot(different_tenant);
    let Err(SandboxRuntimePoolError::SandboxPoolNotReady {
        sandbox_expected_state,
        sandbox_actual_state,
    }) = refused
    else {
        panic!("claiming an actively claimed slot must fail");
    };
    assert_eq!(sandbox_expected_state, "ready");
    assert_eq!(
        sandbox_actual_state.as_deref(),
        Some("claimed"),
        "the single active claim keeps the slot out of the ready inventory",
    );
}

#[test]
fn claim_ttl_is_bounded_and_the_attempt_budget_closes_retries() {
    let (registry, _clock) = sandbox_registry(1_000);
    let token = sandbox_make_ready(&registry, "slot-1", &"b".repeat(64));
    let bumped = sandbox_bump_slot_token(&registry, "slot-1", "operation-bump");
    assert!(bumped > token);

    let mut oversized = sandbox_claim_request("slot-1", bumped, "operation-1");
    oversized.sandbox_ttl_seconds = SANDBOX_POOL_CLAIM_TTL_SECONDS_MAX + 1;
    assert!(registry.sandbox_claim_pool_slot(oversized).is_err());

    // Drive the failed-attempt budget to the ceiling with stale tokens.
    for attempt in 0..SANDBOX_POOL_CLAIM_ATTEMPT_COUNT_MAX {
        let stale = registry.sandbox_claim_pool_slot(sandbox_claim_request(
            "slot-1",
            token,
            "operation-budget",
        ));
        assert!(matches!(
            stale,
            Err(SandboxRuntimePoolError::SandboxPoolStaleFencing)
        ));
        let _ = attempt;
    }
    let exhausted = registry.sandbox_claim_pool_slot(sandbox_claim_request(
        "slot-1",
        bumped,
        "operation-budget",
    ));
    assert!(matches!(
        exhausted,
        Err(SandboxRuntimePoolError::SandboxPoolClaimConflict {
            sandbox_conflict_kind: SandboxPoolClaimConflictKind::ClaimAttemptBudgetExhausted,
        }),
    ));
}

#[test]
fn release_is_idempotent_and_completion_requires_fresh_cleanup_evidence() {
    let (registry, _clock) = sandbox_registry(1_000);
    let token = sandbox_make_ready(&registry, "slot-1", &"b".repeat(64));
    let claim = registry
        .sandbox_claim_pool_slot(sandbox_claim_request("slot-1", token, "operation-1"))
        .expect("claim");
    let claim_token = claim.sandbox_fencing_token;
    let claim_token_i64 = claim_token.sandbox_as_i64();

    let outcome = registry
        .sandbox_release_pool_claim(SandboxPoolReleaseRequest {
            sandbox_pool_claim_id: claim.sandbox_pool_claim_id.clone(),
            sandbox_expected_fencing_token: claim_token,
        })
        .expect("release");
    assert_eq!(
        outcome.sandbox_pool_claim.sandbox_claim_state,
        SandboxPoolClaimState::Releasing
    );
    assert_eq!(
        outcome.sandbox_pool_slot.sandbox_slot_state,
        SandboxPoolSlotState::Sanitizing
    );

    let replay = registry
        .sandbox_release_pool_claim(SandboxPoolReleaseRequest {
            sandbox_pool_claim_id: claim.sandbox_pool_claim_id.clone(),
            sandbox_expected_fencing_token: claim_token,
        })
        .expect("idempotent release");
    assert_eq!(
        replay.sandbox_pool_claim.sandbox_claim_state,
        SandboxPoolClaimState::Releasing
    );

    let stale_token = registry.sandbox_release_pool_claim(SandboxPoolReleaseRequest {
        sandbox_pool_claim_id: claim.sandbox_pool_claim_id.clone(),
        sandbox_expected_fencing_token: SandboxPoolFencingToken::try_from(token)
            .expect("valid token"),
    });
    assert!(matches!(
        stale_token,
        Err(SandboxRuntimePoolError::SandboxPoolStaleFencing)
    ));

    // The old preparation evidence is not fresh cleanup evidence.
    let reused = registry.sandbox_mark_pool_slot_ready(sandbox_ready_request(
        "slot-1",
        claim_token_i64,
        &"b".repeat(64),
    ));
    assert!(matches!(
        reused,
        Err(SandboxRuntimePoolError::SandboxPoolInternal)
    ));

    let ready = registry
        .sandbox_mark_pool_slot_ready(sandbox_ready_request(
            "slot-1",
            claim_token_i64,
            &"d".repeat(64),
        ))
        .expect("fresh cleanup evidence returns the slot to ready");
    assert_eq!(ready.sandbox_slot_state, SandboxPoolSlotState::Ready);
    let finalized = registry
        .sandbox_claim(&claim.sandbox_pool_claim_id)
        .expect("claim");
    assert_eq!(
        finalized.sandbox_claim_state,
        SandboxPoolClaimState::Released
    );
    assert_eq!(
        registry.sandbox_ready_slot_count(&SandboxResourceProfileId::new("profile").expect("p")),
        1,
    );
}

#[test]
fn quarantine_keeps_capacity_consumed_and_only_lets_the_slot_retire() {
    let (registry, _clock) = sandbox_registry(1_000);
    let token = sandbox_make_ready(&registry, "slot-1", &"b".repeat(64));
    let claim = registry
        .sandbox_claim_pool_slot(sandbox_claim_request("slot-1", token, "operation-1"))
        .expect("claim");
    let claim_token = claim.sandbox_fencing_token.sandbox_as_i64();
    let quarantined = registry
        .sandbox_quarantine_pool_slot(SandboxPoolQuarantineRequest {
            sandbox_pool_slot_id: SandboxPoolSlotId::new("slot-1").expect("valid slot id"),
            sandbox_expected_fencing_token: claim.sandbox_fencing_token,
            sandbox_quarantine_reason: SandboxPoolQuarantineReason::CleanupUncertain,
        })
        .expect("quarantine");
    assert_eq!(
        quarantined.sandbox_slot_state,
        SandboxPoolSlotState::Quarantined
    );

    let profile = SandboxResourceProfileId::new("profile").expect("valid profile");
    assert_eq!(registry.sandbox_ready_slot_count(&profile), 0);
    assert_eq!(registry.sandbox_quarantined_slot_count(&profile), 1);

    let quarantined_claim = registry
        .sandbox_claim(&claim.sandbox_pool_claim_id)
        .expect("claim");
    assert_eq!(
        quarantined_claim.sandbox_claim_state,
        SandboxPoolClaimState::Quarantined,
    );

    let claim_again = registry.sandbox_claim_pool_slot(sandbox_claim_request(
        "slot-1",
        claim_token,
        "operation-2",
    ));
    assert!(matches!(
        claim_again,
        Err(SandboxRuntimePoolError::SandboxPoolSlotQuarantined)
    ));

    let ready_again = registry.sandbox_mark_pool_slot_ready(sandbox_ready_request(
        "slot-1",
        claim_token,
        &"e".repeat(64),
    ));
    assert!(matches!(
        ready_again,
        Err(SandboxRuntimePoolError::SandboxPoolSlotQuarantined)
    ));

    let retired = registry
        .sandbox_retire_pool_slot(SandboxPoolRetireRequest {
            sandbox_pool_slot_id: SandboxPoolSlotId::new("slot-1").expect("valid slot id"),
            sandbox_expected_fencing_token: claim.sandbox_fencing_token,
        })
        .expect("retire");
    assert_eq!(retired.sandbox_slot_state, SandboxPoolSlotState::Retired);
    assert_eq!(
        registry.sandbox_quarantined_slot_count(&profile),
        0,
        "retired leaves the profile"
    );
}

#[test]
fn expired_claims_are_quarantined_not_returned_to_ready() {
    let (registry, clock) = sandbox_registry(1_000);
    let token = sandbox_make_ready(&registry, "slot-1", &"b".repeat(64));
    registry
        .sandbox_claim_pool_slot(sandbox_claim_request("slot-1", token, "operation-1"))
        .expect("claim");
    clock.fetch_add(31, Ordering::SeqCst);

    let report = registry
        .sandbox_reconcile_pool_slots(SandboxPoolSlotReconciliationRequest {
            sandbox_batch_size: 10,
        })
        .expect("reconcile");
    assert_eq!(report.sandbox_examined, 1);
    assert_eq!(report.sandbox_quarantined.len(), 1);

    let slot = registry
        .sandbox_slot(&SandboxPoolSlotId::new("slot-1").expect("valid slot id"))
        .expect("slot");
    assert_eq!(
        slot.sandbox_slot_state,
        SandboxPoolSlotState::Quarantined,
        "a TTL alone may never return the slot to ready",
    );
}

#[test]
fn reconciliation_is_bounded_and_tenant_fair() {
    let (registry, clock) = sandbox_registry(1_000);
    for index in 0..4 {
        let slot_name = format!("slot-{index}");
        let evidence = format!("{:064}", index + 1);
        let token = sandbox_make_ready(&registry, &slot_name, &evidence);
        let mut request = sandbox_claim_request(&slot_name, token, &format!("operation-{index}"));
        request.sandbox_runtime_binding_id =
            SandboxPoolOpaqueRef::new(format!("binding-{index}")).expect("valid binding");
        let tenant = if index % 2 == 0 {
            "tenant-a"
        } else {
            "tenant-b"
        };
        request.sandbox_tenant_id = SandboxPoolTenantId::new(tenant).expect("valid tenant");
        registry.sandbox_claim_pool_slot(request).expect("claim");
    }
    clock.fetch_add(31, Ordering::SeqCst);

    let report = registry
        .sandbox_reconcile_pool_slots(SandboxPoolSlotReconciliationRequest {
            sandbox_batch_size: 2,
        })
        .expect("reconcile");
    assert_eq!(report.sandbox_examined, 2, "the batch bound holds");
    assert_eq!(report.sandbox_quarantined.len(), 2);
    let tenants: Vec<bool> = report
        .sandbox_quarantined
        .iter()
        .map(|(slot_id, _)| slot_id.as_str().ends_with('0') || slot_id.as_str().ends_with('1'))
        .collect();
    assert!(
        tenants.iter().all(|fair| *fair),
        "round-robin visits tenant-a and tenant-b first, not one tenant twice",
    );

    let oversized = registry.sandbox_reconcile_pool_slots(SandboxPoolSlotReconciliationRequest {
        sandbox_batch_size: SANDBOX_POOL_RECONCILIATION_BATCH_SIZE_MAX + 1,
    });
    assert!(matches!(
        oversized,
        Err(SandboxRuntimePoolError::SandboxPoolInternal)
    ));
}

#[test]
fn target_reconciliation_is_bounded_and_excludes_quarantined_capacity() {
    let (registry, _clock) = sandbox_registry(1_000);
    let profile = SandboxResourceProfileId::new("profile").expect("valid profile");
    let node = SandboxPoolOpaqueRef::new("node-a").expect("valid node");
    let plan = registry
        .sandbox_reconcile_pool_targets(vec![SandboxPoolCapacityTarget {
            sandbox_resource_profile_id: profile.clone(),
            sandbox_node_reference: node.clone(),
            sandbox_ready_target: 4,
        }])
        .expect("plan");
    assert_eq!(plan.sandbox_refill_actions.len(), 1);
    assert_eq!(plan.sandbox_refill_actions[0].sandbox_refill_count, 4);

    // Two of four slots ready, one quarantined: quarantined capacity must not
    // count toward the target.
    let token_a = sandbox_make_ready(&registry, "slot-a", &"b".repeat(64));
    let _token_b = sandbox_make_ready(&registry, "slot-b", &"c".repeat(64));
    registry
        .sandbox_prepare_pool_slot(sandbox_preparation_request("slot-q", "profile", "node-a"))
        .expect("prepare");
    registry
        .sandbox_quarantine_pool_slot(SandboxPoolQuarantineRequest {
            sandbox_pool_slot_id: SandboxPoolSlotId::new("slot-q").expect("valid slot id"),
            sandbox_expected_fencing_token: SandboxPoolFencingToken::try_from(0)
                .expect("valid token"),
            sandbox_quarantine_reason: SandboxPoolQuarantineReason::OperatorDecision,
        })
        .expect("quarantine");
    let _ = token_a;
    let plan = registry
        .sandbox_reconcile_pool_targets(vec![SandboxPoolCapacityTarget {
            sandbox_resource_profile_id: profile.clone(),
            sandbox_node_reference: node.clone(),
            sandbox_ready_target: 4,
        }])
        .expect("plan");
    assert_eq!(plan.sandbox_refill_actions[0].sandbox_refill_count, 2);

    let oversized = registry.sandbox_reconcile_pool_targets(vec![SandboxPoolCapacityTarget {
        sandbox_resource_profile_id: profile,
        sandbox_node_reference: node,
        sandbox_ready_target: 1_001,
    }]);
    assert!(matches!(
        oversized,
        Err(SandboxRuntimePoolError::SandboxPoolInternal)
    ));
}

#[test]
fn the_registry_fails_closed_at_the_candidate_bound() {
    let (registry, _clock) = sandbox_registry(1_000);
    for index in 0..SANDBOX_POOL_CANDIDATE_SLOT_COUNT_MAX {
        let slot_name = format!("slot-{index}");
        registry
            .sandbox_prepare_pool_slot(sandbox_preparation_request(&slot_name, "profile", "node-a"))
            .unwrap_or_else(|error| panic!("slot {index} must prepare: {error}"));
    }
    let saturated = registry
        .sandbox_prepare_pool_slot(sandbox_preparation_request(
            "slot-over",
            "profile",
            "node-a",
        ))
        .expect_err("the candidate bound fails closed");
    assert!(matches!(
        saturated,
        SandboxRuntimePoolError::SandboxPoolExhausted { .. }
    ));
    assert!(saturated.sandbox_retry_after().is_some_and(
        |retry| retry.as_seconds() <= crate::bounds::SANDBOX_POOL_RETRY_AFTER_SECONDS_MAX
    ),);
    assert!(saturated.sandbox_retryable());
    assert!(!saturated.sandbox_error_code().is_empty());
}
