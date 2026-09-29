//! The Firecracker executor driven through the shared REQ-2026-0007
//! conformance matrix (`crates/sdkwork-sandbox-provider-spi/src/command_conformance.rs`).
//!
//! The fixture is the provider's own admission surface: allowlisted
//! echo-style guest executables over the fake guest channel, and the denial
//! shapes the guest boundary owes. The scenario statuses are pinned so any
//! enforcement change - a regression, or a new slice landing (platform
//! supervision, durable arbitration, composition quarantine) - must move this
//! file in the same commit.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::command_executor::SandboxFirecrackerCommandExecutor;
use crate::fake_host::SandboxFakeGuestChannel;
use crate::guest_boundary::SandboxFirecrackerGuestBoundary;
use sdkwork_sandbox_provider_spi::{
    sandbox_run_command_conformance, SandboxCommandConformanceFixture,
    SandboxCommandConformanceStatus, SandboxCommandExecutionRequest, SandboxCommandLimits,
};

fn sandbox_fixture() -> SandboxCommandConformanceFixture {
    SandboxCommandConformanceFixture {
        sandbox_base_request: SandboxCommandExecutionRequest {
            sandbox_tenant_id: "tenant-conformance".to_owned(),
            sandbox_provider_id: "provider-firecracker-conformance".to_owned(),
            sandbox_workspace_id: "workspace-conformance".to_owned(),
            sandbox_session_id: "session-conformance".to_owned(),
            sandbox_id: "sandbox-conformance".to_owned(),
            sandbox_runtime_binding_id: "binding-conformance".to_owned(),
            sandbox_fencing_token: 11,
            sandbox_command_operation_id: "sandbox-conformance-base-operation".to_owned(),
            sandbox_executable: "toybox".to_owned(),
            sandbox_arguments: vec!["alpha beta".to_owned(), "gamma/delta".to_owned()],
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
        sandbox_echo_arguments: vec!["alpha beta".to_owned(), "gamma/delta".to_owned()],
        sandbox_echo_output_fragments: vec!["alpha beta".to_owned(), "gamma/delta".to_owned()],
        sandbox_slow_executable: "guestsleep".to_owned(),
        sandbox_slow_arguments: vec!["30".to_owned()],
        sandbox_shell_string_executables: vec![
            "sh -c 'id'".to_owned(),
            "cmd /c echo".to_owned(),
            "../escaped".to_owned(),
        ],
        sandbox_escape_working_directories: vec![
            "/tmp".to_owned(),
            "../escape".to_owned(),
            "..\\escape".to_owned(),
            "C:\\Windows".to_owned(),
        ],
        sandbox_denied_environment_names: vec![
            "NOT_ON_THE_ALLOWLIST".to_owned(),
            "CONFORMANCE_TOKEN".to_owned(),
        ],
        sandbox_slow_timeout_ms: 30_000,
    }
}

fn sandbox_executor() -> SandboxFirecrackerCommandExecutor {
    let sandbox_boundary = SandboxFirecrackerGuestBoundary::new(
        ["toybox", "guestsleep"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        [].into_iter().map(str::to_owned).collect(),
    );
    let sandbox_channel = SandboxFakeGuestChannel::new("guestsleep");
    SandboxFirecrackerCommandExecutor::new(Arc::new(sandbox_boundary), Arc::new(sandbox_channel))
}

/// The Firecracker executor's enforced scenario set. `PartiallyEnforced`
/// entries name the pending slice that owns the remainder; `Pending` entries
/// are wholly owned by a pending slice. A status that drifts from this pin -
/// in either direction - must be re-landed deliberately.
const SANDBOX_FIRECRACKER_PENDING_SCENARIOS: [&str; 6] = [
    "timeout-and-descendant-cleanup",
    "cancellation-and-descendant-cleanup",
    "same-operation-same-fingerprint-replay",
    "in-progress-operation-does-not-spawn-duplicate",
    "terminal-race-single-winner-and-replay",
    "cleanup-failure-visible-and-binding-quarantined",
];

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn firecracker_executor_holds_the_shared_command_conformance_matrix() {
    let sandbox_executor = sandbox_executor();
    let sandbox_fixture = sandbox_fixture();
    let sandbox_report = sandbox_run_command_conformance(&sandbox_executor, &sandbox_fixture).await;

    assert!(
        sandbox_report.sandbox_held(),
        "the shared conformance matrix must hold on the Firecracker executor: {:?}",
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
    for sandbox_scenario_id in SANDBOX_FIRECRACKER_PENDING_SCENARIOS {
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
