//! The launch-plan lifecycle state machine
//! (`specs/sandbox-instance-fast-start.contract.json`: `launchPlan.states`).
//!
//! Four states with a closed transition table: `planned` is the only initial
//! state and the record is immutable once created
//! (`launchPlan.immutableAfterCreation`); `consumed`/`expired`/`quarantined`
//! are terminal, so a terminal plan is immutable by construction. Binding
//! uncertainty lands in `quarantined`
//! (`bindingSemantics.bindingUncertaintyQuarantines`); an expired or released
//! claim can only ever expire its plan
//! (`bindingSemantics.expiredOrReleasedClaimMayNeverExecute`).

use std::fmt;

/// Lifecycle states of one launch plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxLaunchPlanState {
    /// The plan is recorded and bound to a live fenced claim; it authorizes
    /// no execution by itself.
    Planned,
    /// The authorized worker consumed the plan; terminal.
    Consumed,
    /// The bound claim expired or was released before consumption; the plan
    /// can never execute; terminal.
    Expired,
    /// Binding uncertainty (lost claim state, unverifiable binding); the
    /// plan is out of service; terminal.
    Quarantined,
}

impl SandboxLaunchPlanState {
    /// The contract state name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Consumed => "consumed",
            Self::Expired => "expired",
            Self::Quarantined => "quarantined",
        }
    }

    /// Parses a contract state name; unknown names are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "planned" => Self::Planned,
            "consumed" => Self::Consumed,
            "expired" => Self::Expired,
            "quarantined" => Self::Quarantined,
            _ => return None,
        })
    }

    /// The closed transition table. Terminal states have no outbound edges.
    #[must_use]
    pub const fn sandbox_can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Planned, Self::Consumed)
                | (Self::Planned, Self::Expired)
                | (Self::Planned, Self::Quarantined)
        )
    }

    /// Whether the plan has reached a terminal state.
    #[must_use]
    pub const fn sandbox_is_terminal(self) -> bool {
        matches!(self, Self::Consumed | Self::Expired | Self::Quarantined)
    }
}

impl fmt::Display for SandboxLaunchPlanState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SandboxLaunchPlanState {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use SandboxLaunchPlanState::*;

    const ALL_STATES: [SandboxLaunchPlanState; 4] = [
        SandboxLaunchPlanState::Planned,
        SandboxLaunchPlanState::Consumed,
        SandboxLaunchPlanState::Expired,
        SandboxLaunchPlanState::Quarantined,
    ];

    #[test]
    fn state_names_round_trip_and_reject_unknowns() {
        for state in ALL_STATES {
            assert_eq!(SandboxLaunchPlanState::parse(state.as_str()), Some(state));
        }
        assert!(SandboxLaunchPlanState::parse("Planned").is_none());
        assert!(SandboxLaunchPlanState::parse("").is_none());
        assert!(SandboxLaunchPlanState::parse("executing").is_none());
    }

    #[test]
    fn transitions_are_exactly_the_contracted_closed_set() {
        for from in ALL_STATES {
            for to in ALL_STATES {
                let contracted = matches!(
                    (from, to),
                    (Planned, Consumed) | (Planned, Expired) | (Planned, Quarantined)
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
        assert!(!Planned.sandbox_is_terminal());
        assert!(Consumed.sandbox_is_terminal());
        assert!(Expired.sandbox_is_terminal());
        assert!(Quarantined.sandbox_is_terminal());
    }
}
