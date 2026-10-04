//! The fixed 21-stage orchestration order (`contract`: `orchestrationOrder`
//! and `orderRules`).
//!
//! Stages complete strictly in order. A stage that does not apply to the
//! selected path still completes — with a durable typed not-applicable
//! evidence fingerprint (`sandbox_not_applicable_stage_requires_durable_typed_
//! noop_evidence`) — and no stage may be inferred only from a later stage
//! (`sandbox_no_stage_may_be_inferred_only_from_later_stage`).

use crate::bounds::SANDBOX_ORCHESTRATION_STAGE_COUNT_MAX;
use crate::error::{SandboxWorkspaceRuntimeError, SandboxWorkspaceRuntimeResult};

/// The ordered orchestration stages.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(u8)]
pub enum SandboxOrchestrationStage {
    /// Workspace authorization and revision verified.
    WorkspaceAuthorizationVerified = 1,
    /// Kernel execution placement reference and generation verified.
    KernelPlacementVerified,
    /// Admission reservation confirmed or local policy admitted.
    AdmissionConfirmed,
    /// Verified node and capacity reservation confirmed (or local N/A).
    CapacityReservationConfirmed,
    /// Runtime binding persisted and fenced.
    RuntimeBindingFenced,
    /// Pool claimed, or the cold or local path selected.
    PoolClaimedOrPathSelected,
    /// Fresh identity and workspace/network/resource grants verified.
    FreshGrantsVerified,
    /// Provider allocate and start.
    ProviderAllocateAndStart,
    /// Workspace projection attached and guest/host acknowledged.
    ProjectionAttached,
    /// Effective workspace/network/resource/command policy readiness verified.
    EffectiveReadinessVerified,
    /// Environment ready.
    EnvironmentReady,
    /// Command admission opened.
    CommandAdmissionOpened,
    /// Command admission frozen and active commands drained.
    CommandAdmissionFrozenAndDrained,
    /// Workspace write access revoked and writes flushed.
    WriteAccessRevokedAndFlushed,
    /// Checkpoint candidate durable, or a read-only noop recorded.
    CheckpointCandidateDurable,
    /// Checkpoint handoff durable, or a read-only noop recorded.
    CheckpointHandoffDurable,
    /// Provider and descendants stopped.
    ProviderAndDescendantsStopped,
    /// Workspace projection detached.
    ProjectionDetached,
    /// Network, resource and ephemeral runtime sanitized.
    RuntimeSanitized,
    /// Cross-tenant residue scan passed (or local zero-residue verified).
    ResidueScanPassed,
    /// Pool capacity and admission released (or local noop recorded).
    CapacityAndAdmissionReleased,
}

use SandboxOrchestrationStage::*;

/// The full ordered stage list, index 0 = stage 1.
///
/// The slice length is far below [`SANDBOX_ORCHESTRATION_STAGE_COUNT_MAX`].
#[must_use]
pub fn sandbox_orchestration_stages() -> &'static [SandboxOrchestrationStage] {
    &[
        WorkspaceAuthorizationVerified,
        KernelPlacementVerified,
        AdmissionConfirmed,
        CapacityReservationConfirmed,
        RuntimeBindingFenced,
        PoolClaimedOrPathSelected,
        FreshGrantsVerified,
        ProviderAllocateAndStart,
        ProjectionAttached,
        EffectiveReadinessVerified,
        EnvironmentReady,
        CommandAdmissionOpened,
        CommandAdmissionFrozenAndDrained,
        WriteAccessRevokedAndFlushed,
        CheckpointCandidateDurable,
        CheckpointHandoffDurable,
        ProviderAndDescendantsStopped,
        ProjectionDetached,
        RuntimeSanitized,
        ResidueScanPassed,
        CapacityAndAdmissionReleased,
    ]
}

impl SandboxOrchestrationStage {
    /// The one-based contract position.
    #[must_use]
    pub const fn sandbox_position(self) -> usize {
        self as usize
    }

    /// The contract stage name.
    #[must_use]
    pub fn sandbox_name(self) -> &'static str {
        sandbox_orchestration_stages()[self.sandbox_position() - 1].sandbox_name_const()
    }

    const fn sandbox_name_const(self) -> &'static str {
        match self {
            WorkspaceAuthorizationVerified => {
                "sandbox_workspace_authorization_and_revision_verified"
            }
            KernelPlacementVerified => {
                "sandbox_kernel_execution_placement_reference_and_generation_verified"
            }
            AdmissionConfirmed => "sandbox_admission_reservation_confirmed_or_local_policy_admitted",
            CapacityReservationConfirmed => {
                "sandbox_verified_node_and_capacity_reservation_confirmed_or_local_capacity_marked_not_applicable"
            }
            RuntimeBindingFenced => "sandbox_runtime_binding_persisted_and_fenced",
            PoolClaimedOrPathSelected => "sandbox_pool_claimed_or_cold_or_local_path_selected",
            FreshGrantsVerified => {
                "sandbox_fresh_identity_workspace_network_and_resource_grants_verified"
            }
            ProviderAllocateAndStart => "sandbox_provider_allocate_and_start",
            ProjectionAttached => {
                "sandbox_workspace_projection_attached_and_guest_or_host_acknowledged"
            }
            EffectiveReadinessVerified => {
                "sandbox_effective_workspace_network_resource_and_command_policy_readiness_verified"
            }
            EnvironmentReady => "sandbox_environment_ready",
            CommandAdmissionOpened => "sandbox_command_admission_opened",
            CommandAdmissionFrozenAndDrained => {
                "sandbox_command_admission_frozen_and_active_commands_drained"
            }
            WriteAccessRevokedAndFlushed => {
                "sandbox_workspace_write_access_revoked_and_writes_flushed"
            }
            CheckpointCandidateDurable => {
                "sandbox_workspace_checkpoint_candidate_made_durable_or_read_only_noop_recorded"
            }
            CheckpointHandoffDurable => {
                "sandbox_checkpoint_handoff_made_durable_or_read_only_noop_recorded"
            }
            ProviderAndDescendantsStopped => "sandbox_provider_and_descendants_stopped",
            ProjectionDetached => "sandbox_workspace_projection_detached",
            RuntimeSanitized => "sandbox_network_resource_and_ephemeral_runtime_sanitized",
            ResidueScanPassed => {
                "sandbox_cross_tenant_residue_scan_passed_or_local_zero_residue_verified"
            }
            CapacityAndAdmissionReleased => {
                "sandbox_pool_capacity_and_admission_released_or_local_noop_recorded"
            }
        }
    }
}

/// How a stage completed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SandboxStageEvidence {
    /// The stage ran and completed; the fingerprint identifies the durable
    /// evidence record (lowercase hex SHA-256).
    Completed {
        sandbox_evidence_fingerprint: String,
    },
    /// The stage does not apply to the selected path; the fingerprint
    /// identifies the durable typed not-applicable record.
    NotApplicable {
        sandbox_evidence_fingerprint: String,
    },
}

impl SandboxStageEvidence {
    fn sandbox_validated(&self) -> SandboxWorkspaceRuntimeResult<()> {
        let fingerprint = match self {
            Self::Completed {
                sandbox_evidence_fingerprint,
            }
            | SandboxStageEvidence::NotApplicable {
                sandbox_evidence_fingerprint,
            } => sandbox_evidence_fingerprint,
        };
        let valid = fingerprint.len() == 64
            && fingerprint
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if valid {
            Ok(())
        } else {
            Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest)
        }
    }
}

/// The ordered completion ledger of one transaction.
///
/// Backed by a fixed-size completion vector; stages outside the contract
/// order cannot exist, and completion past the last stage is impossible by
/// construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxOrchestrationLedger {
    sandbox_completed: [Option<SandboxStageEvidence>; SANDBOX_ORCHESTRATION_STAGE_COUNT_MAX],
    sandbox_highest_completed_position: usize,
}

impl SandboxOrchestrationLedger {
    /// A fresh ledger with no stage completed.
    #[must_use]
    pub const fn sandbox_new() -> Self {
        Self {
            sandbox_completed: [
                None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                None, None, None, None, None, None, None, None, None, None, None, None, None, None,
                None, None, None, None,
            ],
            sandbox_highest_completed_position: 0,
        }
    }

    /// Whether the stage has completed (with either evidence kind).
    #[must_use]
    pub fn sandbox_is_completed(&self, stage: SandboxOrchestrationStage) -> bool {
        self.sandbox_completed[stage.sandbox_position() - 1].is_some()
    }

    /// The recorded evidence of one stage, when completed.
    #[must_use]
    pub fn sandbox_completed_ref(
        &self,
        stage: SandboxOrchestrationStage,
    ) -> Option<&SandboxStageEvidence> {
        self.sandbox_completed[stage.sandbox_position() - 1].as_ref()
    }

    /// The one-based position of the highest completed stage; zero when none.
    #[must_use]
    pub const fn sandbox_highest_completed_position(&self) -> usize {
        self.sandbox_highest_completed_position
    }

    /// Records one stage completion, enforcing the contract order: the stage
    /// must be exactly the next incomplete stage — no stage may be inferred
    /// from a later stage, and completed stages are immutable.
    pub fn sandbox_record(
        &mut self,
        stage: SandboxOrchestrationStage,
        evidence: SandboxStageEvidence,
    ) -> SandboxWorkspaceRuntimeResult<()> {
        evidence.sandbox_validated()?;
        let position = stage.sandbox_position();
        if position > SANDBOX_ORCHESTRATION_STAGE_COUNT_MAX {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        if self.sandbox_completed[position - 1].is_some() {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        if position != self.sandbox_highest_completed_position + 1 {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        self.sandbox_completed[position - 1] = Some(evidence);
        self.sandbox_highest_completed_position = position;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(seed: u8) -> String {
        format!("{seed:064}")
    }

    #[test]
    fn stages_are_exactly_the_contract_order() {
        let stages = sandbox_orchestration_stages();
        assert_eq!(stages.len(), 21);
        assert_eq!(stages[0], WorkspaceAuthorizationVerified);
        assert_eq!(stages[20], CapacityAndAdmissionReleased);
        for (index, stage) in stages.iter().enumerate() {
            assert_eq!(stage.sandbox_position(), index + 1);
        }
        assert!(stages.len() <= SANDBOX_ORCHESTRATION_STAGE_COUNT_MAX);
    }

    #[test]
    fn the_ledger_enforces_strict_order_and_immutable_completion() {
        let mut ledger = SandboxOrchestrationLedger::sandbox_new();
        // Skipping stage 1 is refused.
        assert!(ledger
            .sandbox_record(
                CapacityReservationConfirmed,
                SandboxStageEvidence::Completed {
                    sandbox_evidence_fingerprint: fingerprint(1)
                }
            )
            .is_err());
        ledger
            .sandbox_record(
                WorkspaceAuthorizationVerified,
                SandboxStageEvidence::Completed {
                    sandbox_evidence_fingerprint: fingerprint(1),
                },
            )
            .expect("stage 1");
        // Re-completing a completed stage is refused.
        assert!(ledger
            .sandbox_record(
                WorkspaceAuthorizationVerified,
                SandboxStageEvidence::Completed {
                    sandbox_evidence_fingerprint: fingerprint(2)
                }
            )
            .is_err());
        // A typed not-applicable record completes a stage too.
        ledger
            .sandbox_record(
                KernelPlacementVerified,
                SandboxStageEvidence::NotApplicable {
                    sandbox_evidence_fingerprint: fingerprint(3),
                },
            )
            .expect("stage 2 N/A");
        assert_eq!(ledger.sandbox_highest_completed_position(), 2);
        // Malformed evidence is refused before anything is recorded.
        assert!(ledger
            .sandbox_record(
                AdmissionConfirmed,
                SandboxStageEvidence::Completed {
                    sandbox_evidence_fingerprint: "nope".into()
                }
            )
            .is_err());
        assert_eq!(ledger.sandbox_highest_completed_position(), 2);
    }
}
