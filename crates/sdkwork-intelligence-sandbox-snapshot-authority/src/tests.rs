//! Authority-model slice tests: the snapshot record, lifecycle, fork
//! semantics, evidence gates and artifact layering of
//! `specs/sandbox-snapshot-fork.contract.json`.

use crate::bounds::MAX_SANDBOX_SNAPSHOT_DERIVATIONS_PER_PLAN;
use crate::error::SandboxSnapshotAuthorityError;
use crate::fork::{
    SandboxForkDerivation, SANDBOX_SNAPSHOT_FORK_FRESH_IDENTITY_REQUIRED,
    SANDBOX_SNAPSHOT_FORK_PARALLEL_RUNNING, SANDBOX_SNAPSHOT_FORK_SOURCE_SNAPSHOT_IMMUTABLE,
    SANDBOX_SNAPSHOT_FORK_SOURCE_TENANT_STATE_REUSE_ALLOWED,
};
use crate::gates::{
    sandbox_second_supply_chain_authority_allowed, sandbox_snapshot_owns_evidence_or_signature,
    sandbox_snapshot_references_artifact_tuple, SANDBOX_SNAPSHOT_ARTIFACT_AUTHORITY,
    SANDBOX_SNAPSHOT_CROSS_TENANT_RESIDUE_EVIDENCE_REQUIRED,
    SANDBOX_SNAPSHOT_DERIVED_IDENTITY_ROTATION_EVIDENCE_REQUIRED,
    SANDBOX_SNAPSHOT_LINE_LAYERING_CHECKPOINT_AUTHORITY,
    SANDBOX_SNAPSHOT_REAL_KVM_RESTORE_EVIDENCE_REQUIRED,
    SANDBOX_SNAPSHOT_REGISTRATION_ENABLES_WARM_SLOT,
    SANDBOX_SNAPSHOT_WARM_SLOT_REUSE_REQUIRES_POOL_EVIDENCE_GATE,
};
use crate::snapshot::SandboxSnapshot;
use crate::state::SandboxSnapshotState;

fn sandbox_fingerprint(seed: u8) -> String {
    format!("{seed:064}")
}

fn sandbox_snapshot() -> SandboxSnapshot {
    SandboxSnapshot::sandbox_new(
        "snapshot-1",
        "session-ref-1",
        true,
        &sandbox_fingerprint(7),
        "artifact-tuple-r12",
        1_000,
    )
    .expect("valid snapshot")
}

#[test]
fn a_created_snapshot_carries_every_contract_field_and_starts_creating() {
    let snapshot = sandbox_snapshot();
    assert_eq!(snapshot.sandbox_snapshot_id(), "snapshot-1");
    assert_eq!(snapshot.sandbox_source_session_ref(), "session-ref-1");
    assert!(snapshot.sandbox_memory_included());
    assert_eq!(
        snapshot.sandbox_filesystem_fingerprint(),
        &sandbox_fingerprint(7)
    );
    assert_eq!(snapshot.sandbox_artifact_tuple_ref(), "artifact-tuple-r12");
    assert_eq!(
        snapshot.sandbox_snapshot_state(),
        SandboxSnapshotState::Creating
    );
    assert_eq!(snapshot.sandbox_created_at(), 1_000);
    assert!(!snapshot.sandbox_is_derivable());
}

#[test]
fn snapshot_validation_is_fail_closed() {
    assert!(SandboxSnapshot::sandbox_new(
        "/etc/passwd",
        "session-ref-1",
        false,
        &sandbox_fingerprint(7),
        "tuple",
        1,
    )
    .is_err());
    assert!(SandboxSnapshot::sandbox_new(
        "snapshot-1",
        "session-ref-1",
        false,
        "not-a-fingerprint",
        "tuple",
        1,
    )
    .is_err());
    assert!(SandboxSnapshot::sandbox_new(
        "snapshot-1",
        "",
        false,
        &sandbox_fingerprint(7),
        "tuple",
        1,
    )
    .is_err());
}

#[test]
fn the_lifecycle_is_the_contracted_closed_set() {
    let mut snapshot = sandbox_snapshot();
    // creating -> deleted skips materialization: illegal.
    assert!(matches!(
        snapshot.sandbox_mark_deleted(),
        Err(SandboxSnapshotAuthorityError::SandboxSnapshotIllegalTransition),
    ));
    snapshot.sandbox_mark_available().expect("available");
    snapshot.sandbox_mark_restoring().expect("restoring");
    snapshot
        .sandbox_mark_restore_finished()
        .expect("back to available");
    snapshot.sandbox_mark_quarantined().expect("quarantine");
    // Quarantined snapshots cannot serve derivations and cannot reopen.
    assert!(!snapshot.sandbox_is_derivable());
    assert!(matches!(
        snapshot.sandbox_mark_available(),
        Err(SandboxSnapshotAuthorityError::SandboxSnapshotIllegalTransition),
    ));
    assert!(matches!(
        snapshot.sandbox_mark_deleted(),
        Err(SandboxSnapshotAuthorityError::SandboxSnapshotIllegalTransition),
    ));

    // A deterministic deletion from available is legal and terminal.
    let mut other = sandbox_snapshot();
    other.sandbox_mark_available().expect("available");
    other.sandbox_mark_deleted().expect("deleted");
    assert_eq!(
        other.sandbox_snapshot_state(),
        SandboxSnapshotState::Deleted
    );
}

#[test]
fn fork_plans_are_bounded_and_require_a_derivable_source() {
    let mut snapshot = sandbox_snapshot();
    // A creating snapshot cannot serve derivations.
    assert!(matches!(
        SandboxForkDerivation::sandbox_plan(&snapshot, 3),
        Err(SandboxSnapshotAuthorityError::SandboxSnapshotInvalidForkDerivation),
    ));
    snapshot.sandbox_mark_available().expect("available");
    let plan = SandboxForkDerivation::sandbox_plan(&snapshot, 8).expect("plan");
    assert_eq!(plan.sandbox_snapshot_id(), "snapshot-1");
    assert_eq!(plan.sandbox_derivation_count(), 8);
    assert!(SandboxForkDerivation::sandbox_plan(&snapshot, 0).is_err());
    assert!(SandboxForkDerivation::sandbox_plan(
        &snapshot,
        MAX_SANDBOX_SNAPSHOT_DERIVATIONS_PER_PLAN + 1,
    )
    .is_err());
}

#[test]
fn fork_semantics_and_evidence_gates_hold() {
    // The source never mutates; derivation parallelism, fresh identity and
    // no tenant-state reuse are constants.
    const {
        assert!(SANDBOX_SNAPSHOT_FORK_SOURCE_SNAPSHOT_IMMUTABLE);
        assert!(SANDBOX_SNAPSHOT_FORK_PARALLEL_RUNNING);
        assert!(SANDBOX_SNAPSHOT_FORK_FRESH_IDENTITY_REQUIRED);
        assert!(!SANDBOX_SNAPSHOT_FORK_SOURCE_TENANT_STATE_REUSE_ALLOWED);
        assert!(SANDBOX_SNAPSHOT_REAL_KVM_RESTORE_EVIDENCE_REQUIRED);
        assert!(SANDBOX_SNAPSHOT_CROSS_TENANT_RESIDUE_EVIDENCE_REQUIRED);
        assert!(SANDBOX_SNAPSHOT_DERIVED_IDENTITY_ROTATION_EVIDENCE_REQUIRED);
        assert!(SANDBOX_SNAPSHOT_WARM_SLOT_REUSE_REQUIRES_POOL_EVIDENCE_GATE);
        assert!(!SANDBOX_SNAPSHOT_REGISTRATION_ENABLES_WARM_SLOT);
    }
    assert_eq!(SANDBOX_SNAPSHOT_ARTIFACT_AUTHORITY, "REQ-2026-0012");
    assert!(sandbox_snapshot_references_artifact_tuple());
    assert!(!sandbox_snapshot_owns_evidence_or_signature());
    assert!(!sandbox_second_supply_chain_authority_allowed());
    assert_eq!(
        SANDBOX_SNAPSHOT_LINE_LAYERING_CHECKPOINT_AUTHORITY,
        "REQ-2026-0021",
    );
}
