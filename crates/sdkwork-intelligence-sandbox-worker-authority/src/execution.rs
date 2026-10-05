//! The [`SandboxWorkerExecution`] record (`contract`: `execution`).
//!
//! Every contract required field appears verbatim; references are opaque and
//! fields are private with read-only accessors. The allocation and start-
//! command references are earned in lifecycle order — provisioning earns the
//! allocation reference, starting earns the start-command reference — and a
//! completion claim (`started`) is reachable only from `starting`, so a
//! phantom success has no path into the record.

use crate::bounds::MAX_SANDBOX_WORKER_EXECUTION_ID_LENGTH;
use crate::error::{SandboxWorkerAuthorityError, SandboxWorkerAuthorityResult};
use crate::state::SandboxWorkerExecutionState;

fn sandbox_validated_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SANDBOX_WORKER_EXECUTION_ID_LENGTH
        && value
            .chars()
            .all(|c| c.is_ascii_graphic() && !c.is_whitespace())
        && !value.starts_with('/')
        && !value.contains("://")
}

/// One created, lifecycle-tracked worker execution of one launch plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxWorkerExecution {
    sandbox_worker_execution_id: String,
    sandbox_launch_plan_ref: String,
    sandbox_provider_allocation_ref: Option<String>,
    sandbox_start_command_execution_ref: Option<String>,
    sandbox_worker_execution_state: SandboxWorkerExecutionState,
    sandbox_accepted_at: u64,
    sandbox_updated_at: u64,
}

impl SandboxWorkerExecution {
    /// Creates one execution record in `accepted` for one launch plan,
    /// validating every field shape fail-closed. The plan state itself moves
    /// through the launch authority (`REQ-2026-0033`) — this record never
    /// writes it.
    pub fn sandbox_new(
        sandbox_worker_execution_id: &str,
        sandbox_launch_plan_ref: &str,
        sandbox_accepted_at: u64,
    ) -> SandboxWorkerAuthorityResult<Self> {
        let references_valid = [sandbox_worker_execution_id, sandbox_launch_plan_ref]
            .iter()
            .all(|reference| sandbox_validated_reference(reference));
        if !references_valid {
            return Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionInvalidRecord);
        }
        Ok(Self {
            sandbox_worker_execution_id: sandbox_worker_execution_id.to_owned(),
            sandbox_launch_plan_ref: sandbox_launch_plan_ref.to_owned(),
            sandbox_provider_allocation_ref: None,
            sandbox_start_command_execution_ref: None,
            sandbox_worker_execution_state: SandboxWorkerExecutionState::Accepted,
            sandbox_accepted_at,
            sandbox_updated_at: sandbox_accepted_at,
        })
    }

    fn sandbox_advance_timestamp(
        &self,
        sandbox_updated_at: u64,
    ) -> SandboxWorkerAuthorityResult<()> {
        if sandbox_updated_at < self.sandbox_updated_at {
            return Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionInvalidRecord);
        }
        Ok(())
    }

    fn sandbox_transition(
        &mut self,
        next: SandboxWorkerExecutionState,
        sandbox_updated_at: u64,
    ) -> SandboxWorkerAuthorityResult<()> {
        self.sandbox_advance_timestamp(sandbox_updated_at)?;
        if !self
            .sandbox_worker_execution_state
            .sandbox_can_transition_to(next)
        {
            return Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionIllegalTransition);
        }
        self.sandbox_worker_execution_state = next;
        self.sandbox_updated_at = sandbox_updated_at;
        Ok(())
    }

    /// Records that provider allocation and start are in flight, earning the
    /// allocation reference. Only `accepted -> provisioning` is legal.
    pub fn sandbox_record_provisioning(
        &mut self,
        sandbox_provider_allocation_ref: &str,
        sandbox_updated_at: u64,
    ) -> SandboxWorkerAuthorityResult<()> {
        self.sandbox_advance_timestamp(sandbox_updated_at)?;
        if !self
            .sandbox_worker_execution_state
            .sandbox_can_transition_to(SandboxWorkerExecutionState::Provisioning)
        {
            return Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionIllegalTransition);
        }
        if !sandbox_validated_reference(sandbox_provider_allocation_ref) {
            return Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionInvalidRecord);
        }
        self.sandbox_provider_allocation_ref = Some(sandbox_provider_allocation_ref.to_owned());
        self.sandbox_worker_execution_state = SandboxWorkerExecutionState::Provisioning;
        self.sandbox_updated_at = sandbox_updated_at;
        Ok(())
    }

    /// Records that the start command is dispatched through the command-
    /// executor port, earning the start-command execution reference. Only
    /// `provisioning -> starting` is legal.
    pub fn sandbox_record_starting(
        &mut self,
        sandbox_start_command_execution_ref: &str,
        sandbox_updated_at: u64,
    ) -> SandboxWorkerAuthorityResult<()> {
        self.sandbox_advance_timestamp(sandbox_updated_at)?;
        if !self
            .sandbox_worker_execution_state
            .sandbox_can_transition_to(SandboxWorkerExecutionState::Starting)
        {
            return Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionIllegalTransition);
        }
        if !sandbox_validated_reference(sandbox_start_command_execution_ref) {
            return Err(SandboxWorkerAuthorityError::SandboxWorkerExecutionInvalidRecord);
        }
        self.sandbox_start_command_execution_ref =
            Some(sandbox_start_command_execution_ref.to_owned());
        self.sandbox_worker_execution_state = SandboxWorkerExecutionState::Starting;
        self.sandbox_updated_at = sandbox_updated_at;
        Ok(())
    }

    /// Records the executor-reported success; terminal.
    pub fn sandbox_record_started(
        &mut self,
        sandbox_updated_at: u64,
    ) -> SandboxWorkerAuthorityResult<()> {
        self.sandbox_transition(SandboxWorkerExecutionState::Started, sandbox_updated_at)
    }

    /// Records a deterministic failure; terminal.
    pub fn sandbox_record_failed(
        &mut self,
        sandbox_updated_at: u64,
    ) -> SandboxWorkerAuthorityResult<()> {
        self.sandbox_transition(SandboxWorkerExecutionState::Failed, sandbox_updated_at)
    }

    /// Records an uncertain outcome (lost worker, unverifiable result);
    /// terminal.
    pub fn sandbox_quarantine(
        &mut self,
        sandbox_updated_at: u64,
    ) -> SandboxWorkerAuthorityResult<()> {
        self.sandbox_transition(SandboxWorkerExecutionState::Quarantined, sandbox_updated_at)
    }

    /// The execution identity.
    #[must_use]
    pub fn sandbox_worker_execution_id(&self) -> &str {
        &self.sandbox_worker_execution_id
    }

    /// The consumed launch plan (authority: `REQ-2026-0033`; the handoff is
    /// the only bridge between the two records).
    #[must_use]
    pub fn sandbox_launch_plan_ref(&self) -> &str {
        &self.sandbox_launch_plan_ref
    }

    /// The provider allocation reference, present once provisioning started.
    #[must_use]
    pub fn sandbox_provider_allocation_ref(&self) -> Option<&str> {
        self.sandbox_provider_allocation_ref.as_deref()
    }

    /// The start-command execution reference (through the `REQ-2026-0007`
    /// executor port), present once starting began.
    #[must_use]
    pub fn sandbox_start_command_execution_ref(&self) -> Option<&str> {
        self.sandbox_start_command_execution_ref.as_deref()
    }

    /// The current lifecycle state.
    #[must_use]
    pub const fn sandbox_worker_execution_state(&self) -> SandboxWorkerExecutionState {
        self.sandbox_worker_execution_state
    }

    /// The acceptance timestamp (whole seconds from the caller's clock).
    #[must_use]
    pub const fn sandbox_accepted_at(&self) -> u64 {
        self.sandbox_accepted_at
    }

    /// The last transition timestamp (whole seconds from the caller's
    /// clock).
    #[must_use]
    pub const fn sandbox_updated_at(&self) -> u64 {
        self.sandbox_updated_at
    }
}
