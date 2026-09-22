#![forbid(unsafe_code)]
//! Single-process in-memory adapter for the Sandbox Session repository port.

use std::collections::{BTreeMap, HashMap};
use std::ops::Bound::{Excluded, Unbounded};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use sdkwork_intelligence_sandbox_service::{
    validate_sandbox_session_persisted_invariants, SandboxSession, SandboxSessionLease,
    SandboxSessionReconciliationCandidate, SandboxSessionRepository, SandboxSessionRepositoryError,
    SandboxSessionRepositoryResult, SandboxSessionState,
};
use sdkwork_sandbox_provider_spi::{
    OperationId, SandboxFencingToken, SandboxLeaseOwnerId, SandboxSessionId, TenantId,
};
use sdkwork_utils_rust::datetime;
use tokio::sync::RwLock;

#[derive(Default)]
struct SandboxRepositoryState {
    sandbox_sessions: BTreeMap<TenantId, BTreeMap<SandboxSessionId, SandboxSession>>,
    sandbox_operations: HashMap<(TenantId, OperationId), SandboxSessionId>,
    sandbox_leases: HashMap<(TenantId, SandboxSessionId), SandboxMemoryLease>,
}

struct SandboxMemoryLease {
    sandbox_lease_owner_id: Option<SandboxLeaseOwnerId>,
    sandbox_fencing_token: u64,
    sandbox_lease_expires_at: Option<Instant>,
    sandbox_lease_expires_at_unix_millis: Option<i64>,
}

#[derive(Default)]
pub struct InMemorySandboxSessionRepository {
    sandbox_state: RwLock<SandboxRepositoryState>,
}

impl InMemorySandboxSessionRepository {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

    fn sandbox_session_lease(
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
        sandbox_memory_lease: &SandboxMemoryLease,
    ) -> SandboxSessionRepositoryResult<SandboxSessionLease> {
        SandboxSessionLease::new(
            tenant_id.clone(),
            sandbox_session_id.clone(),
            sandbox_memory_lease
                .sandbox_lease_owner_id
                .clone()
                .ok_or(SandboxSessionRepositoryError::InvalidStoredData)?,
            SandboxFencingToken::new(sandbox_memory_lease.sandbox_fencing_token)
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
            sandbox_memory_lease
                .sandbox_lease_expires_at_unix_millis
                .ok_or(SandboxSessionRepositoryError::InvalidStoredData)?,
        )
    }

    fn collect_sandbox_reconciliation_page<'a>(
        sandbox_sessions: impl Iterator<Item = (&'a SandboxSessionId, &'a SandboxSession)>,
        sandbox_page_size: u16,
    ) -> Vec<SandboxSessionReconciliationCandidate> {
        sandbox_sessions
            .filter(|(_, sandbox_session)| {
                matches!(
                    sandbox_session.sandbox_session_state(),
                    SandboxSessionState::Starting
                        | SandboxSessionState::Stopping
                        | SandboxSessionState::Destroying
                )
            })
            .take(usize::from(sandbox_page_size))
            .map(|(sandbox_session_id, sandbox_session)| {
                SandboxSessionReconciliationCandidate::new(
                    sandbox_session_id.clone(),
                    sandbox_session.sandbox_session_state(),
                )
            })
            .collect()
    }
}

#[async_trait]
impl SandboxSessionRepository for InMemorySandboxSessionRepository {
    async fn find_by_sandbox_operation(
        &self,
        tenant_id: &TenantId,
        sandbox_operation_id: &OperationId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>> {
        let sandbox_state = self.sandbox_state.read().await;
        let Some(sandbox_session_id) = sandbox_state
            .sandbox_operations
            .get(&(tenant_id.clone(), sandbox_operation_id.clone()))
        else {
            return Ok(None);
        };
        Ok(sandbox_state
            .sandbox_sessions
            .get(tenant_id)
            .and_then(|sandbox_sessions| sandbox_sessions.get(sandbox_session_id))
            .cloned())
    }

    async fn get_sandbox_session(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>> {
        let sandbox_state = self.sandbox_state.read().await;
        Ok(sandbox_state
            .sandbox_sessions
            .get(tenant_id)
            .and_then(|sandbox_sessions| sandbox_sessions.get(sandbox_session_id))
            .cloned())
    }

    async fn insert_sandbox_session(
        &self,
        sandbox_session: SandboxSession,
    ) -> SandboxSessionRepositoryResult<()> {
        // Same admission invariants as the PostgreSQL adapter's
        // `Snapshot::capture`: an invalid ledger or state/binding matrix must
        // be rejected on write, not only on read (REQ-2026-0005 acceptance:
        // memory and PostgreSQL adapter semantics must agree).
        validate_sandbox_session_persisted_invariants(&sandbox_session)?;
        let mut sandbox_state = self.sandbox_state.write().await;
        let tenant_id = sandbox_session.tenant_id().clone();
        let sandbox_session_id = sandbox_session.sandbox_session_id().clone();
        let sandbox_session_key = (tenant_id.clone(), sandbox_session_id.clone());
        if sandbox_state
            .sandbox_sessions
            .get(&tenant_id)
            .is_some_and(|sandbox_sessions| sandbox_sessions.contains_key(&sandbox_session_id))
        {
            return Err(SandboxSessionRepositoryError::VersionConflict);
        }
        for sandbox_operation in sandbox_session.sandbox_operations() {
            let sandbox_operation_key = (
                sandbox_session.tenant_id().clone(),
                sandbox_operation.sandbox_operation_id().clone(),
            );
            if sandbox_state
                .sandbox_operations
                .contains_key(&sandbox_operation_key)
            {
                return Err(SandboxSessionRepositoryError::DuplicateOperation);
            }
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
            .entry(tenant_id)
            .or_default()
            .insert(sandbox_session_id, sandbox_session);
        sandbox_state.sandbox_leases.insert(
            sandbox_session_key,
            SandboxMemoryLease {
                sandbox_lease_owner_id: None,
                sandbox_fencing_token: 0,
                sandbox_lease_expires_at: None,
                sandbox_lease_expires_at_unix_millis: None,
            },
        );
        Ok(())
    }

    async fn save_sandbox_session(
        &self,
        sandbox_session: SandboxSession,
        expected_sandbox_version: u64,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<()> {
        // Same admission invariants as the PostgreSQL adapter's
        // `Snapshot::capture`; see `insert_sandbox_session`.
        validate_sandbox_session_persisted_invariants(&sandbox_session)?;
        let mut sandbox_state = self.sandbox_state.write().await;
        let tenant_id = sandbox_session.tenant_id().clone();
        let sandbox_session_id = sandbox_session.sandbox_session_id().clone();
        let sandbox_session_key = (tenant_id.clone(), sandbox_session_id.clone());
        let current_sandbox_session = sandbox_state
            .sandbox_sessions
            .get(&tenant_id)
            .and_then(|sandbox_sessions| sandbox_sessions.get(&sandbox_session_id))
            .ok_or(SandboxSessionRepositoryError::NotFound)?;
        let sandbox_memory_lease = sandbox_state
            .sandbox_leases
            .get(&sandbox_session_key)
            .ok_or(SandboxSessionRepositoryError::LeaseConflict)?;
        if sandbox_memory_lease.sandbox_lease_owner_id.as_ref()
            != Some(sandbox_session_lease.sandbox_lease_owner_id())
            || sandbox_memory_lease.sandbox_fencing_token
                != sandbox_session_lease.sandbox_fencing_token().value()
            || sandbox_memory_lease
                .sandbox_lease_expires_at
                .is_none_or(|sandbox_lease_expires_at| sandbox_lease_expires_at <= Instant::now())
        {
            return Err(SandboxSessionRepositoryError::LeaseConflict);
        }
        let next_sandbox_version = expected_sandbox_version
            .checked_add(1)
            .filter(|sandbox_version| *sandbox_version <= i64::MAX as u64)
            .ok_or(SandboxSessionRepositoryError::VersionConflict)?;
        if current_sandbox_session.sandbox_version() != expected_sandbox_version
            || sandbox_session.sandbox_version() != next_sandbox_version
        {
            return Err(SandboxSessionRepositoryError::VersionConflict);
        }
        for sandbox_operation in sandbox_session.sandbox_operations() {
            let sandbox_operation_key = (
                sandbox_session.tenant_id().clone(),
                sandbox_operation.sandbox_operation_id().clone(),
            );
            if let Some(sandbox_operation_owner) =
                sandbox_state.sandbox_operations.get(&sandbox_operation_key)
            {
                if sandbox_operation_owner != sandbox_session.sandbox_session_id() {
                    return Err(SandboxSessionRepositoryError::DuplicateOperation);
                }
            }
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
            .get_mut(&tenant_id)
            .ok_or(SandboxSessionRepositoryError::NotFound)?
            .insert(sandbox_session_id, sandbox_session);
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
        let mut sandbox_state = self.sandbox_state.write().await;
        let sandbox_session_key = (tenant_id.clone(), sandbox_session_id.clone());
        if !sandbox_state
            .sandbox_sessions
            .get(tenant_id)
            .is_some_and(|sandbox_sessions| sandbox_sessions.contains_key(sandbox_session_id))
        {
            return Err(SandboxSessionRepositoryError::NotFound);
        }
        let sandbox_memory_lease = sandbox_state
            .sandbox_leases
            .get_mut(&sandbox_session_key)
            .ok_or(SandboxSessionRepositoryError::InvalidStoredData)?;
        let sandbox_now = Instant::now();
        if sandbox_memory_lease
            .sandbox_lease_expires_at
            .is_some_and(|sandbox_lease_expires_at| sandbox_lease_expires_at > sandbox_now)
        {
            return Ok(None);
        }
        sandbox_memory_lease.sandbox_fencing_token = sandbox_memory_lease
            .sandbox_fencing_token
            .checked_add(1)
            .filter(|sandbox_fencing_token| *sandbox_fencing_token <= i64::MAX as u64)
            .ok_or(SandboxSessionRepositoryError::LeaseConflict)?;
        sandbox_memory_lease.sandbox_lease_owner_id = Some(sandbox_lease_owner_id.clone());
        sandbox_memory_lease.sandbox_lease_expires_at = Some(sandbox_now + sandbox_lease_duration);
        sandbox_memory_lease.sandbox_lease_expires_at_unix_millis = Some(
            datetime::now()
                .timestamp_millis()
                .checked_add(sandbox_lease_duration_millis)
                .ok_or(SandboxSessionRepositoryError::LeaseConflict)?,
        );
        Self::sandbox_session_lease(tenant_id, sandbox_session_id, sandbox_memory_lease).map(Some)
    }

    async fn renew_sandbox_session_lease(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
        sandbox_lease_duration: Duration,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSessionLease>> {
        let sandbox_lease_duration_millis =
            Self::sandbox_lease_duration_millis(sandbox_lease_duration)?;
        let mut sandbox_state = self.sandbox_state.write().await;
        let sandbox_memory_lease = sandbox_state
            .sandbox_leases
            .get_mut(&(
                sandbox_session_lease.tenant_id().clone(),
                sandbox_session_lease.sandbox_session_id().clone(),
            ))
            .ok_or(SandboxSessionRepositoryError::NotFound)?;
        let sandbox_now = Instant::now();
        if sandbox_memory_lease.sandbox_lease_owner_id.as_ref()
            != Some(sandbox_session_lease.sandbox_lease_owner_id())
            || sandbox_memory_lease.sandbox_fencing_token
                != sandbox_session_lease.sandbox_fencing_token().value()
            || sandbox_memory_lease
                .sandbox_lease_expires_at
                .is_none_or(|sandbox_lease_expires_at| sandbox_lease_expires_at <= sandbox_now)
        {
            return Ok(None);
        }
        sandbox_memory_lease.sandbox_lease_expires_at = Some(sandbox_now + sandbox_lease_duration);
        sandbox_memory_lease.sandbox_lease_expires_at_unix_millis = Some(
            datetime::now()
                .timestamp_millis()
                .checked_add(sandbox_lease_duration_millis)
                .ok_or(SandboxSessionRepositoryError::LeaseConflict)?,
        );
        Self::sandbox_session_lease(
            sandbox_session_lease.tenant_id(),
            sandbox_session_lease.sandbox_session_id(),
            sandbox_memory_lease,
        )
        .map(Some)
    }

    async fn release_sandbox_session_lease(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<bool> {
        let mut sandbox_state = self.sandbox_state.write().await;
        let sandbox_memory_lease = sandbox_state
            .sandbox_leases
            .get_mut(&(
                sandbox_session_lease.tenant_id().clone(),
                sandbox_session_lease.sandbox_session_id().clone(),
            ))
            .ok_or(SandboxSessionRepositoryError::NotFound)?;
        if sandbox_memory_lease.sandbox_lease_owner_id.as_ref()
            != Some(sandbox_session_lease.sandbox_lease_owner_id())
            || sandbox_memory_lease.sandbox_fencing_token
                != sandbox_session_lease.sandbox_fencing_token().value()
        {
            return Ok(false);
        }
        sandbox_memory_lease.sandbox_lease_owner_id = None;
        sandbox_memory_lease.sandbox_lease_expires_at = None;
        sandbox_memory_lease.sandbox_lease_expires_at_unix_millis = None;
        Ok(true)
    }

    async fn list_sandbox_sessions_requiring_reconciliation(
        &self,
        tenant_id: &TenantId,
        after_sandbox_session_id: Option<&SandboxSessionId>,
        sandbox_page_size: u16,
    ) -> SandboxSessionRepositoryResult<Vec<SandboxSessionReconciliationCandidate>> {
        if !(1..=200).contains(&sandbox_page_size) {
            return Err(SandboxSessionRepositoryError::InvalidPageRequest);
        }
        let sandbox_state = self.sandbox_state.read().await;
        let Some(sandbox_sessions) = sandbox_state.sandbox_sessions.get(tenant_id) else {
            return Ok(Vec::new());
        };
        Ok(match after_sandbox_session_id {
            Some(after_sandbox_session_id) => Self::collect_sandbox_reconciliation_page(
                sandbox_sessions.range((Excluded(after_sandbox_session_id), Unbounded)),
                sandbox_page_size,
            ),
            None => Self::collect_sandbox_reconciliation_page(
                sandbox_sessions.iter(),
                sandbox_page_size,
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;
    use std::time::Duration;

    use sdkwork_intelligence_sandbox_service::{
        CreateSandboxSessionCommand, SandboxLifecycleService, SandboxSessionRepository,
        SandboxSessionRepositoryError,
    };
    use sdkwork_sandbox_provider_spi::{
        IsolationAssurance, OperationId, SandboxLeaseOwnerId, SandboxSessionId, SandboxWorkspaceId,
        TenantId,
    };

    use super::InMemorySandboxSessionRepository;

    fn tenant_id(value: &str) -> TenantId {
        TenantId::parse(value).unwrap_or_else(|error| panic!("invalid test tenant id: {error}"))
    }

    fn sandbox_workspace_id() -> SandboxWorkspaceId {
        SandboxWorkspaceId::parse("workspace-a")
            .unwrap_or_else(|error| panic!("invalid test sandbox workspace id: {error}"))
    }

    fn sandbox_session_id() -> SandboxSessionId {
        SandboxSessionId::parse("session-a")
            .unwrap_or_else(|error| panic!("invalid test sandbox session id: {error}"))
    }

    #[tokio::test]
    async fn isolates_sandbox_sessions_by_tenant_and_indexes_sandbox_create_operations() {
        let sandbox_session_repository = Arc::new(InMemorySandboxSessionRepository::new());
        let sandbox_lifecycle_service =
            SandboxLifecycleService::new(sandbox_session_repository.clone(), Vec::new())
                .unwrap_or_else(|error| panic!("valid sandbox lifecycle service: {error}"));
        let tenant_a = tenant_id("tenant-a");
        let tenant_b = tenant_id("tenant-b");
        let sandbox_operation_id = OperationId::generate();
        let sandbox_session = sandbox_lifecycle_service
            .create_sandbox_session(CreateSandboxSessionCommand {
                tenant_id: tenant_a.clone(),
                sandbox_workspace_id: sandbox_workspace_id(),
                sandbox_session_id: sandbox_session_id(),
                sandbox_operation_id: sandbox_operation_id.clone(),
                sandbox_required_capabilities: BTreeSet::new(),
                sandbox_minimum_assurance: IsolationAssurance::HostUser,
            })
            .await
            .unwrap_or_else(|error| panic!("sandbox session creation failed: {error}"));

        let visible_sandbox_session = sandbox_session_repository
            .get_sandbox_session(&tenant_a, sandbox_session.sandbox_session_id())
            .await;
        assert!(matches!(visible_sandbox_session, Ok(Some(_))));
        let hidden_sandbox_session = sandbox_session_repository
            .get_sandbox_session(&tenant_b, sandbox_session.sandbox_session_id())
            .await;
        assert!(matches!(hidden_sandbox_session, Ok(None)));
        let indexed_sandbox_session = sandbox_session_repository
            .find_by_sandbox_operation(&tenant_a, &sandbox_operation_id)
            .await;
        assert!(matches!(indexed_sandbox_session, Ok(Some(_))));
    }

    #[tokio::test]
    async fn rejects_stale_sandbox_session_compare_and_swap() {
        let sandbox_session_repository = Arc::new(InMemorySandboxSessionRepository::new());
        let sandbox_lifecycle_service =
            SandboxLifecycleService::new(sandbox_session_repository.clone(), Vec::new())
                .unwrap_or_else(|error| panic!("valid sandbox lifecycle service: {error}"));
        let sandbox_session = sandbox_lifecycle_service
            .create_sandbox_session(CreateSandboxSessionCommand {
                tenant_id: tenant_id("tenant-a"),
                sandbox_workspace_id: sandbox_workspace_id(),
                sandbox_session_id: sandbox_session_id(),
                sandbox_operation_id: OperationId::generate(),
                sandbox_required_capabilities: BTreeSet::new(),
                sandbox_minimum_assurance: IsolationAssurance::HostUser,
            })
            .await
            .unwrap_or_else(|error| panic!("sandbox session creation failed: {error}"));

        let sandbox_session_lease = sandbox_session_repository
            .acquire_sandbox_session_lease(
                sandbox_session.tenant_id(),
                sandbox_session.sandbox_session_id(),
                &SandboxLeaseOwnerId::generate(),
                Duration::from_secs(30),
            )
            .await
            .unwrap_or_else(|error| panic!("sandbox lease acquisition failed: {error}"))
            .unwrap_or_else(|| panic!("sandbox lease must be available"));
        let stale_sandbox_save_result = sandbox_session_repository
            .save_sandbox_session(sandbox_session, 7, &sandbox_session_lease)
            .await;
        assert_eq!(
            stale_sandbox_save_result,
            Err(SandboxSessionRepositoryError::VersionConflict)
        );
    }

    #[tokio::test]
    async fn rejects_invalid_sandbox_reconciliation_page_sizes() {
        let sandbox_session_repository = InMemorySandboxSessionRepository::new();
        let tenant_id = tenant_id("tenant-a");

        for sandbox_page_size in [0, 201] {
            assert_eq!(
                sandbox_session_repository
                    .list_sandbox_sessions_requiring_reconciliation(
                        &tenant_id,
                        None,
                        sandbox_page_size,
                    )
                    .await,
                Err(SandboxSessionRepositoryError::InvalidPageRequest)
            );
        }
    }

    #[tokio::test]
    async fn sandbox_session_insert_conflicts_are_scoped_by_tenant() {
        let sandbox_session_repository = Arc::new(InMemorySandboxSessionRepository::new());
        let sandbox_lifecycle_service =
            SandboxLifecycleService::new(sandbox_session_repository.clone(), Vec::new())
                .unwrap_or_else(|error| panic!("valid sandbox lifecycle service: {error}"));
        let sandbox_tenant_a = tenant_id("tenant-a");
        let sandbox_tenant_b = tenant_id("tenant-b");
        let sandbox_session_id = sandbox_session_id();

        // The same session id under another tenant is isolated and accepted.
        let sandbox_session_a = sandbox_lifecycle_service
            .create_sandbox_session(CreateSandboxSessionCommand {
                tenant_id: sandbox_tenant_a.clone(),
                sandbox_workspace_id: sandbox_workspace_id(),
                sandbox_session_id: sandbox_session_id.clone(),
                sandbox_operation_id: OperationId::generate(),
                sandbox_required_capabilities: BTreeSet::new(),
                sandbox_minimum_assurance: IsolationAssurance::HostUser,
            })
            .await
            .unwrap_or_else(|error| panic!("sandbox session creation failed: {error}"));
        let sandbox_session_b = sandbox_lifecycle_service
            .create_sandbox_session(CreateSandboxSessionCommand {
                tenant_id: sandbox_tenant_b.clone(),
                sandbox_workspace_id: sandbox_workspace_id(),
                sandbox_session_id: sandbox_session_id.clone(),
                sandbox_operation_id: OperationId::generate(),
                sandbox_required_capabilities: BTreeSet::new(),
                sandbox_minimum_assurance: IsolationAssurance::HostUser,
            })
            .await
            .unwrap_or_else(|error| panic!("sandbox session creation failed: {error}"));

        // A direct duplicate insert of an existing session is a version
        // conflict within the owning tenant, and each tenant keeps its own
        // session projection under the same session id.
        assert_eq!(
            sandbox_session_repository
                .insert_sandbox_session(sandbox_session_a.clone())
                .await,
            Err(SandboxSessionRepositoryError::VersionConflict)
        );
        assert_eq!(
            sandbox_session_repository
                .insert_sandbox_session(sandbox_session_b.clone())
                .await,
            Err(SandboxSessionRepositoryError::VersionConflict)
        );
        assert!(matches!(
            sandbox_session_repository
                .get_sandbox_session(&sandbox_tenant_a, sandbox_session_a.sandbox_session_id())
                .await,
            Ok(Some(_))
        ));
        assert!(matches!(
            sandbox_session_repository
                .get_sandbox_session(&sandbox_tenant_b, sandbox_session_b.sandbox_session_id())
                .await,
            Ok(Some(_))
        ));
    }

    #[tokio::test]
    async fn enforces_sandbox_lease_competition_takeover_and_stale_token_rejection() {
        let sandbox_session_repository = Arc::new(InMemorySandboxSessionRepository::new());
        let sandbox_lifecycle_service =
            SandboxLifecycleService::new(sandbox_session_repository.clone(), Vec::new())
                .unwrap_or_else(|error| panic!("valid sandbox lifecycle service: {error}"));
        let sandbox_session = sandbox_lifecycle_service
            .create_sandbox_session(CreateSandboxSessionCommand {
                tenant_id: tenant_id("tenant-a"),
                sandbox_workspace_id: sandbox_workspace_id(),
                sandbox_session_id: sandbox_session_id(),
                sandbox_operation_id: OperationId::generate(),
                sandbox_required_capabilities: BTreeSet::new(),
                sandbox_minimum_assurance: IsolationAssurance::HostUser,
            })
            .await
            .unwrap_or_else(|error| panic!("sandbox session creation failed: {error}"));
        let first_sandbox_lease_owner_id = SandboxLeaseOwnerId::generate();
        let second_sandbox_lease_owner_id = SandboxLeaseOwnerId::generate();

        let first_sandbox_session_lease = sandbox_session_repository
            .acquire_sandbox_session_lease(
                sandbox_session.tenant_id(),
                sandbox_session.sandbox_session_id(),
                &first_sandbox_lease_owner_id,
                Duration::from_secs(30),
            )
            .await
            .unwrap_or_else(|error| panic!("first sandbox lease acquisition failed: {error}"))
            .unwrap_or_else(|| panic!("first sandbox lease must be available"));
        assert_eq!(
            first_sandbox_session_lease.sandbox_fencing_token().value(),
            1
        );
        let competing_sandbox_session_lease = sandbox_session_repository
            .acquire_sandbox_session_lease(
                sandbox_session.tenant_id(),
                sandbox_session.sandbox_session_id(),
                &second_sandbox_lease_owner_id,
                Duration::from_secs(30),
            )
            .await
            .unwrap_or_else(|error| panic!("competing sandbox lease lookup failed: {error}"));
        assert!(competing_sandbox_session_lease.is_none());
        assert!(sandbox_session_repository
            .renew_sandbox_session_lease(&first_sandbox_session_lease, Duration::from_secs(30),)
            .await
            .unwrap_or_else(|error| panic!("first sandbox lease renewal failed: {error}"))
            .is_some());
        assert!(sandbox_session_repository
            .release_sandbox_session_lease(&first_sandbox_session_lease)
            .await
            .unwrap_or_else(|error| panic!("first sandbox lease release failed: {error}")));

        let second_sandbox_session_lease = sandbox_session_repository
            .acquire_sandbox_session_lease(
                sandbox_session.tenant_id(),
                sandbox_session.sandbox_session_id(),
                &second_sandbox_lease_owner_id,
                Duration::from_secs(30),
            )
            .await
            .unwrap_or_else(|error| panic!("second sandbox lease acquisition failed: {error}"))
            .unwrap_or_else(|| panic!("second sandbox lease must be available"));
        assert_eq!(
            second_sandbox_session_lease.sandbox_fencing_token().value(),
            2
        );
        assert!(sandbox_session_repository
            .release_sandbox_session_lease(&second_sandbox_session_lease)
            .await
            .unwrap_or_else(|error| panic!("second sandbox lease release failed: {error}")));

        let expiring_sandbox_session_lease = sandbox_session_repository
            .acquire_sandbox_session_lease(
                sandbox_session.tenant_id(),
                sandbox_session.sandbox_session_id(),
                &first_sandbox_lease_owner_id,
                Duration::from_millis(1),
            )
            .await
            .unwrap_or_else(|error| panic!("expiring sandbox lease acquisition failed: {error}"))
            .unwrap_or_else(|| panic!("expiring sandbox lease must be available"));
        assert_eq!(
            expiring_sandbox_session_lease
                .sandbox_fencing_token()
                .value(),
            3
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
        let takeover_sandbox_session_lease = sandbox_session_repository
            .acquire_sandbox_session_lease(
                sandbox_session.tenant_id(),
                sandbox_session.sandbox_session_id(),
                &second_sandbox_lease_owner_id,
                Duration::from_secs(30),
            )
            .await
            .unwrap_or_else(|error| panic!("takeover sandbox lease acquisition failed: {error}"))
            .unwrap_or_else(|| panic!("takeover sandbox lease must be available"));
        assert_eq!(
            takeover_sandbox_session_lease
                .sandbox_fencing_token()
                .value(),
            4
        );

        assert!(sandbox_session_repository
            .renew_sandbox_session_lease(&expiring_sandbox_session_lease, Duration::from_secs(30),)
            .await
            .unwrap_or_else(|error| panic!("stale sandbox lease renewal failed: {error}"))
            .is_none());
        assert!(!sandbox_session_repository
            .release_sandbox_session_lease(&expiring_sandbox_session_lease)
            .await
            .unwrap_or_else(|error| panic!("stale sandbox lease release failed: {error}")));
        assert_eq!(
            sandbox_session_repository
                .save_sandbox_session(sandbox_session, 0, &expiring_sandbox_session_lease)
                .await,
            Err(SandboxSessionRepositoryError::LeaseConflict)
        );
        assert!(sandbox_session_repository
            .release_sandbox_session_lease(&takeover_sandbox_session_lease)
            .await
            .unwrap_or_else(|error| panic!("takeover sandbox lease release failed: {error}")));
    }
}
