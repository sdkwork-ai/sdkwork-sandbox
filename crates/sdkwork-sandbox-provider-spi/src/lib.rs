#![forbid(unsafe_code)]
//! Provider-neutral lifecycle port for SDKWork Sandbox execution adapters.

mod capability;
mod command;
mod error;
mod identity;
mod provider;

pub use capability::{IsolationAssurance, RuntimeCapability};
pub use command::{
    sandbox_command_execution_fingerprint, SandboxCommandExecutionError,
    SandboxCommandExecutionRequest, SandboxCommandExecutor, SandboxCommandLimits,
    SandboxCommandLimitsError, SandboxCommandOutcome,
};
pub use error::{
    SandboxProviderError, SandboxProviderErrorKind, SandboxProviderOperation, SandboxProviderResult,
};
pub use identity::{
    OperationId, SandboxFencingToken, SandboxId, SandboxIdentifierError, SandboxLeaseOwnerId,
    SandboxProviderAllocationRef, SandboxProviderId, SandboxProviderKind, SandboxRuntimeBindingId,
    SandboxSessionId, SandboxWorkspaceId, TenantId,
};
pub use provider::{
    SandboxProvider, SandboxProviderAllocation, SandboxProviderAllocationRequest,
    SandboxProviderDescriptor, SandboxProviderDestroyRequest, SandboxProviderHealth,
    SandboxProviderHealthStatus, SandboxProviderReadiness, SandboxProviderStartRequest,
    SandboxProviderStopRequest,
};
