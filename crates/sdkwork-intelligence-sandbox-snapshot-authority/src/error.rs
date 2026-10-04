//! Typed failures of the Sandbox Snapshot/Fork authority.
//!
//! The contract declares no wire error codes for this capability, so these
//! variants are domain-internal: composition maps them onto API problems at
//! its own boundary. Display strings stay generic — record content can carry
//! tenant material and must never surface through an error.

/// Typed Snapshot/Fork authority failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SandboxSnapshotAuthorityError {
    /// A snapshot record failed validation; nothing was recorded.
    #[error("sandbox snapshot record failed validation")]
    SandboxSnapshotInvalidSnapshot,
    /// A fork derivation failed the consistency boundary; nothing was
    /// recorded.
    #[error("sandbox fork derivation failed the consistency boundary")]
    SandboxSnapshotInvalidForkDerivation,
    /// The lifecycle transition is outside the closed table.
    #[error("sandbox snapshot lifecycle transition is illegal")]
    SandboxSnapshotIllegalTransition,
}

/// The result type every Snapshot/Fork authority entrypoint returns.
pub type SandboxSnapshotAuthorityResult<T> = Result<T, SandboxSnapshotAuthorityError>;
