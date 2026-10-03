use std::collections::BTreeSet;

use sdkwork_sandbox_provider_spi::{
    IsolationAssurance, OperationId, RuntimeCapability, SandboxId, SandboxProviderAllocationRef,
    SandboxProviderId, SandboxRuntimeBindingId, SandboxSessionId, SandboxWorkspaceId, TenantId,
};

use crate::{SandboxLifecycleError, SandboxLifecycleResult};

pub(crate) const MAX_SANDBOX_SESSION_VERSION: u64 = i64::MAX as u64;

/// Single source of the session operation-history bound. Writes fail closed at
/// the bound instead of growing a history the authoritative store refuses to
/// load; the PostgreSQL adapter reuses this constant for its read bound. A
/// dedicated retention policy is owned by REQ-2026-0020.
pub const MAX_SANDBOX_SESSION_OPERATIONS: usize = 10_000;

/// Hard persisted-row bound per session ledger: the retention bound plus one
/// grace entry reserved for the terminal `Destroy` operation. The bound never
/// blocks reaching a terminal state — a session at the retention bound must
/// stay destroyable so its provider allocation cannot leak — while ordinary
/// lifecycle growth still fails closed instead of growing without limit.
/// Production retention (ledger truncation and post-retention late-retry
/// semantics) remains owned by REQ-2026-0020.
pub const MAX_SANDBOX_SESSION_PERSISTED_OPERATIONS: usize = MAX_SANDBOX_SESSION_OPERATIONS + 1;

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
    /// Number of leading ledger entries already persisted by the authoritative
    /// store. Sessions restored from a store mark their whole loaded history
    /// persisted; saves then carry only the unpersisted tail. At most one
    /// `InProgress` entry exists and it is always last, so terminal entries
    /// below the frontier never change.
    sandbox_persisted_operation_count: usize,
    /// Outcome the store last recorded for the final ledger entry. The last
    /// entry is the only mutable one — an `InProgress` entry resolving after
    /// a crash — so the next save re-persists it exactly when its outcome
    /// differs from this fingerprint; terminal entries are never rewritten.
    sandbox_persisted_last_operation_outcome: Option<SandboxOperationOutcome>,
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
            sandbox_persisted_operation_count: 0,
            sandbox_persisted_last_operation_outcome: None,
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
        let sandbox_persisted_operation_count = sandbox_operations.len();
        let sandbox_persisted_last_operation_outcome = sandbox_operations
            .last()
            .map(SandboxSessionOperation::sandbox_operation_outcome);
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
            sandbox_persisted_operation_count,
            sandbox_persisted_last_operation_outcome,
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

    /// Ledger entries a save must persist: the unpersisted tail, plus the
    /// final entry exactly when its outcome diverges from what the store
    /// recorded (the only mutable entry — an `InProgress` operation that
    /// resolved). Fully persisted terminal ledgers yield an empty window.
    pub(crate) fn sandbox_operations_to_persist(&self) -> &[SandboxSessionOperation] {
        let sandbox_ledger_len = self.sandbox_operations.len();
        let mut sandbox_first_unpersisted_operation = self
            .sandbox_persisted_operation_count
            .min(sandbox_ledger_len);
        if sandbox_first_unpersisted_operation == sandbox_ledger_len && sandbox_ledger_len > 0 {
            let sandbox_last_operation = &self.sandbox_operations[sandbox_ledger_len - 1];
            let sandbox_outcome_is_unchanged = self
                .sandbox_persisted_last_operation_outcome
                .is_some_and(|sandbox_persisted_outcome| {
                    sandbox_persisted_outcome == sandbox_last_operation.sandbox_operation_outcome()
                });
            if !sandbox_outcome_is_unchanged {
                sandbox_first_unpersisted_operation = sandbox_ledger_len - 1;
            }
        }
        &self.sandbox_operations[sandbox_first_unpersisted_operation..]
    }

    /// Records that the whole current ledger is persisted, so the next save
    /// carries only operations appended or resolved after this point.
    pub(crate) fn mark_sandbox_operations_persisted(&mut self) {
        self.sandbox_persisted_operation_count = self.sandbox_operations.len();
        self.sandbox_persisted_last_operation_outcome = self
            .sandbox_operations
            .last()
            .map(SandboxSessionOperation::sandbox_operation_outcome);
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

    /// Appends one in-progress operation to the session's ledger.
    ///
    /// # Errors
    ///
    /// Returns `SandboxLifecycleError::InvariantViolation` when the ledger is
    /// at its bound and the operation would grow it further: writes fail
    /// closed instead of growing the history without limit, matching the
    /// repository read bound that refuses to load a session above it. The
    /// terminal `Destroy` kind is exempt up to
    /// [`MAX_SANDBOX_SESSION_PERSISTED_OPERATIONS`]: the bound must never
    /// block reaching a terminal state, because a session that cannot be
    /// destroyed leaks its provider allocation.
    pub(crate) fn begin_sandbox_operation(
        &mut self,
        sandbox_operation_id: OperationId,
        sandbox_operation_kind: SandboxSessionOperationKind,
    ) -> SandboxLifecycleResult<()> {
        let sandbox_ledger_is_at_retention_bound =
            self.sandbox_operations.len() >= MAX_SANDBOX_SESSION_OPERATIONS;
        let sandbox_destroy_is_within_grace_bound = sandbox_operation_kind
            == SandboxSessionOperationKind::Destroy
            && self.sandbox_operations.len() < MAX_SANDBOX_SESSION_PERSISTED_OPERATIONS;
        if sandbox_ledger_is_at_retention_bound && !sandbox_destroy_is_within_grace_bound {
            return Err(SandboxLifecycleError::InvariantViolation(
                "sandbox operation ledger is at the retention bound",
            ));
        }
        self.sandbox_operations.push(SandboxSessionOperation {
            sandbox_operation_id,
            sandbox_operation_kind,
            sandbox_operation_outcome: SandboxOperationOutcome::InProgress,
        });
        Ok(())
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
        sandbox_session
            .begin_sandbox_operation(
                sandbox_start_operation_id.clone(),
                SandboxSessionOperationKind::Start,
            )
            .expect("first begin below the retention bound");

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

    #[test]
    fn sandbox_session_operations_to_persist_tracks_the_unpersisted_tail_and_last_entry() {
        let mut sandbox_session = sandbox_session_in_state(SandboxSessionState::Created);
        // A restored session's whole loaded history is persisted and its
        // terminal last entry cannot change, so a save before any mutation
        // carries nothing.
        assert!(sandbox_session.sandbox_operations_to_persist().is_empty());

        let sandbox_start_operation_id = OperationId::generate();
        sandbox_session
            .begin_sandbox_operation(
                sandbox_start_operation_id.clone(),
                SandboxSessionOperationKind::Start,
            )
            .expect("begin below the retention bound");
        assert_eq!(sandbox_session.sandbox_operations_to_persist().len(), 1);
        assert_eq!(
            sandbox_session.sandbox_operations_to_persist()[0].sandbox_operation_id(),
            &sandbox_start_operation_id
        );

        // An appended operation that resolves before the next save is
        // covered by the tail window; the fingerprint path is exercised by
        // the crash-recovery completion below.
        sandbox_session.complete_sandbox_operation(&sandbox_start_operation_id);
        assert_eq!(sandbox_session.sandbox_operations_to_persist().len(), 1);
        assert!(matches!(
            sandbox_session.sandbox_operations_to_persist()[0].sandbox_operation_outcome(),
            SandboxOperationOutcome::Succeeded
        ));

        // Crash-recovery: the InProgress entry is loaded as persisted, its
        // later resolution must be re-persisted, and after the save nothing
        // remains in the window.
        sandbox_session.mark_sandbox_operations_persisted();
        assert!(sandbox_session.sandbox_operations_to_persist().is_empty());
        sandbox_session
            .fail_sandbox_operation(&sandbox_start_operation_id, SandboxSessionFailure::Provider);
        assert_eq!(sandbox_session.sandbox_operations_to_persist().len(), 1);
        assert!(matches!(
            sandbox_session.sandbox_operations_to_persist()[0].sandbox_operation_outcome(),
            SandboxOperationOutcome::Failed(SandboxSessionFailure::Provider)
        ));
        sandbox_session.mark_sandbox_operations_persisted();
        assert!(sandbox_session.sandbox_operations_to_persist().is_empty());

        let sandbox_created_session = SandboxSession::create(
            TenantId::parse("tenant-test")
                .unwrap_or_else(|error| panic!("invalid test tenant id: {error}")),
            SandboxWorkspaceId::parse("workspace-test")
                .unwrap_or_else(|error| panic!("invalid test workspace id: {error}")),
            SandboxSessionId::parse("session-create-test")
                .unwrap_or_else(|error| panic!("invalid test session id: {error}")),
            OperationId::generate(),
            BTreeSet::new(),
            IsolationAssurance::HostUser,
        );
        assert_eq!(
            sandbox_created_session
                .sandbox_operations_to_persist()
                .len(),
            1
        );
        assert_eq!(
            sandbox_created_session.sandbox_operations_to_persist()[0].sandbox_operation_kind(),
            SandboxSessionOperationKind::Create
        );
    }

    #[test]
    fn sandbox_session_crash_recovery_repersists_only_the_resolved_in_progress_entry() {
        // A session restored with a trailing InProgress entry: the entry is
        // already persisted, so the window is empty until its outcome flips.
        let sandbox_start_operation_id = OperationId::generate();
        let mut sandbox_session = sandbox_session_in_state(SandboxSessionState::Starting);
        sandbox_session
            .begin_sandbox_operation(
                sandbox_start_operation_id.clone(),
                SandboxSessionOperationKind::Start,
            )
            .expect("begin below the retention bound");
        sandbox_session.mark_sandbox_operations_persisted();

        let sandbox_restored_session = SandboxSession::restore(
            sandbox_session.tenant_id().clone(),
            sandbox_session.sandbox_workspace_id().clone(),
            sandbox_session.sandbox_session_id().clone(),
            SandboxSessionState::Starting,
            BTreeSet::new(),
            IsolationAssurance::HostUser,
            None,
            None,
            sandbox_session.sandbox_operations().to_vec(),
            1,
        );
        assert!(sandbox_restored_session
            .sandbox_operations_to_persist()
            .is_empty());

        let mut sandbox_session = sandbox_restored_session;
        sandbox_session.complete_sandbox_operation(&sandbox_start_operation_id);
        assert_eq!(sandbox_session.sandbox_operations_to_persist().len(), 1);
        assert_eq!(
            sandbox_session.sandbox_operations_to_persist()[0].sandbox_operation_id(),
            &sandbox_start_operation_id
        );
    }

    #[test]
    fn sandbox_session_retention_bound_never_blocks_the_terminal_destroy() {
        let mut sandbox_session = sandbox_session_in_state(SandboxSessionState::Created);
        let sandbox_start_operation_id = OperationId::generate();
        sandbox_session
            .begin_sandbox_operation(
                sandbox_start_operation_id.clone(),
                SandboxSessionOperationKind::Start,
            )
            .expect("begin below the retention bound");
        sandbox_session.complete_sandbox_operation(&sandbox_start_operation_id);

        // Pad the ledger to exactly the retention bound.
        while sandbox_session.sandbox_operations.len() < MAX_SANDBOX_SESSION_OPERATIONS {
            let sandbox_operation_id = OperationId::generate();
            sandbox_session
                .begin_sandbox_operation(
                    sandbox_operation_id.clone(),
                    SandboxSessionOperationKind::Start,
                )
                .expect("begin below the retention bound");
            sandbox_session.complete_sandbox_operation(&sandbox_operation_id);
        }

        // Ordinary growth fails closed at the bound.
        assert!(matches!(
            sandbox_session.begin_sandbox_operation(
                OperationId::generate(),
                SandboxSessionOperationKind::Start,
            ),
            Err(SandboxLifecycleError::InvariantViolation(
                "sandbox operation ledger is at the retention bound"
            ))
        ));

        // The terminal Destroy stays reachable for exactly one grace entry.
        let sandbox_destroy_operation_id = OperationId::generate();
        sandbox_session
            .begin_sandbox_operation(
                sandbox_destroy_operation_id.clone(),
                SandboxSessionOperationKind::Destroy,
            )
            .expect("terminal destroy within the grace bound");
        assert_eq!(
            sandbox_session.sandbox_operations.len(),
            MAX_SANDBOX_SESSION_PERSISTED_OPERATIONS
        );

        // Past the grace bound even Destroy fails closed; a same-id retry
        // still replays from the persisted ledger without appending.
        assert!(matches!(
            sandbox_session.begin_sandbox_operation(
                OperationId::generate(),
                SandboxSessionOperationKind::Destroy,
            ),
            Err(SandboxLifecycleError::InvariantViolation(
                "sandbox operation ledger is at the retention bound"
            ))
        ));
        assert!(matches!(
            sandbox_session.replay_sandbox_operation(
                &sandbox_destroy_operation_id,
                SandboxSessionOperationKind::Destroy,
            ),
            Ok(Some(SandboxOperationOutcome::InProgress))
        ));
    }
}
