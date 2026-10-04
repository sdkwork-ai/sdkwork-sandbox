//! Slot and claim state machines for the Sandbox runtime pool.
//!
//! The two enums restate `specs/sandbox-runtime-pool.contract.json`
//! (`slot.states`, `claim.states`) one-to-one. Parsing rejects unknown names
//! (`slot.unknownStateRejected`), and the transition tables are closed: any
//! pair outside the table fails deterministically
//! (`slot.illegalTransitionRejected`).

use std::fmt;

/// Lifecycle states of a [`crate::slot::SandboxPoolSlot`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxPoolSlotState {
    /// Host preparation is in progress; no tenant state may be present.
    Preparing,
    /// Trusted-node, immutable-artifact and runtime-directory identity are
    /// verified; the slot is claimable and carries no tenant state.
    Ready,
    /// A claim bind is in flight.
    Claiming,
    /// One fenced claim owns the slot.
    Claimed,
    /// Release cleanup is running; capacity stays consumed.
    Sanitizing,
    /// Cleanup or reconciliation was uncertain; the slot and its capacity are
    /// out of service and may only retire.
    Quarantined,
    /// Terminal; the slot identity never returns to service.
    Retired,
}

impl SandboxPoolSlotState {
    /// The contract state name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Ready => "ready",
            Self::Claiming => "claiming",
            Self::Claimed => "claimed",
            Self::Sanitizing => "sanitizing",
            Self::Quarantined => "quarantined",
            Self::Retired => "retired",
        }
    }

    /// Parses a contract state name; unknown names are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "preparing" => Self::Preparing,
            "ready" => Self::Ready,
            "claiming" => Self::Claiming,
            "claimed" => Self::Claimed,
            "sanitizing" => Self::Sanitizing,
            "quarantined" => Self::Quarantined,
            "retired" => Self::Retired,
            _ => return None,
        })
    }

    /// The closed transition table. `quarantined` may never return to `ready`
    /// (`releaseAndSanitization.sandbox_quarantineMayBeBypassedForAvailability`
    /// is false) and `retired` is terminal.
    #[must_use]
    pub const fn sandbox_can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Preparing, Self::Ready)
                | (Self::Preparing, Self::Quarantined)
                | (Self::Preparing, Self::Retired)
                | (Self::Ready, Self::Claiming)
                | (Self::Ready, Self::Quarantined)
                | (Self::Ready, Self::Retired)
                | (Self::Claiming, Self::Claimed)
                | (Self::Claiming, Self::Ready)
                | (Self::Claiming, Self::Quarantined)
                | (Self::Claiming, Self::Retired)
                | (Self::Claimed, Self::Sanitizing)
                | (Self::Claimed, Self::Quarantined)
                | (Self::Claimed, Self::Retired)
                | (Self::Sanitizing, Self::Ready)
                | (Self::Sanitizing, Self::Quarantined)
                | (Self::Sanitizing, Self::Retired)
                | (Self::Quarantined, Self::Retired)
        )
    }
}

impl fmt::Display for SandboxPoolSlotState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SandboxPoolSlotState {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(())
    }
}

/// Lifecycle states of a [`crate::claim::SandboxPoolClaim`].
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxPoolClaimState {
    /// The claim bind is in flight; no side effect has happened yet.
    Claiming,
    /// The claim durably owns its slot.
    Bound,
    /// Release cleanup is in flight; the claim is idempotent here.
    Releasing,
    /// Terminal; the slot returned to service through fresh cleanup evidence.
    Released,
    /// Terminal; the claim and its slot capacity are out of service.
    Quarantined,
}

impl SandboxPoolClaimState {
    /// The contract state name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claiming => "claiming",
            Self::Bound => "bound",
            Self::Releasing => "releasing",
            Self::Released => "released",
            Self::Quarantined => "quarantined",
        }
    }

    /// Parses a contract state name; unknown names are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "claiming" => Self::Claiming,
            "bound" => Self::Bound,
            "releasing" => Self::Releasing,
            "released" => Self::Released,
            "quarantined" => Self::Quarantined,
            _ => return None,
        })
    }

    /// The closed transition table. Only the `releasing` path may end in
    /// `released`; uncertainty anywhere lands in `quarantined`.
    #[must_use]
    pub const fn sandbox_can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Claiming, Self::Bound)
                | (Self::Claiming, Self::Released)
                | (Self::Claiming, Self::Quarantined)
                | (Self::Bound, Self::Releasing)
                | (Self::Bound, Self::Quarantined)
                | (Self::Releasing, Self::Released)
                | (Self::Releasing, Self::Quarantined)
        )
    }
}

impl fmt::Display for SandboxPoolClaimState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SandboxPoolClaimState {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_SLOT_STATES: [SandboxPoolSlotState; 7] = [
        SandboxPoolSlotState::Preparing,
        SandboxPoolSlotState::Ready,
        SandboxPoolSlotState::Claiming,
        SandboxPoolSlotState::Claimed,
        SandboxPoolSlotState::Sanitizing,
        SandboxPoolSlotState::Quarantined,
        SandboxPoolSlotState::Retired,
    ];

    const ALL_CLAIM_STATES: [SandboxPoolClaimState; 5] = [
        SandboxPoolClaimState::Claiming,
        SandboxPoolClaimState::Bound,
        SandboxPoolClaimState::Releasing,
        SandboxPoolClaimState::Released,
        SandboxPoolClaimState::Quarantined,
    ];

    #[test]
    fn slot_state_names_round_trip_and_reject_unknowns() {
        for state in ALL_SLOT_STATES {
            assert_eq!(SandboxPoolSlotState::parse(state.as_str()), Some(state));
        }
        assert!(SandboxPoolSlotState::parse("Ready").is_none());
        assert!(SandboxPoolSlotState::parse("").is_none());
        assert!(SandboxPoolSlotState::parse("unknown").is_none());
    }

    #[test]
    fn claim_state_names_round_trip_and_reject_unknowns() {
        for state in ALL_CLAIM_STATES {
            assert_eq!(SandboxPoolClaimState::parse(state.as_str()), Some(state));
        }
        assert!(SandboxPoolClaimState::parse("Bound").is_none());
        assert!(SandboxPoolClaimState::parse("active").is_none());
    }

    #[test]
    fn slot_transitions_are_exactly_the_contracted_closed_set() {
        for (from, to) in [
            (SandboxPoolSlotState::Preparing, SandboxPoolSlotState::Ready),
            (
                SandboxPoolSlotState::Preparing,
                SandboxPoolSlotState::Quarantined,
            ),
            (
                SandboxPoolSlotState::Preparing,
                SandboxPoolSlotState::Retired,
            ),
            (SandboxPoolSlotState::Ready, SandboxPoolSlotState::Claiming),
            (
                SandboxPoolSlotState::Ready,
                SandboxPoolSlotState::Quarantined,
            ),
            (SandboxPoolSlotState::Ready, SandboxPoolSlotState::Retired),
            (
                SandboxPoolSlotState::Claiming,
                SandboxPoolSlotState::Claimed,
            ),
            (SandboxPoolSlotState::Claiming, SandboxPoolSlotState::Ready),
            (
                SandboxPoolSlotState::Claiming,
                SandboxPoolSlotState::Quarantined,
            ),
            (
                SandboxPoolSlotState::Claiming,
                SandboxPoolSlotState::Retired,
            ),
            (
                SandboxPoolSlotState::Claimed,
                SandboxPoolSlotState::Sanitizing,
            ),
            (
                SandboxPoolSlotState::Claimed,
                SandboxPoolSlotState::Quarantined,
            ),
            (SandboxPoolSlotState::Claimed, SandboxPoolSlotState::Retired),
            (
                SandboxPoolSlotState::Sanitizing,
                SandboxPoolSlotState::Ready,
            ),
            (
                SandboxPoolSlotState::Sanitizing,
                SandboxPoolSlotState::Quarantined,
            ),
            (
                SandboxPoolSlotState::Sanitizing,
                SandboxPoolSlotState::Retired,
            ),
            (
                SandboxPoolSlotState::Quarantined,
                SandboxPoolSlotState::Retired,
            ),
        ] {
            assert!(
                from.sandbox_can_transition_to(to),
                "{from} -> {to} must be legal",
            );
        }
        for from in ALL_SLOT_STATES {
            for to in ALL_SLOT_STATES {
                let contracted = matches!(
                    (from, to),
                    (SandboxPoolSlotState::Preparing, SandboxPoolSlotState::Ready)
                        | (
                            SandboxPoolSlotState::Preparing,
                            SandboxPoolSlotState::Quarantined
                        )
                        | (
                            SandboxPoolSlotState::Preparing,
                            SandboxPoolSlotState::Retired
                        )
                        | (SandboxPoolSlotState::Ready, SandboxPoolSlotState::Claiming)
                        | (
                            SandboxPoolSlotState::Ready,
                            SandboxPoolSlotState::Quarantined
                        )
                        | (SandboxPoolSlotState::Ready, SandboxPoolSlotState::Retired)
                        | (
                            SandboxPoolSlotState::Claiming,
                            SandboxPoolSlotState::Claimed
                        )
                        | (SandboxPoolSlotState::Claiming, SandboxPoolSlotState::Ready)
                        | (
                            SandboxPoolSlotState::Claiming,
                            SandboxPoolSlotState::Quarantined
                        )
                        | (
                            SandboxPoolSlotState::Claiming,
                            SandboxPoolSlotState::Retired
                        )
                        | (
                            SandboxPoolSlotState::Claimed,
                            SandboxPoolSlotState::Sanitizing
                        )
                        | (
                            SandboxPoolSlotState::Claimed,
                            SandboxPoolSlotState::Quarantined
                        )
                        | (SandboxPoolSlotState::Claimed, SandboxPoolSlotState::Retired)
                        | (
                            SandboxPoolSlotState::Sanitizing,
                            SandboxPoolSlotState::Ready
                        )
                        | (
                            SandboxPoolSlotState::Sanitizing,
                            SandboxPoolSlotState::Quarantined
                        )
                        | (
                            SandboxPoolSlotState::Sanitizing,
                            SandboxPoolSlotState::Retired
                        )
                        | (
                            SandboxPoolSlotState::Quarantined,
                            SandboxPoolSlotState::Retired
                        )
                );
                assert_eq!(
                    from.sandbox_can_transition_to(to),
                    contracted,
                    "{from} -> {to}",
                );
            }
        }
    }

    #[test]
    fn claim_transitions_are_exactly_the_contracted_closed_set() {
        for from in ALL_CLAIM_STATES {
            for to in ALL_CLAIM_STATES {
                let contracted = matches!(
                    (from, to),
                    (
                        SandboxPoolClaimState::Claiming,
                        SandboxPoolClaimState::Bound
                    ) | (
                        SandboxPoolClaimState::Claiming,
                        SandboxPoolClaimState::Released
                    ) | (
                        SandboxPoolClaimState::Claiming,
                        SandboxPoolClaimState::Quarantined
                    ) | (
                        SandboxPoolClaimState::Bound,
                        SandboxPoolClaimState::Releasing
                    ) | (
                        SandboxPoolClaimState::Bound,
                        SandboxPoolClaimState::Quarantined
                    ) | (
                        SandboxPoolClaimState::Releasing,
                        SandboxPoolClaimState::Released
                    ) | (
                        SandboxPoolClaimState::Releasing,
                        SandboxPoolClaimState::Quarantined
                    )
                );
                assert_eq!(
                    from.sandbox_can_transition_to(to),
                    contracted,
                    "{from} -> {to}",
                );
            }
        }
        assert!(!SandboxPoolClaimState::Bound
            .sandbox_can_transition_to(SandboxPoolClaimState::Released,));
        assert!(!SandboxPoolClaimState::Quarantined
            .sandbox_can_transition_to(SandboxPoolClaimState::Bound),);
    }
}
