//! The execution lifecycle state machine
//! (`specs/sandbox-worker.contract.json`: `execution.states`).
//!
//! Six states with a closed transition table: `accepted` is the only initial
//! state, `started`/`failed`/`quarantined` are terminal and immutable
//! (`execution.immutableAfterTerminalState`), and an uncertain outcome lands
//! in `quarantined` instead of silently succeeding or failing
//! (`bindingSemantics.silentSuccessAllowed`/`silentFailureAllowed` are
//! false). `started` is reachable only from `starting` — a completion claim
//! must be earned through the executor.

use std::fmt;

/// Lifecycle states of one worker execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxWorkerExecutionState {
    /// The worker accepted one launch plan; nothing has executed yet and no
    /// completion is claimed.
    Accepted,
    /// Provider allocation and start are in flight; the allocation reference
    /// is being earned.
    Provisioning,
    /// The start command is dispatched through the command-executor port.
    Starting,
    /// The executor reported success; terminal and immutable.
    Started,
    /// A deterministic failure; terminal and immutable.
    Failed,
    /// The outcome ended uncertain (lost worker, unverifiable result);
    /// terminal and immutable.
    Quarantined,
}

impl SandboxWorkerExecutionState {
    /// The contract state name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Provisioning => "provisioning",
            Self::Starting => "starting",
            Self::Started => "started",
            Self::Failed => "failed",
            Self::Quarantined => "quarantined",
        }
    }

    /// Parses a contract state name; unknown names are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "accepted" => Self::Accepted,
            "provisioning" => Self::Provisioning,
            "starting" => Self::Starting,
            "started" => Self::Started,
            "failed" => Self::Failed,
            "quarantined" => Self::Quarantined,
            _ => return None,
        })
    }

    /// The closed transition table. Terminal states have no outbound edges.
    #[must_use]
    pub const fn sandbox_can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Accepted, Self::Provisioning)
                | (Self::Accepted, Self::Quarantined)
                | (Self::Provisioning, Self::Starting)
                | (Self::Provisioning, Self::Failed)
                | (Self::Provisioning, Self::Quarantined)
                | (Self::Starting, Self::Started)
                | (Self::Starting, Self::Failed)
                | (Self::Starting, Self::Quarantined)
        )
    }

    /// Whether the execution has reached a terminal state.
    #[must_use]
    pub const fn sandbox_is_terminal(self) -> bool {
        matches!(self, Self::Started | Self::Failed | Self::Quarantined)
    }
}

impl fmt::Display for SandboxWorkerExecutionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SandboxWorkerExecutionState {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use SandboxWorkerExecutionState::*;

    const ALL_STATES: [SandboxWorkerExecutionState; 6] = [
        SandboxWorkerExecutionState::Accepted,
        SandboxWorkerExecutionState::Provisioning,
        SandboxWorkerExecutionState::Starting,
        SandboxWorkerExecutionState::Started,
        SandboxWorkerExecutionState::Failed,
        SandboxWorkerExecutionState::Quarantined,
    ];

    #[test]
    fn state_names_round_trip_and_reject_unknowns() {
        for state in ALL_STATES {
            assert_eq!(
                SandboxWorkerExecutionState::parse(state.as_str()),
                Some(state)
            );
        }
        assert!(SandboxWorkerExecutionState::parse("Accepted").is_none());
        assert!(SandboxWorkerExecutionState::parse("").is_none());
        assert!(SandboxWorkerExecutionState::parse("running").is_none());
    }

    #[test]
    fn transitions_are_exactly_the_contracted_closed_set() {
        for from in ALL_STATES {
            for to in ALL_STATES {
                let contracted = matches!(
                    (from, to),
                    (Accepted, Provisioning)
                        | (Accepted, Quarantined)
                        | (Provisioning, Starting)
                        | (Provisioning, Failed)
                        | (Provisioning, Quarantined)
                        | (Starting, Started)
                        | (Starting, Failed)
                        | (Starting, Quarantined)
                );
                assert_eq!(
                    from.sandbox_can_transition_to(to),
                    contracted,
                    "{from} -> {to}"
                );
            }
        }
    }

    #[test]
    fn terminal_states_are_exactly_the_contracted_three() {
        assert!(!Accepted.sandbox_is_terminal());
        assert!(!Provisioning.sandbox_is_terminal());
        assert!(!Starting.sandbox_is_terminal());
        assert!(Started.sandbox_is_terminal());
        assert!(Failed.sandbox_is_terminal());
        assert!(Quarantined.sandbox_is_terminal());
    }
}
