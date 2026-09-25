//! The local provider's command executor: admission, registry, then delegation
//! to the real process runner.
//!
//! This type implements the provider-neutral [`SandboxCommandExecutor`] port
//! for the Local Provider lane. Admission (contract limits, fencing-token
//! validity, the pure-data host boundary, and the recomputed fingerprint) runs
//! before any delegation; live executions are tracked in a bounded registry
//! keyed by the durable operation id, so a fingerprint change under the same
//! id is an [`SandboxCommandExecutionError::IdempotencyConflict`] and a
//! fenced cancellation reaches the live child through the runner's
//! cancellation handle. The process lifecycle itself lives in
//! [`crate::process_runner::SandboxLocalTokioProcessRunner`].

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    sandbox_command_execution_fingerprint, SandboxCommandExecutionError,
    SandboxCommandExecutionRequest, SandboxCommandExecutor, SandboxCommandLimits,
    SandboxCommandOutcome,
};
use tokio::sync::Mutex;

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

/// One live execution's registry entry.
struct SandboxLiveCommand {
    sandbox_fencing_token: u64,
    sandbox_fingerprint: String,
    sandbox_cancellation: SandboxLiveCommandHandle,
}

/// Bounded live-execution registry. Every completion path removes its entry
/// exactly once, so the map size tracks actually-live children only.
#[derive(Default)]
struct SandboxLiveCommandRegistry {
    sandbox_live: Mutex<HashMap<String, SandboxLiveCommand>>,
}

impl SandboxLiveCommandRegistry {
    async fn sandbox_insert(
        &self,
        sandbox_operation_id: &str,
        sandbox_entry: SandboxLiveCommand,
    ) -> Result<(), SandboxCommandExecutionError> {
        let mut sandbox_live = self.sandbox_live.lock().await;
        if let Some(sandbox_existing) = sandbox_live.get(sandbox_operation_id) {
            if sandbox_existing.sandbox_fingerprint != sandbox_entry.sandbox_fingerprint {
                tracing::warn!(
                    sandbox_operation = %sandbox_operation_id,
                    "a live sandbox command operation id was replayed with a different fingerprint"
                );
            }
            // A duplicate operation id is a replay of a live execution: a
            // matching fingerprint still may not run twice concurrently.
            return Err(SandboxCommandExecutionError::IdempotencyConflict);
        }
        if sandbox_live.len() >= MAX_SANDBOX_LIVE_COMMANDS {
            return Err(SandboxCommandExecutionError::PolicyDenied);
        }
        sandbox_live.insert(sandbox_operation_id.to_owned(), sandbox_entry);
        Ok(())
    }

    /// Removes the entry and returns its cancellation handle so the run's
    /// select loop keeps listening even while the entry is already gone.
    async fn sandbox_remove(&self, sandbox_operation_id: &str) -> Option<SandboxLiveCommandHandle> {
        let mut sandbox_live = self.sandbox_live.lock().await;
        sandbox_live
            .remove(sandbox_operation_id)
            .map(|entry| entry.sandbox_cancellation)
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
        self.sandbox_registry
            .sandbox_insert(
                &sandbox_request.sandbox_command_operation_id,
                SandboxLiveCommand {
                    sandbox_fencing_token: sandbox_request.sandbox_fencing_token,
                    sandbox_fingerprint: sandbox_command_execution_fingerprint(sandbox_request),
                    sandbox_cancellation: sandbox_command.sandbox_cancellation.clone(),
                },
            )
            .await?;
        let sandbox_outcome = self
            .sandbox_runner
            .sandbox_run_admitted(&sandbox_command)
            .await;
        // Every path — terminal outcome or runner-level error — removes the
        // live entry, so the registry never grows beyond live children.
        self.sandbox_registry
            .sandbox_remove(&sandbox_request.sandbox_command_operation_id)
            .await;
        sandbox_outcome
    }

    async fn sandbox_cancel(
        &self,
        sandbox_operation_id: &str,
        sandbox_fencing_token: u64,
    ) -> Result<(), SandboxCommandExecutionError> {
        let sandbox_live = self.sandbox_registry.sandbox_live.lock().await;
        match sandbox_live.get(sandbox_operation_id) {
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
