//! The real tokio-based process runner for the Local Provider command slice.
//!
//! This is the first real implementation behind the
//! [`SandboxLocalCommandProcessRunner`] seam. It enforces the declared hard
//! bounds at the process level: executable resolution from provider-owned
//! roots only (never the caller-controlled environment), an empty base
//! environment plus the admitted allowlist, a validated host working directory
//! under the composition-supplied workspace root, hard wall-clock timeout with
//! kill and reap, bounded streamed output capture (no unbounded buffering), a
//! cooperative cancellation signal, and kill-on-drop so an aborted await can
//! never leak a live child.
//!
//! Containment honesty: this slice runs the child under the runner process
//! with `process_group(0)` on Unix and kill-on-drop, which bounds the direct
//! child. It does NOT yet provide the Windows suspended Job Object or Linux
//! delegated cgroup v2 descendant containment the host-boundary contract
//! requires for a Terminal capability claim — the provider descriptor keeps
//! that capability unclaimed until that evidence-gated slice lands
//! (REQ-2026-0003).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    SandboxCommandExecutionError, SandboxCommandLimits, SandboxCommandOutcome,
};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Notify;

use crate::command_executor::{SandboxLocalAdmittedCommand, SandboxLocalCommandProcessRunner};

/// One live execution's cancellation handle published to the executor registry.
#[derive(Clone, Default)]
pub struct SandboxLiveCommandHandle {
    sandbox_cancelled: Arc<Notify>,
}

impl SandboxLiveCommandHandle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation; idempotent and safe after completion.
    pub fn sandbox_cancel(&self) {
        self.sandbox_cancelled.notify_waiters();
    }

    /// Waits for one cancellation notification. Crate-visible so the
    /// executor's tests can block a runner on it.
    pub(crate) async fn sandbox_cancelled(&self) {
        self.sandbox_cancelled.notified().await;
    }
}

/// Runner configuration owned by the composition root.
#[derive(Clone, Debug)]
pub struct SandboxLocalProcessRunnerConfig {
    /// Host directories the runner may resolve a bare executable name from,
    /// in order. The caller-controlled environment never contributes a search
    /// path, so an allowlisted `PATH` entry cannot smuggle a resolution root.
    pub sandbox_executable_roots: Vec<PathBuf>,
    /// Host workspace root every admitted logical working directory resolves
    /// under. Required: a command without a validated workspace root fails
    /// closed instead of inheriting the runner's own working directory.
    pub sandbox_workspace_root: PathBuf,
}

impl SandboxLocalProcessRunnerConfig {
    ///
    /// # Errors
    ///
    /// Returns [`SandboxCommandExecutionError::InvalidRequest`] semantics as a
    /// construction error when the configuration is empty.
    pub fn validate(&self) -> Result<(), SandboxCommandExecutionError> {
        if self.sandbox_executable_roots.is_empty()
            || self.sandbox_workspace_root.as_os_str().is_empty()
        {
            return Err(SandboxCommandExecutionError::InvalidRequest);
        }
        Ok(())
    }
}

/// The real platform runner.
pub struct SandboxLocalTokioProcessRunner {
    sandbox_config: SandboxLocalProcessRunnerConfig,
}

impl SandboxLocalTokioProcessRunner {
    #[must_use]
    pub fn new(sandbox_config: SandboxLocalProcessRunnerConfig) -> Self {
        Self { sandbox_config }
    }

    /// Resolves the bare executable name to one host path under the
    /// provider-owned roots. Resolution is a filesystem lookup per root, so
    /// the request environment cannot influence it.
    fn sandbox_resolve_executable(
        &self,
        sandbox_executable: &str,
    ) -> Result<PathBuf, SandboxCommandExecutionError> {
        if sandbox_executable.is_empty() || sandbox_executable.contains(['/', '\\']) {
            return Err(SandboxCommandExecutionError::InvalidRequest);
        }
        for root in &self.sandbox_config.sandbox_executable_roots {
            for candidate in windows_and_plain_names(sandbox_executable) {
                let resolved = root.join(&candidate);
                if resolved.is_file() {
                    let canonical = resolved
                        .canonicalize()
                        .map_err(|_| SandboxCommandExecutionError::UnsupportedCapability)?;
                    let canonical_root = root
                        .canonicalize()
                        .map_err(|_| SandboxCommandExecutionError::UnsupportedCapability)?;
                    if canonical.starts_with(&canonical_root) {
                        return Ok(canonical);
                    }
                    // A symlink escape from the root is policy-denied, not a
                    // silent fallback to the next root.
                    return Err(SandboxCommandExecutionError::PolicyDenied);
                }
            }
        }
        Err(SandboxCommandExecutionError::InvalidRequest)
    }

    /// Resolves the admitted logical working directory under the workspace
    /// root, re-validating that the canonical path stays inside the root so a
    /// planted symlink cannot pivot the command out.
    fn sandbox_resolve_working_directory(
        &self,
        sandbox_working_directory: &str,
    ) -> Result<PathBuf, SandboxCommandExecutionError> {
        let root = &self.sandbox_config.sandbox_workspace_root;
        let joined = root.join(sandbox_working_directory);
        let canonical = joined
            .canonicalize()
            .map_err(|_| SandboxCommandExecutionError::InvalidRequest)?;
        let canonical_root = root
            .canonicalize()
            .map_err(|_| SandboxCommandExecutionError::UnsupportedCapability)?;
        if canonical.starts_with(&canonical_root) {
            Ok(canonical)
        } else {
            Err(SandboxCommandExecutionError::PolicyDenied)
        }
    }
}

/// Candidate file names for one bare executable: the plain name everywhere and
/// the `.exe`-suffixed name first on Windows.
fn windows_and_plain_names(sandbox_executable: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![
            format!("{sandbox_executable}.exe"),
            sandbox_executable.to_owned(),
        ]
    }
    #[cfg(not(windows))]
    {
        vec![sandbox_executable.to_owned()]
    }
}

/// Reads one child stream into a bounded buffer, draining (and discarding)
/// past the cap so a chatty child can never block on a full pipe and stall the
/// run until the timeout kill. Memory is bounded by `cap + one read chunk`.
async fn sandbox_read_bounded(
    mut stream: impl tokio::io::AsyncRead + Unpin,
    cap: u64,
) -> (Vec<u8>, bool) {
    const READ_CHUNK: usize = 8 * 1024;
    let mut buffer = Vec::new();
    let mut truncated = false;
    let mut chunk = [0u8; READ_CHUNK];
    loop {
        match stream.read(&mut chunk).await {
            Ok(0) => break,
            Ok(read) => {
                let room =
                    usize::try_from(cap.saturating_sub(buffer.len() as u64)).unwrap_or(usize::MAX);
                let take = read.min(room);
                buffer.extend_from_slice(&chunk[..take]);
                if take < read {
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }
    (buffer, truncated)
}

#[async_trait]
impl SandboxLocalCommandProcessRunner for SandboxLocalTokioProcessRunner {
    async fn sandbox_run_admitted(
        &self,
        sandbox_command: &SandboxLocalAdmittedCommand,
    ) -> Result<SandboxCommandOutcome, SandboxCommandExecutionError> {
        self.sandbox_config.validate()?;
        let sandbox_executable_path =
            self.sandbox_resolve_executable(&sandbox_command.sandbox_executable)?;
        let sandbox_working_dir =
            self.sandbox_resolve_working_directory(&sandbox_command.sandbox_working_directory)?;
        let limits: &SandboxCommandLimits = &sandbox_command.sandbox_command_limits;

        let mut sandbox_child_command = Command::new(&sandbox_executable_path);
        sandbox_child_command
            .args(&sandbox_command.sandbox_arguments)
            .current_dir(&sandbox_working_dir)
            .env_clear()
            .envs(sandbox_environment_admitted(
                &sandbox_command.sandbox_environment,
            ))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        sandbox_child_command.process_group(0);

        let mut sandbox_child = sandbox_child_command
            .spawn()
            .map_err(|_| SandboxCommandExecutionError::UnsupportedCapability)?;
        let sandbox_stdout_pipe = sandbox_child
            .stdout
            .take()
            .ok_or(SandboxCommandExecutionError::UnsupportedCapability)?;
        let sandbox_stderr_pipe = sandbox_child
            .stderr
            .take()
            .ok_or(SandboxCommandExecutionError::UnsupportedCapability)?;

        let sandbox_stdout_task = tokio::spawn(sandbox_read_bounded(
            sandbox_stdout_pipe,
            limits.sandbox_stdout_byte_limit,
        ));
        let sandbox_stderr_task = tokio::spawn(sandbox_read_bounded(
            sandbox_stderr_pipe,
            limits.sandbox_stderr_byte_limit,
        ));

        let sandbox_timed =
            tokio::time::timeout(Duration::from_millis(limits.sandbox_timeout_ms), async {
                tokio::select! {
                    sandbox_status = sandbox_child.wait() => {
                        // A signal-terminated child has no exit code, which
                        // the contract reports as a non-terminal exit status.
                        sandbox_status.ok().and_then(|status| status.code())
                    }
                    () = sandbox_command.sandbox_cancellation.sandbox_cancelled() => {
                        let _ = sandbox_child.start_kill();
                        None
                    }
                }
            })
            .await;

        let sandbox_exit_code = match sandbox_timed {
            Ok(sandbox_exit_code) => sandbox_exit_code,
            // Hard timeout: kill and reap within the cleanup bound so the
            // child can never outlive its declared wall-clock bound.
            Err(_) => {
                let _ = sandbox_child.start_kill();
                let _ = tokio::time::timeout(
                    Duration::from_millis(limits.sandbox_cleanup_timeout_ms),
                    sandbox_child.wait(),
                )
                .await;
                None
            }
        };

        let (sandbox_stdout, sandbox_stdout_truncated) =
            sandbox_stdout_task.await.unwrap_or((Vec::new(), false));
        let (sandbox_stderr, sandbox_stderr_truncated) =
            sandbox_stderr_task.await.unwrap_or((Vec::new(), false));

        Ok(SandboxCommandOutcome {
            sandbox_exit_code,
            sandbox_stdout,
            sandbox_stderr,
            sandbox_stdout_truncated,
            sandbox_stderr_truncated,
        })
    }
}

/// The environment actually passed to the child: the admitted allowlist only.
/// The runner never forwards its own process environment.
fn sandbox_environment_admitted(
    sandbox_environment: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    sandbox_environment.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox_runner() -> SandboxLocalTokioProcessRunner {
        let roots = if cfg!(windows) {
            vec![PathBuf::from("C:\\Windows\\System32")]
        } else {
            vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")]
        };
        SandboxLocalTokioProcessRunner::new(SandboxLocalProcessRunnerConfig {
            sandbox_executable_roots: roots,
            sandbox_workspace_root: std::env::temp_dir(),
        })
    }

    fn sandbox_admitted(
        executable: &str,
        arguments: Vec<String>,
        timeout_ms: u64,
    ) -> SandboxLocalAdmittedCommand {
        SandboxLocalAdmittedCommand {
            sandbox_executable: executable.to_owned(),
            sandbox_arguments: arguments,
            sandbox_working_directory: ".".to_owned(),
            sandbox_environment: BTreeMap::new(),
            sandbox_command_limits: SandboxCommandLimits {
                sandbox_timeout_ms: timeout_ms,
                sandbox_stdout_byte_limit: 64 * 1024,
                sandbox_stderr_byte_limit: 64 * 1024,
                sandbox_cleanup_timeout_ms: 1_000,
                sandbox_max_process_count: 1,
            },
            sandbox_cancellation: SandboxLiveCommandHandle::new(),
        }
    }

    fn sandbox_executable_name() -> &'static str {
        if cfg!(windows) {
            "cmd"
        } else {
            "echo"
        }
    }

    #[tokio::test]
    async fn runner_executes_a_real_child_and_captures_bounded_output() {
        let runner = sandbox_runner();
        let arguments: Vec<String> = if cfg!(windows) {
            vec!["/c".to_owned(), "echo".to_owned(), "ok".to_owned()]
        } else {
            vec!["ok".to_owned()]
        };
        let outcome = runner
            .sandbox_run_admitted(&sandbox_admitted(
                sandbox_executable_name(),
                arguments,
                5_000,
            ))
            .await
            .expect("real child must run");
        assert_eq!(Some(0), outcome.sandbox_exit_code);
        assert!(!outcome.sandbox_stdout_truncated);
        assert!(outcome.sandbox_stdout.starts_with(b"ok"));
    }

    #[tokio::test]
    async fn runner_enforces_the_hard_timeout_and_reports_no_exit_code() {
        let runner = sandbox_runner();
        // `timeout.exe` refuses to wait without a console stdin, so the
        // canonical Windows "sleep" in a pipe context is `ping -n`.
        let (executable, arguments) = if cfg!(windows) {
            (
                "ping",
                vec!["-n".to_owned(), "10".to_owned(), "127.0.0.1".to_owned()],
            )
        } else {
            ("sleep", vec!["10".to_owned()])
        };
        let outcome = runner
            .sandbox_run_admitted(&sandbox_admitted(executable, arguments, 500))
            .await
            .expect("timed child must terminate through the kill path");
        assert_eq!(None, outcome.sandbox_exit_code);
    }

    #[tokio::test]
    async fn runner_rejects_a_resolution_escape() {
        let runner = sandbox_runner();
        let outcome = runner
            .sandbox_run_admitted(&sandbox_admitted("../escaped", vec![], 1_000))
            .await;
        assert_eq!(Err(SandboxCommandExecutionError::InvalidRequest), outcome);
    }
}
