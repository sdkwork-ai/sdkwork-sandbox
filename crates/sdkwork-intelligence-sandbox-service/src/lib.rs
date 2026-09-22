#![forbid(unsafe_code)]
//! Provider-neutral lifecycle policy and orchestration for SDKWork Sandbox.

mod command;
mod error;
mod model;
mod port;
mod reconciliation;
mod repository;
mod service;

pub use command::{CreateSandboxSessionCommand, SandboxSessionLifecycleCommand};
pub use error::{SandboxLifecycleError, SandboxLifecycleResult};
pub use model::{
    SandboxOperationOutcome, SandboxRuntimeBinding, SandboxSession, SandboxSessionFailure,
    SandboxSessionOperation, SandboxSessionOperationKind, SandboxSessionState,
};
pub use port::SandboxSessionLifecyclePort;
pub use reconciliation::{
    SandboxSessionReconciliationCandidate, SandboxSessionReconciliationItem,
    SandboxSessionReconciliationOutcome, SandboxSessionReconciliationPage,
};
pub use repository::{
    validate_sandbox_session_persisted_invariants, SandboxProtectedProviderAllocationRef,
    SandboxProviderAllocationProtectionContext, SandboxProviderAllocationProtectionVersion,
    SandboxProviderAllocationProtector, SandboxRuntimeBindingRepositorySnapshot,
    SandboxSessionLease, SandboxSessionOperationRepositorySnapshot, SandboxSessionRepository,
    SandboxSessionRepositoryError, SandboxSessionRepositoryResult,
    SandboxSessionRepositorySnapshot,
};
pub use service::SandboxLifecycleService;

#[cfg(test)]
mod tests;
