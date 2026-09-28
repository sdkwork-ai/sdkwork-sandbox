//! The local provider's command executor: admission, registry, then delegation
//! to the real process runner.
//!
//! This type implements the provider-neutral [`SandboxCommandExecutor`] port
//! for the Local Provider lane. Admission (contract limits, fencing-token
//! validity, the pure-data host boundary, and the recomputed fingerprint) runs
//! before any delegation; live executions are tracked in a bounded registry
//! keyed by the contract's execution key triple (`tenantId`,
//! `sandboxProviderId`, `sandboxCommandOperationId`), so a fingerprint change
//! under the same key is an
//! [`SandboxCommandExecutionError::IdempotencyConflict`], a same-fingerprint
//! live replay is an
//! [`SandboxCommandExecutionError::OperationInProgress`], and a fenced
//! cancellation reaches the live child through the runner's cancellation
//! handle. The process lifecycle itself lives in
//! [`crate::process_runner::SandboxLocalTokioProcessRunner`].

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    sandbox_command_execution_fingerprint, SandboxCommandExecutionError,
    SandboxCommandExecutionRequest, SandboxCommandExecutor, SandboxCommandLimits,
    SandboxCommandOutcome,
};

use crate::command_admission::{admit_sandbox_command, SandboxLocalCommandAdmissionError};
use crate::host_boundary::{SandboxLocalHostBoundary, SandboxLocalHostBoundaryError};
use crate::process_runner::SandboxLiveCommandHandle;

/// One process-level execution handed to the runner seam, after admission passed.
#[derive(Clone)]
pub struct SandboxLocalAdmittedCommand {
    /// Bare executable name, allowlist-verified.
    pub sandbox_executable: String,
    /// Argument vector, order preserved.
    pub sandbox_arguments: Vec<String>,
    /// Logical working directory relative to the Workspace root.
    pub sandbox_working_directory: String,
    /// Environment additions that passed the boundary rules.
    pub sandbox_environment: std::collections::BTreeMap<String, String>,
    /// The hard bounds the runner must enforce.
    pub sandbox_command_limits: SandboxCommandLimits,
    /// Cancellation handle published in the live registry; the executor's
    /// `sandbox_cancel` notifies it and the runner's select loop observes it.
    pub sandbox_cancellation: SandboxLiveCommandHandle,
}

/// The process-runner seam. The real implementation
/// ([`crate::process_runner::SandboxLocalTokioProcessRunner`]) spawns the
/// child, enforces the admitted hard bounds, and reports the bounded outcome;
/// the executor and its callers never touch processes directly.
#[async_trait]
pub trait SandboxLocalCommandProcessRunner: Send + Sync {
    /// Runs one admitted command to a terminal outcome under the declared hard bounds.
    ///
    /// # Errors
    ///
    /// Returns the typed execution error for runner-level failures (unavailable, cleanup
    /// uncertainty quarantines the binding at the composition layer).
    async fn sandbox_run_admitted(
        &self,
        sandbox_command: &SandboxLocalAdmittedCommand,
    ) -> Result<SandboxCommandOutcome, SandboxCommandExecutionError>;
}

/// Upper bound on concurrently live executions. The registry must stay
/// bounded: an unbounded live map would grow with request volume and never
/// shrink under a misbehaving caller (OOM rule). At capacity the executor
/// refuses new work as policy-denied instead of growing without limit.
const MAX_SANDBOX_LIVE_COMMANDS: usize = 1024;

/// Node-level budget on buffered command output. Every live execution
/// reserves its declared stdout plus stderr byte caps at admission (the
/// worst case the contract allows), so the node's captured-output memory
/// stays bounded no matter how many executions are admitted or how chatty
/// their children are. An admission that would exceed the budget is
/// policy-denied instead of being allowed to grow the node toward OOM.
const MAX_SANDBOX_LIVE_OUTPUT_BYTES: u64 = 1 << 30;

/// The contract's execution key triple (`tenantId`, `sandboxProviderId`,
/// `sandboxCommandOperationId`). Keying live executions by the triple, not the
/// bare operation id, keeps two tenants that reuse an id string out of each
/// other's registry cells.
type SandboxLiveCommandKey = (String, String, String);

/// One live execution's registry entry.
struct SandboxLiveCommand {
    sandbox_fencing_token: u64,
    sandbox_fingerprint: String,
    sandbox_output_bytes: u64,
    sandbox_cancellation: SandboxLiveCommandHandle,
}

/// Bounded live-execution registry. The map locks only for the brief insert,
/// remove, and lookup map operations (never across an await), and the
/// [`SandboxLiveCommandGuard`] removes an entry on every drop path — including
/// a cancelled or panicked execution future — so the map size tracks
/// actually-live children only.
#[derive(Default)]
struct SandboxLiveCommandRegistry {
    sandbox_live: Mutex<HashMap<SandboxLiveCommandKey, SandboxLiveCommand>>,
}

impl SandboxLiveCommandRegistry {
    /// Registers one live execution under the contract's execution key triple.
    ///
    /// A duplicate key is a replay of a live execution: the same fingerprint
    /// is [`SandboxCommandExecutionError::OperationInProgress`] (the contract's
    /// `sameFingerprintInProgress` outcome), a moved fingerprint is
    /// [`SandboxCommandExecutionError::IdempotencyConflict`] — never a second
    /// spawn.
    fn sandbox_insert(
        &self,
        sandbox_key: &SandboxLiveCommandKey,
        sandbox_entry: SandboxLiveCommand,
    ) -> Result<(), SandboxCommandExecutionError> {
        let mut sandbox_live = self
            .sandbox_live
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(sandbox_existing) = sandbox_live.get(sandbox_key) {
            if sandbox_existing.sandbox_fingerprint != sandbox_entry.sandbox_fingerprint {
                tracing::warn!(
                    sandbox_tenant = %sandbox_key.0,
                    sandbox_provider = %sandbox_key.1,
                    sandbox_operation = %sandbox_key.2,
                    "a live sandbox command operation was replayed with a different fingerprint"
                );
                return Err(SandboxCommandExecutionError::IdempotencyConflict);
            }
            return Err(SandboxCommandExecutionError::OperationInProgress);
        }
        if sandbox_live.len() >= MAX_SANDBOX_LIVE_COMMANDS {
            return Err(SandboxCommandExecutionError::PolicyDenied);
        }
        let sandbox_reserved: u64 = sandbox_live
            .values()
            .map(|entry| entry.sandbox_output_bytes)
            .sum();
        if sandbox_reserved.saturating_add(sandbox_entry.sandbox_output_bytes)
            > MAX_SANDBOX_LIVE_OUTPUT_BYTES
        {
            return Err(SandboxCommandExecutionError::PolicyDenied);
        }
        sandbox_live.insert(sandbox_key.clone(), sandbox_entry);
        Ok(())
    }

    fn sandbox_remove(&self, sandbox_key: &SandboxLiveCommandKey) {
        self.sandbox_live
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(sandbox_key);
    }

    fn sandbox_lookup_cancel(
        &self,
        sandbox_key: &SandboxLiveCommandKey,
        sandbox_fencing_token: u64,
    ) -> Result<(), SandboxCommandExecutionError> {
        let sandbox_live = self
            .sandbox_live
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match sandbox_live.get(sandbox_key) {
            // Unknown or already-terminal operation: idempotent no-op.
            None => Ok(()),
            Some(entry) if entry.sandbox_fencing_token == sandbox_fencing_token => {
                entry.sandbox_cancellation.sandbox_cancel();
                Ok(())
            }
            // A cancellation under a stale token must not touch a newer
            // execution of the same operation id.
            Some(_) => Err(SandboxCommandExecutionError::StaleFencing),
        }
    }
}

/// Removes the registry entry for one key when dropped, on every path —
/// terminal outcome, runner error, cancelled future, or panic. This is what
/// keeps the bounded registry from leaking slots under the service layer's
/// operation-timeout cancellation of long-running executions.
struct SandboxLiveCommandGuard {
    sandbox_registry: Arc<SandboxLiveCommandRegistry>,
    sandbox_key: SandboxLiveCommandKey,
}

impl Drop for SandboxLiveCommandGuard {
    fn drop(&mut self) {
        self.sandbox_registry.sandbox_remove(&self.sandbox_key);
    }
}

/// The Local Provider command executor: admission gate, bounded registry, and
/// real runner delegation.
#[derive(Clone)]
pub struct SandboxLocalCommandExecutor {
    sandbox_boundary: Arc<SandboxLocalHostBoundary>,
    sandbox_runner: Arc<dyn SandboxLocalCommandProcessRunner>,
    sandbox_registry: Arc<SandboxLiveCommandRegistry>,
}

impl SandboxLocalCommandExecutor {
    /// Builds an executor over the shared boundary and a process runner.
    #[must_use]
    pub fn new(
        sandbox_boundary: Arc<SandboxLocalHostBoundary>,
        sandbox_runner: Arc<dyn SandboxLocalCommandProcessRunner>,
    ) -> Self {
        Self {
            sandbox_boundary,
            sandbox_runner,
            sandbox_registry: Arc::new(SandboxLiveCommandRegistry::default()),
        }
    }

    fn sandbox_admit(
        &self,
        sandbox_request: &SandboxCommandExecutionRequest,
    ) -> Result<SandboxLocalAdmittedCommand, SandboxCommandExecutionError> {
        // Fencing token 0 is the unset-lease sentinel (`SandboxFencingToken`
        // rejects it); a request carrying it is malformed, not merely fenced.
        if sandbox_request.sandbox_fencing_token == 0 {
            return Err(SandboxCommandExecutionError::InvalidRequest);
        }
        // The contract requires the executor to recompute the fingerprint from
        // the request. Verification is against the caller-supplied value when
        // one is supplied; the recomputed value is always registered, so a
        // replay under the same operation id with a different request is a
        // conflict at the registry, not a silent re-execution.
        let sandbox_fingerprint = sandbox_command_execution_fingerprint(sandbox_request);
        match admit_sandbox_command(
            &self.sandbox_boundary,
            sandbox_request,
            Some(&sandbox_fingerprint),
        ) {
            Ok(()) => Ok(SandboxLocalAdmittedCommand {
                sandbox_executable: sandbox_request.sandbox_executable.clone(),
                sandbox_arguments: sandbox_request.sandbox_arguments.clone(),
                sandbox_working_directory: sandbox_request.sandbox_working_directory.clone(),
                sandbox_environment: sandbox_request.sandbox_environment.clone(),
                sandbox_command_limits: sandbox_request.sandbox_command_limits,
                sandbox_cancellation: SandboxLiveCommandHandle::new(),
            }),
            Err(
                SandboxLocalCommandAdmissionError::LimitsOverBound(_)
                | SandboxLocalCommandAdmissionError::FingerprintMismatch,
            ) => Err(SandboxCommandExecutionError::InvalidRequest),
            Err(SandboxLocalCommandAdmissionError::BoundaryDenied(
                SandboxLocalHostBoundaryError::EnvironmentSensitive,
            ))
            | Err(SandboxLocalCommandAdmissionError::BoundaryDenied(
                SandboxLocalHostBoundaryError::EnvironmentProtected,
            ))
            | Err(SandboxLocalCommandAdmissionError::BoundaryDenied(
                SandboxLocalHostBoundaryError::EnvironmentDenied,
            )) => Err(SandboxCommandExecutionError::PolicyDenied),
            // A denied executable is a policy refusal of the requested
            // program, not a malformed request: the request shape may be
            // perfect and still not be on the allowlist.
            Err(SandboxLocalCommandAdmissionError::BoundaryDenied(
                SandboxLocalHostBoundaryError::ExecutableDenied,
            )) => Err(SandboxCommandExecutionError::PolicyDenied),
            Err(SandboxLocalCommandAdmissionError::BoundaryDenied(_)) => {
                Err(SandboxCommandExecutionError::InvalidRequest)
            }
        }
    }
}

#[async_trait]
impl SandboxCommandExecutor for SandboxLocalCommandExecutor {
    async fn sandbox_execute(
        &self,
        sandbox_request: &SandboxCommandExecutionRequest,
    ) -> Result<SandboxCommandOutcome, SandboxCommandExecutionError> {
        let sandbox_command = self.sandbox_admit(sandbox_request)?;
        // The executor recomputes the fingerprint from the request and
        // registers exactly this value, so a replay under the same execution
        // key with a different request is a conflict at the registry, not a
        // silent re-execution.
        let sandbox_fingerprint = sandbox_command_execution_fingerprint(sandbox_request);
        let sandbox_key: SandboxLiveCommandKey = (
            sandbox_request.sandbox_tenant_id.clone(),
            sandbox_request.sandbox_provider_id.clone(),
            sandbox_request.sandbox_command_operation_id.clone(),
        );
        self.sandbox_registry.sandbox_insert(
            &sandbox_key,
            SandboxLiveCommand {
                sandbox_fencing_token: sandbox_request.sandbox_fencing_token,
                sandbox_fingerprint,
                sandbox_output_bytes: sandbox_command
                    .sandbox_command_limits
                    .sandbox_stdout_byte_limit
                    .saturating_add(
                        sandbox_command
                            .sandbox_command_limits
                            .sandbox_stderr_byte_limit,
                    ),
                sandbox_cancellation: sandbox_command.sandbox_cancellation.clone(),
            },
        )?;
        // The guard removes the live entry on every drop path, so a cancelled
        // or panicked execution can never strand its registry slot.
        let sandbox_live_guard = SandboxLiveCommandGuard {
            sandbox_registry: Arc::clone(&self.sandbox_registry),
            sandbox_key,
        };
        let sandbox_outcome = self
            .sandbox_runner
            .sandbox_run_admitted(&sandbox_command)
            .await;
        drop(sandbox_live_guard);
        sandbox_outcome
    }

    async fn sandbox_cancel(
        &self,
        sandbox_tenant_id: &str,
        sandbox_provider_id: &str,
        sandbox_operation_id: &str,
        sandbox_fencing_token: u64,
    ) -> Result<(), SandboxCommandExecutionError> {
        self.sandbox_registry.sandbox_lookup_cancel(
            &(
                sandbox_tenant_id.to_owned(),
                sandbox_provider_id.to_owned(),
                sandbox_operation_id.to_owned(),
            ),
            sandbox_fencing_token,
        )
    }
}
