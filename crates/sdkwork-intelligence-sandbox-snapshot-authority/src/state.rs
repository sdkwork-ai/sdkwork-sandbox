//! The snapshot lifecycle state machine
//! (`specs/sandbox-snapshot-fork.contract.json`: `snapshot.states`).
//!
//! Five states with a closed transition table: a snapshot becomes immutable
//! once `available` (`snapshot.immutableAfterCreation`), deletion is a real
//! transition (never a silent disappearance,
//! `deletion.silentDisappearanceAllowed` is false), and an uncertain deletion
//! lands in `quarantined` (`deletion.uncertainDeletionQuarantines`).

use std::fmt;

/// Lifecycle states of one snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxSnapshotState {
    /// The snapshot is being materialized.
    Creating,
    /// The snapshot is complete, immutable and derivable.
    Available,
    /// A restore or derivation read is in progress (content unchanged).
    Restoring,
    /// Deletion completed deterministically; terminal.
    Deleted,
    /// Deletion or verification ended uncertain; the snapshot is out of
    /// service and cannot serve derivations.
    Quarantined,
}

impl SandboxSnapshotState {
    /// The contract state name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Creating => "creating",
            Self::Available => "available",
            Self::Restoring => "restoring",
            Self::Deleted => "deleted",
            Self::Quarantined => "quarantined",
        }
    }

    /// Parses a contract state name; unknown names are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "creating" => Self::Creating,
            "available" => Self::Available,
            "restoring" => Self::Restoring,
            "deleted" => Self::Deleted,
            "quarantined" => Self::Quarantined,
            _ => return None,
        })
    }

    /// The closed transition table.
    #[must_use]
    pub const fn sandbox_can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Creating, Self::Available)
                | (Self::Creating, Self::Quarantined)
                | (Self::Available, Self::Restoring)
                | (Self::Available, Self::Deleted)
                | (Self::Available, Self::Quarantined)
                | (Self::Restoring, Self::Available)
                | (Self::Restoring, Self::Quarantined)
        )
    }
}

impl fmt::Display for SandboxSnapshotState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SandboxSnapshotState {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value).ok_or(())
    }
}
