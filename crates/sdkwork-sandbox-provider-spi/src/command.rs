//! Provider-neutral command execution contract (`REQ-2026-0007`).
//!
//! The types here are the SPI surface of `apis/commands/sandbox-command-contract.json`:
//! executable-plus-argv requests with hard bounds, the contract-pinned canonical request
//! fingerprint every executor must recompute itself, and the port a Provider implements to run
//! and cancel one command. The module owns no process: implementing `SandboxCommandExecutor`
//! with real host execution is a Provider execution slice governed by the
//! local-provider-host-boundary contract and its real-platform evidence gates.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use async_trait::async_trait;
use sha2::{Digest, Sha256};

/// The hard bounds a command request declares, mirroring the contract's `bounds` block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxCommandLimits {
    /// Hard wall-clock bound for one execution, in milliseconds (contract maximum: 86_400_000).
    pub sandbox_timeout_ms: u64,
    /// Hard stdout byte bound (contract maximum: 67_108_864).
    pub sandbox_stdout_byte_limit: u64,
    /// Hard stderr byte bound (contract maximum: 67_108_864).
    pub sandbox_stderr_byte_limit: u64,
    /// Hard cleanup bound applied after termination, in milliseconds (contract maximum: 60_000).
    pub sandbox_cleanup_timeout_ms: u64,
    /// Maximum live descendant processes the execution may keep (contract maximum: 128).
    pub sandbox_max_process_count: u32,
}

impl SandboxCommandLimits {
    /// Rejects a limits set outside the contract's `bounds` maxima.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxCommandLimitsError::FieldOverBound`] when any field exceeds the
    /// contract maximum, so an over-broad request fails closed before fingerprinting.
    pub fn validate(&self) -> Result<(), SandboxCommandLimitsError> {
        const CONTRACT_MAX_TIMEOUT_MS: u64 = 86_400_000;
        const CONTRACT_MAX_STDOUT_BYTES: u64 = 67_108_864;
        const CONTRACT_MAX_STDERR_BYTES: u64 = 67_108_864;
        const CONTRACT_MAX_CLEANUP_TIMEOUT_MS: u64 = 60_000;
        const CONTRACT_MAX_PROCESS_COUNT: u32 = 128;

        if self.sandbox_timeout_ms == 0 || self.sandbox_timeout_ms > CONTRACT_MAX_TIMEOUT_MS {
            return Err(SandboxCommandLimitsError::FieldOverBound);
        }
        if self.sandbox_cleanup_timeout_ms == 0
            || self.sandbox_cleanup_timeout_ms > CONTRACT_MAX_CLEANUP_TIMEOUT_MS
        {
            return Err(SandboxCommandLimitsError::FieldOverBound);
        }
        if self.sandbox_stdout_byte_limit > CONTRACT_MAX_STDOUT_BYTES
            || self.sandbox_stderr_byte_limit > CONTRACT_MAX_STDERR_BYTES
        {
            return Err(SandboxCommandLimitsError::FieldOverBound);
        }
        if self.sandbox_max_process_count == 0
            || self.sandbox_max_process_count > CONTRACT_MAX_PROCESS_COUNT
        {
            return Err(SandboxCommandLimitsError::FieldOverBound);
        }
        Ok(())
    }
}

/// Why a limits set failed contract validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxCommandLimitsError {
    /// A field is zero where a positive bound is required, or over the contract maximum.
    FieldOverBound,
}

impl fmt::Display for SandboxCommandLimitsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FieldOverBound => {
                f.write_str("sandbox command limits exceed the contract bounds")
            }
        }
    }
}

impl Error for SandboxCommandLimitsError {}

/// The canonical execution request the contract's `requestSchema` defines.
///
/// Identity fields are opaque strings at this boundary; the fingerprint covers them verbatim in
/// the contract's declared field order, so any serialization drift shows up as a fingerprint
/// mismatch instead of a silent reinterpretation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxCommandExecutionRequest {
    /// Tenant the execution is scoped to.
    pub sandbox_tenant_id: String,
    /// Provider the execution is addressed to.
    pub sandbox_provider_id: String,
    /// Workspace the execution runs inside.
    pub sandbox_workspace_id: String,
    /// Session the execution belongs to.
    pub sandbox_session_id: String,
    /// Runtime sandbox identifier.
    pub sandbox_id: String,
    /// Runtime binding the execution is fenced to.
    pub sandbox_runtime_binding_id: String,
    /// Monotonic fencing token proving single-writer authority.
    pub sandbox_fencing_token: u64,
    /// Durable operation id making the execution idempotent.
    pub sandbox_command_operation_id: String,
    /// Bare executable name (allowlist-checked by the provider boundary).
    pub sandbox_executable: String,
    /// Argument vector, order preserved exactly as issued.
    pub sandbox_arguments: Vec<String>,
    /// Logical working directory relative to the Workspace root.
    pub sandbox_working_directory: String,
    /// Environment additions, name-ascending by the canonical encoding.
    pub sandbox_environment: BTreeMap<String, String>,
    /// The hard bounds for this execution.
    pub sandbox_command_limits: SandboxCommandLimits,
}

/// `sha256` over the contract's canonical encoding: domain separation, then every field as
/// length-prefixed UTF-8 (lengths as `u64` big-endian), fencing token and numeric limit fields as
/// `u64` big-endian, environment names ascending, argument order preserved. The contract requires
/// the executor to recompute this from the request and reject a caller-supplied mismatch.
#[must_use]
pub fn sandbox_command_execution_fingerprint(
    sandbox_request: &SandboxCommandExecutionRequest,
) -> String {
    const DOMAIN: &str = "sdkwork-sandbox-command-v1";
    let mut hasher = Sha256::new();
    Digest::update(&mut hasher, (DOMAIN.len() as u64).to_be_bytes());
    Digest::update(&mut hasher, DOMAIN.as_bytes());

    let hash_length_prefixed = |hasher: &mut Sha256, field: &str| {
        Digest::update(hasher, (field.len() as u64).to_be_bytes());
        Digest::update(hasher, field.as_bytes());
    };
    for field in [
        sandbox_request.sandbox_tenant_id.as_str(),
        sandbox_request.sandbox_provider_id.as_str(),
        sandbox_request.sandbox_workspace_id.as_str(),
        sandbox_request.sandbox_session_id.as_str(),
        sandbox_request.sandbox_id.as_str(),
        sandbox_request.sandbox_runtime_binding_id.as_str(),
    ] {
        hash_length_prefixed(&mut hasher, field);
    }
    Digest::update(
        &mut hasher,
        sandbox_request.sandbox_fencing_token.to_be_bytes(),
    );
    hash_length_prefixed(&mut hasher, &sandbox_request.sandbox_command_operation_id);
    hash_length_prefixed(&mut hasher, &sandbox_request.sandbox_executable);
    Digest::update(
        &mut hasher,
        (sandbox_request.sandbox_arguments.len() as u64).to_be_bytes(),
    );
    for argument in &sandbox_request.sandbox_arguments {
        hash_length_prefixed(&mut hasher, argument);
    }
    hash_length_prefixed(&mut hasher, &sandbox_request.sandbox_working_directory);
    Digest::update(
        &mut hasher,
        (sandbox_request.sandbox_environment.len() as u64).to_be_bytes(),
    );
    // `BTreeMap` iterates in ascending name order, which is the canonical ordering.
    for (name, value) in &sandbox_request.sandbox_environment {
        hash_length_prefixed(&mut hasher, name);
        hash_length_prefixed(&mut hasher, value);
    }
    let limits = &sandbox_request.sandbox_command_limits;
    for number in [
        limits.sandbox_timeout_ms,
        limits.sandbox_stdout_byte_limit,
        limits.sandbox_stderr_byte_limit,
        limits.sandbox_cleanup_timeout_ms,
        u64::from(limits.sandbox_max_process_count),
    ] {
        Digest::update(&mut hasher, number.to_be_bytes());
    }

    format!("{:x}", hasher.finalize())
}

/// The canonical cancellation request the contract's `cancellationRequestSchema` defines.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxCommandCancellationRequest {
    /// Tenant the cancellation is scoped to.
    pub sandbox_tenant_id: String,
    /// Provider the cancellation is addressed to.
    pub sandbox_provider_id: String,
    /// Workspace the target execution runs inside.
    pub sandbox_workspace_id: String,
    /// Session the target execution belongs to.
    pub sandbox_session_id: String,
    /// Runtime sandbox identifier.
    pub sandbox_id: String,
    /// Runtime binding the target execution is fenced to.
    pub sandbox_runtime_binding_id: String,
    /// Monotonic fencing token proving single-writer authority.
    pub sandbox_fencing_token: u64,
    /// Durable operation id of the target execution.
    pub sandbox_command_operation_id: String,
    /// Durable operation id making this cancellation idempotent.
    pub sandbox_cancellation_operation_id: String,
}

/// `sha256` over the contract's canonical encoding of the nine `cancellationFields`, in the same
/// domain-separated length-prefixed encoding as the execution fingerprint.
#[must_use]
pub fn sandbox_command_cancellation_fingerprint(
    sandbox_request: &SandboxCommandCancellationRequest,
) -> String {
    const DOMAIN: &str = "sdkwork-sandbox-command-v1";
    let mut hasher = Sha256::new();
    Digest::update(&mut hasher, (DOMAIN.len() as u64).to_be_bytes());
    Digest::update(&mut hasher, DOMAIN.as_bytes());
    for field in [
        sandbox_request.sandbox_tenant_id.as_str(),
        sandbox_request.sandbox_provider_id.as_str(),
        sandbox_request.sandbox_workspace_id.as_str(),
        sandbox_request.sandbox_session_id.as_str(),
        sandbox_request.sandbox_id.as_str(),
        sandbox_request.sandbox_runtime_binding_id.as_str(),
    ] {
        Digest::update(&mut hasher, (field.len() as u64).to_be_bytes());
        Digest::update(&mut hasher, field.as_bytes());
    }
    Digest::update(
        &mut hasher,
        sandbox_request.sandbox_fencing_token.to_be_bytes(),
    );
    for field in [
        sandbox_request.sandbox_command_operation_id.as_str(),
        sandbox_request.sandbox_cancellation_operation_id.as_str(),
    ] {
        Digest::update(&mut hasher, (field.len() as u64).to_be_bytes());
        Digest::update(&mut hasher, field.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

/// Whether the caller-supplied fingerprint matches the executor's own recomputation. The contract
/// requires executors to recompute rather than trust this value; a mismatch fails closed as
/// `invalid-request`.
#[must_use]
pub fn sandbox_verify_request_fingerprint(
    sandbox_request: &SandboxCommandExecutionRequest,
    sandbox_claimed_fingerprint: &str,
) -> bool {
    sandbox_command_execution_fingerprint(sandbox_request) == sandbox_claimed_fingerprint
}

/// The Provider-implemented port for running and cancelling one sandbox command.
///
/// Implementations must recompute the canonical fingerprint from the request, enforce the
/// request's limits as hard bounds, and treat the `(tenant, provider, operation)` idempotency key
/// as replay-safe. The port owns no process lifecycle itself; host execution belongs to the
/// Provider execution slice behind the local-provider-host-boundary contract.
#[async_trait]
pub trait SandboxCommandExecutor: Send + Sync {
    /// Runs one command request to a terminal outcome, or fails without a terminal outcome when
    /// the result is unavailable (the caller retries the same operation id).
    ///
    /// # Errors
    ///
    /// Returns the provider's typed execution error; the contract's error-code families
    /// (`invalid-request`, `policy-denied`, `stale-fencing`, `idempotency-conflict`, ...) map
    /// onto the implementation's error type.
    async fn sandbox_execute(
        &self,
        sandbox_request: &SandboxCommandExecutionRequest,
    ) -> Result<SandboxCommandOutcome, SandboxCommandExecutionError>;

    /// Requests fenced cancellation of a started execution. Cancellation is idempotent: cancelling
    /// an unknown or already-terminal operation is not an error.
    ///
    /// # Errors
    ///
    /// Returns the provider's typed execution error for stale fencing or unavailable providers.
    async fn sandbox_cancel(
        &self,
        sandbox_operation_id: &str,
        sandbox_fencing_token: u64,
    ) -> Result<(), SandboxCommandExecutionError>;
}

/// The terminal summary of one executed command.
///
/// Captured output is binary-safe and bounded by the request's
/// `sandbox_stdout_byte_limit` / `sandbox_stderr_byte_limit`; the transport
/// layer encodes it as `sandboxStdoutBase64` / `sandboxStderrBase64`
/// (`sandbox-command-execution-result.schema.json`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxCommandOutcome {
    /// Process exit code, when the process reached a terminal state.
    pub sandbox_exit_code: Option<i32>,
    /// Captured stdout, at most `sandbox_stdout_byte_limit` bytes.
    pub sandbox_stdout: Vec<u8>,
    /// Captured stderr, at most `sandbox_stderr_byte_limit` bytes.
    pub sandbox_stderr: Vec<u8>,
    /// Whether stdout hit its byte bound and was truncated.
    pub sandbox_stdout_truncated: bool,
    /// Whether stderr hit its byte bound and was truncated.
    pub sandbox_stderr_truncated: bool,
}

/// Why an execution failed without producing a terminal outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxCommandExecutionError {
    /// The request failed contract validation.
    InvalidRequest,
    /// The provider does not offer the requested capability.
    UnsupportedCapability,
    /// The execution policy denied the request.
    PolicyDenied,
    /// The fencing token is stale; retry requires new fencing authority.
    StaleFencing,
    /// The operation id is already bound to a different fingerprint.
    IdempotencyConflict,
}

impl fmt::Display for SandboxCommandExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidRequest => "sandbox command request failed validation",
            Self::UnsupportedCapability => "sandbox command capability is not supported",
            Self::PolicyDenied => "sandbox command was denied by execution policy",
            Self::StaleFencing => "sandbox command fencing token is stale",
            Self::IdempotencyConflict => {
                "sandbox command operation id conflicts with a different fingerprint"
            }
        };
        f.write_str(message)
    }
}

impl Error for SandboxCommandExecutionError {}

#[cfg(test)]
mod tests {
    use super::{
        sandbox_command_execution_fingerprint, SandboxCommandExecutionRequest,
        SandboxCommandLimits, SandboxCommandLimitsError,
    };
    use std::collections::BTreeMap;

    fn sandbox_request() -> SandboxCommandExecutionRequest {
        SandboxCommandExecutionRequest {
            sandbox_tenant_id: "tenant-1".to_owned(),
            sandbox_provider_id: "provider-1".to_owned(),
            sandbox_workspace_id: "workspace-1".to_owned(),
            sandbox_session_id: "session-1".to_owned(),
            sandbox_id: "sandbox-1".to_owned(),
            sandbox_runtime_binding_id: "binding-1".to_owned(),
            sandbox_fencing_token: 7,
            sandbox_command_operation_id: "operation-1".to_owned(),
            sandbox_executable: "toybox".to_owned(),
            sandbox_arguments: vec!["echo".to_owned(), "hello".to_owned()],
            sandbox_working_directory: "workspace/out".to_owned(),
            sandbox_environment: BTreeMap::from([("LANG".to_owned(), "c".to_owned())]),
            sandbox_command_limits: SandboxCommandLimits {
                sandbox_timeout_ms: 5_000,
                sandbox_stdout_byte_limit: 1_024,
                sandbox_stderr_byte_limit: 1_024,
                sandbox_cleanup_timeout_ms: 1_000,
                sandbox_max_process_count: 1,
            },
        }
    }

    #[test]
    fn fingerprint_is_deterministic_for_identical_requests() {
        assert_eq!(
            sandbox_command_execution_fingerprint(&sandbox_request()),
            sandbox_command_execution_fingerprint(&sandbox_request())
        );
    }

    #[test]
    fn fingerprint_moves_when_any_covered_field_moves() {
        let mut moved = sandbox_request();
        moved.sandbox_arguments = vec!["echo".to_owned(), "world".to_owned()];
        assert_ne!(
            sandbox_command_execution_fingerprint(&sandbox_request()),
            sandbox_command_execution_fingerprint(&moved)
        );

        let mut swapped = sandbox_request();
        swapped.sandbox_arguments = vec!["hello".to_owned(), "echo".to_owned()];
        assert_ne!(
            sandbox_command_execution_fingerprint(&sandbox_request()),
            sandbox_command_execution_fingerprint(&swapped),
            "argument order is preserved by the canonical encoding"
        );

        let mut fenced = sandbox_request();
        fenced.sandbox_fencing_token = 8;
        assert_ne!(
            sandbox_command_execution_fingerprint(&sandbox_request()),
            sandbox_command_execution_fingerprint(&fenced)
        );

        let mut limited = sandbox_request();
        limited.sandbox_command_limits.sandbox_timeout_ms = 6_000;
        assert_ne!(
            sandbox_command_execution_fingerprint(&sandbox_request()),
            sandbox_command_execution_fingerprint(&limited)
        );
    }

    #[test]
    fn cancellation_fingerprint_is_deterministic_and_field_sensitive() {
        use super::{sandbox_command_cancellation_fingerprint, SandboxCommandCancellationRequest};

        let request = SandboxCommandCancellationRequest {
            sandbox_tenant_id: "tenant-1".to_owned(),
            sandbox_provider_id: "provider-1".to_owned(),
            sandbox_workspace_id: "workspace-1".to_owned(),
            sandbox_session_id: "session-1".to_owned(),
            sandbox_id: "sandbox-1".to_owned(),
            sandbox_runtime_binding_id: "binding-1".to_owned(),
            sandbox_fencing_token: 7,
            sandbox_command_operation_id: "operation-1".to_owned(),
            sandbox_cancellation_operation_id: "cancel-1".to_owned(),
        };
        assert_eq!(
            sandbox_command_cancellation_fingerprint(&request),
            sandbox_command_cancellation_fingerprint(&request)
        );

        let mut moved = request.clone();
        moved.sandbox_cancellation_operation_id = "cancel-2".to_owned();
        assert_ne!(
            sandbox_command_cancellation_fingerprint(&request),
            sandbox_command_cancellation_fingerprint(&moved)
        );

        // A cancellation is bound to its target execution: a different command operation id
        // changes the fingerprint even with everything else fixed.
        let mut retargeted = request.clone();
        retargeted.sandbox_command_operation_id = "operation-2".to_owned();
        assert_ne!(
            sandbox_command_cancellation_fingerprint(&request),
            sandbox_command_cancellation_fingerprint(&retargeted)
        );
    }

    #[test]
    fn limits_validation_enforces_the_contract_maxima() {
        let valid = sandbox_request().sandbox_command_limits;
        assert_eq!(valid.validate(), Ok(()));

        let over = SandboxCommandLimits {
            sandbox_timeout_ms: 86_400_001,
            ..valid
        };
        assert_eq!(
            over.validate(),
            Err(SandboxCommandLimitsError::FieldOverBound)
        );

        let zero = SandboxCommandLimits {
            sandbox_max_process_count: 0,
            ..valid
        };
        assert_eq!(
            zero.validate(),
            Err(SandboxCommandLimitsError::FieldOverBound)
        );
    }
}
