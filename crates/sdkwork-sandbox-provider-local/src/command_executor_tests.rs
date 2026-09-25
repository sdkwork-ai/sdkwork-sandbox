use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::command_executor::{
    SandboxLocalAdmittedCommand, SandboxLocalCommandExecutor, SandboxLocalCommandProcessRunner,
};
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
            sandbox_command
                .sandbox_environment
                .get("SANDBOX_MODE")
                .map(String::as_str),
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
        Arc::new(RecordingRunner {
            sandbox_outcome: outcome,
        }),
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

fn sandbox_terminal_outcome() -> SandboxCommandOutcome {
    SandboxCommandOutcome {
        sandbox_exit_code: Some(0),
        sandbox_stdout: b"ok".to_vec(),
        sandbox_stderr: Vec::new(),
        sandbox_stdout_truncated: false,
        sandbox_stderr_truncated: false,
    }
}

#[tokio::test]
async fn admitted_commands_reach_the_runner_and_map_the_outcome() {
    let executor = sandbox_executor(Ok(sandbox_terminal_outcome()));
    let outcome = executor.sandbox_execute(&sandbox_request()).await.unwrap();
    assert_eq!(outcome.sandbox_exit_code, Some(0));
}

#[tokio::test]
async fn boundary_denials_fail_before_the_runner_is_consulted() {
    let executor = sandbox_executor(Ok(sandbox_terminal_outcome()));
    let mut denied = sandbox_request();
    denied.sandbox_executable = "curl".to_owned();
    // A denied executable is a policy refusal, not a malformed request.
    assert_eq!(
        executor.sandbox_execute(&denied).await.unwrap_err(),
        SandboxCommandExecutionError::PolicyDenied
    );

    let mut sensitive = sandbox_request();
    sensitive
        .sandbox_environment
        .insert("API_TOKEN".to_owned(), "x".to_owned());
    assert_eq!(
        executor.sandbox_execute(&sensitive).await.unwrap_err(),
        SandboxCommandExecutionError::PolicyDenied
    );
}

#[tokio::test]
async fn runner_failures_map_to_the_typed_execution_error() {
    let executor = sandbox_executor(Err(SandboxCommandExecutionError::UnsupportedCapability));
    assert_eq!(
        executor
            .sandbox_execute(&sandbox_request())
            .await
            .unwrap_err(),
        SandboxCommandExecutionError::UnsupportedCapability
    );
}

#[tokio::test]
async fn a_zero_fencing_token_is_a_malformed_request() {
    let executor = sandbox_executor(Ok(sandbox_terminal_outcome()));
    let mut zero_token = sandbox_request();
    zero_token.sandbox_fencing_token = 0;
    assert_eq!(
        executor.sandbox_execute(&zero_token).await.unwrap_err(),
        SandboxCommandExecutionError::InvalidRequest
    );
}

#[tokio::test]
async fn a_fenced_cancel_reaches_the_live_execution_and_a_stale_token_is_refused() {
    // A runner that blocks until cancelled proves the cancel signal lands.
    struct BlockingRunner;

    #[async_trait]
    impl SandboxLocalCommandProcessRunner for BlockingRunner {
        async fn sandbox_run_admitted(
            &self,
            sandbox_command: &SandboxLocalAdmittedCommand,
        ) -> Result<SandboxCommandOutcome, SandboxCommandExecutionError> {
            sandbox_command
                .sandbox_cancellation
                .sandbox_cancelled()
                .await;
            Ok(SandboxCommandOutcome {
                sandbox_exit_code: None,
                sandbox_stdout: Vec::new(),
                sandbox_stderr: Vec::new(),
                sandbox_stdout_truncated: false,
                sandbox_stderr_truncated: false,
            })
        }
    }

    let executor = Arc::new(SandboxLocalCommandExecutor::new(
        Arc::new(SandboxLocalHostBoundary::new(
            BTreeSet::from(["toybox".to_owned()]),
            BTreeSet::from(["SANDBOX_MODE".to_owned()]),
        )),
        Arc::new(BlockingRunner),
    ));
    let sandbox_request = sandbox_request();
    let sandbox_executor = executor.clone();
    let sandbox_execution =
        tokio::spawn(async move { sandbox_executor.sandbox_execute(&sandbox_request).await });
    // Give the execution a moment to register, then cancel under the wrong
    // and the right token.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    assert_eq!(
        executor.sandbox_cancel("operation-1", 6).await.unwrap_err(),
        SandboxCommandExecutionError::StaleFencing
    );
    executor
        .sandbox_cancel("operation-1", 7)
        .await
        .expect("matching token must cancel");

    let sandbox_outcome = sandbox_execution
        .await
        .expect("join")
        .expect("cancelled run");
    assert_eq!(None, sandbox_outcome.sandbox_exit_code);
}

#[tokio::test]
async fn a_duplicate_live_operation_id_is_an_idempotency_conflict() {
    struct BlockingRunner;

    #[async_trait]
    impl SandboxLocalCommandProcessRunner for BlockingRunner {
        async fn sandbox_run_admitted(
            &self,
            sandbox_command: &SandboxLocalAdmittedCommand,
        ) -> Result<SandboxCommandOutcome, SandboxCommandExecutionError> {
            sandbox_command
                .sandbox_cancellation
                .sandbox_cancelled()
                .await;
            Ok(SandboxCommandOutcome {
                sandbox_exit_code: None,
                sandbox_stdout: Vec::new(),
                sandbox_stderr: Vec::new(),
                sandbox_stdout_truncated: false,
                sandbox_stderr_truncated: false,
            })
        }
    }

    let executor = Arc::new(SandboxLocalCommandExecutor::new(
        Arc::new(SandboxLocalHostBoundary::new(
            BTreeSet::from(["toybox".to_owned()]),
            BTreeSet::from(["SANDBOX_MODE".to_owned()]),
        )),
        Arc::new(BlockingRunner),
    ));
    let first = sandbox_request();
    let sandbox_executor = executor.clone();
    let sandbox_execution =
        tokio::spawn(async move { sandbox_executor.sandbox_execute(&first).await });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let replay = sandbox_request();
    assert_eq!(
        executor.sandbox_execute(&replay).await.unwrap_err(),
        SandboxCommandExecutionError::IdempotencyConflict
    );

    executor
        .sandbox_cancel("operation-1", 7)
        .await
        .expect("unblock the first run");
    let _ = sandbox_execution.await;
}
