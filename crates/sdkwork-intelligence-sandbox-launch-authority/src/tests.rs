//! Authority-model slice tests: the record validation, the closed lifecycle
//! with terminal immutability, the binding rules and the evidence/layering
//! gates of `specs/sandbox-instance-fast-start.contract.json`.

use crate::error::SandboxLaunchAuthorityError;
use crate::gates::{
    sandbox_launch_authority_model_slice_authorized, sandbox_launch_execution_or_worker_authorized,
    SANDBOX_LAUNCH_BUILD_AUTHORITY, SANDBOX_LAUNCH_COMMAND_EXECUTION_AUTHORITY,
    SANDBOX_LAUNCH_FIRST_COMMAND_ZERO_WAIT_EVIDENCE_REQUIRED,
    SANDBOX_LAUNCH_PLANNED_ALONE_AUTHORIZES_EXECUTION, SANDBOX_LAUNCH_POOL_CLAIM_AUTHORITY,
    SANDBOX_LAUNCH_REAL_FAST_START_RUNTIME_EVIDENCE_REQUIRED,
    SANDBOX_LAUNCH_REGISTRATION_ENABLES_WARM_SLOT,
    SANDBOX_LAUNCH_REGISTRATION_EXECUTES_NO_COMMANDS, SANDBOX_LAUNCH_SNAPSHOT_FORK_AUTHORITY,
    SANDBOX_LAUNCH_TEMPLATE_AUTHORITY, SANDBOX_LAUNCH_WARM_SLOT_GATE,
};
use crate::launch::SandboxInstanceLaunchPlan;
use crate::state::SandboxLaunchPlanState;

fn launch_plan() -> SandboxInstanceLaunchPlan {
    SandboxInstanceLaunchPlan::sandbox_new(
        "plan-1",
        "version-1",
        "claim-1",
        "identity-evidence-1",
        1_000,
    )
    .expect("valid plan")
}

#[test]
fn plans_reject_malformed_references_and_path_or_url_shapes() {
    for malformed in [
        ("", "version-1", "claim-1", "identity-evidence-1"),
        ("plan-1", "/absolute", "claim-1", "identity-evidence-1"),
        ("plan-1", "version-1", "https://x", "identity-evidence-1"),
        ("plan-1", "version-1", "claim-1", "has space"),
    ] {
        assert!(matches!(
            SandboxInstanceLaunchPlan::sandbox_new(
                malformed.0,
                malformed.1,
                malformed.2,
                malformed.3,
                1_000,
            ),
            Err(SandboxLaunchAuthorityError::SandboxLaunchInvalidPlan),
        ));
    }
    let plan = launch_plan();
    assert_eq!(
        plan.sandbox_instance_launch_plan_state(),
        SandboxLaunchPlanState::Planned
    );
}

#[test]
fn the_lifecycle_walks_planned_to_each_terminal_and_stops_there() {
    let mut consumed = launch_plan();
    consumed.sandbox_mark_consumed(1_100).expect("consumed");
    assert_eq!(
        consumed.sandbox_instance_launch_plan_state(),
        SandboxLaunchPlanState::Consumed
    );
    assert!(consumed.sandbox_expire(1_200).is_err());
    assert!(consumed.sandbox_quarantine(1_200).is_err());

    let mut expired = launch_plan();
    expired.sandbox_expire(1_100).expect("expired");
    assert_eq!(
        expired.sandbox_instance_launch_plan_state(),
        SandboxLaunchPlanState::Expired
    );
    assert!(expired.sandbox_mark_consumed(1_200).is_err());

    let mut quarantined = launch_plan();
    quarantined.sandbox_quarantine(1_100).expect("quarantined");
    assert_eq!(
        quarantined.sandbox_instance_launch_plan_state(),
        SandboxLaunchPlanState::Quarantined
    );
    assert!(quarantined.sandbox_mark_consumed(1_200).is_err());
    assert_eq!(quarantined.sandbox_planned_at(), 1_000);
    assert_eq!(quarantined.sandbox_updated_at(), 1_100);
}

#[test]
fn timestamps_may_never_move_backwards() {
    let mut plan = launch_plan();
    plan.sandbox_mark_consumed(1_100).expect("consumed");
    // The monotonic-timestamp check runs before the transition check, so a
    // backwards clock is a validation failure even on a terminal plan.
    assert!(matches!(
        plan.sandbox_expire(1_050),
        Err(SandboxLaunchAuthorityError::SandboxLaunchInvalidPlan),
    ));
}

#[test]
fn the_plan_references_never_copies_the_template_world() {
    let plan = launch_plan();
    assert_eq!(plan.sandbox_template_version_ref(), "version-1");
    assert_eq!(plan.sandbox_pool_claim_ref(), "claim-1");
    assert_eq!(
        plan.sandbox_fresh_guest_identity_evidence_ref(),
        "identity-evidence-1"
    );
}

#[test]
fn the_evidence_and_layering_gates_match_the_contract() {
    assert_eq!(SANDBOX_LAUNCH_TEMPLATE_AUTHORITY, "REQ-2026-0029");
    assert_eq!(SANDBOX_LAUNCH_BUILD_AUTHORITY, "REQ-2026-0032");
    assert_eq!(SANDBOX_LAUNCH_POOL_CLAIM_AUTHORITY, "REQ-2026-0019");
    assert_eq!(SANDBOX_LAUNCH_COMMAND_EXECUTION_AUTHORITY, "REQ-2026-0007");
    assert_eq!(SANDBOX_LAUNCH_SNAPSHOT_FORK_AUTHORITY, "REQ-2026-0031");
    assert_eq!(SANDBOX_LAUNCH_WARM_SLOT_GATE, "REQ-2026-0019");
    // The gate constants hold at compile time, like the other authority
    // lines.
    const {
        assert!(SANDBOX_LAUNCH_REAL_FAST_START_RUNTIME_EVIDENCE_REQUIRED);
        assert!(SANDBOX_LAUNCH_FIRST_COMMAND_ZERO_WAIT_EVIDENCE_REQUIRED);
        assert!(!SANDBOX_LAUNCH_REGISTRATION_ENABLES_WARM_SLOT);
        assert!(SANDBOX_LAUNCH_REGISTRATION_EXECUTES_NO_COMMANDS);
        assert!(!SANDBOX_LAUNCH_PLANNED_ALONE_AUTHORIZES_EXECUTION);
    }
    assert!(sandbox_launch_authority_model_slice_authorized());
    assert!(!sandbox_launch_execution_or_worker_authorized());
}
