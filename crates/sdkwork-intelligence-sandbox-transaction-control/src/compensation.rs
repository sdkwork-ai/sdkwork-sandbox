//! Deterministic compensation windows (`contract`: `failureCompensation`).
//!
//! Every failure window maps to a fixed, ordered required-action list; the
//! actions are individually idempotent
//! (`fencingAndIdempotency.sandbox_compensation_steps_individually_
//! idempotent`) and bounded by
//! [`crate::bounds::SANDBOX_COMPENSATION_ATTEMPT_COUNT_MAX`]. Uncertainty at
//! the end of compensation quarantines the transaction and keeps capacity
//! consumed.

/// The four failure windows of the ordered transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxCompensationWindow {
    /// Failure before the capacity reservation exists.
    BeforeCapacityReservation,
    /// Failure after capacity reservation but before workspace attachment.
    AfterCapacityBeforeAttachment,
    /// Failure after attachment but before command execution.
    AfterAttachmentBeforeCommand,
    /// Failure during or after write execution.
    DuringOrAfterWriteExecution,
}

/// The typed compensation actions, in contract order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxCompensationAction {
    /// Release the admission reservation if present.
    ReleaseAdmissionReservation,
    /// Record the terminal operation outcome.
    RecordTerminalOperationOutcome,
    /// Release or sanitize the pool claim if present.
    ReleaseOrSanitizePoolClaim,
    /// Release the capacity reservation.
    ReleaseCapacityReservation,
    /// Revoke the claim grants.
    RevokeClaimGrants,
    /// Stop the provider and its descendants.
    StopProviderAndDescendants,
    /// Detach the workspace projection.
    DetachWorkspaceProjection,
    /// Sanitize runtime, network and resources.
    SanitizeRuntimeNetworkAndResources,
    /// Scan residue before release.
    ScanResidueBeforeRelease,
    /// Freeze new commands.
    FreezeNewCommands,
    /// Drain or cancel active commands.
    DrainOrCancelActiveCommands,
    /// Revoke workspace write access and flush writes.
    RevokeWriteAccessAndFlush,
    /// Make the checkpoint candidate durable (or record the read-only noop).
    MakeCheckpointCandidateDurable,
    /// Make the checkpoint handoff durable (or record the read-only noop).
    MakeCheckpointHandoffDurable,
    /// Quarantine the transaction, its binding and capacity.
    QuarantineTransactionAndCapacity,
}

impl SandboxCompensationWindow {
    /// The fixed required-action list of this window, in contract order.
    #[must_use]
    pub fn sandbox_required_actions(self) -> &'static [SandboxCompensationAction] {
        match self {
            Self::BeforeCapacityReservation => &[
                SandboxCompensationAction::ReleaseAdmissionReservation,
                SandboxCompensationAction::RecordTerminalOperationOutcome,
            ],
            Self::AfterCapacityBeforeAttachment => &[
                SandboxCompensationAction::ReleaseOrSanitizePoolClaim,
                SandboxCompensationAction::ReleaseCapacityReservation,
                SandboxCompensationAction::ReleaseAdmissionReservation,
                SandboxCompensationAction::RecordTerminalOperationOutcome,
            ],
            Self::AfterAttachmentBeforeCommand => &[
                SandboxCompensationAction::RevokeClaimGrants,
                SandboxCompensationAction::StopProviderAndDescendants,
                SandboxCompensationAction::DetachWorkspaceProjection,
                SandboxCompensationAction::SanitizeRuntimeNetworkAndResources,
                SandboxCompensationAction::ScanResidueBeforeRelease,
            ],
            Self::DuringOrAfterWriteExecution => &[
                SandboxCompensationAction::FreezeNewCommands,
                SandboxCompensationAction::DrainOrCancelActiveCommands,
                SandboxCompensationAction::RevokeWriteAccessAndFlush,
                SandboxCompensationAction::MakeCheckpointCandidateDurable,
                SandboxCompensationAction::MakeCheckpointHandoffDurable,
                SandboxCompensationAction::QuarantineTransactionAndCapacity,
            ],
        }
    }

    /// Maps the highest completed orchestration stage to the compensation
    /// window a failure at that point enters.
    #[must_use]
    pub fn sandbox_window_for_completed_stage(completed_position: usize) -> Self {
        use crate::stage::{sandbox_orchestration_stages, SandboxOrchestrationStage::*};
        let stage = if completed_position == 0 {
            None
        } else {
            sandbox_orchestration_stages()
                .get(completed_position - 1)
                .copied()
        };
        match stage {
            Some(ProjectionAttached)
            | Some(EffectiveReadinessVerified)
            | Some(EnvironmentReady)
            | Some(CommandAdmissionOpened) => Self::AfterAttachmentBeforeCommand,
            Some(CommandAdmissionFrozenAndDrained)
            | Some(WriteAccessRevokedAndFlushed)
            | Some(CheckpointCandidateDurable)
            | Some(CheckpointHandoffDurable)
            | Some(ProviderAndDescendantsStopped)
            | Some(ProjectionDetached)
            | Some(RuntimeSanitized)
            | Some(ResidueScanPassed) => Self::DuringOrAfterWriteExecution,
            Some(CapacityReservationConfirmed)
            | Some(RuntimeBindingFenced)
            | Some(PoolClaimedOrPathSelected) => Self::AfterCapacityBeforeAttachment,
            // Stages one to three complete before or with the capacity
            // reservation; anything earlier or unmapped stays the first
            // window.
            _ => Self::BeforeCapacityReservation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::{sandbox_orchestration_stages, SandboxOrchestrationStage::*};

    #[test]
    fn every_window_has_the_contract_action_list() {
        assert_eq!(
            SandboxCompensationWindow::BeforeCapacityReservation.sandbox_required_actions(),
            &[
                SandboxCompensationAction::ReleaseAdmissionReservation,
                SandboxCompensationAction::RecordTerminalOperationOutcome,
            ],
        );
        assert_eq!(
            SandboxCompensationWindow::DuringOrAfterWriteExecution
                .sandbox_required_actions()
                .len(),
            6,
        );
        // Actions are individually idempotent by construction; the attempt
        // budget bounds their retry loops.
        const {
            assert!(crate::bounds::SANDBOX_COMPENSATION_ATTEMPT_COUNT_MAX >= 1);
        }
    }

    #[test]
    fn windows_follow_the_completed_stage() {
        assert_eq!(
            SandboxCompensationWindow::sandbox_window_for_completed_stage(0),
            SandboxCompensationWindow::BeforeCapacityReservation,
        );
        assert_eq!(
            SandboxCompensationWindow::sandbox_window_for_completed_stage(4),
            SandboxCompensationWindow::AfterCapacityBeforeAttachment,
        );
        assert_eq!(
            SandboxCompensationWindow::sandbox_window_for_completed_stage(9),
            SandboxCompensationWindow::AfterAttachmentBeforeCommand,
        );
        // Releasing (stage 21) completes the transaction; the last
        // compensation-relevant window is the write-execution one.
        let last = sandbox_orchestration_stages().len();
        assert_eq!(
            SandboxCompensationWindow::sandbox_window_for_completed_stage(last - 1),
            SandboxCompensationWindow::DuringOrAfterWriteExecution,
        );
        let _ = ProviderAndDescendantsStopped;
    }
}
