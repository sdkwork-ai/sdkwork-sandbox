//! Typed failures of the Workspace runtime transaction control plane.
//!
//! The variants carry exactly the thirteen contract error codes with their
//! declared retryability (`specs/sandbox-workspace-runtime-transaction.
//! contract.json`: `errors`). Display strings stay generic: record content
//! can carry tenant material and must never surface through an error.

/// Typed transaction control-plane failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SandboxWorkspaceRuntimeError {
    /// A request failed validation; nothing was mutated.
    #[error("sandbox workspace runtime request is invalid")]
    SandboxWorkspaceRuntimeInvalidRequest,
    /// Workspace authorization or revision verification denied the request.
    #[error("sandbox workspace runtime authorization denied")]
    SandboxWorkspaceRuntimeAuthorizationDenied,
    /// The Agents workspace revision moved; nothing was overwritten.
    #[error("sandbox workspace runtime revision conflict")]
    SandboxWorkspaceRuntimeRevisionConflict,
    /// The kernel execution placement reference or generation is stale.
    #[error("sandbox workspace runtime execution placement is stale")]
    SandboxWorkspaceRuntimeExecutionPlacementStale,
    /// Another transaction owns the writer lease or the runtime binding.
    #[error("sandbox workspace runtime writer conflict")]
    SandboxWorkspaceRuntimeWriterConflict,
    /// Admission rejected the request.
    #[error("sandbox workspace runtime admission rejected")]
    SandboxWorkspaceRuntimeAdmissionRejected,
    /// No capacity is available for the transaction.
    #[error("sandbox workspace runtime capacity unavailable")]
    SandboxWorkspaceRuntimeCapacityUnavailable,
    /// A stale fencing token was presented before a side effect.
    #[error("sandbox workspace runtime fencing token is stale")]
    SandboxWorkspaceRuntimeStaleFencing,
    /// The workspace projection failed to attach or acknowledge.
    #[error("sandbox workspace runtime attachment failed")]
    SandboxWorkspaceRuntimeAttachmentFailed,
    /// The command plane is unavailable for the transaction.
    #[error("sandbox workspace runtime command unavailable")]
    SandboxWorkspaceRuntimeCommandUnavailable,
    /// The durable checkpoint candidate or handoff failed.
    #[error("sandbox workspace runtime checkpoint failed")]
    SandboxWorkspaceRuntimeCheckpointFailed,
    /// Cleanup could not be proven complete; the transaction quarantines.
    #[error("sandbox workspace runtime cleanup incomplete")]
    SandboxWorkspaceRuntimeCleanupIncomplete,
    /// The transaction or its capacity is quarantined.
    #[error("sandbox workspace runtime is quarantined")]
    SandboxWorkspaceRuntimeQuarantined,
    /// A control-plane invariant failed (for example poisoned registry
    /// state); nothing was mutated.
    #[error("sandbox workspace runtime internal failure")]
    SandboxWorkspaceRuntimeInternalFailure,
}

impl SandboxWorkspaceRuntimeError {
    /// The exact contract error code.
    #[must_use]
    pub const fn sandbox_error_code(&self) -> &'static str {
        match self {
            Self::SandboxWorkspaceRuntimeInvalidRequest => {
                "sandbox_workspace_runtime_invalid_request"
            }
            Self::SandboxWorkspaceRuntimeAuthorizationDenied => {
                "sandbox_workspace_runtime_authorization_denied"
            }
            Self::SandboxWorkspaceRuntimeRevisionConflict => {
                "sandbox_workspace_runtime_revision_conflict"
            }
            Self::SandboxWorkspaceRuntimeExecutionPlacementStale => {
                "sandbox_workspace_runtime_execution_placement_stale"
            }
            Self::SandboxWorkspaceRuntimeWriterConflict => {
                "sandbox_workspace_runtime_writer_conflict"
            }
            Self::SandboxWorkspaceRuntimeAdmissionRejected => {
                "sandbox_workspace_runtime_admission_rejected"
            }
            Self::SandboxWorkspaceRuntimeCapacityUnavailable => {
                "sandbox_workspace_runtime_capacity_unavailable"
            }
            Self::SandboxWorkspaceRuntimeStaleFencing => "sandbox_workspace_runtime_stale_fencing",
            Self::SandboxWorkspaceRuntimeAttachmentFailed => {
                "sandbox_workspace_runtime_attachment_failed"
            }
            Self::SandboxWorkspaceRuntimeCommandUnavailable => {
                "sandbox_workspace_runtime_command_unavailable"
            }
            Self::SandboxWorkspaceRuntimeCheckpointFailed => {
                "sandbox_workspace_runtime_checkpoint_failed"
            }
            Self::SandboxWorkspaceRuntimeCleanupIncomplete => {
                "sandbox_workspace_runtime_cleanup_incomplete"
            }
            Self::SandboxWorkspaceRuntimeQuarantined => "sandbox_workspace_runtime_quarantined",
            Self::SandboxWorkspaceRuntimeInternalFailure => {
                "sandbox_workspace_runtime_internal_failure"
            }
        }
    }

    /// The contract-declared retryability.
    #[must_use]
    pub const fn sandbox_retryable(&self) -> bool {
        matches!(
            self,
            Self::SandboxWorkspaceRuntimeExecutionPlacementStale
                | Self::SandboxWorkspaceRuntimeWriterConflict
                | Self::SandboxWorkspaceRuntimeAdmissionRejected
                | Self::SandboxWorkspaceRuntimeCapacityUnavailable
                | Self::SandboxWorkspaceRuntimeStaleFencing
                | Self::SandboxWorkspaceRuntimeAttachmentFailed
                | Self::SandboxWorkspaceRuntimeCommandUnavailable
                | Self::SandboxWorkspaceRuntimeCheckpointFailed
                | Self::SandboxWorkspaceRuntimeCleanupIncomplete
                | Self::SandboxWorkspaceRuntimeInternalFailure
        )
    }
}

/// The result type every transaction control-plane entrypoint returns.
pub type SandboxWorkspaceRuntimeResult<T> = Result<T, SandboxWorkspaceRuntimeError>;
