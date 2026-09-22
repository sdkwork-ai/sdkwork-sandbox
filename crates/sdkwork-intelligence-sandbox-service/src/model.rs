use std::collections::BTreeSet;

use sdkwork_sandbox_provider_spi::{
    IsolationAssurance, OperationId, RuntimeCapability, SandboxId, SandboxProviderAllocationRef,
    SandboxProviderId, SandboxRuntimeBindingId, SandboxSessionId, SandboxWorkspaceId, TenantId,
};

use crate::{SandboxLifecycleError, SandboxLifecycleResult};

pub(crate) const MAX_SANDBOX_SESSION_VERSION: u64 = i64::MAX as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxSessionState {
    Created,
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
    Destroying,
    Destroyed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxSessionOperationKind {
    Create,
    Start,
    Stop,
    Destroy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxSessionFailure {
    Provider,
    Readiness,
    Cleanup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxOperationOutcome {
    InProgress,
    Succeeded,
    Failed(SandboxSessionFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxSessionOperation {
    sandbox_operation_id: OperationId,
    sandbox_operation_kind: SandboxSessionOperationKind,
    sandbox_operation_outcome: SandboxOperationOutcome,
}

impl SandboxSessionOperation {
    pub(crate) fn restore(
        sandbox_operation_id: OperationId,
        sandbox_operation_kind: SandboxSessionOperationKind,
        sandbox_operation_outcome: SandboxOperationOutcome,
    ) -> Self {
        Self {
            sandbox_operation_id,
            sandbox_operation_kind,
            sandbox_operation_outcome,
        }
    }

    #[must_use]
    pub fn sandbox_operation_id(&self) -> &OperationId {
        &self.sandbox_operation_id
    }

    #[must_use]
    pub fn sandbox_operation_kind(&self) -> SandboxSessionOperationKind {
        self.sandbox_operation_kind
    }

    #[must_use]
    pub fn sandbox_operation_outcome(&self) -> SandboxOperationOutcome {
        self.sandbox_operation_outcome
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxRuntimeBinding {
    sandbox_id: SandboxId,
    sandbox_runtime_binding_id: SandboxRuntimeBindingId,
    sandbox_provider_id: SandboxProviderId,
    sandbox_allocation_reference: Option<SandboxProviderAllocationRef>,
}

impl SandboxRuntimeBinding {
    pub(crate) fn new_intent(
        sandbox_id: SandboxId,
        sandbox_runtime_binding_id: SandboxRuntimeBindingId,
        sandbox_provider_id: SandboxProviderId,
    ) -> Self {
        Self {
            sandbox_id,
            sandbox_runtime_binding_id,
            sandbox_provider_id,
            sandbox_allocation_reference: None,
        }
    }

    pub(crate) fn restore(
        sandbox_id: SandboxId,
        sandbox_runtime_binding_id: SandboxRuntimeBindingId,
        sandbox_provider_id: SandboxProviderId,
        sandbox_allocation_reference: Option<SandboxProviderAllocationRef>,
    ) -> Self {
        Self {
            sandbox_id,
            sandbox_runtime_binding_id,
            sandbox_provider_id,
            sandbox_allocation_reference,
        }
    }

    #[must_use]
    pub fn sandbox_id(&self) -> &SandboxId {
        &self.sandbox_id
    }

    #[must_use]
    pub fn sandbox_runtime_binding_id(&self) -> &SandboxRuntimeBindingId {
        &self.sandbox_runtime_binding_id
    }

    #[must_use]
    pub fn sandbox_provider_id(&self) -> &SandboxProviderId {
        &self.sandbox_provider_id
    }

    pub(crate) fn sandbox_allocation_reference(&self) -> Option<&SandboxProviderAllocationRef> {
        self.sandbox_allocation_reference.as_ref()
    }

    pub(crate) fn set_sandbox_allocation_reference(
        &mut self,
        sandbox_allocation_reference: SandboxProviderAllocationRef,
    ) {
        self.sandbox_allocation_reference = Some(sandbox_allocation_reference);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxSession {
    tenant_id: TenantId,
    sandbox_workspace_id: SandboxWorkspaceId,
    sandbox_session_id: SandboxSessionId,
    sandbox_session_state: SandboxSessionState,
    sandbox_required_capabilities: BTreeSet<RuntimeCapability>,
    sandbox_minimum_assurance: IsolationAssurance,
    sandbox_runtime_binding: Option<SandboxRuntimeBinding>,
    sandbox_last_failure: Option<SandboxSessionFailure>,
    sandbox_operations: Vec<SandboxSessionOperation>,
    sandbox_version: u64,
}

impl SandboxSession {
    pub(crate) fn create(
        tenant_id: TenantId,
        sandbox_workspace_id: SandboxWorkspaceId,
        sandbox_session_id: SandboxSessionId,
        sandbox_operation_id: OperationId,
        sandbox_required_capabilities: BTreeSet<RuntimeCapability>,
        sandbox_minimum_assurance: IsolationAssurance,
    ) -> Self {
        Self {
            tenant_id,
            sandbox_workspace_id,
            sandbox_session_id,
            sandbox_session_state: SandboxSessionState::Created,
            sandbox_required_capabilities,
            sandbox_minimum_assurance,
            sandbox_runtime_binding: None,
            sandbox_last_failure: None,
            sandbox_operations: vec![SandboxSessionOperation {
                sandbox_operation_id,
                sandbox_operation_kind: SandboxSessionOperationKind::Create,
                sandbox_operation_outcome: SandboxOperationOutcome::Succeeded,
            }],
            sandbox_version: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        tenant_id: TenantId,
        sandbox_workspace_id: SandboxWorkspaceId,
        sandbox_session_id: SandboxSessionId,
        sandbox_session_state: SandboxSessionState,
        sandbox_required_capabilities: BTreeSet<RuntimeCapability>,
        sandbox_minimum_assurance: IsolationAssurance,
        sandbox_runtime_binding: Option<SandboxRuntimeBinding>,
        sandbox_last_failure: Option<SandboxSessionFailure>,
        sandbox_operations: Vec<SandboxSessionOperation>,
        sandbox_version: u64,
    ) -> Self {
        Self {
            tenant_id,
            sandbox_workspace_id,
            sandbox_session_id,
            sandbox_session_state,
            sandbox_required_capabilities,
            sandbox_minimum_assurance,
            sandbox_runtime_binding,
            sandbox_last_failure,
            sandbox_operations,
            sandbox_version,
        }
    }

    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    #[must_use]
    pub fn sandbox_workspace_id(&self) -> &SandboxWorkspaceId {
        &self.sandbox_workspace_id
    }

    #[must_use]
    pub fn sandbox_session_id(&self) -> &SandboxSessionId {
        &self.sandbox_session_id
    }

    #[must_use]
    pub fn sandbox_session_state(&self) -> SandboxSessionState {
        self.sandbox_session_state
    }

    #[must_use]
    pub fn sandbox_required_capabilities(&self) -> &BTreeSet<RuntimeCapability> {
        &self.sandbox_required_capabilities
    }

    #[must_use]
    pub fn sandbox_minimum_assurance(&self) -> IsolationAssurance {
        self.sandbox_minimum_assurance
    }

    #[must_use]
    pub fn sandbox_runtime_binding(&self) -> Option<&SandboxRuntimeBinding> {
        self.sandbox_runtime_binding.as_ref()
    }

    #[must_use]
    pub fn sandbox_last_failure(&self) -> Option<SandboxSessionFailure> {
        self.sandbox_last_failure
    }

    #[must_use]
    pub fn sandbox_operations(&self) -> &[SandboxSessionOperation] {
        &self.sandbox_operations
    }

    #[must_use]
    pub fn sandbox_version(&self) -> u64 {
        self.sandbox_version
    }

    pub(crate) fn matches_create(
        &self,
        sandbox_workspace_id: &SandboxWorkspaceId,
        sandbox_session_id: &SandboxSessionId,
        sandbox_required_capabilities: &BTreeSet<RuntimeCapability>,
        sandbox_minimum_assurance: IsolationAssurance,
    ) -> bool {
        self.sandbox_workspace_id == *sandbox_workspace_id
            && self.sandbox_session_id == *sandbox_session_id
            && self.sandbox_required_capabilities == *sandbox_required_capabilities
            && self.sandbox_minimum_assurance == sandbox_minimum_assurance
    }

    pub(crate) fn replay_sandbox_operation(
        &self,
        sandbox_operation_id: &OperationId,
        sandbox_operation_kind: SandboxSessionOperationKind,
    ) -> SandboxLifecycleResult<Option<SandboxOperationOutcome>> {
        let Some(sandbox_operation) = self.sandbox_operations.iter().find(|sandbox_operation| {
            sandbox_operation.sandbox_operation_id == *sandbox_operation_id
        }) else {
            return Ok(None);
        };

        if sandbox_operation.sandbox_operation_kind != sandbox_operation_kind {
            return Err(SandboxLifecycleError::IdempotencyConflict {
                sandbox_operation_id: sandbox_operation_id.clone(),
            });
        }

        Ok(Some(sandbox_operation.sandbox_operation_outcome))
    }

    pub(crate) fn begin_sandbox_operation(
        &mut self,
        sandbox_operation_id: OperationId,
        sandbox_operation_kind: SandboxSessionOperationKind,
    ) {
        self.sandbox_operations.push(SandboxSessionOperation {
            sandbox_operation_id,
            sandbox_operation_kind,
            sandbox_operation_outcome: SandboxOperationOutcome::InProgress,
        });
    }

    pub(crate) fn complete_sandbox_operation(&mut self, sandbox_operation_id: &OperationId) {
        if let Some(sandbox_operation) =
            self.sandbox_operations
                .iter_mut()
                .find(|sandbox_operation| {
                    sandbox_operation.sandbox_operation_id == *sandbox_operation_id
                })
        {
            sandbox_operation.sandbox_operation_outcome = SandboxOperationOutcome::Succeeded;
        }
    }

    pub(crate) fn fail_sandbox_operation(
        &mut self,
        sandbox_operation_id: &OperationId,
        sandbox_session_failure: SandboxSessionFailure,
    ) {
        if let Some(sandbox_operation) =
            self.sandbox_operations
                .iter_mut()
                .find(|sandbox_operation| {
                    sandbox_operation.sandbox_operation_id == *sandbox_operation_id
                })
        {
            sandbox_operation.sandbox_operation_outcome =
                SandboxOperationOutcome::Failed(sandbox_session_failure);
        }
        self.sandbox_last_failure = Some(sandbox_session_failure);
    }

    pub(crate) fn transition_sandbox_session(
        &mut self,
        target_sandbox_session_state: SandboxSessionState,
        sandbox_operation_kind: SandboxSessionOperationKind,
    ) -> SandboxLifecycleResult<()> {
        let valid = matches!(
            (self.sandbox_session_state, target_sandbox_session_state),
            (SandboxSessionState::Created, SandboxSessionState::Starting)
                | (
                    SandboxSessionState::Created,
                    SandboxSessionState::Destroying
                )
                | (SandboxSessionState::Starting, SandboxSessionState::Running)
                | (SandboxSessionState::Starting, SandboxSessionState::Failed)
                | (SandboxSessionState::Running, SandboxSessionState::Stopping)
                | (SandboxSessionState::Stopping, SandboxSessionState::Stopped)
                | (SandboxSessionState::Stopping, SandboxSessionState::Failed)
                | (SandboxSessionState::Stopped, SandboxSessionState::Starting)
                | (
                    SandboxSessionState::Stopped,
                    SandboxSessionState::Destroying
                )
                | (SandboxSessionState::Failed, SandboxSessionState::Starting)
                | (SandboxSessionState::Failed, SandboxSessionState::Destroying)
                | (
                    SandboxSessionState::Destroying,
                    SandboxSessionState::Destroyed
                )
                | (SandboxSessionState::Destroying, SandboxSessionState::Failed)
        );

        if !valid {
            return Err(SandboxLifecycleError::InvalidTransition {
                sandbox_session_state: self.sandbox_session_state,
                sandbox_operation_kind,
            });
        }
        self.sandbox_session_state = target_sandbox_session_state;
        if target_sandbox_session_state != SandboxSessionState::Failed {
            self.sandbox_last_failure = None;
        }
        Ok(())
    }

    pub(crate) fn set_sandbox_runtime_binding(
        &mut self,
        sandbox_runtime_binding: SandboxRuntimeBinding,
    ) {
        self.sandbox_runtime_binding = Some(sandbox_runtime_binding);
    }

    pub(crate) fn clear_sandbox_runtime_binding(&mut self) {
        self.sandbox_runtime_binding = None;
    }

    pub(crate) fn next_sandbox_version(&mut self) -> SandboxLifecycleResult<u64> {
        let current_sandbox_version = self.sandbox_version;
        self.sandbox_version = self
            .sandbox_version
            .checked_add(1)
            .filter(|sandbox_version| *sandbox_version <= MAX_SANDBOX_SESSION_VERSION)
            .ok_or(SandboxLifecycleError::InvariantViolation(
                "sandbox session version exceeds the persistence maximum",
            ))?;
        Ok(current_sandbox_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox_session_in_state(sandbox_session_state: SandboxSessionState) -> SandboxSession {
        SandboxSession::restore(
            TenantId::parse("tenant-test")
                .unwrap_or_else(|error| panic!("invalid test tenant id: {error}")),
            SandboxWorkspaceId::parse("workspace-test")
                .unwrap_or_else(|error| panic!("invalid test workspace id: {error}")),
            SandboxSessionId::parse("session-test")
                .unwrap_or_else(|error| panic!("invalid test session id: {error}")),
            sandbox_session_state,
            BTreeSet::new(),
            IsolationAssurance::HostUser,
            None,
            None,
            vec![SandboxSessionOperation::restore(
                OperationId::generate(),
                SandboxSessionOperationKind::Create,
                SandboxOperationOutcome::Succeeded,
            )],
            0,
        )
    }

    #[test]
    fn sandbox_session_state_transition_matrix_matches_the_documented_state_machine() {
        let sandbox_valid_transitions = [
            (SandboxSessionState::Created, SandboxSessionState::Starting),
            (
                SandboxSessionState::Created,
                SandboxSessionState::Destroying,
            ),
            (SandboxSessionState::Starting, SandboxSessionState::Running),
            (SandboxSessionState::Starting, SandboxSessionState::Failed),
            (SandboxSessionState::Running, SandboxSessionState::Stopping),
            (SandboxSessionState::Stopping, SandboxSessionState::Stopped),
            (SandboxSessionState::Stopping, SandboxSessionState::Failed),
            (SandboxSessionState::Stopped, SandboxSessionState::Starting),
            (
                SandboxSessionState::Stopped,
                SandboxSessionState::Destroying,
            ),
            (SandboxSessionState::Failed, SandboxSessionState::Starting),
            (SandboxSessionState::Failed, SandboxSessionState::Destroying),
            (
                SandboxSessionState::Destroying,
                SandboxSessionState::Destroyed,
            ),
            (SandboxSessionState::Destroying, SandboxSessionState::Failed),
        ];
        let sandbox_all_states = [
            SandboxSessionState::Created,
            SandboxSessionState::Starting,
            SandboxSessionState::Running,
            SandboxSessionState::Stopping,
            SandboxSessionState::Stopped,
            SandboxSessionState::Failed,
            SandboxSessionState::Destroying,
            SandboxSessionState::Destroyed,
        ];
        for sandbox_from_state in sandbox_all_states {
            for sandbox_to_state in sandbox_all_states {
                let mut sandbox_session = sandbox_session_in_state(sandbox_from_state);
                let sandbox_result = sandbox_session.transition_sandbox_session(
                    sandbox_to_state,
                    SandboxSessionOperationKind::Start,
                );
                if sandbox_valid_transitions.contains(&(sandbox_from_state, sandbox_to_state)) {
                    assert!(
                        sandbox_result.is_ok(),
                        "transition {sandbox_from_state:?} -> {sandbox_to_state:?} must be valid"
                    );
                    assert_eq!(sandbox_session.sandbox_session_state(), sandbox_to_state);
                } else {
                    assert!(
                        matches!(
                            sandbox_result,
                            Err(SandboxLifecycleError::InvalidTransition { .. })
                        ),
                        "transition {sandbox_from_state:?} -> {sandbox_to_state:?} must be rejected"
                    );
                    assert_eq!(sandbox_session.sandbox_session_state(), sandbox_from_state);
                }
            }
        }
    }

    #[test]
    fn sandbox_session_transition_away_from_failed_clears_last_failure() {
        let mut sandbox_session = sandbox_session_in_state(SandboxSessionState::Starting);
        sandbox_session
            .fail_sandbox_operation(&OperationId::generate(), SandboxSessionFailure::Readiness);
        sandbox_session
            .transition_sandbox_session(
                SandboxSessionState::Failed,
                SandboxSessionOperationKind::Start,
            )
            .unwrap_or_else(|error| panic!("valid transition to failed: {error}"));
        assert_eq!(
            sandbox_session.sandbox_last_failure(),
            Some(SandboxSessionFailure::Readiness)
        );

        sandbox_session
            .transition_sandbox_session(
                SandboxSessionState::Starting,
                SandboxSessionOperationKind::Start,
            )
            .unwrap_or_else(|error| panic!("valid transition away from failed: {error}"));
        assert_eq!(sandbox_session.sandbox_last_failure(), None);
    }

    #[test]
    fn sandbox_session_replay_distinguishes_matching_conflicting_and_missing_operations() {
        let mut sandbox_session = sandbox_session_in_state(SandboxSessionState::Created);
        let sandbox_start_operation_id = OperationId::generate();
        sandbox_session.begin_sandbox_operation(
            sandbox_start_operation_id.clone(),
            SandboxSessionOperationKind::Start,
        );

        assert!(matches!(
            sandbox_session.replay_sandbox_operation(
                &sandbox_start_operation_id,
                SandboxSessionOperationKind::Start,
            ),
            Ok(Some(SandboxOperationOutcome::InProgress))
        ));
        assert!(matches!(
            sandbox_session.replay_sandbox_operation(
                &OperationId::generate(),
                SandboxSessionOperationKind::Start,
            ),
            Ok(None)
        ));
        assert!(matches!(
            sandbox_session.replay_sandbox_operation(
                &sandbox_start_operation_id,
                SandboxSessionOperationKind::Destroy,
            ),
            Err(SandboxLifecycleError::IdempotencyConflict {
                sandbox_operation_id: conflict_operation_id,
            }) if conflict_operation_id == sandbox_start_operation_id
        ));
    }

    #[test]
    fn sandbox_session_version_fails_closed_at_the_persistence_maximum() {
        let mut sandbox_session = SandboxSession::restore(
            TenantId::parse("tenant-test")
                .unwrap_or_else(|error| panic!("invalid test tenant id: {error}")),
            SandboxWorkspaceId::parse("workspace-test")
                .unwrap_or_else(|error| panic!("invalid test workspace id: {error}")),
            SandboxSessionId::parse("session-test")
                .unwrap_or_else(|error| panic!("invalid test session id: {error}")),
            SandboxSessionState::Created,
            BTreeSet::new(),
            IsolationAssurance::HostUser,
            None,
            None,
            vec![SandboxSessionOperation::restore(
                OperationId::generate(),
                SandboxSessionOperationKind::Create,
                SandboxOperationOutcome::Succeeded,
            )],
            MAX_SANDBOX_SESSION_VERSION,
        );

        assert!(matches!(
            sandbox_session.next_sandbox_version(),
            Err(SandboxLifecycleError::InvariantViolation(
                "sandbox session version exceeds the persistence maximum"
            ))
        ));
        assert_eq!(
            sandbox_session.sandbox_version(),
            MAX_SANDBOX_SESSION_VERSION
        );
    }
}
