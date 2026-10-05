//! Real-process end-to-end test: the start command runs as a real host
//! process through the boundary allowlist, the tokio runner, the executor,
//! the runtime bridge and the port — reaching `Started` on a zero exit.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use sdkwork_intelligence_sandbox_worker_local::SandboxLaunchStartCommandPort;
use sdkwork_intelligence_sandbox_worker_local_exec::{
    SandboxLocalStartCommandExecutor, SandboxStartCommandScope,
};
use sdkwork_sandbox_provider_local::command_executor::SandboxLocalCommandExecutor;
use sdkwork_sandbox_provider_local::host_boundary::SandboxLocalHostBoundary;
use sdkwork_sandbox_provider_local::process_runner::{
    SandboxLocalProcessRunnerConfig, SandboxLocalTokioProcessRunner,
};
use sdkwork_sandbox_provider_spi::SandboxCommandLimits;

fn sandbox_executable_roots() -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![PathBuf::from("C:\\Windows\\System32")]
    } else {
        vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")]
    }
}

fn sandbox_echo_name() -> &'static str {
    if cfg!(windows) {
        // `echo` is a cmd builtin on Windows; the fixture resolves `cmd`
        // from System32 and drives the builtin through /c.
        "cmd"
    } else {
        "echo"
    }
}

fn sandbox_echo_arguments() -> Vec<String> {
    if cfg!(windows) {
        vec!["/C".to_owned(), "echo".to_owned(), "fast-start".to_owned()]
    } else {
        vec!["fast-start".to_owned()]
    }
}

fn sandbox_limits() -> SandboxCommandLimits {
    SandboxCommandLimits {
        sandbox_timeout_ms: 10_000,
        sandbox_stdout_byte_limit: 64 * 1024,
        sandbox_stderr_byte_limit: 64 * 1024,
        sandbox_cleanup_timeout_ms: 2_000,
        sandbox_max_process_count: 8,
    }
}

fn sandbox_scope(executable: &str) -> SandboxStartCommandScope {
    SandboxStartCommandScope {
        sandbox_tenant_id: "tenant-1".to_owned(),
        sandbox_provider_id: "provider-local".to_owned(),
        sandbox_workspace_id: "workspace-1".to_owned(),
        sandbox_session_id: "session-1".to_owned(),
        sandbox_id: "sandbox-1".to_owned(),
        sandbox_runtime_binding_id: "binding-1".to_owned(),
        sandbox_fencing_token: 1,
        sandbox_executable: executable.to_owned(),
        sandbox_arguments: sandbox_echo_arguments(),
        sandbox_working_directory: ".".to_owned(),
        sandbox_environment: BTreeMap::new(),
        sandbox_command_limits: sandbox_limits(),
    }
}

fn sandbox_executor() -> Arc<SandboxLocalCommandExecutor> {
    Arc::new(SandboxLocalCommandExecutor::new(
        Arc::new(SandboxLocalHostBoundary::new(
            ["cmd", "echo"].into_iter().map(str::to_owned).collect(),
            [].into_iter().map(str::to_owned).collect(),
        )),
        Arc::new(SandboxLocalTokioProcessRunner::new(
            SandboxLocalProcessRunnerConfig {
                sandbox_executable_roots: sandbox_executable_roots(),
                sandbox_workspace_root: std::env::temp_dir(),
            },
        )),
    ))
}

#[test]
fn a_real_start_command_process_reaches_started_through_the_port() {
    let wiring = SandboxLocalStartCommandExecutor::sandbox_new(
        sandbox_executor(),
        sandbox_scope(sandbox_echo_name()),
    )
    .expect("bridge runtime builds");
    assert_eq!(
        wiring.sandbox_start("plan-1"),
        sdkwork_intelligence_sandbox_worker_local::SandboxStartCommandOutcome::Started,
        "a real echo process must carry the plan to started",
    );
}

#[test]
fn a_boundary_refusal_lands_uncertain_never_started() {
    let wiring = SandboxLocalStartCommandExecutor::sandbox_new(
        sandbox_executor(),
        sandbox_scope("not-in-allowlist"),
    )
    .expect("bridge runtime builds");
    assert_eq!(
        wiring.sandbox_start("plan-1"),
        sdkwork_intelligence_sandbox_worker_local::SandboxStartCommandOutcome::Uncertain,
        "an executor error without a terminal outcome is uncertainty",
    );
}
