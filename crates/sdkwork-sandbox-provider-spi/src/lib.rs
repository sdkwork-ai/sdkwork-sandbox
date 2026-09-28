#![forbid(unsafe_code)]
//! Provider-neutral lifecycle port for SDKWork Sandbox execution adapters.

mod capability;
mod command;
mod command_conformance;
mod error;
mod identity;
mod provider;

pub use capability::{IsolationAssurance, RuntimeCapability};
pub use command::{
    sandbox_command_cancellation_fingerprint, sandbox_command_execution_fingerprint,
    sandbox_verify_request_fingerprint, SandboxCommandCancellationRequest,
    SandboxCommandExecutionError, SandboxCommandExecutionRequest, SandboxCommandExecutor,
    SandboxCommandLimits, SandboxCommandLimitsError, SandboxCommandOutcome,
};
pub use command_conformance::{
    sandbox_run_command_conformance, SandboxCommandConformanceFinding,
    SandboxCommandConformanceFixture, SandboxCommandConformancePending,
    SandboxCommandConformanceReport, SandboxCommandConformanceStatus,
    SANDBOX_COMMAND_CONFORMANCE_SCENARIOS,
};
pub use error::{
    SandboxProviderError, SandboxProviderErrorKind, SandboxProviderOperation, SandboxProviderResult,
};
pub use identity::{
    OperationId, SandboxFencingToken, SandboxId, SandboxIdentifierError, SandboxInstanceId,
    SandboxInstanceOwnerId, SandboxLeaseOwnerId, SandboxProviderAllocationRef, SandboxProviderId,
    SandboxProviderKind, SandboxRuntimeBindingId, SandboxSessionId, SandboxWorkspaceId, TenantId,
};
pub use provider::{
    SandboxProvider, SandboxProviderAllocation, SandboxProviderAllocationRequest,
    SandboxProviderDescriptor, SandboxProviderDestroyRequest, SandboxProviderHealth,
    SandboxProviderHealthStatus, SandboxProviderReadiness, SandboxProviderStartRequest,
    SandboxProviderStopRequest,
};
