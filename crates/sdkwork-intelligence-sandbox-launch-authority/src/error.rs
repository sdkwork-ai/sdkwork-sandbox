//! Typed failures of the Sandbox Fast-Start Launch authority.
//!
//! The contract declares no wire error codes for this capability, so these
//! variants are domain-internal: composition maps them onto API problems at
//! its own boundary. Display strings stay generic — record content can carry
//! tenant material and must never surface through an error.

/// Typed Fast-Start Launch authority failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SandboxLaunchAuthorityError {
    /// A launch plan or transition input failed validation; nothing was
    /// recorded.
    #[error("sandbox launch plan failed validation")]
    SandboxLaunchInvalidPlan,
    /// The lifecycle transition is outside the closed table.
    #[error("sandbox launch plan lifecycle transition is illegal")]
    SandboxLaunchIllegalTransition,
}

/// The result type every Fast-Start Launch authority entrypoint returns.
pub type SandboxLaunchAuthorityResult<T> = Result<T, SandboxLaunchAuthorityError>;
