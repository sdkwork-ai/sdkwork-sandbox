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
    let outcome = executor
        .sandbox_execute(&sandbox_request())
        .await
        .expect("admitted execution should produce a terminal outcome");
    assert_eq!(outcome.sandbox_exit_code, Some(0));
}

#[tokio::test]
async fn boundary_denials_fail_before_the_runner_is_consulted() {
    let executor = sandbox_executor(Ok(sandbox_terminal_outcome()));
    let mut denied = sandbox_request();
    denied.sandbox_executable = "curl".to_owned();
    // A denied executable is a policy refusal, not a malformed request.
    assert_eq!(
        executor
            .sandbox_execute(&denied)
            .await
            .expect_err("expected an execution error"),
        SandboxCommandExecutionError::PolicyDenied
    );

    let mut sensitive = sandbox_request();
    sensitive
        .sandbox_environment
        .insert("API_TOKEN".to_owned(), "x".to_owned());
    assert_eq!(
        executor
            .sandbox_execute(&sensitive)
            .await
            .expect_err("expected an execution error"),
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
            .expect_err("expected an execution error"),
        SandboxCommandExecutionError::UnsupportedCapability
    );
}

#[tokio::test]
async fn a_zero_fencing_token_is_a_malformed_request() {
    let executor = sandbox_executor(Ok(sandbox_terminal_outcome()));
    let mut zero_token = sandbox_request();
    zero_token.sandbox_fencing_token = 0;
    assert_eq!(
        executor
            .sandbox_execute(&zero_token)
            .await
            .expect_err("expected an execution error"),
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
        executor
            .sandbox_cancel("tenant-1", "provider-local", "operation-1", 6)
            .await
            .expect_err("expected an execution error"),
        SandboxCommandExecutionError::StaleFencing
    );
    executor
        .sandbox_cancel("tenant-1", "provider-local", "operation-1", 7)
        .await
        .expect("matching token must cancel");

    let sandbox_outcome = sandbox_execution
        .await
        .expect("join")
        .expect("cancelled run");
    assert_eq!(None, sandbox_outcome.sandbox_exit_code);
}

#[tokio::test]
async fn a_same_fingerprint_live_replay_is_operation_in_progress() {
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
        executor
            .sandbox_execute(&replay)
            .await
            .expect_err("expected an execution error"),
        SandboxCommandExecutionError::OperationInProgress
    );

    executor
        .sandbox_cancel("tenant-1", "provider-local", "operation-1", 7)
        .await
        .expect("unblock the first run");
    let _ = sandbox_execution.await;
}

#[tokio::test]
async fn a_moved_fingerprint_under_a_live_operation_id_is_an_idempotency_conflict() {
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

    // The same execution key with a moved covered field is the contract's
    // `differentFingerprint` outcome: an idempotency conflict, never a run.
    let mut tampered = sandbox_request();
    tampered.sandbox_arguments.push("moved".to_owned());
    assert_eq!(
        executor
            .sandbox_execute(&tampered)
            .await
            .expect_err("expected an execution error"),
        SandboxCommandExecutionError::IdempotencyConflict
    );

    executor
        .sandbox_cancel("tenant-1", "provider-local", "operation-1", 7)
        .await
        .expect("unblock the first run");
    let _ = sandbox_execution.await;
}

#[tokio::test]
async fn the_same_operation_id_under_another_tenant_never_conflicts() {
    let (sandbox_admitted_sender, mut sandbox_admitted_receiver) = tokio::sync::mpsc::channel(16);
    let executor = sandbox_signalling_executor(sandbox_admitted_sender);
    // The registry keys by the contract's execution key triple, so a second
    // tenant reusing the operation id string runs in its own cell.
    let first = sandbox_request();
    let mut second = sandbox_request();
    second.sandbox_tenant_id = "tenant-2".to_owned();
    let sandbox_first_executor = executor.clone();
    let sandbox_first =
        tokio::spawn(async move { sandbox_first_executor.sandbox_execute(&first).await });
    await_sandbox_admissions(&mut sandbox_admitted_receiver, 1).await;
    let sandbox_second_executor = executor.clone();
    let sandbox_second =
        tokio::spawn(async move { sandbox_second_executor.sandbox_execute(&second).await });
    await_sandbox_admissions(&mut sandbox_admitted_receiver, 1).await;

    executor
        .sandbox_cancel("tenant-1", "provider-local", "operation-1", 7)
        .await
        .expect("the tenant-1 cancel must reach its own cell");
    executor
        .sandbox_cancel("tenant-2", "provider-local", "operation-1", 7)
        .await
        .expect("the tenant-2 cancel must reach its own cell");

    let sandbox_first = sandbox_first.await.expect("join").expect("first run");
    let sandbox_second = sandbox_second.await.expect("join").expect("second run");
    assert_eq!(None, sandbox_first.sandbox_exit_code);
    assert_eq!(None, sandbox_second.sandbox_exit_code);
}
/// Blocking runner that signals each admission on a channel before parking
/// on cancellation. Tests await the signals instead of sleeping, so
/// assertions about registry occupancy stay deterministic under load.
struct SignallingBlockingRunner {
    sandbox_admitted_sender: tokio::sync::mpsc::Sender<()>,
}

#[async_trait]
impl SandboxLocalCommandProcessRunner for SignallingBlockingRunner {
    async fn sandbox_run_admitted(
        &self,
        sandbox_command: &SandboxLocalAdmittedCommand,
    ) -> Result<SandboxCommandOutcome, SandboxCommandExecutionError> {
        let _ = self.sandbox_admitted_sender.send(()).await;
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

fn sandbox_signalling_executor(
    sandbox_admitted_sender: tokio::sync::mpsc::Sender<()>,
) -> Arc<SandboxLocalCommandExecutor> {
    Arc::new(SandboxLocalCommandExecutor::new(
        Arc::new(SandboxLocalHostBoundary::new(
            BTreeSet::from(["toybox".to_owned()]),
            BTreeSet::from(["SANDBOX_MODE".to_owned()]),
        )),
        Arc::new(SignallingBlockingRunner {
            sandbox_admitted_sender,
        }),
    ))
}

/// Awaits exactly `sandbox_expected_admissions` runner-entry signals with a
/// generous timeout, so a loaded CI machine cannot race the assertions.
/// Async on purpose: the default `#[tokio::test]` runtime is single-threaded,
/// so a blocking recv here would starve the very tasks being awaited.
async fn await_sandbox_admissions(
    sandbox_admitted_receiver: &mut tokio::sync::mpsc::Receiver<()>,
    sandbox_expected_admissions: usize,
) {
    for _ in 0..sandbox_expected_admissions {
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            sandbox_admitted_receiver.recv(),
        )
        .await
        .expect("the admitted execution must signal the runner in time")
        .expect("the admission signal channel must stay open");
    }
}

#[tokio::test]
async fn the_node_output_budget_refuses_admissions_beyond_the_declared_caps() {
    let (sandbox_admitted_sender, mut sandbox_admitted_receiver) = tokio::sync::mpsc::channel(16);
    let executor = sandbox_signalling_executor(sandbox_admitted_sender);
    // Eight executions from eight different tenants at the contract's
    // maximum output caps (64 MiB per stream, 128 MiB reserved each) fill
    // exactly the node's 1 GiB output budget. Each tenant stays within its
    // own 256 MiB share, so the node-wide budget is what saturates.
    let mut sandbox_executions = Vec::new();
    for sandbox_index in 0..8 {
        let mut sandbox_request = sandbox_request();
        sandbox_request.sandbox_tenant_id = format!("budget-tenant-{sandbox_index}");
        sandbox_request.sandbox_command_operation_id = format!("budget-operation-{sandbox_index}");
        sandbox_request
            .sandbox_command_limits
            .sandbox_stdout_byte_limit = 67_108_864;
        sandbox_request
            .sandbox_command_limits
            .sandbox_stderr_byte_limit = 67_108_864;
        let sandbox_executor = executor.clone();
        sandbox_executions.push(tokio::spawn(async move {
            sandbox_executor.sandbox_execute(&sandbox_request).await
        }));
    }
    await_sandbox_admissions(&mut sandbox_admitted_receiver, 8).await;

    // A ninth max-cap admission would push the worst-case buffered output
    // past the node budget, so it is policy-denied instead of admitted.
    let mut sandbox_ninth = sandbox_request();
    sandbox_ninth.sandbox_tenant_id = "budget-tenant-8".to_owned();
    sandbox_ninth.sandbox_command_operation_id = "budget-operation-8".to_owned();
    sandbox_ninth
        .sandbox_command_limits
        .sandbox_stdout_byte_limit = 67_108_864;
    sandbox_ninth
        .sandbox_command_limits
        .sandbox_stderr_byte_limit = 67_108_864;
    assert_eq!(
        executor
            .sandbox_execute(&sandbox_ninth)
            .await
            .expect_err("expected an execution error"),
        SandboxCommandExecutionError::PolicyDenied
    );

    for sandbox_index in 0..8 {
        executor
            .sandbox_cancel(
                &format!("budget-tenant-{sandbox_index}"),
                "provider-local",
                &format!("budget-operation-{sandbox_index}"),
                7,
            )
            .await
            .expect("settle every budget reservation");
    }
    for sandbox_execution in sandbox_executions {
        let _ = sandbox_execution.await;
    }
}
#[tokio::test]
async fn the_per_tenant_budget_partitions_shares_so_one_tenant_cannot_starve_another() {
    let (sandbox_admitted_sender, mut sandbox_admitted_receiver) = tokio::sync::mpsc::channel(16);
    let executor = sandbox_signalling_executor(sandbox_admitted_sender);
    // Two max-cap executions reserve exactly tenant-1's per-tenant output
    // share (2 x 128 MiB = 256 MiB) without denting the node-wide budget.
    let mut sandbox_tenant_one_executions = Vec::new();
    for sandbox_index in 0..2 {
        let mut sandbox_request = sandbox_request();
        sandbox_request.sandbox_command_operation_id =
            format!("tenant-share-operation-{sandbox_index}");
        sandbox_request
            .sandbox_command_limits
            .sandbox_stdout_byte_limit = 67_108_864;
        sandbox_request
            .sandbox_command_limits
            .sandbox_stderr_byte_limit = 67_108_864;
        let sandbox_executor = executor.clone();
        sandbox_tenant_one_executions.push(tokio::spawn(async move {
            sandbox_executor.sandbox_execute(&sandbox_request).await
        }));
    }
    await_sandbox_admissions(&mut sandbox_admitted_receiver, 2).await;

    // Tenant-1's third max-cap admission exceeds its own share only.
    let mut sandbox_tenant_one_third = sandbox_request();
    sandbox_tenant_one_third.sandbox_command_operation_id = "tenant-share-operation-2".to_owned();
    sandbox_tenant_one_third
        .sandbox_command_limits
        .sandbox_stdout_byte_limit = 67_108_864;
    sandbox_tenant_one_third
        .sandbox_command_limits
        .sandbox_stderr_byte_limit = 67_108_864;
    assert_eq!(
        executor
            .sandbox_execute(&sandbox_tenant_one_third)
            .await
            .expect_err("expected an execution error"),
        SandboxCommandExecutionError::PolicyDenied
    );

    // A different tenant runs at the same caps: its own share is untouched
    // by tenant-1's reservations, so the admission succeeds.
    let mut sandbox_tenant_two_first = sandbox_request();
    sandbox_tenant_two_first.sandbox_tenant_id = "tenant-2".to_owned();
    sandbox_tenant_two_first.sandbox_command_operation_id = "tenant-share-operation-0".to_owned();
    sandbox_tenant_two_first
        .sandbox_command_limits
        .sandbox_stdout_byte_limit = 67_108_864;
    sandbox_tenant_two_first
        .sandbox_command_limits
        .sandbox_stderr_byte_limit = 67_108_864;
    let sandbox_tenant_two_executor = executor.clone();
    let sandbox_tenant_two_execution = tokio::spawn(async move {
        sandbox_tenant_two_executor
            .sandbox_execute(&sandbox_tenant_two_first)
            .await
    });
    await_sandbox_admissions(&mut sandbox_admitted_receiver, 1).await;
    executor
        .sandbox_cancel("tenant-2", "provider-local", "tenant-share-operation-0", 7)
        .await
        .expect("settle the tenant-2 reservation");
    let sandbox_tenant_two_outcome = sandbox_tenant_two_execution
        .await
        .expect("join")
        .expect("tenant-2 admission must succeed while tenant-1 is at its share");
    assert_eq!(None, sandbox_tenant_two_outcome.sandbox_exit_code);

    for sandbox_index in 0..2 {
        executor
            .sandbox_cancel(
                "tenant-1",
                "provider-local",
                &format!("tenant-share-operation-{sandbox_index}"),
                7,
            )
            .await
            .expect("settle every tenant-1 reservation");
    }
    for sandbox_execution in sandbox_tenant_one_executions {
        let _ = sandbox_execution.await;
    }
}
