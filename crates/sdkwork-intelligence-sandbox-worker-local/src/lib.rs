#![forbid(unsafe_code)]
//! Local-lane adapter composing the ready fast-start authorities into one
//! audited execution flow (`REQ-2026-0034`, `REVIEW-20261006-local-lane`).
//!
//! The adapter owns no second state machine: plan consumption goes through
//! the launch-authority API ([`SandboxInstanceLaunchPlan`]), execution state
//! goes through the worker-authority API ([`SandboxWorkerExecution`]), the
//! allocation reference is earned through the declared provisioner port,
//! and the start command is dispatched through the single narrow
//! [`SandboxLaunchStartCommandPort`]. A port success report is the only
//! path to `started`; a port failure lands `failed`; port uncertainty lands
//! `quarantined` — nothing in this crate succeeds silently.
//!
//! What this crate deliberately does not own: the `REQ-2026-0007` executor
//! wiring (host boundary fixture, real tokio process — the named next
//! slice), any provisioner implementation (the port is the seam), the
//! Firecracker lane (KVM-locked), warm-slot consumption, CLI, public
//! API/SDK or deployment profile.

use std::sync::Arc;

use sdkwork_intelligence_sandbox_launch_authority::SandboxInstanceLaunchPlan;
use sdkwork_intelligence_sandbox_worker_authority::SandboxWorkerExecution;

/// Why an adapter run stopped without a terminal `started` outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SandboxWorkerLocalAdapterError {
    /// The start-command port reported deterministic failure.
    #[error("sandbox local-lane start command failed")]
    SandboxWorkerLocalStartFailed,
    /// The start-command port ended uncertain; the execution quarantines.
    #[error("sandbox local-lane start command ended uncertain")]
    SandboxWorkerLocalStartUncertain,
    /// The launch plan was not in a consumable state.
    #[error("sandbox local-lane launch plan is not consumable")]
    SandboxWorkerLocalPlanNotConsumable,
    /// A worker-authority transition refused the adapter's input.
    #[error("sandbox local-lane worker authority refused the transition")]
    SandboxWorkerLocalAuthorityRefused,
}

/// A run that stopped without reaching `started`, carrying the terminal
/// (or last-known) execution record so the caller can audit what state was
/// actually reached — a failed run must not hide its record. The record is
/// `None` only when the caller's own identifiers failed record validation,
/// so no record ever existed to hide.
#[derive(Debug)]
pub struct SandboxWorkerLocalRunFailure {
    /// Why the run stopped.
    pub sandbox_error: SandboxWorkerLocalAdapterError,
    /// The execution record in the state reached before the run stopped.
    /// Boxed to keep the `Err` variant small.
    pub sandbox_execution: Option<Box<SandboxWorkerExecution>>,
}

/// The declared provisioner seam: earns the allocation reference for one
/// consumed launch plan. The local provider's host boundary implements this
/// in its own slice; this crate ships no implementation.
pub trait SandboxLaunchProvisionerPort: Send + Sync {
    /// Provisions one launch plan's slot and returns the opaque allocation
    /// reference.
    ///
    /// # Errors
    ///
    /// Returns the adapter error when allocation fails; the adapter carries
    /// it back alongside the execution record.
    fn sandbox_provision(
        &self,
        sandbox_launch_plan_ref: &str,
    ) -> Result<String, SandboxWorkerLocalAdapterError>;
}

/// The outcome one start-command dispatch may report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxStartCommandOutcome {
    /// The start command ran to success.
    Started,
    /// The start command failed deterministically.
    Failed,
    /// The outcome ended uncertain (timeout, lost dispatcher).
    Uncertain,
}

/// The single narrow port the start command is dispatched through
/// (`EXE-02`). The `REQ-2026-0007` executor adapter implements this in its
/// own slice; tests script it.
pub trait SandboxLaunchStartCommandPort: Send + Sync {
    /// Dispatches the start command for one consumed launch plan and
    /// reports the outcome.
    fn sandbox_start(&self, sandbox_launch_plan_ref: &str) -> SandboxStartCommandOutcome;
}

/// The local-lane execution adapter.
#[derive(Clone)]
pub struct SandboxLocalLaunchAdapter {
    sandbox_provisioner: Arc<dyn SandboxLaunchProvisionerPort>,
    sandbox_start_commands: Arc<dyn SandboxLaunchStartCommandPort>,
}

impl SandboxLocalLaunchAdapter {
    /// Builds the adapter over the declared ports.
    #[must_use]
    pub fn sandbox_new(
        sandbox_provisioner: Arc<dyn SandboxLaunchProvisionerPort>,
        sandbox_start_commands: Arc<dyn SandboxLaunchStartCommandPort>,
    ) -> Self {
        Self {
            sandbox_provisioner,
            sandbox_start_commands,
        }
    }

    /// Drives one planned launch plan through the full local-lane flow and
    /// returns the terminal execution record:
    ///
    /// 1. accept the plan as a worker execution record (`accepted`);
    /// 2. consume the plan through the launch authority (`consumed`) — the
    ///    only plan handoff;
    /// 3. provision through the provisioner port, earning the allocation
    ///    reference (`provisioning`);
    /// 4. dispatch the start command through the start-command port
    ///    (`starting`);
    /// 5. reach the port-reported terminal: `started`, `failed`, or — for
    ///    uncertain ports — `quarantined`.
    ///
    /// A plan that is not `planned` is refused before any mutation. A run
    /// that stops after acceptance returns the execution record inside the
    /// failure, so a failed run never hides its record.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxWorkerLocalRunFailure`] when the plan is not
    /// consumable, the provisioner fails, or a port outcome is not success.
    pub fn sandbox_execute(
        &self,
        plan: &mut SandboxInstanceLaunchPlan,
        sandbox_worker_execution_id: &str,
        sandbox_now: u64,
    ) -> Result<SandboxWorkerExecution, SandboxWorkerLocalRunFailure> {
        let carry = |sandbox_error: SandboxWorkerLocalAdapterError,
                     sandbox_execution: Option<Box<SandboxWorkerExecution>>|
         -> SandboxWorkerLocalRunFailure {
            SandboxWorkerLocalRunFailure {
                sandbox_error,
                sandbox_execution,
            }
        };
        let plan_ref = plan.sandbox_instance_launch_plan_id().to_owned();
        let mut execution = SandboxWorkerExecution::sandbox_new(
            sandbox_worker_execution_id,
            &plan_ref,
            sandbox_now,
        )
        .map_err(|_sandbox_error| {
            carry(
                SandboxWorkerLocalAdapterError::SandboxWorkerLocalAuthorityRefused,
                None,
            )
        })?;
        if plan.sandbox_instance_launch_plan_state()
            != sdkwork_intelligence_sandbox_launch_authority::SandboxLaunchPlanState::Planned
        {
            return Err(carry(
                SandboxWorkerLocalAdapterError::SandboxWorkerLocalPlanNotConsumable,
                Some(Box::new(execution)),
            ));
        }
        plan.sandbox_mark_consumed(sandbox_now).map_err(|_| {
            carry(
                SandboxWorkerLocalAdapterError::SandboxWorkerLocalPlanNotConsumable,
                Some(Box::new(execution.clone())),
            )
        })?;

        let allocation_ref = self
            .sandbox_provisioner
            .sandbox_provision(&plan_ref)
            .map_err(|sandbox_error| carry(sandbox_error, Some(Box::new(execution.clone()))))?;
        execution
            .sandbox_record_provisioning(&allocation_ref, sandbox_now)
            .map_err(|_sandbox_error| {
                carry(
                    SandboxWorkerLocalAdapterError::SandboxWorkerLocalAuthorityRefused,
                    Some(Box::new(execution.clone())),
                )
            })?;
        execution
            .sandbox_record_starting(&plan_ref, sandbox_now)
            .map_err(|_sandbox_error| {
                carry(
                    SandboxWorkerLocalAdapterError::SandboxWorkerLocalAuthorityRefused,
                    Some(Box::new(execution.clone())),
                )
            })?;

        match self.sandbox_start_commands.sandbox_start(&plan_ref) {
            SandboxStartCommandOutcome::Started => {
                execution
                    .sandbox_record_started(sandbox_now)
                    .map_err(|_sandbox_error| {
                        carry(
                            SandboxWorkerLocalAdapterError::SandboxWorkerLocalAuthorityRefused,
                            Some(Box::new(execution.clone())),
                        )
                    })?;
                Ok(execution)
            }
            SandboxStartCommandOutcome::Failed => {
                execution
                    .sandbox_record_failed(sandbox_now)
                    .map_err(|_sandbox_error| {
                        carry(
                            SandboxWorkerLocalAdapterError::SandboxWorkerLocalAuthorityRefused,
                            Some(Box::new(execution.clone())),
                        )
                    })?;
                Err(carry(
                    SandboxWorkerLocalAdapterError::SandboxWorkerLocalStartFailed,
                    Some(Box::new(execution)),
                ))
            }
            SandboxStartCommandOutcome::Uncertain => {
                execution
                    .sandbox_quarantine(sandbox_now)
                    .map_err(|_sandbox_error| {
                        carry(
                            SandboxWorkerLocalAdapterError::SandboxWorkerLocalAuthorityRefused,
                            Some(Box::new(execution.clone())),
                        )
                    })?;
                Err(carry(
                    SandboxWorkerLocalAdapterError::SandboxWorkerLocalStartUncertain,
                    Some(Box::new(execution)),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_intelligence_sandbox_worker_authority::SandboxWorkerExecutionState;

    struct ScriptedProvisioner;
    impl SandboxLaunchProvisionerPort for ScriptedProvisioner {
        fn sandbox_provision(
            &self,
            _plan_ref: &str,
        ) -> Result<String, SandboxWorkerLocalAdapterError> {
            Ok("allocation-1".to_owned())
        }
    }

    struct ScriptedStart(SandboxStartCommandOutcome);
    impl SandboxLaunchStartCommandPort for ScriptedStart {
        fn sandbox_start(&self, _plan_ref: &str) -> SandboxStartCommandOutcome {
            self.0
        }
    }

    fn adapter(outcome: SandboxStartCommandOutcome) -> SandboxLocalLaunchAdapter {
        SandboxLocalLaunchAdapter::sandbox_new(
            Arc::new(ScriptedProvisioner),
            Arc::new(ScriptedStart(outcome)),
        )
    }

    fn planned_plan() -> SandboxInstanceLaunchPlan {
        SandboxInstanceLaunchPlan::sandbox_new(
            "plan-1",
            "version-1",
            "claim-1",
            "identity-evidence-1",
            1_000,
        )
        .expect("valid plan")
    }

    fn carried_state(failure: &SandboxWorkerLocalRunFailure) -> SandboxWorkerExecutionState {
        failure
            .sandbox_execution
            .as_ref()
            .expect("record carried")
            .sandbox_worker_execution_state()
    }

    #[test]
    fn a_successful_port_run_earns_every_reference_and_reaches_started() {
        let mut plan = planned_plan();
        let execution = adapter(SandboxStartCommandOutcome::Started)
            .sandbox_execute(&mut plan, "execution-1", 1_100)
            .expect("started");
        assert_eq!(
            execution.sandbox_worker_execution_state(),
            SandboxWorkerExecutionState::Started
        );
        assert_eq!(
            execution.sandbox_provider_allocation_ref(),
            Some("allocation-1")
        );
        assert_eq!(execution.sandbox_launch_plan_ref(), "plan-1");
        // The plan handoff happened through the launch authority.
        assert_eq!(
            plan.sandbox_instance_launch_plan_state(),
            sdkwork_intelligence_sandbox_launch_authority::SandboxLaunchPlanState::Consumed
        );
    }

    #[test]
    fn a_failed_or_uncertain_port_never_reports_started_and_also_carries_its_record() {
        let mut failing = planned_plan();
        let failure = adapter(SandboxStartCommandOutcome::Failed)
            .sandbox_execute(&mut failing, "execution-1", 1_100)
            .expect_err("failed start");
        assert_eq!(
            failure.sandbox_error,
            SandboxWorkerLocalAdapterError::SandboxWorkerLocalStartFailed
        );
        assert_eq!(carried_state(&failure), SandboxWorkerExecutionState::Failed);

        let mut uncertain = planned_plan();
        let failure = adapter(SandboxStartCommandOutcome::Uncertain)
            .sandbox_execute(&mut uncertain, "execution-1", 1_100)
            .expect_err("uncertain start");
        assert_eq!(
            carried_state(&failure),
            SandboxWorkerExecutionState::Quarantined
        );
    }

    #[test]
    fn a_non_planned_plan_is_refused_before_any_mutation() {
        let mut consumed = planned_plan();
        consumed
            .sandbox_mark_consumed(1_050)
            .expect("first consumption");
        let failure = adapter(SandboxStartCommandOutcome::Started)
            .sandbox_execute(&mut consumed, "execution-1", 1_100)
            .expect_err("not consumable");
        assert_eq!(
            failure.sandbox_error,
            SandboxWorkerLocalAdapterError::SandboxWorkerLocalPlanNotConsumable
        );
        assert_eq!(
            carried_state(&failure),
            SandboxWorkerExecutionState::Accepted
        );
        // The plan was not re-consumed.
        assert_eq!(
            consumed.sandbox_instance_launch_plan_state(),
            sdkwork_intelligence_sandbox_launch_authority::SandboxLaunchPlanState::Consumed
        );
    }
}
