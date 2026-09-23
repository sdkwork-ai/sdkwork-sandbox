//! The local provider's command executor: admission, then delegation to a process runner.
//!
//! This type implements the provider-neutral [`SandboxCommandExecutor`] port for the Local
//! Provider lane. Admission (contract limits, the pure-data host boundary, and the recomputed
//! fingerprint) runs before any delegation; the actual process lifecycle lives behind the
//! [`SandboxLocalCommandProcessRunner`] seam, whose real-platform implementation is gated by the
//! local-provider-host-boundary contract's `requiredRealEvidence`. This module contains no
//! process-spawn code.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    SandboxCommandExecutionError, SandboxCommandExecutionRequest, SandboxCommandExecutor,
    SandboxCommandOutcome,
};

use crate::command_admission::{admit_sandbox_command, SandboxLocalCommandAdmissionError};
use crate::host_boundary::{SandboxLocalHostBoundary, SandboxLocalHostBoundaryError};

/// One process-level execution handed to the runner seam, after admission passed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxLocalAdmittedCommand {
    /// Bare executable name, allowlist-verified.
    pub sandbox_executable: String,
    /// Argument vector, order preserved.
    pub sandbox_arguments: Vec<String>,
    /// Logical working directory relative to the Workspace root.
    pub sandbox_working_directory: String,
    /// Environment additions that passed the boundary rules.
    pub sandbox_environment: BTreeMap<String, String>,
}

/// The process-runner seam. A real implementation (Windows Job Object supervision, Linux
/// delegated cgroup v2, hard timeout and output truncation) is the evidence-gated execution
/// slice; the executor and its callers never touch processes directly.
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

/// The Local Provider command executor: admission gate plus runner delegation.
#[derive(Clone)]
pub struct SandboxLocalCommandExecutor {
    sandbox_boundary: Arc<SandboxLocalHostBoundary>,
    sandbox_runner: Arc<dyn SandboxLocalCommandProcessRunner>,
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
        }
    }

    fn sandbox_admit(
        &self,
        sandbox_request: &SandboxCommandExecutionRequest,
    ) -> Result<SandboxLocalAdmittedCommand, SandboxCommandExecutionError> {
        // The executor recomputes the fingerprint at the durable boundary (composition layer);
        // this port layer admits on limits and boundary rules, then delegates.
        match admit_sandbox_command(&self.sandbox_boundary, sandbox_request, None) {
            Ok(()) => Ok(SandboxLocalAdmittedCommand {
                sandbox_executable: sandbox_request.sandbox_executable.clone(),
                sandbox_arguments: sandbox_request.sandbox_arguments.clone(),
                sandbox_working_directory: sandbox_request.sandbox_working_directory.clone(),
                sandbox_environment: sandbox_request.sandbox_environment.clone(),
            }),
            Err(SandboxLocalCommandAdmissionError::LimitsOverBound(_))
            | Err(SandboxLocalCommandAdmissionError::FingerprintMismatch) => {
                Err(SandboxCommandExecutionError::InvalidRequest)
            }
            Err(SandboxLocalCommandAdmissionError::BoundaryDenied(
                SandboxLocalHostBoundaryError::EnvironmentSensitive,
            ))
            | Err(SandboxLocalCommandAdmissionError::BoundaryDenied(
                SandboxLocalHostBoundaryError::EnvironmentProtected,
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
        self.sandbox_runner.sandbox_run_admitted(&sandbox_command).await
    }

    async fn sandbox_cancel(
        &self,
        _sandbox_operation_id: &str,
        _sandbox_fencing_token: u64,
    ) -> Result<(), SandboxCommandExecutionError> {
        // Cancellation reaches the live process through the runner's supervision id; the
        // durable operation lookup that resolves an operation id to its live supervision is
        // part of the evidence-gated execution slice.
        Err(SandboxCommandExecutionError::UnsupportedCapability)
    }
}
