//! The [`SandboxInstanceLaunchPlan`] record (`contract`: `launchPlan`).
//!
//! Every contract required field appears verbatim; references are opaque and
//! fields are private with read-only accessors. A plan is immutable after
//! creation: the lifecycle moves are state-machine transitions whose only
//! outbound edges lead to terminal states, and the template version, pool
//! claim and identity evidence are consumed by opaque reference — the start
//! command's semantics stay owned by `REQ-2026-0029` and are never copied
//! into the plan.

use crate::bounds::MAX_SANDBOX_LAUNCH_PLAN_ID_LENGTH;
use crate::error::{SandboxLaunchAuthorityError, SandboxLaunchAuthorityResult};
use crate::state::SandboxLaunchPlanState;

fn sandbox_validated_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SANDBOX_LAUNCH_PLAN_ID_LENGTH
        && value
            .chars()
            .all(|c| c.is_ascii_graphic() && !c.is_whitespace())
        && !value.starts_with('/')
        && !value.contains("://")
}

/// One created, lifecycle-tracked fast-start launch plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxInstanceLaunchPlan {
    sandbox_instance_launch_plan_id: String,
    sandbox_template_version_ref: String,
    sandbox_pool_claim_ref: String,
    sandbox_fresh_guest_identity_evidence_ref: String,
    sandbox_instance_launch_plan_state: SandboxLaunchPlanState,
    sandbox_planned_at: u64,
    sandbox_updated_at: u64,
}

impl SandboxInstanceLaunchPlan {
    /// Creates one launch plan in `planned`, validating every field shape
    /// fail-closed. A planned plan authorizes no execution
    /// (`bindingSemantics.plannedAloneAuthorizesExecution` is false).
    pub fn sandbox_new(
        sandbox_instance_launch_plan_id: &str,
        sandbox_template_version_ref: &str,
        sandbox_pool_claim_ref: &str,
        sandbox_fresh_guest_identity_evidence_ref: &str,
        sandbox_planned_at: u64,
    ) -> SandboxLaunchAuthorityResult<Self> {
        let references_valid = [
            sandbox_instance_launch_plan_id,
            sandbox_template_version_ref,
            sandbox_pool_claim_ref,
            sandbox_fresh_guest_identity_evidence_ref,
        ]
        .iter()
        .all(|reference| sandbox_validated_reference(reference));
        if !references_valid {
            return Err(SandboxLaunchAuthorityError::SandboxLaunchInvalidPlan);
        }
        Ok(Self {
            sandbox_instance_launch_plan_id: sandbox_instance_launch_plan_id.to_owned(),
            sandbox_template_version_ref: sandbox_template_version_ref.to_owned(),
            sandbox_pool_claim_ref: sandbox_pool_claim_ref.to_owned(),
            sandbox_fresh_guest_identity_evidence_ref: sandbox_fresh_guest_identity_evidence_ref
                .to_owned(),
            sandbox_instance_launch_plan_state: SandboxLaunchPlanState::Planned,
            sandbox_planned_at,
            sandbox_updated_at: sandbox_planned_at,
        })
    }

    fn sandbox_advance_timestamp(
        &self,
        sandbox_updated_at: u64,
    ) -> SandboxLaunchAuthorityResult<()> {
        if sandbox_updated_at < self.sandbox_updated_at {
            return Err(SandboxLaunchAuthorityError::SandboxLaunchInvalidPlan);
        }
        Ok(())
    }

    /// Marks the plan consumed by the authorized worker; terminal.
    pub fn sandbox_mark_consumed(
        &mut self,
        sandbox_updated_at: u64,
    ) -> SandboxLaunchAuthorityResult<()> {
        self.sandbox_transition(SandboxLaunchPlanState::Consumed, sandbox_updated_at)
    }

    /// Expires the plan because its bound claim expired or was released; the
    /// plan can never execute; terminal.
    pub fn sandbox_expire(&mut self, sandbox_updated_at: u64) -> SandboxLaunchAuthorityResult<()> {
        self.sandbox_transition(SandboxLaunchPlanState::Expired, sandbox_updated_at)
    }

    /// Records binding uncertainty; terminal, and the plan never dispatches.
    pub fn sandbox_quarantine(
        &mut self,
        sandbox_updated_at: u64,
    ) -> SandboxLaunchAuthorityResult<()> {
        self.sandbox_transition(SandboxLaunchPlanState::Quarantined, sandbox_updated_at)
    }

    fn sandbox_transition(
        &mut self,
        next: SandboxLaunchPlanState,
        sandbox_updated_at: u64,
    ) -> SandboxLaunchAuthorityResult<()> {
        self.sandbox_advance_timestamp(sandbox_updated_at)?;
        if !self
            .sandbox_instance_launch_plan_state
            .sandbox_can_transition_to(next)
        {
            return Err(SandboxLaunchAuthorityError::SandboxLaunchIllegalTransition);
        }
        self.sandbox_instance_launch_plan_state = next;
        self.sandbox_updated_at = sandbox_updated_at;
        Ok(())
    }

    /// The launch plan identity.
    #[must_use]
    pub fn sandbox_instance_launch_plan_id(&self) -> &str {
        &self.sandbox_instance_launch_plan_id
    }

    /// The template version the start command belongs to (authority:
    /// `REQ-2026-0029`; consumed by reference, never copied).
    #[must_use]
    pub fn sandbox_template_version_ref(&self) -> &str {
        &self.sandbox_template_version_ref
    }

    /// The fenced pool claim the plan is bound to (authority:
    /// `REQ-2026-0019`).
    #[must_use]
    pub fn sandbox_pool_claim_ref(&self) -> &str {
        &self.sandbox_pool_claim_ref
    }

    /// The fresh guest-identity evidence every launch must carry.
    #[must_use]
    pub fn sandbox_fresh_guest_identity_evidence_ref(&self) -> &str {
        &self.sandbox_fresh_guest_identity_evidence_ref
    }

    /// The current lifecycle state.
    #[must_use]
    pub const fn sandbox_instance_launch_plan_state(&self) -> SandboxLaunchPlanState {
        self.sandbox_instance_launch_plan_state
    }

    /// The planning timestamp (whole seconds from the caller's clock).
    #[must_use]
    pub const fn sandbox_planned_at(&self) -> u64 {
        self.sandbox_planned_at
    }

    /// The last transition timestamp (whole seconds from the caller's
    /// clock).
    #[must_use]
    pub const fn sandbox_updated_at(&self) -> u64 {
        self.sandbox_updated_at
    }
}
