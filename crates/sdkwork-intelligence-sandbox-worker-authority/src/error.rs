//! Typed failures of the Sandbox Worker launch-execution authority.
//!
//! The contract declares no wire error codes for this capability, so these
//! variants are domain-internal: composition maps them onto API problems at
//! its own boundary. Display strings stay generic — record content can carry
//! tenant material and must never surface through an error.

/// Typed Worker launch-execution authority failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SandboxWorkerAuthorityError {
    /// An execution record or transition input failed validation; nothing
    /// was recorded.
    #[error("sandbox worker execution record failed validation")]
    SandboxWorkerExecutionInvalidRecord,
    /// The lifecycle transition is outside the closed table.
    #[error("sandbox worker execution lifecycle transition is illegal")]
    SandboxWorkerExecutionIllegalTransition,
}

/// The result type every Worker launch-execution authority entrypoint
/// returns.
pub type SandboxWorkerAuthorityResult<T> = Result<T, SandboxWorkerAuthorityError>;
