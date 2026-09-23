use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::command_executor::{SandboxLocalAdmittedCommand, SandboxLocalCommandExecutor, SandboxLocalCommandProcessRunner};
use crate::host_boundary::SandboxLocalHostBoundary;
use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    SandboxCommandExecutionError, SandboxCommandExecutionRequest, SandboxCommandExecutor,
    SandboxCommandLimits, SandboxCommandOutcome,
};

struct RecordingRunner {
    sandbox_outcome: Result<SandboxCommandOutcome, SandboxCommandExecutionError>,
}

#[async_trait]
impl SandboxLocalCommandProcessRunner for RecordingRunner {
    async fn sandbox_run_admitted(
        &self,
        sandbox_command: &SandboxLocalAdmittedCommand,
    ) -> Result<SandboxCommandOutcome, SandboxCommandExecutionError> {
        assert_eq!(sandbox_command.sandbox_executable, "toybox");
        assert_eq!(
            sandbox_command.sandbox_environment.get("SANDBOX_MODE").map(String::as_str),
            Some("strict")
        );
        self.sandbox_outcome.clone()
    }
}

fn sandbox_executor(
    outcome: Result<SandboxCommandOutcome, SandboxCommandExecutionError>,
) -> SandboxLocalCommandExecutor {
    SandboxLocalCommandExecutor::new(
        Arc::new(SandboxLocalHostBoundary::new(
            BTreeSet::from(["toybox".to_owned()]),
            BTreeSet::from(["SANDBOX_MODE".to_owned()]),
        )),
        Arc::new(RecordingRunner { sandbox_outcome: outcome }),
    )
}

fn sandbox_request() -> SandboxCommandExecutionRequest {
    SandboxCommandExecutionRequest {
        sandbox_tenant_id: "tenant-1".to_owned(),
        sandbox_provider_id: "provider-local".to_owned(),
        sandbox_workspace_id: "workspace-1".to_owned(),
        sandbox_session_id: "session-1".to_owned(),
        sandbox_id: "sandbox-1".to_owned(),
        sandbox_runtime_binding_id: "binding-1".to_owned(),
        sandbox_fencing_token: 7,
        sandbox_command_operation_id: "operation-1".to_owned(),
        sandbox_executable: "toybox".to_owned(),
        sandbox_arguments: vec!["echo".to_owned()],
        sandbox_working_directory: "workspace".to_owned(),
        sandbox_environment: BTreeMap::from([("SANDBOX_MODE".to_owned(), "strict".to_owned())]),
        sandbox_command_limits: SandboxCommandLimits::default_limits(),
    }
}

trait DefaultLimits {
    fn default_limits() -> SandboxCommandLimits;
}

impl DefaultLimits for SandboxCommandLimits {
    fn default_limits() -> Self {
        Self {
            sandbox_timeout_ms: 5_000,
            sandbox_stdout_byte_limit: 1_024,
            sandbox_stderr_byte_limit: 1_024,
            sandbox_cleanup_timeout_ms: 1_000,
            sandbox_max_process_count: 1,
        }
    }
}

#[tokio::test]
async fn admitted_commands_reach_the_runner_and_map_the_outcome() {
    let executor = sandbox_executor(Ok(SandboxCommandOutcome {
        sandbox_exit_code: Some(0),
        sandbox_stdout_truncated: false,
        sandbox_stderr_truncated: false,
    }));
    let outcome = executor.sandbox_execute(&sandbox_request()).await.unwrap();
    assert_eq!(outcome.sandbox_exit_code, Some(0));
}

#[tokio::test]
async fn boundary_denials_fail_before_the_runner_is_consulted() {
    let executor = sandbox_executor(Ok(SandboxCommandOutcome {
        sandbox_exit_code: Some(0),
        sandbox_stdout_truncated: false,
        sandbox_stderr_truncated: false,
    }));
    let mut denied = sandbox_request();
    denied.sandbox_executable = "curl".to_owned();
    assert_eq!(
        executor.sandbox_execute(&denied).await.unwrap_err(),
        SandboxCommandExecutionError::InvalidRequest
    );

    let mut sensitive = sandbox_request();
    sensitive.sandbox_environment.insert("API_TOKEN".to_owned(), "x".to_owned());
    assert_eq!(
        executor.sandbox_execute(&sensitive).await.unwrap_err(),
        SandboxCommandExecutionError::PolicyDenied
    );
}

#[tokio::test]
async fn runner_failures_map_to_the_typed_execution_error() {
    let executor = sandbox_executor(Err(SandboxCommandExecutionError::UnsupportedCapability));
    assert_eq!(
        executor.sandbox_execute(&sandbox_request()).await.unwrap_err(),
        SandboxCommandExecutionError::UnsupportedCapability
    );
}
