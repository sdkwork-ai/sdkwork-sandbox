//! Control-plane slice tests: the transaction state machine, the fixed stage
//! order, checkpoint CAS promotion, compensation windows and the bounded
//! registry of `specs/sandbox-workspace-runtime-transaction.contract.json`.

use std::collections::BTreeSet;

use crate::checkpoint::SandboxWorkspaceCheckpointCandidate;
use crate::compensation::{SandboxCompensationAction, SandboxCompensationWindow};
use crate::error::SandboxWorkspaceRuntimeError;
use crate::registry::{
    BoundedSandboxWorkspaceTransactionControl, SandboxTerminalOutcome, SandboxWorkspaceMountMode,
    SandboxWorkspaceRuntimeTransactionRequest,
};
use crate::stage::{SandboxOrchestrationLedger, SandboxOrchestrationStage, SandboxStageEvidence};
use crate::state::SandboxWorkspaceRuntimeTransactionState;

fn sandbox_fingerprint(seed: u8) -> String {
    format!("{seed:064}")
}

fn sandbox_request(
    operation: &str,
    session: &str,
    mode: SandboxWorkspaceMountMode,
) -> SandboxWorkspaceRuntimeTransactionRequest {
    SandboxWorkspaceRuntimeTransactionRequest {
        sandbox_workspace_runtime_transaction_id: format!("txn-{operation}"),
        sandbox_tenant_scope_hash: "scope-hash-1".into(),
        sandbox_workspace_id: "workspace-1".into(),
        sandbox_workspace_revision_ref: "revision-7".into(),
        sandbox_workspace_authorization_grant_ref: "grant-1".into(),
        sandbox_session_id: session.into(),
        sandbox_kernel_execution_placement_ref: "placement-1".into(),
        sandbox_kernel_execution_placement_generation: 3,
        sandbox_operation_id: operation.into(),
        sandbox_execution_profile_id: "profile-1".into(),
        sandbox_required_capabilities: BTreeSet::from(["fs".into(), "shell".into()])
            .into_iter()
            .collect(),
        sandbox_minimum_assurance: "micro_vm".into(),
        sandbox_workload_class_ref: "workload-standard".into(),
        sandbox_workspace_mount_mode: mode,
        sandbox_request_fingerprint: sandbox_fingerprint(1),
        sandbox_fencing_token: 0,
        sandbox_deadline_at: 2_000,
        sandbox_trace_id: "trace-1".into(),
    }
}

fn sandbox_candidate() -> SandboxWorkspaceCheckpointCandidate {
    SandboxWorkspaceCheckpointCandidate::sandbox_new(
        "candidate-1",
        "workspace-1",
        "revision-7",
        "revision-8",
        "txn-op-1",
        "session-1",
        "binding-1",
        1,
        &sandbox_fingerprint(9),
        4096,
        "storage-authority-ref-1",
        1_500,
        "trace-1",
    )
    .expect("valid candidate")
}

/// Drives a read-write transaction through every stage with completed
/// evidence and returns it released.
fn sandbox_drive_to_release(
    control: &BoundedSandboxWorkspaceTransactionControl,
    operation: &str,
    session: &str,
) -> SandboxTerminalOutcome {
    let handle = control
        .sandbox_begin(sandbox_request(
            operation,
            session,
            SandboxWorkspaceMountMode::ReadWrite,
        ))
        .expect("begin");
    let token = handle.sandbox_fencing_token;
    if operation == "op-1" {
        let mut candidate = sandbox_candidate();
        candidate.sandbox_seal();
        control
            .sandbox_attach_checkpoint_candidate("txn-op-1", candidate, token)
            .expect("attach");
        control
            .sandbox_record_checkpoint_handoff("txn-op-1", token)
            .expect("handoff");
    }
    for stage in crate::stage::sandbox_orchestration_stages() {
        control
            .sandbox_advance_stage(
                handle.sandbox_workspace_runtime_transaction_id.as_str(),
                *stage,
                SandboxStageEvidence::Completed {
                    sandbox_evidence_fingerprint: sandbox_fingerprint(
                        stage.sandbox_position() as u8 + 10,
                    ),
                },
                token,
            )
            .unwrap_or_else(|error| {
                panic!("stage {} must advance: {error}", stage.sandbox_position())
            });
    }
    control
        .sandbox_release(
            handle.sandbox_workspace_runtime_transaction_id.as_str(),
            token,
        )
        .expect("release")
}

#[test]
fn begin_is_fenced_idempotent_and_single_owner_per_binding() {
    let control =
        BoundedSandboxWorkspaceTransactionControl::sandbox_with_clock(std::sync::Arc::new(|| {
            1_000
        }));
    let handle = control
        .sandbox_begin(sandbox_request(
            "op-1",
            "session-1",
            SandboxWorkspaceMountMode::ReadWrite,
        ))
        .expect("begin");
    assert_eq!(
        handle.sandbox_state,
        SandboxWorkspaceRuntimeTransactionState::Requested
    );
    assert!(handle.sandbox_fencing_token > 0);

    // Same operation + same fingerprint replays the same handle.
    let replay = control
        .sandbox_begin(sandbox_request(
            "op-1",
            "session-1",
            SandboxWorkspaceMountMode::ReadWrite,
        ))
        .expect("replay");
    assert_eq!(handle, replay);

    // Same operation, different fingerprint conflicts.
    let mut conflicting =
        sandbox_request("op-1", "session-1", SandboxWorkspaceMountMode::ReadWrite);
    conflicting.sandbox_request_fingerprint = sandbox_fingerprint(2);
    assert!(matches!(
        control.sandbox_begin(conflicting),
        Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest),
    ));

    // A second active transaction on the same session conflicts.
    let second = sandbox_request("op-2", "session-1", SandboxWorkspaceMountMode::ReadWrite);
    assert!(matches!(
        control.sandbox_begin(second),
        Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeWriterConflict),
    ));

    // A stale fencing token is refused.
    let mut stale = sandbox_request("op-3", "session-2", SandboxWorkspaceMountMode::ReadWrite);
    stale.sandbox_fencing_token = -1;
    assert!(control.sandbox_begin(stale).is_err());
}

#[test]
fn read_write_release_requires_durable_candidate_and_handoff() {
    let control =
        BoundedSandboxWorkspaceTransactionControl::sandbox_with_clock(std::sync::Arc::new(|| {
            1_000
        }));
    let handle = control
        .sandbox_begin(sandbox_request(
            "op-1",
            "session-1",
            SandboxWorkspaceMountMode::ReadWrite,
        ))
        .expect("begin");
    let token = handle.sandbox_fencing_token;
    for stage in crate::stage::sandbox_orchestration_stages() {
        control
            .sandbox_advance_stage(
                handle.sandbox_workspace_runtime_transaction_id.as_str(),
                *stage,
                SandboxStageEvidence::Completed {
                    sandbox_evidence_fingerprint: sandbox_fingerprint(
                        stage.sandbox_position() as u8 + 20,
                    ),
                },
                token,
            )
            .expect("stage");
    }
    // Read-write release without a candidate and handoff is refused.
    assert!(matches!(
        control.sandbox_release(
            handle.sandbox_workspace_runtime_transaction_id.as_str(),
            token
        ),
        Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeCheckpointFailed),
    ));
}

#[test]
fn read_only_release_records_the_no_checkpoint_outcome() {
    let control =
        BoundedSandboxWorkspaceTransactionControl::sandbox_with_clock(std::sync::Arc::new(|| {
            1_000
        }));
    let outcome = {
        let handle = control
            .sandbox_begin(sandbox_request(
                "op-ro",
                "session-ro",
                SandboxWorkspaceMountMode::ReadOnly,
            ))
            .expect("begin");
        let token = handle.sandbox_fencing_token;
        for stage in crate::stage::sandbox_orchestration_stages() {
            let evidence = if matches!(
                stage,
                SandboxOrchestrationStage::CheckpointCandidateDurable
                    | SandboxOrchestrationStage::CheckpointHandoffDurable
            ) {
                SandboxStageEvidence::NotApplicable {
                    sandbox_evidence_fingerprint: sandbox_fingerprint(
                        stage.sandbox_position() as u8 + 30,
                    ),
                }
            } else {
                SandboxStageEvidence::Completed {
                    sandbox_evidence_fingerprint: sandbox_fingerprint(
                        stage.sandbox_position() as u8 + 30,
                    ),
                }
            };
            control
                .sandbox_advance_stage(
                    handle.sandbox_workspace_runtime_transaction_id.as_str(),
                    *stage,
                    evidence,
                    token,
                )
                .unwrap_or_else(|error| panic!("stage {:?}: {error}", stage.sandbox_name()));
        }
        control
            .sandbox_release(
                handle.sandbox_workspace_runtime_transaction_id.as_str(),
                token,
            )
            .expect("release")
    };
    assert_eq!(outcome, SandboxTerminalOutcome::Released);
}

#[test]
fn the_full_read_write_path_releases_and_frees_the_binding() {
    let control =
        BoundedSandboxWorkspaceTransactionControl::sandbox_with_clock(std::sync::Arc::new(|| {
            1_000
        }));
    let outcome = sandbox_drive_to_release(&control, "op-1", "session-1");
    assert_eq!(outcome, SandboxTerminalOutcome::Released);
    // The binding is free; a new transaction on the same session begins,
    // observing the fencing token the previous transaction issued.
    let mut second = sandbox_request("op-9", "session-1", SandboxWorkspaceMountMode::ReadOnly);
    second.sandbox_fencing_token = 1;
    assert!(control.sandbox_begin(second).is_ok());
}

#[test]
fn checkpoint_promotion_is_a_non_destructive_agents_only_cas() {
    let mut candidate = sandbox_candidate();
    // An unsealed candidate cannot promote.
    assert!(matches!(
        candidate.sandbox_promote("revision-7"),
        Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeCheckpointFailed),
    ));
    candidate.sandbox_seal();
    assert!(candidate.sandbox_is_sealed());
    // Wrong expected source revision: non-destructive conflict.
    assert!(matches!(
        candidate.sandbox_promote("revision-99"),
        Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeRevisionConflict),
    ));
    assert_eq!(
        candidate.sandbox_promote("revision-7"),
        Ok(crate::checkpoint::SandboxCandidatePromotion::Promoted),
    );
    // Same expected revision replays idempotently.
    assert_eq!(
        candidate.sandbox_promote("revision-7"),
        Ok(crate::checkpoint::SandboxCandidatePromotion::AlreadyPromoted),
    );
}

#[test]
fn compensation_windows_follow_the_completed_stage() {
    let (window, actions) = {
        let control = BoundedSandboxWorkspaceTransactionControl::sandbox_with_clock(
            std::sync::Arc::new(|| 1_000),
        );
        let handle = control
            .sandbox_begin(sandbox_request(
                "op-1",
                "session-1",
                SandboxWorkspaceMountMode::ReadWrite,
            ))
            .expect("begin");
        let token = handle.sandbox_fencing_token;
        for stage in crate::stage::sandbox_orchestration_stages().iter().take(9) {
            control
                .sandbox_advance_stage(
                    handle.sandbox_workspace_runtime_transaction_id.as_str(),
                    *stage,
                    SandboxStageEvidence::Completed {
                        sandbox_evidence_fingerprint: sandbox_fingerprint(
                            stage.sandbox_position() as u8 + 40,
                        ),
                    },
                    token,
                )
                .expect("stage");
        }
        control
            .sandbox_enter_compensation(
                handle.sandbox_workspace_runtime_transaction_id.as_str(),
                token,
            )
            .expect("compensation")
    };
    assert_eq!(
        window,
        SandboxCompensationWindow::AfterAttachmentBeforeCommand
    );
    assert!(actions.contains(&SandboxCompensationAction::ScanResidueBeforeRelease));
}

#[test]
fn quarantine_is_terminal_and_keeps_the_registry_consistent() {
    let control =
        BoundedSandboxWorkspaceTransactionControl::sandbox_with_clock(std::sync::Arc::new(|| {
            1_000
        }));
    let handle = control
        .sandbox_begin(sandbox_request(
            "op-1",
            "session-1",
            SandboxWorkspaceMountMode::ReadWrite,
        ))
        .expect("begin");
    let outcome = control
        .sandbox_quarantine(
            handle.sandbox_workspace_runtime_transaction_id.as_str(),
            handle.sandbox_fencing_token,
        )
        .expect("quarantine");
    assert_eq!(outcome, SandboxTerminalOutcome::Quarantined);
    // Terminal: replay returns the same outcome without re-opening.
    let replay = control
        .sandbox_quarantine(
            handle.sandbox_workspace_runtime_transaction_id.as_str(),
            handle.sandbox_fencing_token,
        )
        .expect("replay");
    assert_eq!(replay, SandboxTerminalOutcome::Quarantined);
    // The binding is free again; a new begin observes the issued token.
    let mut second = sandbox_request("op-2", "session-1", SandboxWorkspaceMountMode::ReadOnly);
    second.sandbox_fencing_token = handle.sandbox_fencing_token;
    assert!(control.sandbox_begin(second).is_ok());
}

#[test]
fn a_ledger_never_infers_a_stage_from_a_later_one() {
    let mut ledger = SandboxOrchestrationLedger::sandbox_new();
    assert_eq!(ledger.sandbox_highest_completed_position(), 0);
    let stages = crate::stage::sandbox_orchestration_stages();
    for (index, stage) in stages.iter().enumerate() {
        ledger
            .sandbox_record(
                *stage,
                SandboxStageEvidence::Completed {
                    sandbox_evidence_fingerprint: sandbox_fingerprint(index as u8 + 50),
                },
            )
            .unwrap_or_else(|error| panic!("stage {}: {error}", stage.sandbox_position()));
        assert_eq!(ledger.sandbox_highest_completed_position(), index + 1);
    }
    assert!(ledger.sandbox_is_completed(SandboxOrchestrationStage::EnvironmentReady));
    assert!(ledger
        .sandbox_completed_ref(SandboxOrchestrationStage::EnvironmentReady)
        .is_some());
}
