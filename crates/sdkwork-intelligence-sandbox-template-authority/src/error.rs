//! Typed failures of the Sandbox Template authority.
//!
//! The contract declares no wire error codes for this capability, so these
//! variants are domain-internal: composition maps them onto API problems at
//! its own boundary. Display strings stay generic — record content can carry
//! tenant material and must never surface through an error message.

/// Typed failures of the Template authority model.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SandboxTemplateAuthorityError {
    /// A template definition failed validation; nothing was recorded.
    #[error("sandbox template definition failed validation")]
    SandboxTemplateInvalidDefinition,
    /// A template version failed validation; nothing was recorded.
    #[error("sandbox template version failed validation")]
    SandboxTemplateInvalidVersion,
    /// A build input failed the opaque-reference boundary; nothing was
    /// recorded.
    #[error("sandbox template build input failed the build-input boundary")]
    SandboxTemplateInvalidBuildInput,
    /// A cache policy failed validation; nothing was recorded.
    #[error("sandbox template cache policy failed validation")]
    SandboxTemplateInvalidCachePolicy,
}

/// The result type every Template authority entrypoint returns.
pub type SandboxTemplateAuthorityResult<T> = Result<T, SandboxTemplateAuthorityError>;
