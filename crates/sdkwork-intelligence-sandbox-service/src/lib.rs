#![forbid(unsafe_code)]
//! Provider-neutral lifecycle policy and orchestration for SDKWork Sandbox.

mod command;
mod error;
mod instance;
mod instance_service;
mod model;
mod port;
mod reconciliation;
mod repository;
mod service;

pub use command::{CreateSandboxSessionCommand, SandboxSessionLifecycleCommand};
pub use error::{SandboxLifecycleError, SandboxLifecycleResult};
pub use instance::{
    CreateSandboxInstanceCommand, SandboxInstance, SandboxInstanceError,
    SandboxInstanceExpiryUpdate, SandboxInstanceListCursor, SandboxInstanceListPage,
    SandboxInstanceProfile, SandboxInstanceRepository, SandboxInstanceRepositoryError,
    SandboxInstanceRepositoryResult, SandboxInstanceResourceBounds, SandboxInstanceResult,
    SandboxInstanceState, UpdateSandboxInstanceCommand, MAX_SANDBOX_INSTANCE_BASE_IMAGE_LENGTH,
    MAX_SANDBOX_INSTANCE_NAME_LENGTH, MAX_SANDBOX_INSTANCE_REQUIRED_CAPABILITIES,
    MAX_SANDBOX_INSTANCE_VERSION,
};
pub use instance_service::{
    SandboxInstanceService, DEFAULT_SANDBOX_INSTANCE_PAGE_SIZE, MAX_SANDBOX_INSTANCE_PAGE_SIZE,
};
pub use model::{
    SandboxOperationOutcome, SandboxRuntimeBinding, SandboxSession, SandboxSessionFailure,
    SandboxSessionOperation, SandboxSessionOperationKind, SandboxSessionState,
    MAX_SANDBOX_SESSION_OPERATIONS,
};
pub use port::SandboxSessionLifecyclePort;
pub use reconciliation::{
    SandboxSessionReconciliationCandidate, SandboxSessionReconciliationItem,
    SandboxSessionReconciliationOutcome, SandboxSessionReconciliationPage,
};
pub use repository::{
    validate_sandbox_session_persisted_invariants, BoundedSandboxSessionRepository,
    SandboxProtectedProviderAllocationRef, SandboxProviderAllocationProtectionContext,
    SandboxProviderAllocationProtectionVersion, SandboxProviderAllocationProtector,
    SandboxRuntimeBindingRepositorySnapshot, SandboxSessionLease,
    SandboxSessionOperationRepositorySnapshot, SandboxSessionRepository,
    SandboxSessionRepositoryError, SandboxSessionRepositoryResult,
    SandboxSessionRepositorySnapshot,
};
pub use service::SandboxLifecycleService;

#[cfg(test)]
mod tests;
