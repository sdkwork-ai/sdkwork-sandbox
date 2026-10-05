//! The build lifecycle state machine
//! (`specs/sandbox-template-build.contract.json`: `build.states`).
//!
//! Five states with a closed transition table: `requested` is the only
//! initial state, `succeeded`/`failed`/`quarantined` are terminal and
//! immutable (`build.immutableAfterTerminalState`), and an uncertain outcome
//! lands in `quarantined` instead of silently succeeding or failing
//! (`failureSemantics.silentSuccessAllowed`/`silentFailureAllowed` are
//! false).

use std::fmt;

/// Lifecycle states of one template build.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxTemplateBuildState {
    /// The build was requested and recorded; no builder ran yet.
    Requested,
    /// A builder is executing; the record content is unchanged.
    Building,
    /// The build produced a bound artifact tuple; terminal and immutable.
    Succeeded,
    /// The build failed deterministically and binds no artifact; terminal
    /// and immutable.
    Failed,
    /// The outcome ended uncertain (timeout, lost builder, unverifiable
    /// result); the build is out of service and binds no artifact.
    Quarantined,
}

impl SandboxTemplateBuildState {
    /// The contract state name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Building => "building",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Quarantined => "quarantined",
        }
    }

    /// Parses a contract state name; unknown names are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "requested" => Self::Requested,
            "building" => Self::Building,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "quarantined" => Self::Quarantined,
            _ => return None,
        })
    }

    /// The closed transition table. Terminal states have no outbound edges,
    /// so a terminal build record is immutable by construction.
    #[must_use]
    pub const fn sandbox_can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Requested, Self::Building)
                | (Self::Requested, Self::Quarantined)
                | (Self::Building, Self::Succeeded)
                | (Self::Building, Self::Failed)
                | (Self::Building, Self::Quarantined)
        )
    }

    /// Whether the build has reached a terminal state.
    #[must_use]
    pub const fn sandbox_is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Quarantined)
    }
}

impl fmt::Display for SandboxTemplateBuildState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SandboxTemplateBuildState {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use SandboxTemplateBuildState::*;

    const ALL_STATES: [SandboxTemplateBuildState; 5] = [
        SandboxTemplateBuildState::Requested,
        SandboxTemplateBuildState::Building,
        SandboxTemplateBuildState::Succeeded,
        SandboxTemplateBuildState::Failed,
        SandboxTemplateBuildState::Quarantined,
    ];

    #[test]
    fn state_names_round_trip_and_reject_unknowns() {
        for state in ALL_STATES {
            assert_eq!(
                SandboxTemplateBuildState::parse(state.as_str()),
                Some(state)
            );
        }
        assert!(SandboxTemplateBuildState::parse("Requested").is_none());
        assert!(SandboxTemplateBuildState::parse("").is_none());
        assert!(SandboxTemplateBuildState::parse("running").is_none());
    }

    #[test]
    fn transitions_are_exactly_the_contracted_closed_set() {
        for from in ALL_STATES {
            for to in ALL_STATES {
                let contracted = matches!(
                    (from, to),
                    (Requested, Building)
                        | (Requested, Quarantined)
                        | (Building, Succeeded)
                        | (Building, Failed)
                        | (Building, Quarantined)
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
        assert!(!Requested.sandbox_is_terminal());
        assert!(!Building.sandbox_is_terminal());
        assert!(Succeeded.sandbox_is_terminal());
        assert!(Failed.sandbox_is_terminal());
        assert!(Quarantined.sandbox_is_terminal());
    }
}
