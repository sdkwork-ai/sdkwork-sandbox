//! Authority-model slice tests: the record validation, the closed lifecycle
//! with terminal immutability and earned references, and the
//! evidence/layering gates of `specs/sandbox-worker.contract.json`.

use crate::error::SandboxWorkerAuthorityError;
use crate::execution::SandboxWorkerExecution;
use crate::gates::{
    sandbox_firecracker_lane_execution_authorized, sandbox_local_lane_execution_slice_authorized,
    sandbox_worker_authority_model_slice_authorized, SANDBOX_WORKER_COMMAND_EXECUTION_AUTHORITY,
    SANDBOX_WORKER_COMPLETION_MUST_BE_EARNED, SANDBOX_WORKER_FIRECRACKER_PROVIDER_AUTHORITY,
    SANDBOX_WORKER_FIRST_COMMAND_ZERO_WAIT_EVIDENCE_REQUIRED,
    SANDBOX_WORKER_KVM_LANE_EVIDENCE_REQUIRED, SANDBOX_WORKER_LAUNCH_PLAN_AUTHORITY,
    SANDBOX_WORKER_PROVIDER_SPI_BOUNDARY, SANDBOX_WORKER_REAL_EXECUTION_EVIDENCE_REQUIRED,
    SANDBOX_WORKER_REGISTRATION_DOES_NOT_TOUCH_THE_VMM,
    SANDBOX_WORKER_REGISTRATION_ENABLES_WARM_SLOT, SANDBOX_WORKER_WARM_SLOT_GATE,
};
use crate::state::SandboxWorkerExecutionState;

fn execution() -> SandboxWorkerExecution {
    SandboxWorkerExecution::sandbox_new("execution-1", "plan-1", 1_000).expect("valid execution")
}

#[test]
fn records_reject_malformed_references_and_path_or_url_shapes() {
    for (id, plan) in [
        ("", "plan-1"),
        ("/absolute", "plan-1"),
        ("execution-1", "https://x"),
        ("execution-1", "has space"),
    ] {
        assert!(matches!(
            SandboxWorkerExecution::sandbox_new(id, plan, 1_000),
            Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionInvalidRecord),
        ));
    }
    let record = execution();
    assert_eq!(
        record.sandbox_worker_execution_state(),
        SandboxWorkerExecutionState::Accepted
    );
    assert_eq!(record.sandbox_provider_allocation_ref(), None);
    assert_eq!(record.sandbox_start_command_execution_ref(), None);
}

#[test]
fn the_lifecycle_earns_references_in_order_and_reaches_started_only_through_starting() {
    let mut record = execution();
    // Provisioning earns the allocation reference.
    record
        .sandbox_record_provisioning("allocation-1", 1_100)
        .expect("provisioning");
    assert_eq!(
        record.sandbox_provider_allocation_ref(),
        Some("allocation-1")
    );
    // Starting earns the start-command reference.
    record
        .sandbox_record_starting("start-exec-1", 1_200)
        .expect("starting");
    assert_eq!(
        record.sandbox_start_command_execution_ref(),
        Some("start-exec-1")
    );
    record.sandbox_record_started(1_300).expect("started");
    assert_eq!(
        record.sandbox_worker_execution_state(),
        SandboxWorkerExecutionState::Started
    );
    assert_eq!(record.sandbox_accepted_at(), 1_000);
    assert_eq!(record.sandbox_updated_at(), 1_300);
    // Terminal: nothing rewrites the record.
    assert!(record.sandbox_record_failed(1_400).is_err());
    assert!(record.sandbox_quarantine(1_400).is_err());
}

#[test]
fn completions_and_references_cannot_be_claimed_out_of_order() {
    // `started` from `accepted` is illegal: completion must be earned.
    let mut eager = execution();
    assert!(matches!(
        eager.sandbox_record_started(1_100),
        Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionIllegalTransition),
    ));
    // `starting` without provisioning is illegal, and the executor reference
    // is never recorded on a refused transition.
    let mut skipping = execution();
    assert!(matches!(
        skipping.sandbox_record_starting("start-exec-1", 1_100),
        Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionIllegalTransition),
    ));
    assert_eq!(skipping.sandbox_start_command_execution_ref(), None);
    // Provisioning with a malformed allocation reference records nothing.
    let mut malformed = execution();
    assert!(matches!(
        malformed.sandbox_record_provisioning("has space", 1_100),
        Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionInvalidRecord),
    ));
    assert_eq!(
        malformed.sandbox_worker_execution_state(),
        SandboxWorkerExecutionState::Accepted
    );
    assert_eq!(malformed.sandbox_provider_allocation_ref(), None);
}

#[test]
fn uncertain_outcomes_quarantine_from_every_non_terminal_state() {
    for prepare in [
        |record: &mut SandboxWorkerExecution| {
            let _ = record;
        },
        |record: &mut SandboxWorkerExecution| {
            record
                .sandbox_record_provisioning("allocation-1", 1_100)
                .expect("provisioning");
        },
        |record: &mut SandboxWorkerExecution| {
            record
                .sandbox_record_provisioning("allocation-1", 1_100)
                .expect("provisioning");
            record
                .sandbox_record_starting("start-exec-1", 1_200)
                .expect("starting");
        },
    ] {
        let mut record = execution();
        prepare(&mut record);
        record.sandbox_quarantine(1_500).expect("quarantine");
        assert_eq!(
            record.sandbox_worker_execution_state(),
            SandboxWorkerExecutionState::Quarantined
        );
        assert!(record.sandbox_record_started(1_600).is_err());
    }
}

#[test]
fn timestamps_may_never_move_backwards() {
    let mut record = execution();
    record
        .sandbox_record_provisioning("allocation-1", 1_100)
        .expect("provisioning");
    // The monotonic-timestamp check runs before the transition check, so a
    // backwards clock is a validation failure even on an illegal transition.
    assert!(matches!(
        record.sandbox_record_starting("start-exec-1", 1_050),
        Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionInvalidRecord),
    ));
}

#[test]
fn the_evidence_and_layering_gates_match_the_contract() {
    assert_eq!(SANDBOX_WORKER_LAUNCH_PLAN_AUTHORITY, "REQ-2026-0033");
    assert_eq!(SANDBOX_WORKER_COMMAND_EXECUTION_AUTHORITY, "REQ-2026-0007");
    assert_eq!(SANDBOX_WORKER_PROVIDER_SPI_BOUNDARY, "REQ-2026-0002");
    assert_eq!(
        SANDBOX_WORKER_FIRECRACKER_PROVIDER_AUTHORITY,
        "REQ-2026-0008"
    );
    assert_eq!(SANDBOX_WORKER_WARM_SLOT_GATE, "REQ-2026-0019");
    // The gate constants hold at compile time, like the other authority
    // lines.
    const {
        assert!(SANDBOX_WORKER_REAL_EXECUTION_EVIDENCE_REQUIRED);
        assert!(SANDBOX_WORKER_KVM_LANE_EVIDENCE_REQUIRED);
        assert!(SANDBOX_WORKER_FIRST_COMMAND_ZERO_WAIT_EVIDENCE_REQUIRED);
        assert!(!SANDBOX_WORKER_REGISTRATION_ENABLES_WARM_SLOT);
        assert!(SANDBOX_WORKER_REGISTRATION_DOES_NOT_TOUCH_THE_VMM);
        assert!(SANDBOX_WORKER_COMPLETION_MUST_BE_EARNED);
    }
    assert!(sandbox_worker_authority_model_slice_authorized());
    assert!(!sandbox_local_lane_execution_slice_authorized());
    assert!(!sandbox_firecracker_lane_execution_authorized());
}
