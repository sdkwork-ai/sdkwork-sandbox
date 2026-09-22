use thiserror::Error;

use crate::SandboxProviderId;

pub type SandboxProviderResult<T> = Result<T, SandboxProviderError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxProviderOperation {
    Health,
    Allocate,
    Start,
    Stop,
    Destroy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxProviderErrorKind {
    Unsupported,
    Unavailable,
    Rejected,
    Conflict,
    Timeout,
    Internal,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("sandbox provider {sandbox_provider_id} failed {sandbox_provider_operation:?} with {sandbox_provider_error_kind:?}")]
pub struct SandboxProviderError {
    sandbox_provider_id: SandboxProviderId,
    sandbox_provider_operation: SandboxProviderOperation,
    sandbox_provider_error_kind: SandboxProviderErrorKind,
}

impl SandboxProviderError {
    #[must_use]
    pub fn new(
        sandbox_provider_id: SandboxProviderId,
        sandbox_provider_operation: SandboxProviderOperation,
        sandbox_provider_error_kind: SandboxProviderErrorKind,
    ) -> Self {
        Self {
            sandbox_provider_id,
            sandbox_provider_operation,
            sandbox_provider_error_kind,
        }
    }

    #[must_use]
    pub fn sandbox_provider_id(&self) -> &SandboxProviderId {
        &self.sandbox_provider_id
    }

    #[must_use]
    pub fn sandbox_provider_operation(&self) -> SandboxProviderOperation {
        self.sandbox_provider_operation
    }

    #[must_use]
    pub fn sandbox_provider_error_kind(&self) -> SandboxProviderErrorKind {
        self.sandbox_provider_error_kind
    }
}
