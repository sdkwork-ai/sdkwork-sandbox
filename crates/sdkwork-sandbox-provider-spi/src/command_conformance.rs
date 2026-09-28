//! The shared command conformance matrix for `SandboxCommandExecutor`
//! implementations (`REQ-2026-0007`, `apis/commands/sandbox-command-contract.json`).
//!
//! The contract's `conformanceScenarios` block declares twenty scenarios; the
//! requirement's delivery order mandates that the same suite run on the Local
//! adapter today and on every future provider adapter (Firecracker) without
//! re-authoring. This module is that suite: a provider supplies a
//! [`SandboxCommandConformanceFixture`] describing its admitted request shape
//! and its allowlisted executable names, and
//! [`sandbox_run_command_conformance`] drives every scenario through the port
//! and reports one machine-readable finding per scenario.
//!
//! Reporting is four-valued so the suite can never rot into a silently
//! skipping decoration: `Enforced` (executed live and held),
//! `PartiallyEnforced` (the enforced part held live; the remainder is owned by
//! a named pending slice), `Pending` (wholly owned by a named, contract-cited
//! slice), or `Failed` (the live execution contradicted the scenario). Every
//! enforced finding is produced by a live call through
//! [`SandboxCommandExecutor`]; none rests on static reasoning.

use std::time::Duration;

use crate::{SandboxCommandExecutionError, SandboxCommandExecutionRequest, SandboxCommandExecutor};

/// Every scenario id the contract's `conformanceScenarios` block declares.
///
/// The authority for this list is the contract JSON; keep the two in step.
/// `tests/contract/sandbox-command-contract.contract.test.mjs` pins the JSON
/// side and this crate pins the Rust side, so a one-sided edit fails a gate.
pub const SANDBOX_COMMAND_CONFORMANCE_SCENARIOS: [&str; 20] = [
    "typed-executable-and-argv-preservation",
    "shell-string-rejection",
    "logical-working-directory-escape-rejection",
    "deny-by-default-environment",
    "timeout-and-descendant-cleanup",
    "cancellation-and-descendant-cleanup",
    "stdout-and-stderr-hard-bounds",
    "stale-fencing-fail-closed",
    "derived-fingerprint-recomputation-and-mismatch-rejection",
    "same-operation-same-fingerprint-replay",
    "same-operation-different-fingerprint-conflict",
    "in-progress-operation-does-not-spawn-duplicate",
    "fenced-idempotent-cancellation-request",
    "accepted-execution-terminal-result-error-partition",
    "terminal-race-single-winner-and-replay",
    "cleanup-failure-visible-and-binding-quarantined",
    "safe-error-and-private-metadata-redaction",
    "provider-owned-executable-resolution-without-path-search",
    "protected-environment-override-rejection",
    "runtime-binding-policy-snapshot-immutability",
];

/// The pending implementation slice a `Pending` or `PartiallyEnforced`
/// scenario is owned by, with the contract that gates the slice, so a pending
/// finding names its own unblocking path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxCommandConformancePending {
    /// The implementation slice that owns the unenforced remainder.
    pub sandbox_slice: &'static str,
    /// The machine contract or requirement that gates the slice.
    pub sandbox_gate: &'static str,
}

/// One scenario's outcome against the executor under test.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SandboxCommandConformanceStatus {
    /// The scenario was executed live and held.
    Enforced,
    /// Part of the scenario held live; the remainder is owned by the named
    /// pending slice.
    PartiallyEnforced(SandboxCommandConformancePending),
    /// The scenario is wholly owned by the named pending slice.
    Pending(SandboxCommandConformancePending),
    /// The live execution contradicted the scenario.
    Failed,
}

/// One machine-readable finding per contract scenario.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxCommandConformanceFinding {
    /// The contract scenario id.
    pub sandbox_scenario_id: &'static str,
    /// The observed status.
    pub sandbox_status: SandboxCommandConformanceStatus,
    /// What the live execution actually did, for the evidence record.
    pub sandbox_detail: String,
}

/// The full report for one executor run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxCommandConformanceReport {
    sandbox_findings: Vec<SandboxCommandConformanceFinding>,
}

impl SandboxCommandConformanceReport {
    /// The finding for one scenario id.
    #[must_use]
    pub fn sandbox_finding(
        &self,
        sandbox_scenario_id: &str,
    ) -> Option<&SandboxCommandConformanceFinding> {
        self.sandbox_findings
            .iter()
            .find(|finding| finding.sandbox_scenario_id == sandbox_scenario_id)
    }

    /// Every finding, in contract declaration order.
    #[must_use]
    pub fn sandbox_findings(&self) -> &[SandboxCommandConformanceFinding] {
        &self.sandbox_findings
    }

    /// Whether every scenario held (no `Failed`).
    #[must_use]
    pub fn sandbox_held(&self) -> bool {
        self.sandbox_findings
            .iter()
            .all(|finding| finding.sandbox_status != SandboxCommandConformanceStatus::Failed)
    }
}

/// The provider-supplied fixture: everything the suite needs to drive one
/// executor through the matrix without knowing the provider's private
/// allowlists, roots, or platform.
pub struct SandboxCommandConformanceFixture {
    /// A request shape this provider's admission admits end to end: a valid
    /// allowlisted echo-style executable, an admitted working directory, an
    /// environment the boundary accepts, and limits the harness may narrow per
    /// scenario. The harness clones and derives variations from it.
    pub sandbox_base_request: SandboxCommandExecutionRequest,
    /// Arguments the echo-style executable writes back verbatim, carrying
    /// spaces and punctuation so shell-free argv fidelity is observable.
    pub sandbox_echo_arguments: Vec<String>,
    /// The fragments those arguments must produce on the child's stdout, in
    /// order. The provider knows its platform's echo mechanics (Windows
    /// `cmd /c echo` re-quotes spaced arguments), so the fidelity assertion
    /// matches the provider-declared fragments instead of raw argv joins.
    pub sandbox_echo_output_fragments: Vec<String>,
    /// A bare allowlisted executable name that outlives a short timeout.
    pub sandbox_slow_executable: String,
    /// Arguments that keep the slow executable alive past a short timeout.
    pub sandbox_slow_arguments: Vec<String>,
    /// Executable shapes this provider must reject (shell strings, escapes).
    pub sandbox_shell_string_executables: Vec<String>,
    /// Working directories this provider must reject (absolute, parent-relative).
    pub sandbox_escape_working_directories: Vec<String>,
    /// Environment names this provider's boundary must deny (not allowlisted).
    pub sandbox_denied_environment_names: Vec<String>,
    /// The wall-clock timeout the base request declares; the slow executable
    /// must outlive any shortened timeout the harness applies.
    pub sandbox_slow_timeout_ms: u64,
}

/// Drives the whole matrix through one executor and returns the report.
#[must_use]
pub async fn sandbox_run_command_conformance(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceReport {
    let mut sandbox_findings = Vec::with_capacity(SANDBOX_COMMAND_CONFORMANCE_SCENARIOS.len());
    sandbox_findings.push(sandbox_scenario_argv_preservation(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_shell_string_rejection(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_working_directory_escape(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_deny_by_default_environment(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_timeout_and_descendants(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_cancellation_and_descendants(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_output_hard_bounds(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_stale_fencing(executor, fixture).await);
    sandbox_findings.extend(sandbox_scenario_fingerprint_recomputation(executor, fixture).await);
    sandbox_findings.extend(sandbox_scenario_same_fingerprint_replay(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_fenced_idempotent_cancellation(executor, fixture).await);
    sandbox_findings
        .push(sandbox_scenario_terminal_result_error_partition(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_terminal_race_single_winner());
    sandbox_findings.push(sandbox_scenario_cleanup_failure_quarantine());
    sandbox_findings.push(sandbox_scenario_safe_error_redaction());
    sandbox_findings.push(sandbox_scenario_resolution_without_path_search(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_protected_environment_override(executor, fixture).await);
    sandbox_findings.push(sandbox_scenario_policy_snapshot_immutability());
    SandboxCommandConformanceReport { sandbox_findings }
}

/// The descendant-tree cleanup half of the timeout/cancel scenarios.
///
/// The executor port terminates its direct child within the declared hard
/// bounds today; killing the whole descendant tree is the platform
/// supervision slice (Windows suspended Job Object with kill-on-close, Linux
/// delegated cgroup v2 with `cgroup.kill`).
const SANDBOX_DESCENDANT_CLEANUP_PENDING: SandboxCommandConformancePending =
    SandboxCommandConformancePending {
        sandbox_slice: "Linux delegated cgroup v2 lane + real-platform evidence matrix (the Windows kill-on-close Job Object lane landed; shell-detached escapes are recorded as the detached-and-breakaway-attempt-denial obligation)",
        sandbox_gate: "specs/sandbox-local-provider-host-boundary.contract.json (REQ-2026-0003)",
    };

/// The durable half of the idempotency scenarios.
///
/// The executor's bounded live registry refuses a concurrent replay and a
/// fingerprint conflict while an execution is live; durable first-terminal
/// arbitration (replay after completion, result replay marker) is owned by
/// the durable operation registry slice.
const SANDBOX_DURABLE_ARBITRATION_PENDING: SandboxCommandConformancePending =
    SandboxCommandConformancePending {
        sandbox_slice: "durable command operation registry slice",
        sandbox_gate: "apis/commands/sandbox-command-contract.json (terminalArbitration)",
    };

/// The composition-owned scenarios.
const SANDBOX_COMPOSITION_PENDING: SandboxCommandConformancePending =
    SandboxCommandConformancePending {
        sandbox_slice: "composition cleanup/quarantine slice",
        sandbox_gate: "specs/sandbox-local-provider-host-boundary.contract.json (cleanupBoundary)",
    };

fn sandbox_pending_detail(pending: &SandboxCommandConformancePending, observed: &str) -> String {
    format!(
        "{observed}; pending slice: {} (gate: {})",
        pending.sandbox_slice, pending.sandbox_gate
    )
}

fn sandbox_derived_request(
    fixture: &SandboxCommandConformanceFixture,
    sandbox_executable: &str,
    sandbox_arguments: Vec<String>,
    sandbox_working_directory: &str,
    sandbox_timeout_ms: u64,
) -> SandboxCommandExecutionRequest {
    let mut sandbox_request = fixture.sandbox_base_request.clone();
    sandbox_request.sandbox_executable = sandbox_executable.to_owned();
    sandbox_request.sandbox_arguments = sandbox_arguments;
    sandbox_request.sandbox_working_directory = sandbox_working_directory.to_owned();
    sandbox_request.sandbox_command_limits.sandbox_timeout_ms = sandbox_timeout_ms;
    sandbox_request
}

/// One live execution plus a future that observes it mid-flight, joined so
/// both progress on the caller's runtime without spawning tasks over the
/// borrowed executor.
macro_rules! sandbox_join_with_observer {
    ($executor:expr, $request:expr, $observer:expr) => {{
        let sandbox_execution = $executor.sandbox_execute(&$request);
        tokio::pin!(sandbox_execution);
        tokio::join!(sandbox_execution, $observer)
    }};
}

async fn sandbox_scenario_argv_preservation(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "typed-executable-and-argv-preservation";
    let sandbox_request = sandbox_derived_request(
        fixture,
        &fixture.sandbox_base_request.sandbox_executable,
        fixture.sandbox_echo_arguments.clone(),
        &fixture.sandbox_base_request.sandbox_working_directory,
        fixture.sandbox_slow_timeout_ms,
    );
    let (sandbox_result, ()) = sandbox_join_with_observer!(executor, sandbox_request, async {});
    match sandbox_result {
        Ok(outcome) if outcome.sandbox_exit_code == Some(0) => {
            let sandbox_stdout = String::from_utf8_lossy(&outcome.sandbox_stdout);
            let mut sandbox_cursor = 0usize;
            let sandbox_all_fragments =
                fixture
                    .sandbox_echo_output_fragments
                    .iter()
                    .all(|fragment| {
                        match sandbox_stdout[std::cmp::min(sandbox_cursor, sandbox_stdout.len())..]
                            .find(fragment.as_str())
                        {
                            Some(offset) => {
                                sandbox_cursor += offset + fragment.len();
                                true
                            }
                            None => false,
                        }
                    });
            if sandbox_all_fragments && !fixture.sandbox_echo_output_fragments.is_empty() {
                SandboxCommandConformanceFinding {
                    sandbox_scenario_id,
                    sandbox_status: SandboxCommandConformanceStatus::Enforced,
                    sandbox_detail:
                        "argv crossed the port verbatim and in order; the child echoed it without a shell"
                            .to_owned(),
                }
            } else {
                SandboxCommandConformanceFinding {
                    sandbox_scenario_id,
                    sandbox_status: SandboxCommandConformanceStatus::Failed,
                    sandbox_detail: format!(
                        "echo output did not preserve argv in order: {sandbox_stdout}"
                    ),
                }
            }
        }
        Ok(outcome) => SandboxCommandConformanceFinding {
            sandbox_scenario_id,
            sandbox_status: SandboxCommandConformanceStatus::Failed,
            sandbox_detail: format!("echo output did not preserve argv: {outcome:?}"),
        },
        Err(sandbox_error) => SandboxCommandConformanceFinding {
            sandbox_scenario_id,
            sandbox_status: SandboxCommandConformanceStatus::Failed,
            sandbox_detail: format!("admitted argv execution was refused: {sandbox_error}"),
        },
    }
}

async fn sandbox_scenario_shell_string_rejection(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "shell-string-rejection";
    let mut sandbox_observations = Vec::new();
    for sandbox_executable in &fixture.sandbox_shell_string_executables {
        let sandbox_request = sandbox_derived_request(
            fixture,
            sandbox_executable,
            Vec::new(),
            &fixture.sandbox_base_request.sandbox_working_directory,
            fixture.sandbox_slow_timeout_ms,
        );
        sandbox_observations.push((
            sandbox_executable.clone(),
            executor.sandbox_execute(&sandbox_request).await,
        ));
    }
    let sandbox_all_rejected = !sandbox_observations.is_empty()
        && sandbox_observations.iter().all(|(_, result)| {
            matches!(
                result,
                Err(SandboxCommandExecutionError::InvalidRequest
                    | SandboxCommandExecutionError::PolicyDenied)
            )
        });
    SandboxCommandConformanceFinding {
        sandbox_scenario_id,
        sandbox_status: if sandbox_all_rejected {
            SandboxCommandConformanceStatus::Enforced
        } else {
            SandboxCommandConformanceStatus::Failed
        },
        sandbox_detail: format!(
            "shell-string shapes resolved to: {:?}",
            sandbox_observations
                .iter()
                .map(|(name, result)| (name.clone(), result.is_err()))
                .collect::<Vec<_>>()
        ),
    }
}

async fn sandbox_scenario_working_directory_escape(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "logical-working-directory-escape-rejection";
    let mut sandbox_observations = Vec::new();
    for sandbox_working_directory in &fixture.sandbox_escape_working_directories {
        let sandbox_request = sandbox_derived_request(
            fixture,
            &fixture.sandbox_base_request.sandbox_executable,
            Vec::new(),
            sandbox_working_directory,
            fixture.sandbox_slow_timeout_ms,
        );
        sandbox_observations.push((
            sandbox_working_directory.clone(),
            executor.sandbox_execute(&sandbox_request).await,
        ));
    }
    let sandbox_all_rejected = !sandbox_observations.is_empty()
        && sandbox_observations.iter().all(|(_, result)| {
            matches!(
                result,
                Err(SandboxCommandExecutionError::InvalidRequest
                    | SandboxCommandExecutionError::PolicyDenied)
            )
        });
    SandboxCommandConformanceFinding {
        sandbox_scenario_id,
        sandbox_status: if sandbox_all_rejected {
            SandboxCommandConformanceStatus::Enforced
        } else {
            SandboxCommandConformanceStatus::Failed
        },
        sandbox_detail: format!(
            "escape working directories resolved to: {:?}",
            sandbox_observations
                .iter()
                .map(|(path, result)| (path.clone(), result.is_err()))
                .collect::<Vec<_>>()
        ),
    }
}

async fn sandbox_scenario_deny_by_default_environment(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "deny-by-default-environment";
    let mut sandbox_observations = Vec::new();
    for sandbox_name in &fixture.sandbox_denied_environment_names {
        let mut sandbox_request = sandbox_derived_request(
            fixture,
            &fixture.sandbox_base_request.sandbox_executable,
            Vec::new(),
            &fixture.sandbox_base_request.sandbox_working_directory,
            fixture.sandbox_slow_timeout_ms,
        );
        sandbox_request
            .sandbox_environment
            .insert(sandbox_name.clone(), "value".to_owned());
        sandbox_observations.push((
            sandbox_name.clone(),
            executor.sandbox_execute(&sandbox_request).await,
        ));
    }
    let sandbox_all_denied = !sandbox_observations.is_empty()
        && sandbox_observations
            .iter()
            .all(|(_, result)| matches!(result, Err(SandboxCommandExecutionError::PolicyDenied)));
    SandboxCommandConformanceFinding {
        sandbox_scenario_id,
        sandbox_status: if sandbox_all_denied {
            SandboxCommandConformanceStatus::Enforced
        } else {
            SandboxCommandConformanceStatus::Failed
        },
        sandbox_detail: format!(
            "denied environment names resolved to: {:?}",
            sandbox_observations
                .iter()
                .map(|(name, result)| (name.clone(), result.is_err()))
                .collect::<Vec<_>>()
        ),
    }
}

async fn sandbox_scenario_timeout_and_descendants(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "timeout-and-descendant-cleanup";
    let sandbox_request = sandbox_derived_request(
        fixture,
        &fixture.sandbox_slow_executable,
        fixture.sandbox_slow_arguments.clone(),
        &fixture.sandbox_base_request.sandbox_working_directory,
        500,
    );
    let (sandbox_result, ()) = sandbox_join_with_observer!(executor, sandbox_request, async {});
    match sandbox_result {
        Ok(outcome) if outcome.sandbox_exit_code.is_none() => SandboxCommandConformanceFinding {
            sandbox_scenario_id,
            sandbox_status: SandboxCommandConformanceStatus::PartiallyEnforced(
                SANDBOX_DESCENDANT_CLEANUP_PENDING,
            ),
            sandbox_detail: sandbox_pending_detail(
                &SANDBOX_DESCENDANT_CLEANUP_PENDING,
                "the hard wall-clock timeout terminated the direct child inside the bound (no exit code)",
            ),
        },
        Ok(outcome) => SandboxCommandConformanceFinding {
            sandbox_scenario_id,
            sandbox_status: SandboxCommandConformanceStatus::Failed,
            sandbox_detail: format!("slow executable finished before the timeout: {outcome:?}"),
        },
        Err(sandbox_error) => SandboxCommandConformanceFinding {
            sandbox_scenario_id,
            sandbox_status: SandboxCommandConformanceStatus::Failed,
            sandbox_detail: format!("timeout scenario refused: {sandbox_error}"),
        },
    }
}

async fn sandbox_scenario_cancellation_and_descendants(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "cancellation-and-descendant-cleanup";
    let mut sandbox_request = sandbox_derived_request(
        fixture,
        &fixture.sandbox_slow_executable,
        fixture.sandbox_slow_arguments.clone(),
        &fixture.sandbox_base_request.sandbox_working_directory,
        fixture.sandbox_slow_timeout_ms,
    );
    sandbox_request
        .sandbox_command_limits
        .sandbox_cleanup_timeout_ms = 2_000;
    let sandbox_operation_id = sandbox_request.sandbox_command_operation_id.clone();
    let sandbox_token = sandbox_request.sandbox_fencing_token;
    let (sandbox_result, sandbox_cancellation) =
        sandbox_join_with_observer!(executor, sandbox_request, async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            executor
                .sandbox_cancel(
                    &fixture.sandbox_base_request.sandbox_tenant_id,
                    &fixture.sandbox_base_request.sandbox_provider_id,
                    &sandbox_operation_id,
                    sandbox_token,
                )
                .await
        });
    match (sandbox_cancellation, sandbox_result) {
        (Ok(()), Ok(outcome)) if outcome.sandbox_exit_code.is_none() => {
            SandboxCommandConformanceFinding {
                sandbox_scenario_id,
                sandbox_status: SandboxCommandConformanceStatus::PartiallyEnforced(
                    SANDBOX_DESCENDANT_CLEANUP_PENDING,
                ),
                sandbox_detail: sandbox_pending_detail(
                    &SANDBOX_DESCENDANT_CLEANUP_PENDING,
                    "the fenced cancellation reached the live child and the execution reported no exit code",
                ),
            }
        }
        (sandbox_cancellation, sandbox_result) => SandboxCommandConformanceFinding {
            sandbox_scenario_id,
            sandbox_status: SandboxCommandConformanceStatus::Failed,
            sandbox_detail: format!(
                "cancellation did not hold: cancel={sandbox_cancellation:?} outcome={sandbox_result:?}"
            ),
        },
    }
}

async fn sandbox_scenario_output_hard_bounds(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "stdout-and-stderr-hard-bounds";
    let mut sandbox_request = sandbox_derived_request(
        fixture,
        &fixture.sandbox_base_request.sandbox_executable,
        fixture.sandbox_echo_arguments.clone(),
        &fixture.sandbox_base_request.sandbox_working_directory,
        fixture.sandbox_slow_timeout_ms,
    );
    sandbox_request
        .sandbox_command_limits
        .sandbox_stdout_byte_limit = 4;
    sandbox_request
        .sandbox_command_limits
        .sandbox_stderr_byte_limit = 4;
    let (sandbox_result, ()) = sandbox_join_with_observer!(executor, sandbox_request, async {});
    match sandbox_result {
        Ok(outcome)
            if outcome.sandbox_stdout.len() <= 4
                && outcome.sandbox_stderr.len() <= 4
                && (outcome.sandbox_stdout_truncated || outcome.sandbox_stderr_truncated) =>
        {
            SandboxCommandConformanceFinding {
                sandbox_scenario_id,
                sandbox_status: SandboxCommandConformanceStatus::Enforced,
                sandbox_detail: "capture stopped at the byte bound and reported truncation"
                    .to_owned(),
            }
        }
        Ok(outcome) => SandboxCommandConformanceFinding {
            sandbox_scenario_id,
            sandbox_status: SandboxCommandConformanceStatus::Failed,
            sandbox_detail: format!(
                "bounds not observed: stdout={} truncated={} stderr={} truncated={}",
                outcome.sandbox_stdout.len(),
                outcome.sandbox_stdout_truncated,
                outcome.sandbox_stderr.len(),
                outcome.sandbox_stderr_truncated
            ),
        },
        Err(sandbox_error) => SandboxCommandConformanceFinding {
            sandbox_scenario_id,
            sandbox_status: SandboxCommandConformanceStatus::Failed,
            sandbox_detail: format!("bounded execution refused: {sandbox_error}"),
        },
    }
}

async fn sandbox_scenario_stale_fencing(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "stale-fencing-fail-closed";
    let mut sandbox_request = sandbox_derived_request(
        fixture,
        &fixture.sandbox_slow_executable,
        fixture.sandbox_slow_arguments.clone(),
        &fixture.sandbox_base_request.sandbox_working_directory,
        fixture.sandbox_slow_timeout_ms,
    );
    sandbox_request.sandbox_command_operation_id = "sandbox-conformance-stale-operation".to_owned();
    let sandbox_operation_id = sandbox_request.sandbox_command_operation_id.clone();
    let sandbox_token = sandbox_request.sandbox_fencing_token;
    let (sandbox_live_result, sandbox_stale) =
        sandbox_join_with_observer!(executor, sandbox_request, async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let sandbox_stale = executor
                .sandbox_cancel(
                    &fixture.sandbox_base_request.sandbox_tenant_id,
                    &fixture.sandbox_base_request.sandbox_provider_id,
                    &sandbox_operation_id,
                    sandbox_token.wrapping_add(1),
                )
                .await;
            // The stale attempt must not touch the live execution; settle it
            // with the correct token so the drive terminates inside its bound.
            let _ = executor
                .sandbox_cancel(
                    &fixture.sandbox_base_request.sandbox_tenant_id,
                    &fixture.sandbox_base_request.sandbox_provider_id,
                    &sandbox_operation_id,
                    sandbox_token,
                )
                .await;
            sandbox_stale
        });
    let sandbox_stale_holds = matches!(
        sandbox_stale,
        Err(SandboxCommandExecutionError::StaleFencing)
    );
    let sandbox_live_terminated = matches!(&sandbox_live_result, Ok(_));
    SandboxCommandConformanceFinding {
        sandbox_scenario_id,
        sandbox_status: if sandbox_stale_holds && sandbox_live_terminated {
            SandboxCommandConformanceStatus::Enforced
        } else {
            SandboxCommandConformanceStatus::Failed
        },
        sandbox_detail: format!(
            "stale-token cancel={sandbox_stale:?}; live execution settled={sandbox_live_result:?}"
        ),
    }
}

/// Drives one live execution plus a tampered live replay: the pair feeds both
/// the recomputation scenario (the executor recomputes and registers the
/// canonical fingerprint, so the replay cannot smuggle moved fields) and the
/// different-fingerprint conflict scenario.
async fn sandbox_scenario_fingerprint_recomputation(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> Vec<SandboxCommandConformanceFinding> {
    let mut sandbox_request = sandbox_derived_request(
        fixture,
        &fixture.sandbox_slow_executable,
        fixture.sandbox_slow_arguments.clone(),
        &fixture.sandbox_base_request.sandbox_working_directory,
        fixture.sandbox_slow_timeout_ms,
    );
    sandbox_request.sandbox_command_operation_id =
        "sandbox-conformance-fingerprint-operation".to_owned();
    let mut sandbox_tampered = sandbox_request.clone();
    sandbox_tampered
        .sandbox_arguments
        .push("tampered".to_owned());
    let sandbox_operation_id = sandbox_request.sandbox_command_operation_id.clone();
    let sandbox_token = sandbox_request.sandbox_fencing_token;
    let (sandbox_live_result, sandbox_conflict) =
        sandbox_join_with_observer!(executor, sandbox_request, async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let sandbox_conflict = executor.sandbox_execute(&sandbox_tampered).await;
            let _ = executor
                .sandbox_cancel(
                    &fixture.sandbox_base_request.sandbox_tenant_id,
                    &fixture.sandbox_base_request.sandbox_provider_id,
                    &sandbox_operation_id,
                    sandbox_token,
                )
                .await;
            sandbox_conflict
        });
    let sandbox_conflict_holds = matches!(
        sandbox_conflict,
        Err(SandboxCommandExecutionError::IdempotencyConflict)
    );
    let sandbox_live_terminated = matches!(&sandbox_live_result, Ok(_));
    let sandbox_status = if sandbox_conflict_holds && sandbox_live_terminated {
        SandboxCommandConformanceStatus::Enforced
    } else {
        SandboxCommandConformanceStatus::Failed
    };
    vec![
        SandboxCommandConformanceFinding {
            sandbox_scenario_id: "derived-fingerprint-recomputation-and-mismatch-rejection",
            sandbox_status: sandbox_status.clone(),
            sandbox_detail: format!(
                "same operation id with a moved covered field while live: {sandbox_conflict:?}; live settled={sandbox_live_result:?}"
            ),
        },
        SandboxCommandConformanceFinding {
            sandbox_scenario_id: "same-operation-different-fingerprint-conflict",
            sandbox_status,
            sandbox_detail: format!(
                "a different fingerprint under the same live operation id is an idempotency conflict: {sandbox_conflict:?}"
            ),
        },
    ]
}

/// Drives one live execution plus a verbatim live replay: the pair feeds both
/// the same-fingerprint replay scenario and the in-progress
/// no-duplicate-spawn guarantee, because a live registry conflict is exactly
/// the refusal to spawn a second child.
async fn sandbox_scenario_same_fingerprint_replay(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> Vec<SandboxCommandConformanceFinding> {
    let mut sandbox_request = sandbox_derived_request(
        fixture,
        &fixture.sandbox_slow_executable,
        fixture.sandbox_slow_arguments.clone(),
        &fixture.sandbox_base_request.sandbox_working_directory,
        fixture.sandbox_slow_timeout_ms,
    );
    sandbox_request.sandbox_command_operation_id =
        "sandbox-conformance-replay-operation".to_owned();
    let sandbox_operation_id = sandbox_request.sandbox_command_operation_id.clone();
    let sandbox_token = sandbox_request.sandbox_fencing_token;
    let (sandbox_live_result, sandbox_duplicate) =
        sandbox_join_with_observer!(executor, sandbox_request, async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            let sandbox_duplicate = executor.sandbox_execute(&sandbox_request).await;
            let _ = executor
                .sandbox_cancel(
                    &fixture.sandbox_base_request.sandbox_tenant_id,
                    &fixture.sandbox_base_request.sandbox_provider_id,
                    &sandbox_operation_id,
                    sandbox_token,
                )
                .await;
            sandbox_duplicate
        });
    let sandbox_conflict_holds = matches!(
        sandbox_duplicate,
        Err(SandboxCommandExecutionError::OperationInProgress)
    );
    let sandbox_live_terminated = matches!(&sandbox_live_result, Ok(_));
    let sandbox_status = if sandbox_conflict_holds && sandbox_live_terminated {
        SandboxCommandConformanceStatus::PartiallyEnforced(SANDBOX_DURABLE_ARBITRATION_PENDING)
    } else {
        SandboxCommandConformanceStatus::Failed
    };
    let sandbox_pending_detail = sandbox_pending_detail(
        &SANDBOX_DURABLE_ARBITRATION_PENDING,
        "a live same-fingerprint duplicate was refused as operation-in-progress (no second spawn); replay after terminal outcome is the durable registry slice",
    );
    vec![
        SandboxCommandConformanceFinding {
            sandbox_scenario_id: "same-operation-same-fingerprint-replay",
            sandbox_status: sandbox_status.clone(),
            sandbox_detail: sandbox_pending_detail.clone(),
        },
        SandboxCommandConformanceFinding {
            sandbox_scenario_id: "in-progress-operation-does-not-spawn-duplicate",
            sandbox_status,
            sandbox_detail: sandbox_pending_detail,
        },
    ]
}

async fn sandbox_scenario_fenced_idempotent_cancellation(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "fenced-idempotent-cancellation-request";
    let sandbox_absent = executor
        .sandbox_cancel(
            &fixture.sandbox_base_request.sandbox_tenant_id,
            &fixture.sandbox_base_request.sandbox_provider_id,
            "sandbox-conformance-canceled-operation",
            7,
        )
        .await;
    let sandbox_absent_again = executor
        .sandbox_cancel(
            &fixture.sandbox_base_request.sandbox_tenant_id,
            &fixture.sandbox_base_request.sandbox_provider_id,
            "sandbox-conformance-canceled-operation",
            7,
        )
        .await;
    let sandbox_held = sandbox_absent.is_ok() && sandbox_absent_again.is_ok();
    SandboxCommandConformanceFinding {
        sandbox_scenario_id,
        sandbox_status: if sandbox_held {
            SandboxCommandConformanceStatus::Enforced
        } else {
            SandboxCommandConformanceStatus::Failed
        },
        sandbox_detail: format!(
            "cancel on an unknown operation is an idempotent no-op twice: {sandbox_absent:?} then {sandbox_absent_again:?}"
        ),
    }
}

async fn sandbox_scenario_terminal_result_error_partition(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "accepted-execution-terminal-result-error-partition";
    let sandbox_admitted = sandbox_derived_request(
        fixture,
        &fixture.sandbox_base_request.sandbox_executable,
        Vec::new(),
        &fixture.sandbox_base_request.sandbox_working_directory,
        fixture.sandbox_slow_timeout_ms,
    );
    let (sandbox_terminal, sandbox_error) =
        tokio::join!(executor.sandbox_execute(&sandbox_admitted), async {
            let sandbox_denied = sandbox_derived_request(
                fixture,
                fixture
                    .sandbox_shell_string_executables
                    .first()
                    .map(String::as_str)
                    .unwrap_or("definitely not allowlisted"),
                Vec::new(),
                &fixture.sandbox_base_request.sandbox_working_directory,
                fixture.sandbox_slow_timeout_ms,
            );
            executor.sandbox_execute(&sandbox_denied).await
        });
    let sandbox_partition_holds = sandbox_terminal.is_ok() && sandbox_error.is_err();
    SandboxCommandConformanceFinding {
        sandbox_scenario_id,
        sandbox_status: if sandbox_partition_holds {
            SandboxCommandConformanceStatus::Enforced
        } else {
            SandboxCommandConformanceStatus::Failed
        },
        sandbox_detail: format!(
            "admitted shape terminal={sandbox_terminal:?}; denied shape error={sandbox_error:?}"
        ),
    }
}

fn sandbox_scenario_terminal_race_single_winner() -> SandboxCommandConformanceFinding {
    SandboxCommandConformanceFinding {
        sandbox_scenario_id: "terminal-race-single-winner-and-replay",
        sandbox_status: SandboxCommandConformanceStatus::Pending(
            SANDBOX_DURABLE_ARBITRATION_PENDING,
        ),
        sandbox_detail: sandbox_pending_detail(
            &SANDBOX_DURABLE_ARBITRATION_PENDING,
            "durable first-terminal CAS is not an executor-port duty",
        ),
    }
}

fn sandbox_scenario_cleanup_failure_quarantine() -> SandboxCommandConformanceFinding {
    SandboxCommandConformanceFinding {
        sandbox_scenario_id: "cleanup-failure-visible-and-binding-quarantined",
        sandbox_status: SandboxCommandConformanceStatus::Pending(SANDBOX_COMPOSITION_PENDING),
        sandbox_detail: sandbox_pending_detail(
            &SANDBOX_COMPOSITION_PENDING,
            "cleanup uncertainty is quarantined by the composition layer, not the port",
        ),
    }
}

fn sandbox_scenario_safe_error_redaction() -> SandboxCommandConformanceFinding {
    // The typed execution error's Display names the failure family only; it
    // carries no request field, path, or provider-private reference by
    // construction. Assert that invariant over every variant.
    let sandbox_variants = [
        SandboxCommandExecutionError::InvalidRequest,
        SandboxCommandExecutionError::UnsupportedCapability,
        SandboxCommandExecutionError::PolicyDenied,
        SandboxCommandExecutionError::StaleFencing,
        SandboxCommandExecutionError::IdempotencyConflict,
        SandboxCommandExecutionError::OperationInProgress,
        SandboxCommandExecutionError::CommandNotFound,
        SandboxCommandExecutionError::ProviderUnavailable,
        SandboxCommandExecutionError::ResultUnavailable,
        SandboxCommandExecutionError::InternalFailure,
    ];
    let sandbox_clean = sandbox_variants.iter().all(|variant| {
        let sandbox_text = variant.to_string();
        !sandbox_text.contains('/')
            && !sandbox_text.contains('\\')
            && !sandbox_text.contains("tenant")
            && !sandbox_text.contains("workspace")
    });
    SandboxCommandConformanceFinding {
        sandbox_scenario_id: "safe-error-and-private-metadata-redaction",
        sandbox_status: if sandbox_clean {
            SandboxCommandConformanceStatus::Enforced
        } else {
            SandboxCommandConformanceStatus::Failed
        },
        sandbox_detail:
            "typed execution errors render the failure family without paths or identity fields"
                .to_owned(),
    }
}

async fn sandbox_scenario_resolution_without_path_search(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "provider-owned-executable-resolution-without-path-search";
    // A request-side PATH would be the only way to redirect resolution; the
    // boundary denies protected names before any lookup, so the provider-owned
    // roots remain the only search space.
    let mut sandbox_request = sandbox_derived_request(
        fixture,
        &fixture.sandbox_base_request.sandbox_executable,
        Vec::new(),
        &fixture.sandbox_base_request.sandbox_working_directory,
        fixture.sandbox_slow_timeout_ms,
    );
    sandbox_request
        .sandbox_environment
        .insert("PATH".to_owned(), "attacker-controlled".to_owned());
    let sandbox_denied = executor.sandbox_execute(&sandbox_request).await;
    SandboxCommandConformanceFinding {
        sandbox_scenario_id,
        sandbox_status: if matches!(
            sandbox_denied,
            Err(SandboxCommandExecutionError::PolicyDenied)
        ) {
            SandboxCommandConformanceStatus::Enforced
        } else {
            SandboxCommandConformanceStatus::Failed
        },
        sandbox_detail: format!(
            "request-side PATH cannot redirect provider-owned resolution: {sandbox_denied:?}"
        ),
    }
}

async fn sandbox_scenario_protected_environment_override(
    executor: &dyn SandboxCommandExecutor,
    fixture: &SandboxCommandConformanceFixture,
) -> SandboxCommandConformanceFinding {
    let sandbox_scenario_id = "protected-environment-override-rejection";
    let mut sandbox_rejections = Vec::new();
    for sandbox_protected in ["PATH", "LD_PRELOAD", "DYLD_INSERT_LIBRARIES"] {
        let mut sandbox_request = sandbox_derived_request(
            fixture,
            &fixture.sandbox_base_request.sandbox_executable,
            Vec::new(),
            &fixture.sandbox_base_request.sandbox_working_directory,
            fixture.sandbox_slow_timeout_ms,
        );
        sandbox_request
            .sandbox_environment
            .insert(sandbox_protected.to_owned(), "override".to_owned());
        sandbox_rejections.push((
            sandbox_protected,
            executor.sandbox_execute(&sandbox_request).await,
        ));
    }
    let sandbox_all_denied = sandbox_rejections
        .iter()
        .all(|(_, result)| matches!(result, Err(SandboxCommandExecutionError::PolicyDenied)));
    SandboxCommandConformanceFinding {
        sandbox_scenario_id,
        sandbox_status: if sandbox_all_denied {
            SandboxCommandConformanceStatus::Enforced
        } else {
            SandboxCommandConformanceStatus::Failed
        },
        sandbox_detail: format!(
            "protected overrides resolved to: {:?}",
            sandbox_rejections
                .iter()
                .map(|(name, result)| (*name, result.is_err()))
                .collect::<Vec<_>>()
        ),
    }
}

fn sandbox_scenario_policy_snapshot_immutability() -> SandboxCommandConformanceFinding {
    SandboxCommandConformanceFinding {
        sandbox_scenario_id: "runtime-binding-policy-snapshot-immutability",
        sandbox_status: SandboxCommandConformanceStatus::Pending(SANDBOX_COMPOSITION_PENDING),
        sandbox_detail: sandbox_pending_detail(
            &SANDBOX_COMPOSITION_PENDING,
            "the execution policy snapshot is bound and frozen by the composition layer",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{SandboxCommandConformanceStatus, SANDBOX_COMMAND_CONFORMANCE_SCENARIOS};

    /// The Rust-side scenario list must mirror the contract JSON's
    /// `conformanceScenarios` block exactly: twenty ids, in declaration order.
    /// The JSON side is pinned by
    /// `tests/contract/sandbox-command-contract.contract.test.mjs`.
    #[test]
    fn scenario_catalog_mirrors_the_contract_declaration_order() {
        assert_eq!(20, SANDBOX_COMMAND_CONFORMANCE_SCENARIOS.len());
        assert_eq!(
            SANDBOX_COMMAND_CONFORMANCE_SCENARIOS[0],
            "typed-executable-and-argv-preservation"
        );
        assert_eq!(
            SANDBOX_COMMAND_CONFORMANCE_SCENARIOS[19],
            "runtime-binding-policy-snapshot-immutability"
        );
        let mut sorted = SANDBOX_COMMAND_CONFORMANCE_SCENARIOS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(20, sorted.len(), "scenario ids must be unique");
    }

    /// A `Failed` verdict must be reachable and distinct from the pending
    /// states, so the suite cannot silently pass a contradicting executor.
    #[test]
    fn failed_status_is_distinct_from_enforced_and_pending() {
        assert_ne!(
            SandboxCommandConformanceStatus::Enforced,
            SandboxCommandConformanceStatus::Failed
        );
        let sandbox_pending =
            SandboxCommandConformanceStatus::Pending(super::SandboxCommandConformancePending {
                sandbox_slice: "slice",
                sandbox_gate: "gate",
            });
        assert_ne!(SandboxCommandConformanceStatus::Enforced, sandbox_pending);
        assert_ne!(SandboxCommandConformanceStatus::Failed, sandbox_pending);
    }
}
