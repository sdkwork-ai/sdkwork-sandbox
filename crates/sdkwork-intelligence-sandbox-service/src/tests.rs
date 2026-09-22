use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    IsolationAssurance, OperationId, RuntimeCapability, SandboxFencingToken, SandboxId,
    SandboxLeaseOwnerId, SandboxProvider, SandboxProviderAllocation, SandboxProviderAllocationRef,
    SandboxProviderAllocationRequest, SandboxProviderDescriptor, SandboxProviderDestroyRequest,
    SandboxProviderError, SandboxProviderErrorKind, SandboxProviderHealth,
    SandboxProviderHealthStatus, SandboxProviderId, SandboxProviderKind, SandboxProviderOperation,
    SandboxProviderReadiness, SandboxProviderResult, SandboxProviderStartRequest,
    SandboxProviderStopRequest, SandboxRuntimeBindingId, SandboxSessionId, SandboxWorkspaceId,
    TenantId,
};

use crate::{
    CreateSandboxSessionCommand, SandboxLifecycleError, SandboxLifecycleService,
    SandboxOperationOutcome, SandboxProtectedProviderAllocationRef,
    SandboxProviderAllocationProtectionVersion, SandboxRuntimeBinding, SandboxSession,
    SandboxSessionFailure, SandboxSessionLease, SandboxSessionLifecycleCommand,
    SandboxSessionOperation, SandboxSessionOperationKind, SandboxSessionReconciliationOutcome,
    SandboxSessionRepository, SandboxSessionRepositoryError, SandboxSessionRepositoryResult,
    SandboxSessionState,
};

static NEXT_SANDBOX_SESSION_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Default)]
struct TestSandboxSessionRepositoryState {
    sandbox_sessions: HashMap<(TenantId, SandboxSessionId), SandboxSession>,
    sandbox_operations: HashMap<(TenantId, OperationId), SandboxSessionId>,
    sandbox_leases: HashMap<(TenantId, SandboxSessionId), TestSandboxSessionLease>,
}

struct TestSandboxSessionLease {
    sandbox_lease_owner_id: Option<SandboxLeaseOwnerId>,
    sandbox_fencing_token: u64,
    sandbox_lease_expires_at: Option<Instant>,
    sandbox_lease_expires_at_unix_millis: Option<i64>,
}

#[derive(Default)]
struct TestSandboxSessionRepository {
    sandbox_state: Mutex<TestSandboxSessionRepositoryState>,
    sandbox_enforce_recoverable_starting_transition: bool,
    sandbox_save_calls: AtomicUsize,
    sandbox_fail_save: Mutex<Option<(usize, SandboxSessionRepositoryError)>>,
    sandbox_renew_calls: AtomicUsize,
    sandbox_fail_renew_call: Mutex<Option<usize>>,
    sandbox_release_calls: AtomicUsize,
    sandbox_fail_release_call: Mutex<Option<usize>>,
    sandbox_insert_calls: AtomicUsize,
    sandbox_fail_insert: Mutex<Option<(usize, SandboxSessionRepositoryError)>>,
    sandbox_insert_race_winner: Mutex<Option<SandboxSession>>,
    sandbox_reconciliation_page_override: Mutex<Option<Vec<SandboxSession>>>,
}

impl TestSandboxSessionRepository {
    fn enforcing_recoverable_starting_transition() -> Self {
        Self {
            sandbox_state: Mutex::new(TestSandboxSessionRepositoryState::default()),
            sandbox_enforce_recoverable_starting_transition: true,
            sandbox_save_calls: AtomicUsize::new(0),
            sandbox_fail_save: Mutex::new(None),
            sandbox_renew_calls: AtomicUsize::new(0),
            sandbox_fail_renew_call: Mutex::new(None),
            sandbox_release_calls: AtomicUsize::new(0),
            sandbox_fail_release_call: Mutex::new(None),
            sandbox_insert_calls: AtomicUsize::new(0),
            sandbox_fail_insert: Mutex::new(None),
            sandbox_insert_race_winner: Mutex::new(None),
            sandbox_reconciliation_page_override: Mutex::new(None),
        }
    }

    fn fail_sandbox_save_call(&self, sandbox_save_call: usize) {
        self.fail_sandbox_save_call_with_error(
            sandbox_save_call,
            SandboxSessionRepositoryError::Unavailable,
        );
    }

    /// Arms the insert failure hook: the armed insert call first commits the
    /// race winner (simulating a concurrent identical create that committed
    /// between the service operation lookup and this insert) and then returns
    /// `VersionConflict`, exactly like the authoritative repository's
    /// sandbox_session primary-key collision.
    fn fail_sandbox_insert_call_with_race_winner(
        &self,
        sandbox_insert_call: usize,
        sandbox_race_winner: SandboxSession,
    ) {
        let mut sandbox_fail_insert = match self.sandbox_fail_insert.lock() {
            Ok(sandbox_fail_insert) => sandbox_fail_insert,
            Err(poisoned_sandbox_fail_insert) => poisoned_sandbox_fail_insert.into_inner(),
        };
        *sandbox_fail_insert = Some((
            sandbox_insert_call,
            SandboxSessionRepositoryError::VersionConflict,
        ));
        let mut sandbox_insert_race_winner = match self.sandbox_insert_race_winner.lock() {
            Ok(sandbox_insert_race_winner) => sandbox_insert_race_winner,
            Err(poisoned_sandbox_insert_race_winner) => {
                poisoned_sandbox_insert_race_winner.into_inner()
            }
        };
        *sandbox_insert_race_winner = Some(sandbox_race_winner);
    }

    fn sandbox_insert_error(&self) -> Option<SandboxSessionRepositoryError> {
        let sandbox_insert_call = self.sandbox_insert_calls.fetch_add(1, Ordering::SeqCst) + 1;
        let mut sandbox_fail_insert = match self.sandbox_fail_insert.lock() {
            Ok(sandbox_fail_insert) => sandbox_fail_insert,
            Err(poisoned_sandbox_fail_insert) => poisoned_sandbox_fail_insert.into_inner(),
        };
        match sandbox_fail_insert.as_ref() {
            Some((failed_sandbox_insert_call, _))
                if *failed_sandbox_insert_call == sandbox_insert_call =>
            {
                sandbox_fail_insert
                    .take()
                    .map(|(_, sandbox_repository_error)| sandbox_repository_error)
            }
            _ => None,
        }
    }

    fn store_sandbox_session_locked(
        sandbox_state: &mut TestSandboxSessionRepositoryState,
        sandbox_session: SandboxSession,
    ) {
        let sandbox_session_key = (
            sandbox_session.tenant_id().clone(),
            sandbox_session.sandbox_session_id().clone(),
        );
        for sandbox_operation in sandbox_session.sandbox_operations() {
            sandbox_state.sandbox_operations.insert(
                (
                    sandbox_session.tenant_id().clone(),
                    sandbox_operation.sandbox_operation_id().clone(),
                ),
                sandbox_session.sandbox_session_id().clone(),
            );
        }
        sandbox_state
            .sandbox_sessions
            .insert(sandbox_session_key.clone(), sandbox_session);
        sandbox_state.sandbox_leases.insert(
            sandbox_session_key,
            TestSandboxSessionLease {
                sandbox_lease_owner_id: None,
                sandbox_fencing_token: 0,
                sandbox_lease_expires_at: None,
                sandbox_lease_expires_at_unix_millis: None,
            },
        );
    }

    fn fail_sandbox_save_call_with_error(
        &self,
        sandbox_save_call: usize,
        sandbox_repository_error: SandboxSessionRepositoryError,
    ) {
        let mut sandbox_fail_save = match self.sandbox_fail_save.lock() {
            Ok(sandbox_fail_save) => sandbox_fail_save,
            Err(poisoned_sandbox_fail_save) => poisoned_sandbox_fail_save.into_inner(),
        };
        *sandbox_fail_save = Some((sandbox_save_call, sandbox_repository_error));
    }

    fn sandbox_save_error(&self) -> Option<SandboxSessionRepositoryError> {
        let sandbox_save_call = self.sandbox_save_calls.fetch_add(1, Ordering::SeqCst) + 1;
        let mut sandbox_fail_save = match self.sandbox_fail_save.lock() {
            Ok(sandbox_fail_save) => sandbox_fail_save,
            Err(poisoned_sandbox_fail_save) => poisoned_sandbox_fail_save.into_inner(),
        };
        match sandbox_fail_save.as_ref() {
            Some((failed_sandbox_save_call, _))
                if *failed_sandbox_save_call == sandbox_save_call =>
            {
                sandbox_fail_save
                    .take()
                    .map(|(_, sandbox_error)| sandbox_error)
            }
            _ => None,
        }
    }

    fn fail_sandbox_renew_call(&self, sandbox_renew_call: usize) {
        let mut sandbox_fail_renew_call = match self.sandbox_fail_renew_call.lock() {
            Ok(sandbox_fail_renew_call) => sandbox_fail_renew_call,
            Err(poisoned_sandbox_fail_renew_call) => poisoned_sandbox_fail_renew_call.into_inner(),
        };
        *sandbox_fail_renew_call = Some(sandbox_renew_call);
    }

    fn should_fail_sandbox_renew(&self) -> bool {
        let sandbox_renew_call = self.sandbox_renew_calls.fetch_add(1, Ordering::SeqCst) + 1;
        let mut sandbox_fail_renew_call = match self.sandbox_fail_renew_call.lock() {
            Ok(sandbox_fail_renew_call) => sandbox_fail_renew_call,
            Err(poisoned_sandbox_fail_renew_call) => poisoned_sandbox_fail_renew_call.into_inner(),
        };
        if *sandbox_fail_renew_call == Some(sandbox_renew_call) {
            *sandbox_fail_renew_call = None;
            true
        } else {
            false
        }
    }

    fn fail_sandbox_release_call(&self, sandbox_release_call: usize) {
        let mut sandbox_fail_release_call = match self.sandbox_fail_release_call.lock() {
            Ok(sandbox_fail_release_call) => sandbox_fail_release_call,
            Err(poisoned_sandbox_fail_release_call) => {
                poisoned_sandbox_fail_release_call.into_inner()
            }
        };
        *sandbox_fail_release_call = Some(sandbox_release_call);
    }

    fn should_fail_sandbox_release(&self) -> bool {
        let sandbox_release_call = self.sandbox_release_calls.fetch_add(1, Ordering::SeqCst) + 1;
        let mut sandbox_fail_release_call = match self.sandbox_fail_release_call.lock() {
            Ok(sandbox_fail_release_call) => sandbox_fail_release_call,
            Err(poisoned_sandbox_fail_release_call) => {
                poisoned_sandbox_fail_release_call.into_inner()
            }
        };
        if *sandbox_fail_release_call == Some(sandbox_release_call) {
            *sandbox_fail_release_call = None;
            true
        } else {
            false
        }
    }

    fn return_sandbox_reconciliation_page_once(
        &self,
        sandbox_reconciliation_page: Vec<SandboxSession>,
    ) {
        let mut sandbox_reconciliation_page_override =
            match self.sandbox_reconciliation_page_override.lock() {
                Ok(sandbox_reconciliation_page_override) => sandbox_reconciliation_page_override,
                Err(poisoned_sandbox_reconciliation_page_override) => {
                    poisoned_sandbox_reconciliation_page_override.into_inner()
                }
            };
        *sandbox_reconciliation_page_override = Some(sandbox_reconciliation_page);
    }

    fn lock_sandbox_state(&self) -> MutexGuard<'_, TestSandboxSessionRepositoryState> {
        match self.sandbox_state.lock() {
            Ok(sandbox_state) => sandbox_state,
            Err(poisoned_sandbox_state) => poisoned_sandbox_state.into_inner(),
        }
    }

    fn sandbox_lease_duration_millis(
        sandbox_lease_duration: Duration,
    ) -> SandboxSessionRepositoryResult<i64> {
        let sandbox_lease_duration_millis = sandbox_lease_duration.as_millis();
        if !(1..=300_000).contains(&sandbox_lease_duration_millis) {
            return Err(SandboxSessionRepositoryError::LeaseConflict);
        }
        i64::try_from(sandbox_lease_duration_millis)
            .map_err(|_| SandboxSessionRepositoryError::LeaseConflict)
    }

    fn unix_millis_now() -> SandboxSessionRepositoryResult<i64> {
        let sandbox_duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SandboxSessionRepositoryError::Unavailable)?;
        i64::try_from(sandbox_duration.as_millis())
            .map_err(|_| SandboxSessionRepositoryError::Unavailable)
    }

    fn sandbox_session_lease(
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
        sandbox_lease: &TestSandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<SandboxSessionLease> {
        SandboxSessionLease::new(
            tenant_id.clone(),
            sandbox_session_id.clone(),
            sandbox_lease
                .sandbox_lease_owner_id
                .clone()
                .ok_or(SandboxSessionRepositoryError::InvalidStoredData)?,
            SandboxFencingToken::new(sandbox_lease.sandbox_fencing_token)
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
            sandbox_lease
                .sandbox_lease_expires_at_unix_millis
                .ok_or(SandboxSessionRepositoryError::InvalidStoredData)?,
        )
    }
}

#[async_trait]
impl SandboxSessionRepository for TestSandboxSessionRepository {
    async fn find_by_sandbox_operation(
        &self,
        tenant_id: &TenantId,
        sandbox_operation_id: &OperationId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>> {
        let sandbox_state = self.lock_sandbox_state();
        let sandbox_session_id = sandbox_state
            .sandbox_operations
            .get(&(tenant_id.clone(), sandbox_operation_id.clone()));
        Ok(sandbox_session_id.and_then(|sandbox_session_id| {
            sandbox_state
                .sandbox_sessions
                .get(&(tenant_id.clone(), sandbox_session_id.clone()))
                .cloned()
        }))
    }

    async fn get_sandbox_session(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>> {
        Ok(self
            .lock_sandbox_state()
            .sandbox_sessions
            .get(&(tenant_id.clone(), sandbox_session_id.clone()))
            .cloned())
    }

    async fn insert_sandbox_session(
        &self,
        sandbox_session: SandboxSession,
    ) -> SandboxSessionRepositoryResult<()> {
        let mut sandbox_state = self.lock_sandbox_state();
        if let Some(sandbox_repository_error) = self.sandbox_insert_error() {
            let mut sandbox_insert_race_winner = match self.sandbox_insert_race_winner.lock() {
                Ok(sandbox_insert_race_winner) => sandbox_insert_race_winner,
                Err(poisoned_sandbox_insert_race_winner) => {
                    poisoned_sandbox_insert_race_winner.into_inner()
                }
            };
            if let Some(sandbox_race_winner) = sandbox_insert_race_winner.take() {
                Self::store_sandbox_session_locked(&mut sandbox_state, sandbox_race_winner);
            }
            return Err(sandbox_repository_error);
        }
        let sandbox_session_key = (
            sandbox_session.tenant_id().clone(),
            sandbox_session.sandbox_session_id().clone(),
        );
        if sandbox_state
            .sandbox_sessions
            .contains_key(&sandbox_session_key)
        {
            return Err(SandboxSessionRepositoryError::VersionConflict);
        }
        for sandbox_operation in sandbox_session.sandbox_operations() {
            if sandbox_state.sandbox_operations.contains_key(&(
                sandbox_session.tenant_id().clone(),
                sandbox_operation.sandbox_operation_id().clone(),
            )) {
                return Err(SandboxSessionRepositoryError::DuplicateOperation);
            }
        }
        Self::store_sandbox_session_locked(&mut sandbox_state, sandbox_session);
        Ok(())
    }

    async fn save_sandbox_session(
        &self,
        sandbox_session: SandboxSession,
        expected_sandbox_version: u64,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<()> {
        if let Some(sandbox_repository_error) = self.sandbox_save_error() {
            return Err(sandbox_repository_error);
        }
        let mut sandbox_state = self.lock_sandbox_state();
        let sandbox_session_key = (
            sandbox_session.tenant_id().clone(),
            sandbox_session.sandbox_session_id().clone(),
        );
        let current_sandbox_session = sandbox_state
            .sandbox_sessions
            .get(&sandbox_session_key)
            .ok_or(SandboxSessionRepositoryError::NotFound)?;
        if self.sandbox_enforce_recoverable_starting_transition
            && current_sandbox_session.sandbox_session_state() != SandboxSessionState::Starting
            && sandbox_session.sandbox_session_state() == SandboxSessionState::Starting
            && sandbox_session
                .sandbox_runtime_binding()
                .is_none_or(|sandbox_runtime_binding| {
                    sandbox_runtime_binding
                        .sandbox_allocation_reference()
                        .is_some()
                })
        {
            return Err(SandboxSessionRepositoryError::InvalidStoredData);
        }
        let sandbox_lease = sandbox_state
            .sandbox_leases
            .get(&sandbox_session_key)
            .ok_or(SandboxSessionRepositoryError::LeaseConflict)?;
        if sandbox_lease.sandbox_lease_owner_id.as_ref()
            != Some(sandbox_session_lease.sandbox_lease_owner_id())
            || sandbox_lease.sandbox_fencing_token
                != sandbox_session_lease.sandbox_fencing_token().value()
            || sandbox_lease
                .sandbox_lease_expires_at
                .is_none_or(|sandbox_lease_expires_at| sandbox_lease_expires_at <= Instant::now())
        {
            return Err(SandboxSessionRepositoryError::LeaseConflict);
        }
        if current_sandbox_session.sandbox_version() != expected_sandbox_version
            || sandbox_session.sandbox_version() != expected_sandbox_version + 1
        {
            return Err(SandboxSessionRepositoryError::VersionConflict);
        }
        for sandbox_operation in sandbox_session.sandbox_operations() {
            sandbox_state.sandbox_operations.insert(
                (
                    sandbox_session.tenant_id().clone(),
                    sandbox_operation.sandbox_operation_id().clone(),
                ),
                sandbox_session.sandbox_session_id().clone(),
            );
        }
        sandbox_state
            .sandbox_sessions
            .insert(sandbox_session_key, sandbox_session);
        Ok(())
    }

    async fn acquire_sandbox_session_lease(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
        sandbox_lease_owner_id: &SandboxLeaseOwnerId,
        sandbox_lease_duration: Duration,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSessionLease>> {
        let sandbox_lease_duration_millis =
            Self::sandbox_lease_duration_millis(sandbox_lease_duration)?;
        let mut sandbox_state = self.lock_sandbox_state();
        let sandbox_session_key = (tenant_id.clone(), sandbox_session_id.clone());
        if !sandbox_state
            .sandbox_sessions
            .contains_key(&sandbox_session_key)
        {
            return Err(SandboxSessionRepositoryError::NotFound);
        }
        let sandbox_lease = sandbox_state
            .sandbox_leases
            .get_mut(&sandbox_session_key)
            .ok_or(SandboxSessionRepositoryError::InvalidStoredData)?;
        let sandbox_now = Instant::now();
        if sandbox_lease
            .sandbox_lease_expires_at
            .is_some_and(|sandbox_lease_expires_at| sandbox_lease_expires_at > sandbox_now)
        {
            return Ok(None);
        }
        sandbox_lease.sandbox_fencing_token = sandbox_lease
            .sandbox_fencing_token
            .checked_add(1)
            .filter(|sandbox_fencing_token| *sandbox_fencing_token <= i64::MAX as u64)
            .ok_or(SandboxSessionRepositoryError::LeaseConflict)?;
        sandbox_lease.sandbox_lease_owner_id = Some(sandbox_lease_owner_id.clone());
        sandbox_lease.sandbox_lease_expires_at = Some(sandbox_now + sandbox_lease_duration);
        sandbox_lease.sandbox_lease_expires_at_unix_millis = Some(
            Self::unix_millis_now()?
                .checked_add(sandbox_lease_duration_millis)
                .ok_or(SandboxSessionRepositoryError::LeaseConflict)?,
        );
        Self::sandbox_session_lease(tenant_id, sandbox_session_id, sandbox_lease).map(Some)
    }

    async fn renew_sandbox_session_lease(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
        sandbox_lease_duration: Duration,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSessionLease>> {
        if self.should_fail_sandbox_renew() {
            return Err(SandboxSessionRepositoryError::Unavailable);
        }
        let sandbox_lease_duration_millis =
            Self::sandbox_lease_duration_millis(sandbox_lease_duration)?;
        let mut sandbox_state = self.lock_sandbox_state();
        let sandbox_lease = sandbox_state
            .sandbox_leases
            .get_mut(&(
                sandbox_session_lease.tenant_id().clone(),
                sandbox_session_lease.sandbox_session_id().clone(),
            ))
            .ok_or(SandboxSessionRepositoryError::NotFound)?;
        let sandbox_now = Instant::now();
        if sandbox_lease.sandbox_lease_owner_id.as_ref()
            != Some(sandbox_session_lease.sandbox_lease_owner_id())
            || sandbox_lease.sandbox_fencing_token
                != sandbox_session_lease.sandbox_fencing_token().value()
            || sandbox_lease
                .sandbox_lease_expires_at
                .is_none_or(|sandbox_lease_expires_at| sandbox_lease_expires_at <= sandbox_now)
        {
            return Ok(None);
        }
        sandbox_lease.sandbox_lease_expires_at = Some(sandbox_now + sandbox_lease_duration);
        sandbox_lease.sandbox_lease_expires_at_unix_millis = Some(
            Self::unix_millis_now()?
                .checked_add(sandbox_lease_duration_millis)
                .ok_or(SandboxSessionRepositoryError::LeaseConflict)?,
        );
        Self::sandbox_session_lease(
            sandbox_session_lease.tenant_id(),
            sandbox_session_lease.sandbox_session_id(),
            sandbox_lease,
        )
        .map(Some)
    }

    async fn release_sandbox_session_lease(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<bool> {
        if self.should_fail_sandbox_release() {
            return Err(SandboxSessionRepositoryError::Unavailable);
        }
        let mut sandbox_state = self.lock_sandbox_state();
        let sandbox_lease = sandbox_state
            .sandbox_leases
            .get_mut(&(
                sandbox_session_lease.tenant_id().clone(),
                sandbox_session_lease.sandbox_session_id().clone(),
            ))
            .ok_or(SandboxSessionRepositoryError::NotFound)?;
        if sandbox_lease.sandbox_lease_owner_id.as_ref()
            != Some(sandbox_session_lease.sandbox_lease_owner_id())
            || sandbox_lease.sandbox_fencing_token
                != sandbox_session_lease.sandbox_fencing_token().value()
        {
            return Ok(false);
        }
        sandbox_lease.sandbox_lease_owner_id = None;
        sandbox_lease.sandbox_lease_expires_at = None;
        sandbox_lease.sandbox_lease_expires_at_unix_millis = None;
        Ok(true)
    }

    async fn list_sandbox_sessions_requiring_reconciliation(
        &self,
        tenant_id: &TenantId,
        after_sandbox_session_id: Option<&SandboxSessionId>,
        sandbox_page_size: u16,
    ) -> SandboxSessionRepositoryResult<Vec<SandboxSession>> {
        if !(1..=200).contains(&sandbox_page_size) {
            return Err(SandboxSessionRepositoryError::InvalidPageRequest);
        }
        let sandbox_reconciliation_page_override = {
            let mut sandbox_reconciliation_page_override = match self
                .sandbox_reconciliation_page_override
                .lock()
            {
                Ok(sandbox_reconciliation_page_override) => sandbox_reconciliation_page_override,
                Err(poisoned_sandbox_reconciliation_page_override) => {
                    poisoned_sandbox_reconciliation_page_override.into_inner()
                }
            };
            sandbox_reconciliation_page_override.take()
        };
        if let Some(sandbox_reconciliation_page_override) = sandbox_reconciliation_page_override {
            return Ok(sandbox_reconciliation_page_override);
        }
        let sandbox_state = self.lock_sandbox_state();
        let mut sandbox_sessions = sandbox_state
            .sandbox_sessions
            .iter()
            .filter(
                |((stored_tenant_id, stored_sandbox_session_id), sandbox_session)| {
                    stored_tenant_id == tenant_id
                        && after_sandbox_session_id.is_none_or(|after_sandbox_session_id| {
                            stored_sandbox_session_id > after_sandbox_session_id
                        })
                        && matches!(
                            sandbox_session.sandbox_session_state(),
                            SandboxSessionState::Starting
                                | SandboxSessionState::Stopping
                                | SandboxSessionState::Destroying
                        )
                },
            )
            .map(|(_, sandbox_session)| sandbox_session.clone())
            .collect::<Vec<_>>();
        sandbox_sessions
            .sort_by(|left, right| left.sandbox_session_id().cmp(right.sandbox_session_id()));
        sandbox_sessions.truncate(usize::from(sandbox_page_size));
        Ok(sandbox_sessions)
    }
}

struct FakeSandboxProvider {
    sandbox_provider_descriptor: SandboxProviderDescriptor,
    sandbox_provider_health: SandboxProviderHealthStatus,
    sandbox_provider_readiness:
        Mutex<VecDeque<Result<SandboxProviderReadiness, SandboxProviderErrorKind>>>,
    sandbox_start_delay: Option<Duration>,
    fail_sandbox_destroy_call: Option<usize>,
    fail_sandbox_stop_call: Option<usize>,
    sandbox_health_calls: AtomicUsize,
    sandbox_allocate_calls: AtomicUsize,
    sandbox_start_calls: AtomicUsize,
    sandbox_stop_calls: AtomicUsize,
    sandbox_destroy_calls: AtomicUsize,
    sandbox_allocate_requests: Mutex<Vec<SandboxProviderAllocationRequest>>,
    sandbox_start_requests: Mutex<Vec<SandboxProviderStartRequest>>,
    sandbox_allocate_fencing_tokens: Mutex<Vec<SandboxFencingToken>>,
    sandbox_start_fencing_tokens: Mutex<Vec<SandboxFencingToken>>,
    sandbox_stop_fencing_tokens: Mutex<Vec<SandboxFencingToken>>,
    sandbox_destroy_fencing_tokens: Mutex<Vec<SandboxFencingToken>>,
}

impl FakeSandboxProvider {
    fn ready(
        sandbox_capabilities: impl IntoIterator<Item = RuntimeCapability>,
        sandbox_assurance: IsolationAssurance,
    ) -> Self {
        Self::with_behavior(
            SandboxProviderHealthStatus::Ready,
            sandbox_capabilities,
            sandbox_assurance,
            VecDeque::new(),
            None,
        )
    }

    fn with_behavior(
        sandbox_provider_health: SandboxProviderHealthStatus,
        sandbox_capabilities: impl IntoIterator<Item = RuntimeCapability>,
        sandbox_assurance: IsolationAssurance,
        sandbox_provider_readiness: VecDeque<
            Result<SandboxProviderReadiness, SandboxProviderErrorKind>,
        >,
        fail_sandbox_destroy_call: Option<usize>,
    ) -> Self {
        Self {
            sandbox_provider_descriptor: SandboxProviderDescriptor::new(
                sandbox_provider_id("provider-test"),
                sandbox_provider_kind("test"),
                sandbox_capabilities,
                sandbox_assurance,
            ),
            sandbox_provider_health,
            sandbox_provider_readiness: Mutex::new(sandbox_provider_readiness),
            sandbox_start_delay: None,
            fail_sandbox_destroy_call,
            fail_sandbox_stop_call: None,
            sandbox_health_calls: AtomicUsize::new(0),
            sandbox_allocate_calls: AtomicUsize::new(0),
            sandbox_start_calls: AtomicUsize::new(0),
            sandbox_stop_calls: AtomicUsize::new(0),
            sandbox_destroy_calls: AtomicUsize::new(0),
            sandbox_allocate_requests: Mutex::new(Vec::new()),
            sandbox_start_requests: Mutex::new(Vec::new()),
            sandbox_allocate_fencing_tokens: Mutex::new(Vec::new()),
            sandbox_start_fencing_tokens: Mutex::new(Vec::new()),
            sandbox_stop_fencing_tokens: Mutex::new(Vec::new()),
            sandbox_destroy_fencing_tokens: Mutex::new(Vec::new()),
        }
    }

    fn with_sandbox_start_delay(mut self, sandbox_start_delay: Duration) -> Self {
        self.sandbox_start_delay = Some(sandbox_start_delay);
        self
    }

    fn fail_sandbox_stop_call(mut self, sandbox_stop_call: usize) -> Self {
        self.fail_sandbox_stop_call = Some(sandbox_stop_call);
        self
    }

    fn record_sandbox_fencing_token(
        sandbox_fencing_tokens: &Mutex<Vec<SandboxFencingToken>>,
        sandbox_fencing_token: SandboxFencingToken,
    ) {
        match sandbox_fencing_tokens.lock() {
            Ok(mut sandbox_fencing_tokens) => {
                sandbox_fencing_tokens.push(sandbox_fencing_token);
            }
            Err(poisoned_sandbox_fencing_tokens) => {
                poisoned_sandbox_fencing_tokens
                    .into_inner()
                    .push(sandbox_fencing_token);
            }
        }
    }

    fn record_sandbox_request<T>(sandbox_requests: &Mutex<Vec<T>>, sandbox_request: T) {
        match sandbox_requests.lock() {
            Ok(mut sandbox_requests) => sandbox_requests.push(sandbox_request),
            Err(poisoned_sandbox_requests) => {
                poisoned_sandbox_requests.into_inner().push(sandbox_request);
            }
        }
    }

    fn sandbox_requests<T: Clone>(sandbox_requests: &Mutex<Vec<T>>) -> Vec<T> {
        match sandbox_requests.lock() {
            Ok(sandbox_requests) => sandbox_requests.clone(),
            Err(poisoned_sandbox_requests) => poisoned_sandbox_requests.into_inner().clone(),
        }
    }

    fn sandbox_fencing_tokens(
        sandbox_fencing_tokens: &Mutex<Vec<SandboxFencingToken>>,
    ) -> Vec<SandboxFencingToken> {
        match sandbox_fencing_tokens.lock() {
            Ok(sandbox_fencing_tokens) => sandbox_fencing_tokens.clone(),
            Err(poisoned_sandbox_fencing_tokens) => {
                poisoned_sandbox_fencing_tokens.into_inner().clone()
            }
        }
    }

    fn next_sandbox_provider_readiness(
        &self,
    ) -> Result<SandboxProviderReadiness, SandboxProviderErrorKind> {
        let mut sandbox_provider_outcomes = match self.sandbox_provider_readiness.lock() {
            Ok(sandbox_provider_outcomes) => sandbox_provider_outcomes,
            Err(poisoned_sandbox_provider_outcomes) => {
                poisoned_sandbox_provider_outcomes.into_inner()
            }
        };
        sandbox_provider_outcomes
            .pop_front()
            .unwrap_or(Ok(SandboxProviderReadiness {
                sandbox_provider_ready: true,
                sandbox_policy_enforced: true,
                sandbox_workspace_attached: true,
            }))
    }

    fn sandbox_provider_error(
        &self,
        sandbox_provider_operation: SandboxProviderOperation,
        sandbox_provider_error_kind: SandboxProviderErrorKind,
    ) -> SandboxProviderError {
        SandboxProviderError::new(
            self.sandbox_provider_descriptor
                .sandbox_provider_id()
                .clone(),
            sandbox_provider_operation,
            sandbox_provider_error_kind,
        )
    }
}

#[async_trait]
impl SandboxProvider for FakeSandboxProvider {
    fn sandbox_provider_descriptor(&self) -> &SandboxProviderDescriptor {
        &self.sandbox_provider_descriptor
    }

    async fn sandbox_provider_health(&self) -> SandboxProviderResult<SandboxProviderHealth> {
        self.sandbox_health_calls.fetch_add(1, Ordering::SeqCst);
        Ok(SandboxProviderHealth {
            sandbox_provider_health_status: self.sandbox_provider_health,
        })
    }

    async fn allocate(
        &self,
        sandbox_request: SandboxProviderAllocationRequest,
    ) -> SandboxProviderResult<SandboxProviderAllocation> {
        Self::record_sandbox_request(&self.sandbox_allocate_requests, sandbox_request.clone());
        Self::record_sandbox_fencing_token(
            &self.sandbox_allocate_fencing_tokens,
            sandbox_request.sandbox_fencing_token,
        );
        let sandbox_allocate_call = self.sandbox_allocate_calls.fetch_add(1, Ordering::SeqCst) + 1;
        let sandbox_allocation_reference =
            SandboxProviderAllocationRef::new(format!("allocation-{sandbox_allocate_call}"));
        match sandbox_allocation_reference {
            Ok(sandbox_allocation_reference) => Ok(SandboxProviderAllocation {
                sandbox_allocation_reference,
            }),
            Err(_) => Err(self.sandbox_provider_error(
                SandboxProviderOperation::Allocate,
                SandboxProviderErrorKind::Internal,
            )),
        }
    }

    async fn start(
        &self,
        sandbox_request: SandboxProviderStartRequest,
    ) -> SandboxProviderResult<SandboxProviderReadiness> {
        Self::record_sandbox_request(&self.sandbox_start_requests, sandbox_request.clone());
        Self::record_sandbox_fencing_token(
            &self.sandbox_start_fencing_tokens,
            sandbox_request.sandbox_fencing_token,
        );
        self.sandbox_start_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(sandbox_start_delay) = self.sandbox_start_delay {
            tokio::time::sleep(sandbox_start_delay).await;
        }
        self.next_sandbox_provider_readiness()
            .map_err(|sandbox_provider_error_kind| {
                self.sandbox_provider_error(
                    SandboxProviderOperation::Start,
                    sandbox_provider_error_kind,
                )
            })
    }

    async fn stop(&self, sandbox_request: SandboxProviderStopRequest) -> SandboxProviderResult<()> {
        Self::record_sandbox_fencing_token(
            &self.sandbox_stop_fencing_tokens,
            sandbox_request.sandbox_fencing_token,
        );
        let sandbox_stop_call = self.sandbox_stop_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_sandbox_stop_call == Some(sandbox_stop_call) {
            Err(self.sandbox_provider_error(
                SandboxProviderOperation::Stop,
                SandboxProviderErrorKind::Conflict,
            ))
        } else {
            Ok(())
        }
    }

    async fn destroy(
        &self,
        sandbox_request: SandboxProviderDestroyRequest,
    ) -> SandboxProviderResult<()> {
        Self::record_sandbox_fencing_token(
            &self.sandbox_destroy_fencing_tokens,
            sandbox_request.sandbox_fencing_token,
        );
        let sandbox_destroy_call = self.sandbox_destroy_calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_sandbox_destroy_call == Some(sandbox_destroy_call) {
            Err(self.sandbox_provider_error(
                SandboxProviderOperation::Destroy,
                SandboxProviderErrorKind::Conflict,
            ))
        } else {
            Ok(())
        }
    }
}

fn sandbox_provider_id(value: &str) -> SandboxProviderId {
    SandboxProviderId::parse(value)
        .unwrap_or_else(|error| panic!("invalid test sandbox provider id: {error}"))
}

fn sandbox_provider_kind(value: &str) -> SandboxProviderKind {
    SandboxProviderKind::parse(value)
        .unwrap_or_else(|error| panic!("invalid test sandbox provider kind: {error}"))
}

fn tenant_id(value: &str) -> TenantId {
    TenantId::parse(value).unwrap_or_else(|error| panic!("invalid test tenant id: {error}"))
}

fn sandbox_workspace_id(value: &str) -> SandboxWorkspaceId {
    SandboxWorkspaceId::parse(value)
        .unwrap_or_else(|error| panic!("invalid test sandbox workspace id: {error}"))
}

fn next_sandbox_session_id() -> SandboxSessionId {
    let sandbox_session_sequence = NEXT_SANDBOX_SESSION_ID.fetch_add(1, Ordering::SeqCst);
    SandboxSessionId::parse(format!("session-{sandbox_session_sequence}"))
        .unwrap_or_else(|error| panic!("invalid test sandbox session id: {error}"))
}

fn create_sandbox_session_command(
    tenant_id: TenantId,
    sandbox_capabilities: impl IntoIterator<Item = RuntimeCapability>,
    sandbox_assurance: IsolationAssurance,
) -> CreateSandboxSessionCommand {
    CreateSandboxSessionCommand {
        tenant_id,
        sandbox_workspace_id: sandbox_workspace_id("workspace-a"),
        sandbox_session_id: next_sandbox_session_id(),
        sandbox_operation_id: OperationId::generate(),
        sandbox_required_capabilities: sandbox_capabilities.into_iter().collect(),
        sandbox_minimum_assurance: sandbox_assurance,
    }
}

fn sandbox_session_lifecycle_command(
    sandbox_session: &SandboxSession,
) -> SandboxSessionLifecycleCommand {
    SandboxSessionLifecycleCommand {
        tenant_id: sandbox_session.tenant_id().clone(),
        sandbox_session_id: sandbox_session.sandbox_session_id().clone(),
        sandbox_operation_id: OperationId::generate(),
    }
}

fn sandbox_lifecycle_service_with(
    sandbox_provider: Arc<FakeSandboxProvider>,
) -> SandboxLifecycleService {
    sandbox_lifecycle_service_with_repository(
        Arc::new(TestSandboxSessionRepository::default()),
        sandbox_provider,
    )
}

fn sandbox_lifecycle_service_with_repository(
    sandbox_session_repository: Arc<TestSandboxSessionRepository>,
    sandbox_provider: Arc<FakeSandboxProvider>,
) -> SandboxLifecycleService {
    let sandbox_session_repository: Arc<dyn SandboxSessionRepository> = sandbox_session_repository;
    let sandbox_providers: Vec<Arc<dyn SandboxProvider>> = vec![sandbox_provider];
    SandboxLifecycleService::new(sandbox_session_repository, sandbox_providers)
        .unwrap_or_else(|error| panic!("invalid sandbox lifecycle service: {error}"))
}

fn sandbox_lifecycle_service_with_operation_policy(
    sandbox_session_repository: Arc<TestSandboxSessionRepository>,
    sandbox_provider: Arc<FakeSandboxProvider>,
    sandbox_lease_duration: Duration,
    sandbox_provider_operation_timeout: Duration,
) -> SandboxLifecycleService {
    let sandbox_session_repository: Arc<dyn SandboxSessionRepository> = sandbox_session_repository;
    let sandbox_providers: Vec<Arc<dyn SandboxProvider>> = vec![sandbox_provider];
    SandboxLifecycleService::new_with_sandbox_operation_policy(
        sandbox_session_repository,
        sandbox_providers,
        SandboxLeaseOwnerId::generate(),
        sandbox_lease_duration,
        sandbox_provider_operation_timeout,
    )
    .unwrap_or_else(|error| panic!("invalid sandbox lifecycle service policy: {error}"))
}

fn transient_sandbox_session(
    sandbox_session_id_value: &str,
    sandbox_session_state: SandboxSessionState,
    sandbox_operation_kind: SandboxSessionOperationKind,
    include_sandbox_allocation_reference: bool,
) -> SandboxSession {
    let mut sandbox_runtime_binding = SandboxRuntimeBinding::new_intent(
        SandboxId::generate(),
        SandboxRuntimeBindingId::generate(),
        sandbox_provider_id("provider-test"),
    );
    if include_sandbox_allocation_reference {
        sandbox_runtime_binding.set_sandbox_allocation_reference(
            SandboxProviderAllocationRef::new(format!("allocation-{sandbox_session_id_value}"))
                .unwrap_or_else(|error| {
                    panic!("invalid test sandbox allocation reference: {error}")
                }),
        );
    }
    SandboxSession::restore(
        tenant_id("tenant-a"),
        sandbox_workspace_id("workspace-a"),
        SandboxSessionId::parse(sandbox_session_id_value)
            .unwrap_or_else(|error| panic!("invalid test sandbox session id: {error}")),
        sandbox_session_state,
        BTreeSet::from([RuntimeCapability::Filesystem]),
        IsolationAssurance::HostUser,
        Some(sandbox_runtime_binding),
        None,
        vec![
            SandboxSessionOperation::restore(
                OperationId::generate(),
                SandboxSessionOperationKind::Create,
                SandboxOperationOutcome::Succeeded,
            ),
            SandboxSessionOperation::restore(
                OperationId::generate(),
                sandbox_operation_kind,
                SandboxOperationOutcome::InProgress,
            ),
        ],
        0,
    )
}

async fn create_sandbox_session(
    sandbox_lifecycle_service: &SandboxLifecycleService,
    sandbox_capabilities: impl IntoIterator<Item = RuntimeCapability>,
    sandbox_assurance: IsolationAssurance,
) -> SandboxSession {
    sandbox_lifecycle_service
        .create_sandbox_session(create_sandbox_session_command(
            tenant_id("tenant-a"),
            sandbox_capabilities,
            sandbox_assurance,
        ))
        .await
        .unwrap_or_else(|error| panic!("sandbox session creation failed: {error}"))
}

#[tokio::test]
async fn sandbox_create_rejects_sandbox_session_id_reuse_across_operations() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        sandbox_session_repository.clone(),
        Arc::new(FakeSandboxProvider::ready(
            BTreeSet::from([RuntimeCapability::Filesystem]),
            IsolationAssurance::HostUser,
        )),
    );
    let sandbox_tenant_id = tenant_id("tenant-a");
    let sandbox_workspace_id = sandbox_workspace_id("workspace-a");
    let sandbox_session_id = next_sandbox_session_id();
    let sandbox_operation_id = OperationId::generate();
    let sandbox_required_capabilities = BTreeSet::from([RuntimeCapability::Filesystem]);

    let first_create_command = CreateSandboxSessionCommand {
        tenant_id: sandbox_tenant_id.clone(),
        sandbox_workspace_id: sandbox_workspace_id.clone(),
        sandbox_session_id: sandbox_session_id.clone(),
        sandbox_operation_id: sandbox_operation_id.clone(),
        sandbox_required_capabilities: sandbox_required_capabilities.clone(),
        sandbox_minimum_assurance: IsolationAssurance::HostUser,
    };
    sandbox_lifecycle_service
        .create_sandbox_session(first_create_command)
        .await
        .unwrap_or_else(|error| panic!("sandbox session creation failed: {error}"));

    let reused_sandbox_session_id = sandbox_session_id.clone();
    let reused_sandbox_session_id_command = CreateSandboxSessionCommand {
        tenant_id: sandbox_tenant_id,
        sandbox_workspace_id,
        sandbox_session_id,
        sandbox_operation_id: OperationId::generate(),
        sandbox_required_capabilities,
        sandbox_minimum_assurance: IsolationAssurance::HostUser,
    };
    assert!(matches!(
        sandbox_lifecycle_service
            .create_sandbox_session(reused_sandbox_session_id_command)
            .await,
        Err(SandboxLifecycleError::SandboxSessionIdConflict {
            tenant_id: conflict_tenant_id,
            sandbox_session_id: conflict_sandbox_session_id,
        }) if conflict_tenant_id == tenant_id("tenant-a")
            && conflict_sandbox_session_id == reused_sandbox_session_id
    ));
}

#[tokio::test]
async fn sandbox_create_recovers_when_a_concurrent_identical_create_commits_first() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        sandbox_session_repository.clone(),
        Arc::new(FakeSandboxProvider::ready(
            BTreeSet::from([RuntimeCapability::Filesystem]),
            IsolationAssurance::HostUser,
        )),
    );
    let sandbox_tenant_id = tenant_id("tenant-a");
    let sandbox_workspace_id = sandbox_workspace_id("workspace-a");
    let sandbox_session_id = next_sandbox_session_id();
    let sandbox_operation_id = OperationId::generate();
    let sandbox_required_capabilities = BTreeSet::from([RuntimeCapability::Filesystem]);
    let sandbox_minimum_assurance = IsolationAssurance::HostUser;
    let sandbox_race_winner = SandboxSession::create(
        sandbox_tenant_id.clone(),
        sandbox_workspace_id.clone(),
        sandbox_session_id.clone(),
        sandbox_operation_id.clone(),
        sandbox_required_capabilities.clone(),
        sandbox_minimum_assurance,
    );
    sandbox_session_repository.fail_sandbox_insert_call_with_race_winner(1, sandbox_race_winner);

    let create_command = CreateSandboxSessionCommand {
        tenant_id: sandbox_tenant_id.clone(),
        sandbox_workspace_id: sandbox_workspace_id.clone(),
        sandbox_session_id: sandbox_session_id.clone(),
        sandbox_operation_id: sandbox_operation_id.clone(),
        sandbox_required_capabilities: sandbox_required_capabilities.clone(),
        sandbox_minimum_assurance,
    };
    let recovered_sandbox_session = sandbox_lifecycle_service
        .create_sandbox_session(create_command.clone())
        .await
        .unwrap_or_else(|error| panic!("concurrent identical create must recover: {error}"));
    assert_eq!(
        recovered_sandbox_session.sandbox_session_id(),
        &sandbox_session_id
    );

    let replayed_sandbox_session = sandbox_lifecycle_service
        .create_sandbox_session(create_command)
        .await
        .unwrap_or_else(|error| panic!("later identical create must be idempotent: {error}"));
    assert_eq!(
        replayed_sandbox_session.sandbox_session_id(),
        &sandbox_session_id
    );
    assert_eq!(replayed_sandbox_session.sandbox_version(), 0);
}

#[tokio::test]
async fn sandbox_workspace_context_is_preserved_across_provider_attachment_requests() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with(Arc::clone(&sandbox_provider));
    let expected_tenant_id = tenant_id("tenant-workspace-attachment");
    let expected_sandbox_workspace_id = sandbox_workspace_id("workspace-authorized");
    let expected_sandbox_session_id = next_sandbox_session_id();
    let sandbox_session = sandbox_lifecycle_service
        .create_sandbox_session(CreateSandboxSessionCommand {
            tenant_id: expected_tenant_id.clone(),
            sandbox_workspace_id: expected_sandbox_workspace_id.clone(),
            sandbox_session_id: expected_sandbox_session_id.clone(),
            sandbox_operation_id: OperationId::generate(),
            sandbox_required_capabilities: BTreeSet::from([RuntimeCapability::Filesystem]),
            sandbox_minimum_assurance: IsolationAssurance::HostUser,
        })
        .await
        .unwrap_or_else(|error| panic!("sandbox session creation failed: {error}"));

    let running_sandbox_session = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
        .await
        .unwrap_or_else(|error| panic!("sandbox session start failed: {error}"));
    assert_eq!(
        running_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Running
    );

    let sandbox_allocate_requests =
        FakeSandboxProvider::sandbox_requests(&sandbox_provider.sandbox_allocate_requests);
    let sandbox_start_requests =
        FakeSandboxProvider::sandbox_requests(&sandbox_provider.sandbox_start_requests);
    assert_eq!(sandbox_allocate_requests.len(), 1);
    assert_eq!(sandbox_start_requests.len(), 1);

    let sandbox_allocate_request = &sandbox_allocate_requests[0];
    let sandbox_start_request = &sandbox_start_requests[0];
    assert_eq!(sandbox_allocate_request.tenant_id, expected_tenant_id);
    assert_eq!(
        sandbox_allocate_request.sandbox_workspace_id,
        expected_sandbox_workspace_id
    );
    assert_eq!(
        sandbox_allocate_request.sandbox_session_id,
        expected_sandbox_session_id
    );
    assert_eq!(
        sandbox_start_request.sandbox_workspace_id,
        sandbox_allocate_request.sandbox_workspace_id
    );
    assert_eq!(
        sandbox_start_request.sandbox_session_id,
        sandbox_allocate_request.sandbox_session_id
    );
    assert_eq!(
        sandbox_start_request.sandbox_id,
        sandbox_allocate_request.sandbox_id
    );
    assert_eq!(
        sandbox_start_request.sandbox_runtime_binding_id,
        sandbox_allocate_request.sandbox_runtime_binding_id
    );
    assert_eq!(
        sandbox_start_request.sandbox_fencing_token,
        sandbox_allocate_request.sandbox_fencing_token
    );
}

#[tokio::test]
async fn sandbox_lifecycle_commands_are_idempotent_without_duplicate_provider_effects() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with(Arc::clone(&sandbox_provider));
    let sandbox_create_command = create_sandbox_session_command(
        tenant_id("tenant-a"),
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    );
    let expected_sandbox_session_id = sandbox_create_command.sandbox_session_id.clone();
    let sandbox_session = sandbox_lifecycle_service
        .create_sandbox_session(sandbox_create_command.clone())
        .await
        .unwrap_or_else(|error| panic!("sandbox session creation failed: {error}"));
    assert_eq!(
        sandbox_session.sandbox_session_id(),
        &expected_sandbox_session_id
    );
    let replayed_sandbox_session = sandbox_lifecycle_service
        .create_sandbox_session(sandbox_create_command)
        .await;
    assert!(matches!(
        replayed_sandbox_session,
        Ok(ref replayed_sandbox_session)
            if replayed_sandbox_session.sandbox_session_id()
                == sandbox_session.sandbox_session_id()
    ));

    let sandbox_start_command = sandbox_session_lifecycle_command(&sandbox_session);
    let running_sandbox_session = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_start_command.clone())
        .await
        .unwrap_or_else(|error| panic!("sandbox session start failed: {error}"));
    assert_eq!(
        running_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Running
    );
    assert!(running_sandbox_session.sandbox_runtime_binding().is_some());
    assert!(sandbox_lifecycle_service
        .start_sandbox_session(sandbox_start_command)
        .await
        .is_ok());
    assert_eq!(
        sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        sandbox_provider.sandbox_start_calls.load(Ordering::SeqCst),
        1
    );
    let sandbox_allocate_fencing_tokens = FakeSandboxProvider::sandbox_fencing_tokens(
        &sandbox_provider.sandbox_allocate_fencing_tokens,
    );
    let sandbox_start_fencing_tokens =
        FakeSandboxProvider::sandbox_fencing_tokens(&sandbox_provider.sandbox_start_fencing_tokens);
    assert_eq!(sandbox_allocate_fencing_tokens.len(), 1);
    assert_eq!(
        sandbox_allocate_fencing_tokens,
        sandbox_start_fencing_tokens
    );

    let sandbox_stop_command = sandbox_session_lifecycle_command(&running_sandbox_session);
    let stopped_sandbox_session = sandbox_lifecycle_service
        .stop_sandbox_session(sandbox_stop_command.clone())
        .await
        .unwrap_or_else(|error| panic!("sandbox session stop failed: {error}"));
    assert_eq!(
        stopped_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Stopped
    );
    assert!(sandbox_lifecycle_service
        .stop_sandbox_session(sandbox_stop_command)
        .await
        .is_ok());
    assert_eq!(
        sandbox_provider.sandbox_stop_calls.load(Ordering::SeqCst),
        1
    );
    let sandbox_stop_fencing_tokens =
        FakeSandboxProvider::sandbox_fencing_tokens(&sandbox_provider.sandbox_stop_fencing_tokens);
    assert_eq!(sandbox_stop_fencing_tokens.len(), 1);
    assert!(sandbox_stop_fencing_tokens[0] > sandbox_start_fencing_tokens[0]);

    let sandbox_destroy_command = sandbox_session_lifecycle_command(&stopped_sandbox_session);
    let destroyed_sandbox_session = sandbox_lifecycle_service
        .destroy_sandbox_session(sandbox_destroy_command.clone())
        .await
        .unwrap_or_else(|error| panic!("sandbox session destroy failed: {error}"));
    assert_eq!(
        destroyed_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Destroyed
    );
    assert!(destroyed_sandbox_session
        .sandbox_runtime_binding()
        .is_none());
    assert!(sandbox_lifecycle_service
        .destroy_sandbox_session(sandbox_destroy_command)
        .await
        .is_ok());
    assert_eq!(
        sandbox_provider
            .sandbox_destroy_calls
            .load(Ordering::SeqCst),
        1
    );
    let sandbox_destroy_fencing_tokens = FakeSandboxProvider::sandbox_fencing_tokens(
        &sandbox_provider.sandbox_destroy_fencing_tokens,
    );
    assert_eq!(sandbox_destroy_fencing_tokens.len(), 1);
    assert!(sandbox_destroy_fencing_tokens[0] > sandbox_stop_fencing_tokens[0]);
}

#[tokio::test]
async fn sandbox_stop_provider_failure_records_failed_operation_and_keeps_binding() {
    let sandbox_provider = Arc::new(
        FakeSandboxProvider::ready(
            [RuntimeCapability::Filesystem],
            IsolationAssurance::HostUser,
        )
        .fail_sandbox_stop_call(1),
    );
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with(Arc::clone(&sandbox_provider));
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;
    let running_sandbox_session = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
        .await
        .unwrap_or_else(|error| panic!("sandbox session start failed: {error}"));

    let sandbox_stop_command = sandbox_session_lifecycle_command(&running_sandbox_session);
    let sandbox_operation_id = sandbox_stop_command.sandbox_operation_id.clone();
    let failed_sandbox_session = sandbox_lifecycle_service
        .stop_sandbox_session(sandbox_stop_command)
        .await;
    assert!(matches!(
        failed_sandbox_session,
        Err(SandboxLifecycleError::Provider(_))
    ));
    let failed_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(
            running_sandbox_session.tenant_id(),
            running_sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("failed sandbox session read: {error}"));
    assert_eq!(
        failed_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Failed
    );
    assert_eq!(
        failed_sandbox_session.sandbox_last_failure(),
        Some(SandboxSessionFailure::Provider)
    );
    assert!(failed_sandbox_session
        .sandbox_operations()
        .iter()
        .any(|sandbox_operation| sandbox_operation.sandbox_operation_id()
            == &sandbox_operation_id
            && sandbox_operation.sandbox_operation_outcome()
                == SandboxOperationOutcome::Failed(SandboxSessionFailure::Provider)));
    // The provider allocation still exists, so the runtime binding with its
    // allocation reference is retained for a later destroy cleanup.
    assert!(failed_sandbox_session
        .sandbox_runtime_binding()
        .is_some_and(|sandbox_runtime_binding| sandbox_runtime_binding
            .sandbox_allocation_reference()
            .is_some()));
    assert_eq!(
        sandbox_provider.sandbox_stop_calls.load(Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn sandbox_destroy_provider_failure_records_cleanup_failure_and_keeps_binding() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::with_behavior(
        SandboxProviderHealthStatus::Ready,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
        VecDeque::new(),
        Some(1),
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with(Arc::clone(&sandbox_provider));
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;
    let running_sandbox_session = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
        .await
        .unwrap_or_else(|error| panic!("sandbox session start failed: {error}"));
    let stopped_sandbox_session = sandbox_lifecycle_service
        .stop_sandbox_session(sandbox_session_lifecycle_command(&running_sandbox_session))
        .await
        .unwrap_or_else(|error| panic!("sandbox session stop failed: {error}"));

    let sandbox_destroy_command = sandbox_session_lifecycle_command(&stopped_sandbox_session);
    let sandbox_operation_id = sandbox_destroy_command.sandbox_operation_id.clone();
    let failed_destroy_result = sandbox_lifecycle_service
        .destroy_sandbox_session(sandbox_destroy_command)
        .await;
    assert!(matches!(
        failed_destroy_result,
        Err(SandboxLifecycleError::Provider(_))
    ));
    let failed_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(
            stopped_sandbox_session.tenant_id(),
            stopped_sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("failed sandbox session read: {error}"));
    assert_eq!(
        failed_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Failed
    );
    assert_eq!(
        failed_sandbox_session.sandbox_last_failure(),
        Some(SandboxSessionFailure::Cleanup)
    );
    assert!(failed_sandbox_session
        .sandbox_operations()
        .iter()
        .any(|sandbox_operation| sandbox_operation.sandbox_operation_id()
            == &sandbox_operation_id
            && sandbox_operation.sandbox_operation_outcome()
                == SandboxOperationOutcome::Failed(SandboxSessionFailure::Cleanup)));
    // Cleanup failed, so the allocation is retained for a retried destroy.
    assert!(failed_sandbox_session
        .sandbox_runtime_binding()
        .is_some_and(|sandbox_runtime_binding| sandbox_runtime_binding
            .sandbox_allocation_reference()
            .is_some()));
    assert_eq!(
        sandbox_provider
            .sandbox_destroy_calls
            .load(Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn sandbox_provider_selection_fails_closed_for_capability_assurance_and_health() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::with_behavior(
        SandboxProviderHealthStatus::Degraded,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
        VecDeque::new(),
        None,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with(sandbox_provider);

    let missing_capability_sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Terminal],
        IsolationAssurance::HostUser,
    )
    .await;
    let missing_capability_result = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(
            &missing_capability_sandbox_session,
        ))
        .await;
    assert!(matches!(
        missing_capability_result,
        Err(SandboxLifecycleError::NoEligibleProvider)
    ));

    let weak_assurance_sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::Container,
    )
    .await;
    let weak_assurance_result = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(
            &weak_assurance_sandbox_session,
        ))
        .await;
    assert!(matches!(
        weak_assurance_result,
        Err(SandboxLifecycleError::NoEligibleProvider)
    ));

    let unhealthy_sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;
    let unhealthy_result = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(
            &unhealthy_sandbox_session,
        ))
        .await;
    assert!(matches!(
        unhealthy_result,
        Err(SandboxLifecycleError::NoHealthyProvider)
    ));
}

#[tokio::test]
async fn sandbox_readiness_gate_cleans_binding_and_records_failed_operation() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::with_behavior(
        SandboxProviderHealthStatus::Ready,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
        VecDeque::from([Ok(SandboxProviderReadiness {
            sandbox_provider_ready: true,
            sandbox_policy_enforced: false,
            sandbox_workspace_attached: true,
        })]),
        None,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with(Arc::clone(&sandbox_provider));
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;
    let sandbox_start_command = sandbox_session_lifecycle_command(&sandbox_session);
    let sandbox_start_result = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_start_command.clone())
        .await;
    assert!(matches!(
        sandbox_start_result,
        Err(SandboxLifecycleError::ProviderReadinessRejected { .. })
    ));
    let failed_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(
            sandbox_session.tenant_id(),
            sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("failed sandbox session lookup: {error}"));
    assert_eq!(
        failed_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Failed
    );
    assert_eq!(
        failed_sandbox_session.sandbox_last_failure(),
        Some(SandboxSessionFailure::Readiness)
    );
    assert!(failed_sandbox_session.sandbox_runtime_binding().is_none());
    assert_eq!(
        sandbox_provider
            .sandbox_destroy_calls
            .load(Ordering::SeqCst),
        1
    );
    let sandbox_replay_result = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_start_command)
        .await;
    assert!(matches!(
        sandbox_replay_result,
        Err(SandboxLifecycleError::OperationPreviouslyFailed {
            sandbox_session_failure: SandboxSessionFailure::Readiness,
            ..
        })
    ));
}

#[tokio::test]
async fn sandbox_retry_start_releases_failed_binding_before_allocating_again() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::with_behavior(
        SandboxProviderHealthStatus::Ready,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
        VecDeque::from([
            Err(SandboxProviderErrorKind::Unavailable),
            Ok(SandboxProviderReadiness {
                sandbox_provider_ready: true,
                sandbox_policy_enforced: true,
                sandbox_workspace_attached: true,
            }),
        ]),
        Some(1),
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with(Arc::clone(&sandbox_provider));
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;

    let first_sandbox_start_result = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
        .await;
    assert!(matches!(
        first_sandbox_start_result,
        Err(SandboxLifecycleError::Provider(_))
    ));
    let failed_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(
            sandbox_session.tenant_id(),
            sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("failed sandbox session lookup: {error}"));
    assert_eq!(
        failed_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Failed
    );
    assert_eq!(
        failed_sandbox_session.sandbox_last_failure(),
        Some(SandboxSessionFailure::Cleanup)
    );
    assert!(failed_sandbox_session.sandbox_runtime_binding().is_some());

    let retried_sandbox_session = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(&failed_sandbox_session))
        .await
        .unwrap_or_else(|error| panic!("sandbox retry start failed: {error}"));
    assert_eq!(
        retried_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Running
    );
    assert_eq!(
        sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst),
        2
    );
    assert_eq!(
        sandbox_provider.sandbox_start_calls.load(Ordering::SeqCst),
        2
    );
    assert_eq!(
        sandbox_provider
            .sandbox_destroy_calls
            .load(Ordering::SeqCst),
        2
    );
}

#[tokio::test]
async fn sandbox_start_persists_recoverable_binding_intent_before_provider_allocation() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::with_behavior(
        SandboxProviderHealthStatus::Ready,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
        VecDeque::from([
            Err(SandboxProviderErrorKind::Unavailable),
            Ok(SandboxProviderReadiness {
                sandbox_provider_ready: true,
                sandbox_policy_enforced: true,
                sandbox_workspace_attached: true,
            }),
        ]),
        Some(1),
    ));
    let sandbox_session_repository =
        Arc::new(TestSandboxSessionRepository::enforcing_recoverable_starting_transition());
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        sandbox_session_repository,
        sandbox_provider.clone(),
    );
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;

    let first_sandbox_start_result = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
        .await;
    assert!(matches!(
        first_sandbox_start_result,
        Err(SandboxLifecycleError::Provider(_))
    ));
    let failed_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(
            sandbox_session.tenant_id(),
            sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("failed sandbox session lookup: {error}"));
    assert_eq!(
        failed_sandbox_session.sandbox_last_failure(),
        Some(SandboxSessionFailure::Cleanup)
    );

    let retried_sandbox_session = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(&failed_sandbox_session))
        .await
        .unwrap_or_else(|error| panic!("sandbox retry start failed: {error}"));

    assert_eq!(
        retried_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Running
    );
    assert_eq!(
        sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst),
        2
    );
    assert_eq!(
        sandbox_provider
            .sandbox_destroy_calls
            .load(Ordering::SeqCst),
        2
    );
}

#[tokio::test]
async fn sandbox_reconciler_recovers_after_allocation_persistence_failure() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_session_repository =
        Arc::new(TestSandboxSessionRepository::enforcing_recoverable_starting_transition());
    sandbox_session_repository.fail_sandbox_save_call(2);
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        sandbox_session_repository,
        sandbox_provider.clone(),
    );
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;

    let sandbox_start_result = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
        .await;
    assert!(matches!(
        sandbox_start_result,
        Err(SandboxLifecycleError::Repository(
            SandboxSessionRepositoryError::Unavailable
        ))
    ));

    let persisted_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(
            sandbox_session.tenant_id(),
            sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("persisted sandbox session lookup failed: {error}"));
    assert_eq!(
        persisted_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Starting
    );
    let persisted_sandbox_runtime_binding = persisted_sandbox_session
        .sandbox_runtime_binding()
        .unwrap_or_else(|| panic!("starting sandbox session must retain its binding intent"));
    assert!(persisted_sandbox_runtime_binding
        .sandbox_allocation_reference()
        .is_none());
    assert_eq!(
        sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        sandbox_provider
            .sandbox_destroy_calls
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        sandbox_provider.sandbox_start_calls.load(Ordering::SeqCst),
        0
    );

    let sandbox_reconciliation_page = sandbox_lifecycle_service
        .reconcile_sandbox_sessions(sandbox_session.tenant_id(), None, 10)
        .await
        .unwrap_or_else(|error| panic!("sandbox reconciliation failed: {error}"));
    assert_eq!(sandbox_reconciliation_page.sandbox_items().len(), 1);
    assert_eq!(
        sandbox_reconciliation_page.sandbox_items()[0].sandbox_reconciliation_outcome(),
        SandboxSessionReconciliationOutcome::Reconciled
    );

    let recovered_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(
            sandbox_session.tenant_id(),
            sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("recovered sandbox session lookup failed: {error}"));
    assert_eq!(
        recovered_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Running
    );
    assert_eq!(
        sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst),
        2
    );
    assert_eq!(
        sandbox_provider.sandbox_start_calls.load(Ordering::SeqCst),
        1
    );
    let sandbox_start_requests =
        FakeSandboxProvider::sandbox_requests(&sandbox_provider.sandbox_start_requests);
    assert_eq!(sandbox_start_requests.len(), 1);
    assert_eq!(
        sandbox_start_requests[0]
            .sandbox_allocation_reference
            .expose_to_provider(),
        "allocation-2"
    );
    assert_eq!(sandbox_start_requests[0].sandbox_fencing_token.value(), 2);
}

#[tokio::test]
async fn sandbox_tenant_scope_and_invalid_transitions_are_enforced() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        BTreeSet::new(),
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with(sandbox_provider);
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        BTreeSet::new(),
        IsolationAssurance::HostUser,
    )
    .await;

    let hidden_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(&tenant_id("tenant-b"), sandbox_session.sandbox_session_id())
        .await;
    assert!(matches!(
        hidden_sandbox_session,
        Err(SandboxLifecycleError::SandboxSessionNotFound { .. })
    ));

    let reused_sandbox_create_operation = SandboxSessionLifecycleCommand {
        tenant_id: sandbox_session.tenant_id().clone(),
        sandbox_session_id: sandbox_session.sandbox_session_id().clone(),
        sandbox_operation_id: sandbox_session.sandbox_operations()[0]
            .sandbox_operation_id()
            .clone(),
    };
    let sandbox_operation_conflict = sandbox_lifecycle_service
        .stop_sandbox_session(reused_sandbox_create_operation)
        .await;
    assert!(matches!(
        sandbox_operation_conflict,
        Err(SandboxLifecycleError::IdempotencyConflict { .. })
    ));

    let stop_created_sandbox_result = sandbox_lifecycle_service
        .stop_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
        .await;
    assert!(matches!(
        stop_created_sandbox_result,
        Err(SandboxLifecycleError::InvalidTransition {
            sandbox_session_state: SandboxSessionState::Created,
            ..
        })
    ));
}

#[tokio::test]
async fn sandbox_reconciler_recovers_transient_sessions_with_bounded_pagination() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    for sandbox_session in [
        transient_sandbox_session(
            "reconcile-1",
            SandboxSessionState::Starting,
            SandboxSessionOperationKind::Start,
            false,
        ),
        transient_sandbox_session(
            "reconcile-2",
            SandboxSessionState::Stopping,
            SandboxSessionOperationKind::Stop,
            true,
        ),
        transient_sandbox_session(
            "reconcile-3",
            SandboxSessionState::Destroying,
            SandboxSessionOperationKind::Destroy,
            true,
        ),
    ] {
        sandbox_session_repository
            .insert_sandbox_session(sandbox_session)
            .await
            .unwrap_or_else(|error| panic!("transient sandbox session insert failed: {error}"));
    }
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        Arc::clone(&sandbox_provider),
    );
    let tenant_id = tenant_id("tenant-a");

    let first_sandbox_page = sandbox_lifecycle_service
        .reconcile_sandbox_sessions(&tenant_id, None, 2)
        .await
        .unwrap_or_else(|error| panic!("first sandbox reconciliation page failed: {error}"));
    assert_eq!(first_sandbox_page.sandbox_items().len(), 2);
    assert!(first_sandbox_page
        .sandbox_items()
        .iter()
        .all(|sandbox_item| {
            sandbox_item.sandbox_reconciliation_outcome()
                == SandboxSessionReconciliationOutcome::Reconciled
        }));
    let next_sandbox_session_id = first_sandbox_page
        .next_sandbox_session_id()
        .cloned()
        .unwrap_or_else(|| panic!("first sandbox reconciliation page must have a cursor"));
    assert_eq!(next_sandbox_session_id.as_str(), "reconcile-2");

    let second_sandbox_page = sandbox_lifecycle_service
        .reconcile_sandbox_sessions(&tenant_id, Some(&next_sandbox_session_id), 2)
        .await
        .unwrap_or_else(|error| panic!("second sandbox reconciliation page failed: {error}"));
    assert_eq!(second_sandbox_page.sandbox_items().len(), 1);
    assert_eq!(
        second_sandbox_page.sandbox_items()[0].sandbox_reconciliation_outcome(),
        SandboxSessionReconciliationOutcome::Reconciled
    );
    assert!(second_sandbox_page.next_sandbox_session_id().is_none());

    for (sandbox_session_id_value, expected_sandbox_session_state) in [
        ("reconcile-1", SandboxSessionState::Running),
        ("reconcile-2", SandboxSessionState::Stopped),
        ("reconcile-3", SandboxSessionState::Destroyed),
    ] {
        let sandbox_session_id = SandboxSessionId::parse(sandbox_session_id_value)
            .unwrap_or_else(|error| panic!("invalid expected sandbox session id: {error}"));
        let sandbox_session = sandbox_session_repository
            .get_sandbox_session(&tenant_id, &sandbox_session_id)
            .await
            .unwrap_or_else(|error| panic!("reconciled sandbox session lookup failed: {error}"))
            .unwrap_or_else(|| panic!("reconciled sandbox session must exist"));
        assert_eq!(
            sandbox_session.sandbox_session_state(),
            expected_sandbox_session_state
        );
        assert_eq!(
            sandbox_session
                .sandbox_operations()
                .last()
                .map(SandboxSessionOperation::sandbox_operation_outcome),
            Some(SandboxOperationOutcome::Succeeded)
        );
    }

    assert_eq!(
        sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        sandbox_provider.sandbox_start_calls.load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        sandbox_provider.sandbox_stop_calls.load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        sandbox_provider
            .sandbox_destroy_calls
            .load(Ordering::SeqCst),
        1
    );
    assert_eq!(
        FakeSandboxProvider::sandbox_fencing_tokens(
            &sandbox_provider.sandbox_allocate_fencing_tokens
        ),
        FakeSandboxProvider::sandbox_fencing_tokens(&sandbox_provider.sandbox_start_fencing_tokens)
    );
}

#[tokio::test]
async fn sandbox_reconciler_omits_a_cursor_when_the_final_page_is_exactly_full() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    for sandbox_session in [
        transient_sandbox_session(
            "reconcile-exact-1",
            SandboxSessionState::Starting,
            SandboxSessionOperationKind::Start,
            false,
        ),
        transient_sandbox_session(
            "reconcile-exact-2",
            SandboxSessionState::Starting,
            SandboxSessionOperationKind::Start,
            false,
        ),
    ] {
        sandbox_session_repository
            .insert_sandbox_session(sandbox_session)
            .await
            .unwrap_or_else(|error| panic!("transient sandbox session insert failed: {error}"));
    }
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        sandbox_provider,
    );

    let sandbox_page = sandbox_lifecycle_service
        .reconcile_sandbox_sessions(&tenant_id("tenant-a"), None, 2)
        .await
        .unwrap_or_else(|error| panic!("exact sandbox reconciliation page failed: {error}"));

    assert_eq!(sandbox_page.sandbox_items().len(), 2);
    assert!(sandbox_page.next_sandbox_session_id().is_none());
}

#[tokio::test]
async fn sandbox_reconciler_rejects_invalid_page_sizes_before_repository_access() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service =
        sandbox_lifecycle_service_with_repository(sandbox_session_repository, sandbox_provider);

    for sandbox_page_size in [0, 201] {
        assert!(matches!(
            sandbox_lifecycle_service
                .reconcile_sandbox_sessions(&tenant_id("tenant-a"), None, sandbox_page_size)
                .await,
            Err(SandboxLifecycleError::Repository(
                SandboxSessionRepositoryError::InvalidPageRequest
            ))
        ));
    }
}

#[tokio::test]
async fn sandbox_reconciler_skips_an_actively_leased_session() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let sandbox_session = transient_sandbox_session(
        "reconcile-leased",
        SandboxSessionState::Starting,
        SandboxSessionOperationKind::Start,
        false,
    );
    let sandbox_session_id = sandbox_session.sandbox_session_id().clone();
    let tenant_id = sandbox_session.tenant_id().clone();
    sandbox_session_repository
        .insert_sandbox_session(sandbox_session)
        .await
        .unwrap_or_else(|error| panic!("transient sandbox session insert failed: {error}"));
    let competing_sandbox_session_lease = sandbox_session_repository
        .acquire_sandbox_session_lease(
            &tenant_id,
            &sandbox_session_id,
            &SandboxLeaseOwnerId::generate(),
            Duration::from_secs(30),
        )
        .await
        .unwrap_or_else(|error| panic!("competing sandbox lease acquisition failed: {error}"))
        .unwrap_or_else(|| panic!("competing sandbox lease must be acquired"));
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        Arc::clone(&sandbox_provider),
    );

    let sandbox_page = sandbox_lifecycle_service
        .reconcile_sandbox_sessions(&tenant_id, None, 20)
        .await
        .unwrap_or_else(|error| panic!("sandbox reconciliation failed: {error}"));
    assert_eq!(sandbox_page.sandbox_items().len(), 1);
    assert_eq!(
        sandbox_page.sandbox_items()[0].sandbox_reconciliation_outcome(),
        SandboxSessionReconciliationOutcome::LeaseUnavailable
    );
    assert_eq!(
        sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst),
        0
    );
    assert!(sandbox_session_repository
        .release_sandbox_session_lease(&competing_sandbox_session_lease)
        .await
        .unwrap_or_else(|error| panic!("competing sandbox lease release failed: {error}")));
}

#[tokio::test]
async fn sandbox_reconciler_preserves_provider_failure_when_sandbox_lease_release_fails() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let sandbox_session = transient_sandbox_session(
        "reconcile-failure",
        SandboxSessionState::Starting,
        SandboxSessionOperationKind::Start,
        true,
    );
    let sandbox_session_id = sandbox_session.sandbox_session_id().clone();
    let tenant_id = sandbox_session.tenant_id().clone();
    sandbox_session_repository
        .insert_sandbox_session(sandbox_session)
        .await
        .unwrap_or_else(|error| panic!("transient sandbox session insert failed: {error}"));
    sandbox_session_repository.fail_sandbox_release_call(1);
    let sandbox_provider = Arc::new(FakeSandboxProvider::with_behavior(
        SandboxProviderHealthStatus::Ready,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
        VecDeque::from([Err(SandboxProviderErrorKind::Unavailable)]),
        None,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        Arc::clone(&sandbox_provider),
    );

    let sandbox_page = sandbox_lifecycle_service
        .reconcile_sandbox_sessions(&tenant_id, None, 20)
        .await
        .unwrap_or_else(|error| panic!("sandbox reconciliation failed: {error}"));
    assert_eq!(sandbox_page.sandbox_items().len(), 1);
    assert_eq!(
        sandbox_page.sandbox_items()[0].sandbox_reconciliation_outcome(),
        SandboxSessionReconciliationOutcome::Failed
    );
    let failed_sandbox_session = sandbox_session_repository
        .get_sandbox_session(&tenant_id, &sandbox_session_id)
        .await
        .unwrap_or_else(|error| panic!("failed sandbox session lookup failed: {error}"))
        .unwrap_or_else(|| panic!("failed sandbox session must exist"));
    assert_eq!(
        failed_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Failed
    );
    assert_eq!(
        failed_sandbox_session.sandbox_last_failure(),
        Some(SandboxSessionFailure::Provider)
    );
    assert!(failed_sandbox_session.sandbox_runtime_binding().is_none());
    assert_eq!(
        FakeSandboxProvider::sandbox_fencing_tokens(&sandbox_provider.sandbox_start_fencing_tokens),
        FakeSandboxProvider::sandbox_fencing_tokens(
            &sandbox_provider.sandbox_destroy_fencing_tokens
        )
    );
}

#[tokio::test]
async fn sandbox_reconciler_reloads_authoritative_session_after_acquiring_the_sandbox_lease() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let stale_sandbox_session = transient_sandbox_session(
        "reconcile-stale",
        SandboxSessionState::Starting,
        SandboxSessionOperationKind::Start,
        true,
    );
    let sandbox_operation_id = stale_sandbox_session.sandbox_operations()[1]
        .sandbox_operation_id()
        .clone();
    let mut authoritative_sandbox_session = stale_sandbox_session.clone();
    authoritative_sandbox_session
        .transition_sandbox_session(
            SandboxSessionState::Running,
            SandboxSessionOperationKind::Start,
        )
        .unwrap_or_else(|error| panic!("sandbox transition to running failed: {error}"));
    authoritative_sandbox_session.complete_sandbox_operation(&sandbox_operation_id);
    authoritative_sandbox_session
        .next_sandbox_version()
        .unwrap_or_else(|error| panic!("sandbox version increment failed: {error}"));
    let tenant_id = authoritative_sandbox_session.tenant_id().clone();
    sandbox_session_repository
        .insert_sandbox_session(authoritative_sandbox_session)
        .await
        .unwrap_or_else(|error| panic!("authoritative sandbox session insert failed: {error}"));
    sandbox_session_repository.return_sandbox_reconciliation_page_once(vec![stale_sandbox_session]);
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        Arc::clone(&sandbox_provider),
    );

    let sandbox_page = sandbox_lifecycle_service
        .reconcile_sandbox_sessions(&tenant_id, None, 20)
        .await
        .unwrap_or_else(|error| panic!("sandbox reconciliation failed: {error}"));

    assert_eq!(sandbox_page.sandbox_items().len(), 1);
    assert_eq!(
        sandbox_page.sandbox_items()[0].sandbox_session_state(),
        SandboxSessionState::Running
    );
    assert_eq!(
        sandbox_page.sandbox_items()[0].sandbox_reconciliation_outcome(),
        SandboxSessionReconciliationOutcome::Reconciled
    );
    assert_eq!(
        sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst),
        0
    );
    assert_eq!(
        sandbox_provider.sandbox_start_calls.load(Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn successful_sandbox_lifecycle_maps_sandbox_lease_release_failure_to_lease_lost() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    sandbox_session_repository.fail_sandbox_release_call(1);
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        sandbox_provider,
    );
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;

    assert!(matches!(
        sandbox_lifecycle_service
            .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
            .await,
        Err(SandboxLifecycleError::LeaseLost)
    ));
    let stored_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(
            sandbox_session.tenant_id(),
            sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("running sandbox session lookup failed: {error}"));
    assert_eq!(
        stored_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Running
    );
}

#[tokio::test]
async fn sandbox_provider_call_maps_sandbox_lease_renewal_failure_to_lease_lost() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    sandbox_session_repository.fail_sandbox_renew_call(1);
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        Arc::clone(&sandbox_provider),
    );
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;

    assert!(matches!(
        sandbox_lifecycle_service
            .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
            .await,
        Err(SandboxLifecycleError::LeaseLost)
    ));
    let stored_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(
            sandbox_session.tenant_id(),
            sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("starting sandbox session lookup failed: {error}"));
    assert_eq!(
        stored_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Starting
    );
    assert_eq!(
        sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn sandbox_persistence_maps_sandbox_lease_conflict_to_lease_lost() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    sandbox_session_repository
        .fail_sandbox_save_call_with_error(1, SandboxSessionRepositoryError::LeaseConflict);
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        Arc::clone(&sandbox_provider),
    );
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;

    assert!(matches!(
        sandbox_lifecycle_service
            .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
            .await,
        Err(SandboxLifecycleError::LeaseLost)
    ));
    let stored_sandbox_session = sandbox_lifecycle_service
        .get_sandbox_session(
            sandbox_session.tenant_id(),
            sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("created sandbox session lookup failed: {error}"));
    assert_eq!(
        stored_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Created
    );
    assert_eq!(
        sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn sandbox_provider_timeout_is_bounded_and_persisted_as_a_typed_failure() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let sandbox_provider = Arc::new(
        FakeSandboxProvider::ready(
            [RuntimeCapability::Filesystem],
            IsolationAssurance::HostUser,
        )
        .with_sandbox_start_delay(Duration::from_millis(25)),
    );
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_operation_policy(
        Arc::clone(&sandbox_session_repository),
        Arc::clone(&sandbox_provider),
        Duration::from_millis(100),
        Duration::from_millis(5),
    );
    let sandbox_session = create_sandbox_session(
        &sandbox_lifecycle_service,
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    )
    .await;

    let sandbox_start_result = sandbox_lifecycle_service
        .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
        .await;
    assert!(matches!(
        sandbox_start_result,
        Err(SandboxLifecycleError::Provider(ref sandbox_provider_error))
            if sandbox_provider_error.sandbox_provider_error_kind()
                == SandboxProviderErrorKind::Timeout
                && sandbox_provider_error.sandbox_provider_operation()
                    == SandboxProviderOperation::Start
    ));
    let failed_sandbox_session = sandbox_session_repository
        .get_sandbox_session(
            sandbox_session.tenant_id(),
            sandbox_session.sandbox_session_id(),
        )
        .await
        .unwrap_or_else(|error| panic!("timed-out sandbox session lookup failed: {error}"))
        .unwrap_or_else(|| panic!("timed-out sandbox session must exist"));
    assert_eq!(
        failed_sandbox_session.sandbox_session_state(),
        SandboxSessionState::Failed
    );
    assert_eq!(
        failed_sandbox_session.sandbox_last_failure(),
        Some(SandboxSessionFailure::Provider)
    );
    assert!(failed_sandbox_session.sandbox_runtime_binding().is_none());
    let sandbox_allocate_fencing_tokens = FakeSandboxProvider::sandbox_fencing_tokens(
        &sandbox_provider.sandbox_allocate_fencing_tokens,
    );
    assert_eq!(
        sandbox_allocate_fencing_tokens,
        FakeSandboxProvider::sandbox_fencing_tokens(&sandbox_provider.sandbox_start_fencing_tokens)
    );
    assert_eq!(
        sandbox_allocate_fencing_tokens,
        FakeSandboxProvider::sandbox_fencing_tokens(
            &sandbox_provider.sandbox_destroy_fencing_tokens
        )
    );
}

#[test]
fn sandbox_lifecycle_service_rejects_invalid_operation_policy_and_duplicate_providers() {
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_lease_owner_id = SandboxLeaseOwnerId::generate();

    for sandbox_invalid_lease_duration in [Duration::from_millis(0), Duration::from_secs(301)] {
        assert!(matches!(
            SandboxLifecycleService::new_with_sandbox_operation_policy(
                Arc::clone(&sandbox_session_repository) as Arc<dyn SandboxSessionRepository>,
                vec![Arc::clone(&sandbox_provider) as Arc<dyn SandboxProvider>],
                sandbox_lease_owner_id.clone(),
                sandbox_invalid_lease_duration,
                Duration::from_secs(30),
            ),
            Err(SandboxLifecycleError::InvariantViolation(_))
        ));
    }
    for sandbox_invalid_provider_timeout in [Duration::from_millis(0), Duration::from_secs(31)] {
        assert!(matches!(
            SandboxLifecycleService::new_with_sandbox_operation_policy(
                Arc::clone(&sandbox_session_repository) as Arc<dyn SandboxSessionRepository>,
                vec![Arc::clone(&sandbox_provider) as Arc<dyn SandboxProvider>],
                sandbox_lease_owner_id.clone(),
                Duration::from_secs(60),
                sandbox_invalid_provider_timeout,
            ),
            Err(SandboxLifecycleError::InvariantViolation(_))
        ));
    }
    assert!(matches!(
        SandboxLifecycleService::new_with_sandbox_operation_policy(
            Arc::clone(&sandbox_session_repository) as Arc<dyn SandboxSessionRepository>,
            vec![
                Arc::clone(&sandbox_provider) as Arc<dyn SandboxProvider>,
                Arc::clone(&sandbox_provider) as Arc<dyn SandboxProvider>,
            ],
            sandbox_lease_owner_id,
            Duration::from_secs(60),
            Duration::from_secs(30),
        ),
        Err(SandboxLifecycleError::DuplicateProvider { .. })
    ));
}

#[test]
fn sandbox_allocation_protection_metadata_rejects_unsafe_key_identity() {
    for sandbox_invalid_key_id in ["", "key id", "key\nid", "密钥"] {
        assert_eq!(
            SandboxProtectedProviderAllocationRef::new(
                "protected-allocation",
                sandbox_invalid_key_id,
                1,
                1,
            ),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
        assert_eq!(
            SandboxProviderAllocationProtectionVersion::new(sandbox_invalid_key_id, 1, 1),
            Err(SandboxSessionRepositoryError::ProtectionFailed)
        );
    }

    assert!(
        SandboxProtectedProviderAllocationRef::new("protected-allocation", "kms/key:v2", 1, 1,)
            .is_ok()
    );
    assert!(SandboxProviderAllocationProtectionVersion::new("kms/key:v2", 1, 1).is_ok());
}

/// Builds a sandbox session in an arbitrary state with or without a runtime binding. The guard
/// matrix uses the binding-free variant to prove the *precedence* between state validation and
/// invariant validation: a lifecycle operation that is forbidden for the current state must be
/// reported as a forbidden transition, not as an internal inconsistency, even when the session is
/// also missing the binding that operation would need.
fn guard_matrix_sandbox_session(
    sandbox_session_id_value: &str,
    sandbox_session_state: SandboxSessionState,
    sandbox_operation_kind: SandboxSessionOperationKind,
    include_sandbox_runtime_binding: bool,
) -> SandboxSession {
    let sandbox_runtime_binding = include_sandbox_runtime_binding.then(|| {
        let mut sandbox_runtime_binding = SandboxRuntimeBinding::new_intent(
            SandboxId::generate(),
            SandboxRuntimeBindingId::generate(),
            sandbox_provider_id("provider-test"),
        );
        sandbox_runtime_binding.set_sandbox_allocation_reference(
            SandboxProviderAllocationRef::new(format!("allocation-{sandbox_session_id_value}"))
                .unwrap_or_else(|error| {
                    panic!("invalid test sandbox allocation reference: {error}")
                }),
        );
        sandbox_runtime_binding
    });
    SandboxSession::restore(
        tenant_id("tenant-a"),
        sandbox_workspace_id("workspace-a"),
        SandboxSessionId::parse(sandbox_session_id_value)
            .unwrap_or_else(|error| panic!("invalid test sandbox session id: {error}")),
        sandbox_session_state,
        BTreeSet::from([RuntimeCapability::Filesystem]),
        IsolationAssurance::HostUser,
        sandbox_runtime_binding,
        None,
        vec![
            SandboxSessionOperation::restore(
                OperationId::generate(),
                SandboxSessionOperationKind::Create,
                SandboxOperationOutcome::Succeeded,
            ),
            SandboxSessionOperation::restore(
                OperationId::generate(),
                sandbox_operation_kind,
                SandboxOperationOutcome::InProgress,
            ),
        ],
        0,
    )
}

/// Total number of provider round trips the lifecycle service performed. A forbidden lifecycle
/// operation must never reach the provider, so this stays at zero.
fn sandbox_provider_rpc_call_count(sandbox_provider: &FakeSandboxProvider) -> usize {
    sandbox_provider.sandbox_health_calls.load(Ordering::SeqCst)
        + sandbox_provider
            .sandbox_allocate_calls
            .load(Ordering::SeqCst)
        + sandbox_provider.sandbox_start_calls.load(Ordering::SeqCst)
        + sandbox_provider.sandbox_stop_calls.load(Ordering::SeqCst)
        + sandbox_provider
            .sandbox_destroy_calls
            .load(Ordering::SeqCst)
}

/// The operation guard an operator actually hits is a function of (current sandbox session state,
/// requested lifecycle operation), not of the state -> state matrix that `model.rs` already locks
/// exhaustively. All 8 x 3 cells are evaluated, each with and without a runtime binding, and a
/// forbidden cell must additionally be rejected *without reaching the provider* and without
/// mutating the persisted sandbox session.
#[tokio::test]
async fn sandbox_lifecycle_guard_matrix_matches_the_documented_operation_contract() {
    const SANDBOX_ALLOWED_OPERATIONS: &[(SandboxSessionState, SandboxSessionOperationKind)] = &[
        (
            SandboxSessionState::Created,
            SandboxSessionOperationKind::Start,
        ),
        (
            SandboxSessionState::Created,
            SandboxSessionOperationKind::Destroy,
        ),
        (
            SandboxSessionState::Running,
            SandboxSessionOperationKind::Stop,
        ),
        (
            SandboxSessionState::Stopped,
            SandboxSessionOperationKind::Start,
        ),
        (
            SandboxSessionState::Stopped,
            SandboxSessionOperationKind::Destroy,
        ),
        (
            SandboxSessionState::Failed,
            SandboxSessionOperationKind::Start,
        ),
        (
            SandboxSessionState::Failed,
            SandboxSessionOperationKind::Destroy,
        ),
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
    let sandbox_all_operations = [
        SandboxSessionOperationKind::Start,
        SandboxSessionOperationKind::Stop,
        SandboxSessionOperationKind::Destroy,
    ];

    let mut sandbox_evaluated_cells = 0;
    for sandbox_session_state in sandbox_all_states {
        for sandbox_operation_kind in sandbox_all_operations {
            for include_sandbox_runtime_binding in [true, false] {
                let sandbox_cell = format!(
                    "{sandbox_session_state:?} + {sandbox_operation_kind:?} (runtime binding: {include_sandbox_runtime_binding})"
                );
                let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
                    [RuntimeCapability::Filesystem],
                    IsolationAssurance::HostUser,
                ));
                let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
                let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
                    Arc::clone(&sandbox_session_repository),
                    Arc::clone(&sandbox_provider),
                );
                let sandbox_session = guard_matrix_sandbox_session(
                    &format!(
                        "guard-{sandbox_session_state:?}-{sandbox_operation_kind:?}-{include_sandbox_runtime_binding}"
                    ),
                    sandbox_session_state,
                    sandbox_operation_kind,
                    include_sandbox_runtime_binding,
                );
                let sandbox_session_id = sandbox_session.sandbox_session_id().clone();
                let sandbox_inserted_version = sandbox_session.sandbox_version();
                let sandbox_inserted_operation_count = sandbox_session.sandbox_operations().len();
                sandbox_session_repository
                    .insert_sandbox_session(sandbox_session)
                    .await
                    .unwrap_or_else(|error| {
                        panic!("guard matrix sandbox session insert failed: {error}")
                    });

                let sandbox_command = SandboxSessionLifecycleCommand {
                    tenant_id: tenant_id("tenant-a"),
                    sandbox_session_id: sandbox_session_id.clone(),
                    sandbox_operation_id: OperationId::generate(),
                };
                let sandbox_result = match sandbox_operation_kind {
                    SandboxSessionOperationKind::Start => {
                        sandbox_lifecycle_service
                            .start_sandbox_session(sandbox_command)
                            .await
                    }
                    SandboxSessionOperationKind::Stop => {
                        sandbox_lifecycle_service
                            .stop_sandbox_session(sandbox_command)
                            .await
                    }
                    SandboxSessionOperationKind::Destroy => {
                        sandbox_lifecycle_service
                            .destroy_sandbox_session(sandbox_command)
                            .await
                    }
                    SandboxSessionOperationKind::Create => {
                        panic!("create is not a lifecycle operation on an existing sandbox session")
                    }
                };

                let sandbox_is_allowed = SANDBOX_ALLOWED_OPERATIONS
                    .contains(&(sandbox_session_state, sandbox_operation_kind));
                match sandbox_result {
                    Err(SandboxLifecycleError::InvalidTransition {
                        sandbox_session_state: rejected_sandbox_session_state,
                        sandbox_operation_kind: rejected_sandbox_operation_kind,
                    }) => {
                        assert!(
                            !sandbox_is_allowed,
                            "sandbox cell {sandbox_cell} is documented as allowed but the guard rejected it"
                        );
                        assert_eq!(
                            rejected_sandbox_session_state, sandbox_session_state,
                            "sandbox cell {sandbox_cell} reported the wrong sandbox session state"
                        );
                        assert_eq!(
                            rejected_sandbox_operation_kind, sandbox_operation_kind,
                            "sandbox cell {sandbox_cell} reported the wrong sandbox operation kind"
                        );
                    }
                    Err(sandbox_other_error) => {
                        assert!(
                            sandbox_is_allowed,
                            "sandbox cell {sandbox_cell} is documented as forbidden but failed with {sandbox_other_error:?} instead of InvalidTransition"
                        );
                    }
                    Ok(_) => {
                        assert!(
                            sandbox_is_allowed,
                            "sandbox cell {sandbox_cell} is documented as forbidden but completed"
                        );
                    }
                }

                if !sandbox_is_allowed {
                    assert_eq!(
                        sandbox_provider_rpc_call_count(&sandbox_provider),
                        0,
                        "sandbox cell {sandbox_cell} is forbidden but still reached the sandbox provider"
                    );
                    let sandbox_persisted = sandbox_session_repository
                        .get_sandbox_session(&tenant_id("tenant-a"), &sandbox_session_id)
                        .await
                        .unwrap_or_else(|error| panic!("guard matrix read-back failed: {error}"))
                        .unwrap_or_else(|| {
                            panic!("guard matrix sandbox session {sandbox_session_id} vanished")
                        });
                    assert_eq!(
                        sandbox_persisted.sandbox_session_state(),
                        sandbox_session_state,
                        "sandbox cell {sandbox_cell} mutated the persisted sandbox session state"
                    );
                    assert_eq!(
                        sandbox_persisted.sandbox_version(),
                        sandbox_inserted_version,
                        "sandbox cell {sandbox_cell} bumped the persisted sandbox session version"
                    );
                    assert_eq!(
                        sandbox_persisted.sandbox_operations().len(),
                        sandbox_inserted_operation_count,
                        "sandbox cell {sandbox_cell} recorded a sandbox operation"
                    );
                }
                sandbox_evaluated_cells += 1;
            }
        }
    }
    assert_eq!(sandbox_evaluated_cells, 48);
}

/// A replayed operation that is still in flight is reported as such, and the idempotency replay
/// wins over state validation: the same session in `Starting` is both "already has this operation
/// in progress" and "cannot start again", and the caller must learn the former.
#[tokio::test]
async fn sandbox_lifecycle_replay_reports_an_in_progress_sandbox_operation() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        Arc::clone(&sandbox_provider),
    );
    let sandbox_session = transient_sandbox_session(
        "in-progress-1",
        SandboxSessionState::Starting,
        SandboxSessionOperationKind::Start,
        false,
    );
    let sandbox_in_progress_operation_id = sandbox_session
        .sandbox_operations()
        .iter()
        .find(|sandbox_operation| {
            sandbox_operation.sandbox_operation_kind() == SandboxSessionOperationKind::Start
                && sandbox_operation.sandbox_operation_outcome()
                    == SandboxOperationOutcome::InProgress
        })
        .map(|sandbox_operation| sandbox_operation.sandbox_operation_id().clone())
        .unwrap_or_else(|| {
            panic!("transient sandbox session must carry an in-progress start operation")
        });
    let sandbox_session_id = sandbox_session.sandbox_session_id().clone();
    sandbox_session_repository
        .insert_sandbox_session(sandbox_session)
        .await
        .unwrap_or_else(|error| panic!("in-progress sandbox session insert failed: {error}"));

    let sandbox_result = sandbox_lifecycle_service
        .start_sandbox_session(SandboxSessionLifecycleCommand {
            tenant_id: tenant_id("tenant-a"),
            sandbox_session_id,
            sandbox_operation_id: sandbox_in_progress_operation_id.clone(),
        })
        .await;

    match sandbox_result {
        Err(SandboxLifecycleError::OperationInProgress { sandbox_operation_id }) => {
            assert_eq!(sandbox_operation_id, sandbox_in_progress_operation_id);
        }
        sandbox_other => panic!(
            "expected OperationInProgress for the replayed in-flight sandbox operation, got {sandbox_other:?}"
        ),
    }
}

/// Only corrupt persisted state can produce a `Running` sandbox session with no runtime binding.
/// The lifecycle service must fail closed with a typed invariant error rather than dereferencing
/// the missing binding or, worse, stopping a sandbox it cannot address.
#[tokio::test]
async fn sandbox_stop_fails_closed_when_a_running_sandbox_session_has_no_runtime_binding() {
    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        Arc::clone(&sandbox_provider),
    );
    let sandbox_session = SandboxSession::restore(
        tenant_id("tenant-a"),
        sandbox_workspace_id("workspace-a"),
        SandboxSessionId::parse("corrupt-running-1")
            .unwrap_or_else(|error| panic!("invalid test sandbox session id: {error}")),
        SandboxSessionState::Running,
        BTreeSet::from([RuntimeCapability::Filesystem]),
        IsolationAssurance::HostUser,
        None,
        None,
        vec![SandboxSessionOperation::restore(
            OperationId::generate(),
            SandboxSessionOperationKind::Create,
            SandboxOperationOutcome::Succeeded,
        )],
        0,
    );
    let sandbox_session_id = sandbox_session.sandbox_session_id().clone();
    sandbox_session_repository
        .insert_sandbox_session(sandbox_session)
        .await
        .unwrap_or_else(|error| panic!("corrupt sandbox session insert failed: {error}"));

    let sandbox_result = sandbox_lifecycle_service
        .stop_sandbox_session(SandboxSessionLifecycleCommand {
            tenant_id: tenant_id("tenant-a"),
            sandbox_session_id,
            sandbox_operation_id: OperationId::generate(),
        })
        .await;

    assert!(matches!(
        sandbox_result,
        Err(SandboxLifecycleError::InvariantViolation(
            "running sandbox session has no sandbox runtime binding"
        ))
    ));
}

/// Performance harness for the provider-neutral create path, i.e. the control-plane cost an agent
/// pays before any machine actually boots. Inert unless `SDKWORK_SANDBOX_BENCH_ITERATIONS` is set,
/// so an ordinary `cargo test` run never pays for it.
///
/// It reports raw nanosecond samples on one machine-readable line.
/// `tools/bench-sandbox-lifecycle.mjs` drives this test, samples process resources from outside, and
/// renders the report. Keeping every OS-specific sampling call out of this file is deliberate: this
/// crate stays free of platform-conditional code, and the portability gate enforces that.
#[tokio::test]
async fn sandbox_lifecycle_create_start_benchmark() {
    let sandbox_iterations = match std::env::var("SDKWORK_SANDBOX_BENCH_ITERATIONS") {
        Ok(sandbox_iterations) => sandbox_iterations.parse::<usize>().unwrap_or_else(|error| {
            panic!("SDKWORK_SANDBOX_BENCH_ITERATIONS must be an integer: {error}")
        }),
        Err(_) => return,
    };
    if sandbox_iterations == 0 {
        return;
    }
    let sandbox_warmup_iterations = (sandbox_iterations / 10).max(1);

    let sandbox_provider = Arc::new(FakeSandboxProvider::ready(
        [RuntimeCapability::Filesystem],
        IsolationAssurance::HostUser,
    ));
    let sandbox_session_repository = Arc::new(TestSandboxSessionRepository::default());
    let sandbox_lifecycle_service = sandbox_lifecycle_service_with_repository(
        Arc::clone(&sandbox_session_repository),
        Arc::clone(&sandbox_provider),
    );

    let mut sandbox_create_nanos = Vec::with_capacity(sandbox_iterations);
    let mut sandbox_start_nanos = Vec::with_capacity(sandbox_iterations);
    let mut sandbox_cold_create_nanos = 0_u128;
    let mut sandbox_cold_start_nanos = 0_u128;
    for sandbox_iteration in 0..(sandbox_warmup_iterations + sandbox_iterations) {
        let sandbox_create_started_at = Instant::now();
        let sandbox_session = sandbox_lifecycle_service
            .create_sandbox_session(create_sandbox_session_command(
                tenant_id("tenant-a"),
                [RuntimeCapability::Filesystem],
                IsolationAssurance::HostUser,
            ))
            .await
            .unwrap_or_else(|error| panic!("benchmark sandbox session creation failed: {error}"));
        let sandbox_create_elapsed = sandbox_create_started_at.elapsed();

        let sandbox_start_started_at = Instant::now();
        sandbox_lifecycle_service
            .start_sandbox_session(sandbox_session_lifecycle_command(&sandbox_session))
            .await
            .unwrap_or_else(|error| panic!("benchmark sandbox session start failed: {error}"));
        let sandbox_start_elapsed = sandbox_start_started_at.elapsed();

        if sandbox_iteration == 0 {
            // The spec requires hot allocation latency and cold start latency to be counted
            // separately, so the very first pass is reported on its own rather than folded into
            // the steady-state distribution.
            sandbox_cold_create_nanos = sandbox_create_elapsed.as_nanos();
            sandbox_cold_start_nanos = sandbox_start_elapsed.as_nanos();
        }
        if sandbox_iteration >= sandbox_warmup_iterations {
            sandbox_create_nanos.push(sandbox_create_elapsed.as_nanos());
            sandbox_start_nanos.push(sandbox_start_elapsed.as_nanos());
        }
    }

    let sandbox_format_nanos = |sandbox_samples: &[u128]| {
        sandbox_samples
            .iter()
            .map(u128::to_string)
            .collect::<Vec<String>>()
            .join(",")
    };
    println!(
        "SDKWORK_BENCH_RESULT {{\"iterations\":{sandbox_iterations},\"warmup\":{sandbox_warmup_iterations},\"coldCreateNanos\":{sandbox_cold_create_nanos},\"coldStartNanos\":{sandbox_cold_start_nanos},\"createNanos\":[{}],\"startNanos\":[{}]}}",
        sandbox_format_nanos(&sandbox_create_nanos),
        sandbox_format_nanos(&sandbox_start_nanos)
    );
}
