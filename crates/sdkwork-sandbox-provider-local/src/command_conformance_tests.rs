//! The Local Provider executor driven through the shared REQ-2026-0007
//! conformance matrix (`crates/sdkwork-sandbox-provider-spi/src/command_conformance.rs`).
//!
//! The fixture is the provider's own admission surface: platform-available
//! echo-style and long-running executables, the workspace root from the
//! runner tests, and the denial shapes the host boundary owes. The scenario
//! statuses are pinned so any enforcement change — a regression, or a new
//! slice landing (platform supervision, durable arbitration, composition
//! quarantine) — must move this file in the same commit.

use std::collections::BTreeMap;

use crate::command_executor::SandboxLocalCommandExecutor;
use crate::host_boundary::SandboxLocalHostBoundary;
use crate::process_runner::{SandboxLocalProcessRunnerConfig, SandboxLocalTokioProcessRunner};
use sdkwork_sandbox_provider_spi::{
    sandbox_run_command_conformance, SandboxCommandConformanceFixture,
    SandboxCommandConformanceStatus, SandboxCommandExecutionRequest, SandboxCommandLimits,
};

fn sandbox_workspace_root() -> std::path::PathBuf {
    std::env::temp_dir()
}

fn sandbox_executable_roots() -> Vec<std::path::PathBuf> {
    if cfg!(windows) {
        vec![std::path::PathBuf::from("C:\\Windows\\System32")]
    } else {
        vec![
            std::path::PathBuf::from("/usr/bin"),
            std::path::PathBuf::from("/bin"),
        ]
    }
}

fn sandbox_echo_name() -> &'static str {
    if cfg!(windows) {
        "cmd"
    } else {
        "echo"
    }
}

fn sandbox_echo_arguments() -> Vec<String> {
    if cfg!(windows) {
        vec![
            "/c".to_owned(),
            "echo".to_owned(),
            "alpha beta".to_owned(),
            "gamma/delta".to_owned(),
        ]
    } else {
        vec!["alpha beta".to_owned(), "gamma/delta".to_owned()]
    }
}

fn sandbox_slow_executable() -> &'static str {
    if cfg!(windows) {
        "ping"
    } else {
        "sleep"
    }
}

fn sandbox_slow_arguments() -> Vec<String> {
    if cfg!(windows) {
        vec!["-n".to_owned(), "30".to_owned(), "127.0.0.1".to_owned()]
    } else {
        vec!["30".to_owned()]
    }
}

fn sandbox_fixture() -> SandboxCommandConformanceFixture {
    SandboxCommandConformanceFixture {
        sandbox_base_request: SandboxCommandExecutionRequest {
            sandbox_tenant_id: "tenant-conformance".to_owned(),
            sandbox_provider_id: "provider-local-conformance".to_owned(),
            sandbox_workspace_id: "workspace-conformance".to_owned(),
            sandbox_session_id: "session-conformance".to_owned(),
            sandbox_id: "sandbox-conformance".to_owned(),
            sandbox_runtime_binding_id: "binding-conformance".to_owned(),
            sandbox_fencing_token: 11,
            sandbox_command_operation_id: "sandbox-conformance-base-operation".to_owned(),
            sandbox_executable: sandbox_echo_name().to_owned(),
            sandbox_arguments: sandbox_echo_arguments(),
            sandbox_working_directory: ".".to_owned(),
            sandbox_environment: BTreeMap::new(),
            sandbox_command_limits: SandboxCommandLimits {
                sandbox_timeout_ms: 30_000,
                sandbox_stdout_byte_limit: 64 * 1024,
                sandbox_stderr_byte_limit: 64 * 1024,
                sandbox_cleanup_timeout_ms: 2_000,
                sandbox_max_process_count: 8,
            },
        },
        sandbox_echo_arguments: sandbox_echo_arguments(),
        sandbox_echo_output_fragments: vec!["alpha beta".to_owned(), "gamma/delta".to_owned()],
        sandbox_slow_executable: sandbox_slow_executable().to_owned(),
        sandbox_slow_arguments: sandbox_slow_arguments(),
        sandbox_shell_string_executables: vec![
            "cmd /c echo".to_owned(),
            "sh -c 'id'".to_owned(),
            "../escaped".to_owned(),
        ],
        sandbox_escape_working_directories: vec![
            "/tmp".to_owned(),
            "..\\escape".to_owned(),
            "C:\\Windows".to_owned(),
            "../escape".to_owned(),
        ],
        sandbox_denied_environment_names: vec![
            "NOT_ON_THE_ALLOWLIST".to_owned(),
            "CONFORMANCE_TOKEN".to_owned(),
        ],
        sandbox_slow_timeout_ms: 30_000,
    }
}

fn sandbox_executor() -> SandboxLocalCommandExecutor {
    let sandbox_boundary = SandboxLocalHostBoundary::new(
        ["cmd", "echo", "ping", "sleep"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        [].into_iter().map(str::to_owned).collect(),
    );
    let sandbox_runner = SandboxLocalTokioProcessRunner::new(SandboxLocalProcessRunnerConfig {
        sandbox_executable_roots: sandbox_executable_roots(),
        sandbox_workspace_root: sandbox_workspace_root(),
    });
    SandboxLocalCommandExecutor::new(
        std::sync::Arc::new(sandbox_boundary),
        std::sync::Arc::new(sandbox_runner),
    )
}

/// The Local executor's enforced scenario set. `PartiallyEnforced` entries
/// name the pending slice that owns the remainder; `Pending` entries are
/// wholly owned by a pending slice. A status that drifts from this pin — in
/// either direction — must be re-landed deliberately.
const SANDBOX_LOCAL_PENDING_SCENARIOS: [&str; 6] = [
    "timeout-and-descendant-cleanup",
    "cancellation-and-descendant-cleanup",
    "same-operation-same-fingerprint-replay",
    "in-progress-operation-does-not-spawn-duplicate",
    "terminal-race-single-winner-and-replay",
    "cleanup-failure-visible-and-binding-quarantined",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn local_executor_holds_the_shared_command_conformance_matrix() {
    let sandbox_executor = sandbox_executor();
    let sandbox_fixture = sandbox_fixture();
    let sandbox_report = sandbox_run_command_conformance(&sandbox_executor, &sandbox_fixture).await;

    assert!(
        sandbox_report.sandbox_held(),
        "the shared conformance matrix must hold on the Local executor: {:?}",
        sandbox_report.sandbox_findings()
    );
    for sandbox_finding in sandbox_report.sandbox_findings() {
        assert_ne!(
            SandboxCommandConformanceStatus::Failed,
            sandbox_finding.sandbox_status,
            "scenario {} failed: {}",
            sandbox_finding.sandbox_scenario_id,
            sandbox_finding.sandbox_detail
        );
    }
    for sandbox_scenario_id in SANDBOX_LOCAL_PENDING_SCENARIOS {
        let sandbox_finding = sandbox_report
            .sandbox_finding(sandbox_scenario_id)
            .unwrap_or_else(|| panic!("scenario {sandbox_scenario_id} must be reported"));
        assert!(
            matches!(
                sandbox_finding.sandbox_status,
                SandboxCommandConformanceStatus::PartiallyEnforced(_)
                    | SandboxCommandConformanceStatus::Pending(_)
            ),
            "scenario {sandbox_scenario_id} is pinned to a pending slice, got {:?}: {}",
            sandbox_finding.sandbox_status,
            sandbox_finding.sandbox_detail
        );
    }
}
