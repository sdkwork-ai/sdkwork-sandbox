//! The transaction state machine (`contract`: `transaction`).
//!
//! Eleven states with a closed transition table
//! (`unknownStateRejected` / `illegalTransitionRejected`). `released` is the
//! successful terminal, `quarantined` the security-failure terminal, and a
//! terminal outcome stays independent from the resource-release outcome
//! (`terminalOutcomeIndependentFromResourceReleaseOutcome`).

use std::fmt;

/// Lifecycle states of one workspace runtime transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxWorkspaceRuntimeTransactionState {
    /// The request was validated and recorded.
    Requested,
    /// Admission or local policy admitted the transaction.
    Admitted,
    /// Capacity (or a typed local not-applicable) is reserved.
    CapacityReserved,
    /// The runtime binding is persisted and fenced.
    RuntimeBound,
    /// The workspace projection is attaching.
    WorkspaceAttaching,
    /// Effective readiness is verified and the environment is ready.
    Ready,
    /// Commands are executing.
    Executing,
    /// Commands are frozen/drained and the checkpoint is being made durable.
    Checkpointing,
    /// A failure window is running deterministic compensation.
    Compensating,
    /// Successful terminal; every ordered stage completed with evidence.
    Released,
    /// Security-failure terminal; capacity stays consumed.
    Quarantined,
}

impl SandboxWorkspaceRuntimeTransactionState {
    /// The contract state name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Admitted => "admitted",
            Self::CapacityReserved => "capacity_reserved",
            Self::RuntimeBound => "runtime_bound",
            Self::WorkspaceAttaching => "workspace_attaching",
            Self::Ready => "ready",
            Self::Executing => "executing",
            Self::Checkpointing => "checkpointing",
            Self::Compensating => "compensating",
            Self::Released => "released",
            Self::Quarantined => "quarantined",
        }
    }

    /// Parses a contract state name; unknown names are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "requested" => Self::Requested,
            "admitted" => Self::Admitted,
            "capacity_reserved" => Self::CapacityReserved,
            "runtime_bound" => Self::RuntimeBound,
            "workspace_attaching" => Self::WorkspaceAttaching,
            "ready" => Self::Ready,
            "executing" => Self::Executing,
            "checkpointing" => Self::Checkpointing,
            "compensating" => Self::Compensating,
            "released" => Self::Released,
            "quarantined" => Self::Quarantined,
            _ => return None,
        })
    }

    /// The closed transition table. `released` and `quarantined` are
    /// terminal; `compensating` may land in either terminal or return to the
    /// state the compensation recovered.
    #[must_use]
    pub const fn sandbox_can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Requested, Self::Admitted)
                | (Self::Requested, Self::Compensating)
                | (Self::Requested, Self::Quarantined)
                | (Self::Admitted, Self::CapacityReserved)
                | (Self::Admitted, Self::Compensating)
                | (Self::Admitted, Self::Quarantined)
                | (Self::CapacityReserved, Self::RuntimeBound)
                | (Self::CapacityReserved, Self::Compensating)
                | (Self::CapacityReserved, Self::Quarantined)
                | (Self::RuntimeBound, Self::WorkspaceAttaching)
                | (Self::RuntimeBound, Self::Compensating)
                | (Self::RuntimeBound, Self::Quarantined)
                | (Self::WorkspaceAttaching, Self::Ready)
                | (Self::WorkspaceAttaching, Self::Compensating)
                | (Self::WorkspaceAttaching, Self::Quarantined)
                | (Self::Ready, Self::Executing)
                | (Self::Ready, Self::Compensating)
                | (Self::Ready, Self::Quarantined)
                | (Self::Executing, Self::Checkpointing)
                | (Self::Executing, Self::Compensating)
                | (Self::Executing, Self::Quarantined)
                | (Self::Checkpointing, Self::Released)
                | (Self::Checkpointing, Self::Compensating)
                | (Self::Checkpointing, Self::Quarantined)
                | (Self::Compensating, Self::Released)
                | (Self::Compensating, Self::Quarantined)
        )
    }
}

impl fmt::Display for SandboxWorkspaceRuntimeTransactionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SandboxWorkspaceRuntimeTransactionState {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(())
    }
}
