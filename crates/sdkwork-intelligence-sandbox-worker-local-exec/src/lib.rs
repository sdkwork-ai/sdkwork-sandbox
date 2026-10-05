//! Local-lane start-command executor wiring (`REQ-2026-0034`,
//! `REVIEW-20261006-exec-wiring` + template-resolution slice): implements
//! the worker start-command port over the `REQ-2026-0007` local
//! tokio-process executor.
//!
//! The start command is resolved per plan through the declared
//! [`SandboxStartCommandResolverPort`] from the plan's template-version
//! reference — the template-resolution slice replacing the round-14
//! constructor-injected command. Outcome mapping stays honest (`WRE-03`):
//! only a zero exit reaches `started`; a non-zero exit lands `failed`; an
//! executor error without a terminal outcome lands `quarantined`; a
//! resolution failure is deterministic and lands `failed`.
//!
//! The command, scope and limits beyond the resolved command remain
//! construction-injected; the registry-backed resolver implementation is
//! the named next slice.

use std::collections::BTreeMap;
use std::sync::Arc;

use sdkwork_sandbox_provider_local::command_executor::SandboxLocalCommandExecutor;
use sdkwork_sandbox_provider_spi::{SandboxCommandExecutionRequest, SandboxCommandExecutor};

/// The resolved start command for one template version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxResolvedStartCommand {
    /// The bare executable name (boundary-allowlist-checked downstream).
    pub sandbox_executable: String,
    /// Argument vector for the start command.
    pub sandbox_arguments: Vec<String>,
}

/// Why a start-command resolution failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SandboxStartCommandResolutionError {
    /// The version reference resolves to no published start command.
    #[error("sandbox start command resolution found no command")]
    SandboxStartCommandNotFound,
}

/// The declared resolution seam: maps one plan's template-version reference
/// to its start command. The registry-backed implementation (backed by the
/// `REQ-2026-0029` authority) is the named next slice; this crate ships the
/// port only.
pub trait SandboxStartCommandResolverPort: Send + Sync {
    /// Resolves the start command for one template-version reference.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxStartCommandResolutionError::
    /// SandboxStartCommandNotFound`] when the reference carries no resolvable
    /// command.
    fn sandbox_resolve(
        &self,
        sandbox_template_version_ref: &str,
    ) -> Result<SandboxResolvedStartCommand, SandboxStartCommandResolutionError>;
}

/// The construction-injected execution scope and bounds (`WRE-04`, command
/// moved to the resolver).
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
    /// Logical working directory relative to the workspace root.
    pub sandbox_working_directory: String,
    /// Environment additions for the start command.
    pub sandbox_environment: BTreeMap<String, String>,
    /// The hard bounds for the start command.
    pub sandbox_command_limits: sdkwork_sandbox_provider_spi::SandboxCommandLimits,
}

/// The local-lane start-command executor: the narrow port's real
/// implementation over the `REQ-2026-0007` executor, resolving the command
/// per plan.
pub struct SandboxLocalStartCommandExecutor {
    sandbox_executor: Arc<SandboxLocalCommandExecutor>,
    sandbox_runtime: tokio::runtime::Runtime,
    sandbox_resolver: Arc<dyn SandboxStartCommandResolverPort>,
    sandbox_scope: SandboxStartCommandScope,
}

impl SandboxLocalStartCommandExecutor {
    /// Builds the wiring over a constructed executor, an injected resolver
    /// and scope, owning the dedicated bridge runtime.
    ///
    /// # Errors
    ///
    /// Returns the tokio runtime build error when the bridge runtime cannot
    /// be created.
    pub fn sandbox_new(
        sandbox_executor: Arc<SandboxLocalCommandExecutor>,
        sandbox_resolver: Arc<dyn SandboxStartCommandResolverPort>,
        sandbox_scope: SandboxStartCommandScope,
    ) -> std::io::Result<Self> {
        let sandbox_runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        Ok(Self {
            sandbox_executor,
            sandbox_runtime,
            sandbox_resolver,
            sandbox_scope,
        })
    }
}

impl sdkwork_intelligence_sandbox_worker_local::SandboxLaunchStartCommandPort
    for SandboxLocalStartCommandExecutor
{
    fn sandbox_start(
        &self,
        sandbox_launch_plan_ref: &str,
        sandbox_template_version_ref: &str,
    ) -> sdkwork_intelligence_sandbox_worker_local::SandboxStartCommandOutcome {
        use sdkwork_intelligence_sandbox_worker_local::SandboxStartCommandOutcome;

        let resolved = match self
            .sandbox_resolver
            .sandbox_resolve(sandbox_template_version_ref)
        {
            Ok(resolved) => resolved,
            // A missing command is a deterministic configuration failure.
            Err(_) => return SandboxStartCommandOutcome::Failed,
        };
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
            sandbox_executable: resolved.sandbox_executable,
            sandbox_arguments: resolved.sandbox_arguments,
            sandbox_working_directory: scope.sandbox_working_directory.clone(),
            sandbox_environment: scope.sandbox_environment.clone(),
            sandbox_command_limits: scope.sandbox_command_limits,
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
