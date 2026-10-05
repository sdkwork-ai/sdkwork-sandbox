//! Typed failures of the Sandbox Template Build authority.
//!
//! The contract declares no wire error codes for this capability, so these
//! variants are domain-internal: composition maps them onto API problems at
//! its own boundary. Display strings stay generic — record content can carry
//! tenant material and must never surface through an error.

/// Typed Template Build authority failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SandboxTemplateBuildAuthorityError {
    /// A build record or outcome failed validation; nothing was recorded.
    #[error("sandbox template build record failed validation")]
    SandboxTemplateBuildInvalidBuild,
    /// The lifecycle transition is outside the closed table.
    #[error("sandbox template build lifecycle transition is illegal")]
    SandboxTemplateBuildIllegalTransition,
}

/// The result type every Template Build authority entrypoint returns.
pub type SandboxTemplateBuildAuthorityResult<T> = Result<T, SandboxTemplateBuildAuthorityError>;
