//! Typed API errors for the Sandbox app-api surface.
//!
//! The service layer's taxonomy is mapped here once, so a handler never invents
//! a status code and the HTTP contract stays in one place (`API_SPEC.md`
//! section 17).

use axum::http::StatusCode;
use sdkwork_intelligence_sandbox_service::{SandboxInstanceError, SandboxInstanceRepositoryError};

/// One API failure, already reduced to the wire status and a client-safe message.
#[derive(Debug)]
pub struct SandboxAppApiError {
    status: StatusCode,
    message: String,
}

impl SandboxAppApiError {
    #[must_use]
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    #[must_use]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    #[must_use]
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    #[must_use]
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, message)
    }

    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    /// The malformed path identifier case. A value that cannot parse can never
    /// name a row, so it is reported as absent rather than as a validation
    /// detail that would confirm the identifier format.
    #[must_use]
    pub fn invalid_sandbox_instance_id() -> Self {
        Self::not_found("sandbox instance does not exist")
    }

    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<SandboxInstanceError> for SandboxAppApiError {
    fn from(error: SandboxInstanceError) -> Self {
        match error {
            SandboxInstanceError::NotFound { .. } => {
                // The lookup is tenant-scoped, so a mismatched tenant and an
                // absent row are indistinguishable on purpose.
                Self::not_found("sandbox instance does not exist")
            }
            SandboxInstanceError::DuplicateName { .. } => {
                Self::conflict("a sandbox instance with this name already exists for this owner")
            }
            SandboxInstanceError::Validation { field, detail } => {
                Self::bad_request(format!("{field} is invalid: {detail}"))
            }
            SandboxInstanceError::InvalidStateTransition { .. } => {
                Self::conflict("the requested state transition is not allowed")
            }
            SandboxInstanceError::InstanceNotDeletable { .. } => {
                Self::conflict("a live sandbox instance must be suspended before it can be deleted")
            }
            SandboxInstanceError::VersionConflict { .. } => {
                Self::conflict("the sandbox instance was modified by another request")
            }
            SandboxInstanceError::InvariantViolation(detail) => {
                Self::internal(format!("sandbox instance state is inconsistent: {detail}"))
            }
            SandboxInstanceError::Repository(repository_error) => repository_error.into(),
        }
    }
}

impl From<SandboxInstanceRepositoryError> for SandboxAppApiError {
    fn from(error: SandboxInstanceRepositoryError) -> Self {
        match error {
            SandboxInstanceRepositoryError::NotFound => {
                Self::not_found("sandbox instance does not exist")
            }
            SandboxInstanceRepositoryError::VersionConflict => {
                Self::conflict("the sandbox instance was modified by another request")
            }
            SandboxInstanceRepositoryError::DuplicateName => {
                Self::conflict("a sandbox instance with this name already exists for this owner")
            }
            SandboxInstanceRepositoryError::InvalidPageRequest => Self::bad_request(
                "page must be between 1 and 10000 and page_size must be between 1 and 200",
            ),
            SandboxInstanceRepositoryError::InvalidStoredData => {
                Self::internal("the stored sandbox instance is not readable")
            }
            SandboxInstanceRepositoryError::Unavailable
            | SandboxInstanceRepositoryError::UnsupportedDatabaseEngine => {
                Self::unavailable("the sandbox instance store is unavailable")
            }
        }
    }
}
