//! Local-lane start-command executor wiring (`REQ-2026-0034`,
//! `REVIEW-20261006-exec-wiring`): implements the worker start-command port
//! over the `REQ-2026-0007` local tokio-process executor.
//!
//! The port is synchronous and the executor is async, so the adapter owns a
//! dedicated multi-thread runtime and bridges through `block_on` (`WRE-02`).
//! Outcome mapping is honest (`WRE-03`): only a zero exit reaches `started`;
//! a non-zero exit lands `failed`; an executor error without a terminal
//! outcome is uncertainty and lands `quarantined`. The command, scope and
//! limits are injected at construction (`WRE-04`) — template-version
//! resolution is the named next slice.

use std::collections::BTreeMap;
use std::sync::Arc;

use sdkwork_intelligence_sandbox_worker_local::{
    SandboxLaunchStartCommandPort, SandboxStartCommandOutcome,
};
use sdkwork_sandbox_provider_local::command_executor::SandboxLocalCommandExecutor;
use sdkwork_sandbox_provider_spi::{SandboxCommandExecutionRequest, SandboxCommandExecutor};

/// The construction-injected execution scope and command (`WRE-04`).
#[derive(Clone, Debug)]
pub struct SandboxStartCommandScope {
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
    /// Fencing token carried on every request.
    pub sandbox_fencing_token: u64,
    /// The bare executable name (boundary-allowlist-checked).
    pub sandbox_executable: String,
    /// Argument vector for the start command.
    pub sandbox_arguments: Vec<String>,
    /// Logical working directory relative to the workspace root.
    pub sandbox_working_directory: String,
    /// Environment additions for the start command.
    pub sandbox_environment: BTreeMap<String, String>,
    /// The hard bounds for the start command.
    pub sandbox_command_limits: sdkwork_sandbox_provider_spi::SandboxCommandLimits,
}

/// The local-lane start-command executor: the narrow port's real
/// implementation over the `REQ-2026-0007` executor.
pub struct SandboxLocalStartCommandExecutor {
    sandbox_executor: Arc<SandboxLocalCommandExecutor>,
    sandbox_runtime: tokio::runtime::Runtime,
    sandbox_scope: SandboxStartCommandScope,
}

impl SandboxLocalStartCommandExecutor {
    /// Builds the wiring over a constructed executor and an injected scope,
    /// owning the dedicated bridge runtime.
    ///
    /// # Errors
    ///
    /// Returns the tokio runtime build error when the bridge runtime cannot
    /// be created.
    pub fn sandbox_new(
        sandbox_executor: Arc<SandboxLocalCommandExecutor>,
        sandbox_scope: SandboxStartCommandScope,
    ) -> std::io::Result<Self> {
        let sandbox_runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        Ok(Self {
            sandbox_executor,
            sandbox_runtime,
            sandbox_scope,
        })
    }
}

impl SandboxLaunchStartCommandPort for SandboxLocalStartCommandExecutor {
    fn sandbox_start(&self, sandbox_launch_plan_ref: &str) -> SandboxStartCommandOutcome {
        let scope = &self.sandbox_scope;
        let request = SandboxCommandExecutionRequest {
            sandbox_tenant_id: scope.sandbox_tenant_id.clone(),
            sandbox_provider_id: scope.sandbox_provider_id.clone(),
            sandbox_workspace_id: scope.sandbox_workspace_id.clone(),
            sandbox_session_id: scope.sandbox_session_id.clone(),
            sandbox_id: scope.sandbox_id.clone(),
            sandbox_runtime_binding_id: scope.sandbox_runtime_binding_id.clone(),
            sandbox_fencing_token: scope.sandbox_fencing_token,
            sandbox_command_operation_id: format!("start-{sandbox_launch_plan_ref}"),
            sandbox_executable: scope.sandbox_executable.clone(),
            sandbox_arguments: scope.sandbox_arguments.clone(),
            sandbox_working_directory: scope.sandbox_working_directory.clone(),
            sandbox_environment: scope.sandbox_environment.clone(),
            sandbox_command_limits: scope.sandbox_command_limits.clone(),
        };
        match self
            .sandbox_runtime
            .block_on(self.sandbox_executor.sandbox_execute(&request))
        {
            Ok(outcome) if outcome.sandbox_exit_code == Some(0) => {
                SandboxStartCommandOutcome::Started
            }
            Ok(_) => SandboxStartCommandOutcome::Failed,
            // No terminal outcome: the result is unavailable, which is
            // uncertainty — never a claimed success.
            Err(_) => SandboxStartCommandOutcome::Uncertain,
        }
    }
}
