//! The Firecracker provider's command executor: admission, bounded live
//! registry, then delegation to the authenticated guest channel.
//!
//! This type implements the provider-neutral [`SandboxCommandExecutor`] port
//! for the Firecracker lane with the same semantics as the Local executor
//! (`REQ-2026-0007` shared contract; no Firecracker-private command DTO):
//! admission (contract limits, the pure-data guest boundary, and the
//! recomputed fingerprint) runs before any delegation; live executions are
//! tracked in a bounded registry keyed by the contract's execution key triple
//! (`tenantId`, `sandboxProviderId`, `sandboxCommandOperationId`), so a
//! fingerprint change under the same key is an
//! [`SandboxCommandExecutionError::IdempotencyConflict`], a same-fingerprint
//! live replay is an
//! [`SandboxCommandExecutionError::OperationInProgress`], and a fenced
//! cancellation reaches the live guest execution through the channel's
//! cancellation handle. The executor enforces the declared output bounds and
//! the wall-clock timeout around the channel call, so the guest transport
//! cannot widen a bound.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    sandbox_command_execution_fingerprint, SandboxCommandExecutionError,
    SandboxCommandExecutionRequest, SandboxCommandExecutor, SandboxCommandLimits,
    SandboxCommandOutcome,
};
use tokio::time::timeout;

use crate::command_admission::{admit_sandbox_command, SandboxFirecrackerCommandAdmissionError};
use crate::guest_boundary::{
    SandboxFirecrackerGuestBoundary, SandboxFirecrackerGuestBoundaryError,
};
use crate::guest_channel::{
    sandbox_map_guest_channel_error, SandboxFirecrackerAdmittedCommand,
    SandboxFirecrackerCancellationHandle, SandboxFirecrackerGuestCommandChannel,
};

/// Upper bound on concurrently live executions node-wide. The registry must
/// stay bounded: an unbounded live map would grow with request volume and
/// never shrink under a misbehaving caller (OOM rule). At capacity the
/// executor refuses new work as policy-denied instead of growing without
/// limit.
const MAX_SANDBOX_LIVE_COMMANDS: usize = 1024;

/// Upper bound on concurrently live executions per tenant. The node-wide
/// bound alone is first-come-first-served: one tenant filling every slot
/// would policy-deny every other tenant for as long as its commands run
/// (contract maxima allow hours). The per-tenant partition caps any single
/// tenant's share so cross-tenant starvation is impossible; fair sharing
/// beyond this hard bound is admission/capacity territory (REQ-2026-0016).
const MAX_SANDBOX_LIVE_COMMANDS_PER_TENANT: usize = 128;

/// Node-level budget on buffered command output. Every live execution
/// reserves its declared stdout plus stderr byte caps at admission (the
/// worst case the contract allows), so the node's captured-output memory
/// stays bounded no matter how many executions are admitted or how chatty
/// their guests are. An admission that would exceed the budget is
/// policy-denied instead of being allowed to grow the node toward OOM.
const MAX_SANDBOX_LIVE_OUTPUT_BYTES: u64 = 1 << 30;

/// Per-tenant share of the buffered-output budget. One tenant can never
/// reserve more than this share, so its output appetite cannot consume the
/// node-wide budget other tenants depend on.
const MAX_SANDBOX_LIVE_OUTPUT_BYTES_PER_TENANT: u64 = 1 << 28;

/// The contract's execution key triple (`tenantId`, `sandboxProviderId`,
/// `sandboxCommandOperationId`). Keying live executions by the triple, not the
/// bare operation id, keeps two tenants that reuse an id string out of each
/// other's registry cells.
type SandboxLiveCommandKey = (String, String, String);

/// One live execution's registry entry.
struct SandboxLiveCommand {
    sandbox_fencing_token: u64,
    sandbox_fingerprint: String,
    sandbox_output_bytes: u64,
    sandbox_cancellation: SandboxFirecrackerCancellationHandle,
}

/// Per-tenant live-execution share of the registry: entry count and the
/// output bytes reserved by those entries, both maintained incrementally so
/// admission checks stay O(1).
#[derive(Default)]
struct SandboxTenantLiveBudget {
    sandbox_live_commands: usize,
    sandbox_reserved_output_bytes: u64,
}

/// Bounded live-execution registry. The lock covers only the brief insert,
/// remove, and lookup map operations (never an await), and the
/// [`SandboxLiveCommandGuard`] removes an entry on every drop path -
/// including a cancelled or panicked execution future - so the map size
/// tracks actually-live guest executions only.
#[derive(Default)]
struct SandboxLiveCommandRegistry {
    sandbox_state: Mutex<SandboxLiveCommandState>,
}

#[derive(Default)]
struct SandboxLiveCommandState {
    sandbox_live: HashMap<SandboxLiveCommandKey, SandboxLiveCommand>,
    sandbox_tenant_live: HashMap<String, SandboxTenantLiveBudget>,
    sandbox_reserved_output_bytes: u64,
}

impl SandboxLiveCommandRegistry {
    /// Registers one live execution under the contract's execution key triple.
    ///
    /// A duplicate key is a replay of a live execution: the same fingerprint
    /// is [`SandboxCommandExecutionError::OperationInProgress`] (the contract's
    /// `sameFingerprintInProgress` outcome), a moved fingerprint is
    /// [`SandboxCommandExecutionError::IdempotencyConflict`] - never a second
    /// guest execution.
    fn sandbox_insert(
        &self,
        sandbox_key: &SandboxLiveCommandKey,
        sandbox_entry: SandboxLiveCommand,
    ) -> Result<(), SandboxCommandExecutionError> {
        let mut sandbox_state = self
            .sandbox_state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(sandbox_existing) = sandbox_state.sandbox_live.get(sandbox_key) {
            if sandbox_existing.sandbox_fingerprint != sandbox_entry.sandbox_fingerprint {
                tracing::warn!(
                    sandbox_tenant = %sandbox_key.0,
                    sandbox_provider = %sandbox_key.1,
                    sandbox_operation = %sandbox_key.2,
                    "a live sandbox command operation was replayed with a different fingerprint"
                );
                return Err(SandboxCommandExecutionError::IdempotencyConflict);
            }
            return Err(SandboxCommandExecutionError::OperationInProgress);
        }
        let sandbox_tenant_budget = sandbox_state.sandbox_tenant_live.get(&sandbox_key.0);
        if sandbox_state.sandbox_live.len() >= MAX_SANDBOX_LIVE_COMMANDS
            || sandbox_tenant_budget.is_some_and(|sandbox_budget| {
                sandbox_budget.sandbox_live_commands >= MAX_SANDBOX_LIVE_COMMANDS_PER_TENANT
            })
        {
            return Err(SandboxCommandExecutionError::PolicyDenied);
        }
        let sandbox_tenant_reserved_output_bytes = sandbox_tenant_budget
            .map(|sandbox_budget| sandbox_budget.sandbox_reserved_output_bytes)
            .unwrap_or_default();
        if sandbox_state
            .sandbox_reserved_output_bytes
            .saturating_add(sandbox_entry.sandbox_output_bytes)
            > MAX_SANDBOX_LIVE_OUTPUT_BYTES
            || sandbox_tenant_reserved_output_bytes
                .saturating_add(sandbox_entry.sandbox_output_bytes)
                > MAX_SANDBOX_LIVE_OUTPUT_BYTES_PER_TENANT
        {
            return Err(SandboxCommandExecutionError::PolicyDenied);
        }
        let sandbox_tenant_budget = sandbox_state
            .sandbox_tenant_live
            .entry(sandbox_key.0.clone())
            .or_default();
        sandbox_tenant_budget.sandbox_live_commands += 1;
        sandbox_tenant_budget.sandbox_reserved_output_bytes = sandbox_tenant_budget
            .sandbox_reserved_output_bytes
            .saturating_add(sandbox_entry.sandbox_output_bytes);
        sandbox_state.sandbox_reserved_output_bytes = sandbox_state
            .sandbox_reserved_output_bytes
            .saturating_add(sandbox_entry.sandbox_output_bytes);
        sandbox_state
            .sandbox_live
            .insert(sandbox_key.clone(), sandbox_entry);
        Ok(())
    }

    fn sandbox_remove(&self, sandbox_key: &SandboxLiveCommandKey) {
        let mut sandbox_state = self
            .sandbox_state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(((sandbox_tenant_id, _, _), sandbox_removed)) =
            sandbox_state.sandbox_live.remove_entry(sandbox_key)
        {
            if let Some(sandbox_tenant_budget) = sandbox_state
                .sandbox_tenant_live
                .get_mut(&sandbox_tenant_id)
            {
                sandbox_tenant_budget.sandbox_live_commands = sandbox_tenant_budget
                    .sandbox_live_commands
                    .saturating_sub(1);
                sandbox_tenant_budget.sandbox_reserved_output_bytes = sandbox_tenant_budget
                    .sandbox_reserved_output_bytes
                    .saturating_sub(sandbox_removed.sandbox_output_bytes);
                if sandbox_tenant_budget.sandbox_live_commands == 0 {
                    // Drop the empty bucket so the per-tenant map never
                    // accumulates stale entries for long-gone tenants.
                    sandbox_state.sandbox_tenant_live.remove(&sandbox_tenant_id);
                }
            }
            sandbox_state.sandbox_reserved_output_bytes = sandbox_state
                .sandbox_reserved_output_bytes
                .saturating_sub(sandbox_removed.sandbox_output_bytes);
        }
    }

    fn sandbox_lookup_cancel(
        &self,
        sandbox_key: &SandboxLiveCommandKey,
        sandbox_fencing_token: u64,
    ) -> Result<(), SandboxCommandExecutionError> {
        let sandbox_state = self
            .sandbox_state
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        match sandbox_state.sandbox_live.get(sandbox_key) {
            // Unknown or already-terminal operation: idempotent no-op.
            None => Ok(()),
            Some(sandbox_entry) if sandbox_entry.sandbox_fencing_token == sandbox_fencing_token => {
                sandbox_entry.sandbox_cancellation.sandbox_cancel();
                Ok(())
            }
            // A cancellation under a stale token must not touch a newer
            // execution of the same operation id.
            Some(_) => Err(SandboxCommandExecutionError::StaleFencing),
        }
    }
}

/// Removes the registry entry for one key when dropped, on every path -
/// terminal outcome, channel error, cancelled future, or panic. This keeps
/// the bounded registry from leaking slots under the service layer's
/// operation-timeout cancellation of long-running executions.
struct SandboxLiveCommandGuard {
    sandbox_registry: Arc<SandboxLiveCommandRegistry>,
    sandbox_key: SandboxLiveCommandKey,
}

impl Drop for SandboxLiveCommandGuard {
    fn drop(&mut self) {
        self.sandbox_registry.sandbox_remove(&self.sandbox_key);
    }
}

/// The Firecracker command executor: admission gate, bounded registry, and
/// guest-channel delegation under hard bounds.
#[derive(Clone)]
pub struct SandboxFirecrackerCommandExecutor {
    sandbox_boundary: Arc<SandboxFirecrackerGuestBoundary>,
    sandbox_channel: Arc<dyn SandboxFirecrackerGuestCommandChannel>,
    sandbox_registry: Arc<SandboxLiveCommandRegistry>,
}

impl SandboxFirecrackerCommandExecutor {
    /// Builds an executor over the shared guest boundary and a guest channel.
    #[must_use]
    pub fn new(
        sandbox_boundary: Arc<SandboxFirecrackerGuestBoundary>,
        sandbox_channel: Arc<dyn SandboxFirecrackerGuestCommandChannel>,
    ) -> Self {
        Self {
            sandbox_boundary,
            sandbox_channel,
            sandbox_registry: Arc::new(SandboxLiveCommandRegistry::default()),
        }
    }

    fn sandbox_admit(
        &self,
        sandbox_request: &SandboxCommandExecutionRequest,
    ) -> Result<SandboxFirecrackerAdmittedCommand, SandboxCommandExecutionError> {
        // Fencing token 0 is the unset-lease sentinel; a request carrying it
        // is malformed, not merely fenced.
        if sandbox_request.sandbox_fencing_token == 0 {
            return Err(SandboxCommandExecutionError::InvalidRequest);
        }
        // The contract requires the executor to recompute the fingerprint from
        // the request. Verification is against the caller-supplied value when
        // one is supplied; the recomputed value is always registered, so a
        // replay under the same operation id with a different request is a
        // conflict at the registry, not a silent re-execution.
        let sandbox_fingerprint = sandbox_command_execution_fingerprint(sandbox_request);
        match admit_sandbox_command(
            &self.sandbox_boundary,
            sandbox_request,
            Some(&sandbox_fingerprint),
        ) {
            Ok(()) => Ok(SandboxFirecrackerAdmittedCommand {
                sandbox_executable: sandbox_request.sandbox_executable.clone(),
                sandbox_arguments: sandbox_request.sandbox_arguments.clone(),
                sandbox_working_directory: sandbox_request.sandbox_working_directory.clone(),
                sandbox_environment: sandbox_request.sandbox_environment.clone(),
                sandbox_command_limits: sandbox_request.sandbox_command_limits,
            }),
            Err(
                SandboxFirecrackerCommandAdmissionError::LimitsOverBound(_)
                | SandboxFirecrackerCommandAdmissionError::FingerprintMismatch,
            ) => Err(SandboxCommandExecutionError::InvalidRequest),
            Err(SandboxFirecrackerCommandAdmissionError::BoundaryDenied(
                SandboxFirecrackerGuestBoundaryError::EnvironmentSensitive,
            ))
            | Err(SandboxFirecrackerCommandAdmissionError::BoundaryDenied(
                SandboxFirecrackerGuestBoundaryError::EnvironmentProtected,
            ))
            | Err(SandboxFirecrackerCommandAdmissionError::BoundaryDenied(
                SandboxFirecrackerGuestBoundaryError::EnvironmentDenied,
            )) => Err(SandboxCommandExecutionError::PolicyDenied),
            // A denied executable is a policy refusal of the requested
            // program, not a malformed request: the request shape may be
            // perfect and still not be on the allowlist the guest agent
            // resolves.
            Err(SandboxFirecrackerCommandAdmissionError::BoundaryDenied(
                SandboxFirecrackerGuestBoundaryError::ExecutableDenied,
            )) => Err(SandboxCommandExecutionError::PolicyDenied),
            Err(SandboxFirecrackerCommandAdmissionError::BoundaryDenied(_)) => {
                Err(SandboxCommandExecutionError::InvalidRequest)
            }
        }
    }
}

/// Applies the declared output bounds to one terminal outcome, truncating and
/// flagging when a bound is hit. The executor calls this after the channel
/// returns, so the node-side capture can never exceed the declared caps even
/// if the guest transport misbehaves.
fn sandbox_bound_outcome(
    mut sandbox_outcome: SandboxCommandOutcome,
    sandbox_limits: &SandboxCommandLimits,
) -> SandboxCommandOutcome {
    let sandbox_truncate =
        |sandbox_buffer: &mut Vec<u8>, sandbox_limit: u64, sandbox_truncated: &mut bool| {
            let sandbox_bound = usize::try_from(sandbox_limit).unwrap_or(usize::MAX);
            if sandbox_buffer.len() > sandbox_bound {
                sandbox_buffer.truncate(sandbox_bound);
                *sandbox_truncated = true;
            }
        };
    sandbox_truncate(
        &mut sandbox_outcome.sandbox_stdout,
        sandbox_limits.sandbox_stdout_byte_limit,
        &mut sandbox_outcome.sandbox_stdout_truncated,
    );
    sandbox_truncate(
        &mut sandbox_outcome.sandbox_stderr,
        sandbox_limits.sandbox_stderr_byte_limit,
        &mut sandbox_outcome.sandbox_stderr_truncated,
    );
    sandbox_outcome
}

#[async_trait]
impl SandboxCommandExecutor for SandboxFirecrackerCommandExecutor {
    async fn sandbox_execute(
        &self,
        sandbox_request: &SandboxCommandExecutionRequest,
    ) -> Result<SandboxCommandOutcome, SandboxCommandExecutionError> {
        let sandbox_command = self.sandbox_admit(sandbox_request)?;
        // The executor recomputes the fingerprint from the request and
        // registers exactly this value, so a replay under the same execution
        // key with a different request is a conflict at the registry, not a
        // silent re-execution.
        let sandbox_fingerprint = sandbox_command_execution_fingerprint(sandbox_request);
        let sandbox_key: SandboxLiveCommandKey = (
            sandbox_request.sandbox_tenant_id.clone(),
            sandbox_request.sandbox_provider_id.clone(),
            sandbox_request.sandbox_command_operation_id.clone(),
        );
        let sandbox_cancellation = SandboxFirecrackerCancellationHandle::new();
        self.sandbox_registry.sandbox_insert(
            &sandbox_key,
            SandboxLiveCommand {
                sandbox_fencing_token: sandbox_request.sandbox_fencing_token,
                sandbox_fingerprint,
                sandbox_output_bytes: sandbox_command
                    .sandbox_command_limits
                    .sandbox_stdout_byte_limit
                    .saturating_add(
                        sandbox_command
                            .sandbox_command_limits
                            .sandbox_stderr_byte_limit,
                    ),
                sandbox_cancellation: sandbox_cancellation.clone(),
            },
        )?;
        // The guard removes the live entry on every drop path, so a cancelled
        // or panicked execution can never strand its registry slot.
        let sandbox_live_guard = SandboxLiveCommandGuard {
            sandbox_registry: Arc::clone(&self.sandbox_registry),
            sandbox_key,
        };
        // The wall-clock bound is the executor's, not the channel's: the
        // timeout fires the cancellation handle (so a real transport can stop
        // the guest execution) and reports a no-exit-code outcome, which is
        // the contract's timeout shape.
        let sandbox_channel_result = timeout(
            Duration::from_millis(sandbox_command.sandbox_command_limits.sandbox_timeout_ms),
            self.sandbox_channel
                .sandbox_execute_admitted(&sandbox_command, &sandbox_cancellation),
        )
        .await;
        let sandbox_outcome = match sandbox_channel_result {
            Ok(Ok(sandbox_outcome)) => Ok(sandbox_bound_outcome(
                sandbox_outcome,
                &sandbox_command.sandbox_command_limits,
            )),
            Ok(Err(sandbox_channel_error)) => {
                Err(sandbox_map_guest_channel_error(&sandbox_channel_error))
            }
            Err(_sandbox_elapsed) => {
                sandbox_cancellation.sandbox_cancel();
                Ok(SandboxCommandOutcome {
                    sandbox_exit_code: None,
                    sandbox_stdout: Vec::new(),
                    sandbox_stderr: Vec::new(),
                    sandbox_stdout_truncated: false,
                    sandbox_stderr_truncated: false,
                })
            }
        };
        drop(sandbox_live_guard);
        sandbox_outcome
    }

    async fn sandbox_cancel(
        &self,
        sandbox_tenant_id: &str,
        sandbox_provider_id: &str,
        sandbox_operation_id: &str,
        sandbox_fencing_token: u64,
    ) -> Result<(), SandboxCommandExecutionError> {
        self.sandbox_registry.sandbox_lookup_cancel(
            &(
                sandbox_tenant_id.to_owned(),
                sandbox_provider_id.to_owned(),
                sandbox_operation_id.to_owned(),
            ),
            sandbox_fencing_token,
        )
    }
}

/// The executor never lets a channel error carry guest-identity or transport
/// detail: the mapped families are the contract's, and this test pins the
/// mapping's totality.
#[cfg(test)]
mod tests {
    use super::{
        sandbox_bound_outcome, SandboxFirecrackerCancellationHandle, SandboxLiveCommand,
        SandboxLiveCommandRegistry,
    };
    use sdkwork_sandbox_provider_spi::{
        SandboxCommandExecutionError, SandboxCommandLimits, SandboxCommandOutcome,
    };

    #[test]
    fn registry_refuses_a_stale_token_cancellation_against_a_newer_execution() {
        let sandbox_registry = SandboxLiveCommandRegistry::default();
        let sandbox_key = (
            "tenant".to_owned(),
            "provider".to_owned(),
            "operation".to_owned(),
        );
        sandbox_registry
            .sandbox_insert(
                &sandbox_key,
                SandboxLiveCommand {
                    sandbox_fencing_token: 7,
                    sandbox_fingerprint: "fingerprint".to_owned(),
                    sandbox_output_bytes: 8,
                    sandbox_cancellation: SandboxFirecrackerCancellationHandle::new(),
                },
            )
            .expect("a fresh key must register");
        assert_eq!(
            Err(SandboxCommandExecutionError::StaleFencing),
            sandbox_registry.sandbox_lookup_cancel(&sandbox_key, 6)
        );
        assert_eq!(
            Ok(()),
            sandbox_registry.sandbox_lookup_cancel(&sandbox_key, 7)
        );
    }

    #[test]
    fn output_bounds_truncate_and_flag() {
        let sandbox_limits = SandboxCommandLimits {
            sandbox_timeout_ms: 1_000,
            sandbox_stdout_byte_limit: 4,
            sandbox_stderr_byte_limit: 2,
            sandbox_cleanup_timeout_ms: 1_000,
            sandbox_max_process_count: 1,
        };
        let sandbox_outcome = sandbox_bound_outcome(
            SandboxCommandOutcome {
                sandbox_exit_code: Some(0),
                sandbox_stdout: b"abcdefgh".to_vec(),
                sandbox_stderr: b"xyz".to_vec(),
                sandbox_stdout_truncated: false,
                sandbox_stderr_truncated: false,
            },
            &sandbox_limits,
        );
        assert_eq!(
            b"abcd".as_slice(),
            sandbox_outcome.sandbox_stdout.as_slice()
        );
        assert!(sandbox_outcome.sandbox_stdout_truncated);
        assert_eq!(b"xy".as_slice(), sandbox_outcome.sandbox_stderr.as_slice());
        assert!(sandbox_outcome.sandbox_stderr_truncated);
    }
}
