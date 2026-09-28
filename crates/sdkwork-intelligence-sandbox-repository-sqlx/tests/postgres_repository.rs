use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use async_trait::async_trait;
use sdkwork_database_sqlx::PoolBuilder;
use sdkwork_intelligence_sandbox_repository_sqlx::{
    SandboxProviderAllocationKey, SandboxProviderAllocationKeySource,
    SdkworkUtilsSandboxProviderAllocationProtector, SqlxSandboxInstanceRepository,
    SqlxSandboxSessionRepository,
};
use sdkwork_intelligence_sandbox_service::{
    CreateSandboxInstanceCommand, SandboxInstance, SandboxInstanceListCursor,
    SandboxInstanceProfile, SandboxInstanceRepository, SandboxOperationOutcome,
    SandboxProtectedProviderAllocationRef, SandboxProviderAllocationProtectionContext,
    SandboxProviderAllocationProtectionVersion, SandboxProviderAllocationProtector,
    SandboxRuntimeBindingRepositorySnapshot, SandboxSession, SandboxSessionOperationKind,
    SandboxSessionOperationRepositorySnapshot, SandboxSessionReconciliationOutcome,
    SandboxSessionRepository, SandboxSessionRepositoryError, SandboxSessionRepositoryResult,
    SandboxSessionRepositorySnapshot, SandboxSessionState,
};
use sdkwork_sandbox_provider_spi::{
    IsolationAssurance, OperationId, RuntimeCapability, SandboxId, SandboxInstanceOwnerId,
    SandboxLeaseOwnerId, SandboxProvider, SandboxProviderAllocation, SandboxProviderAllocationRef,
    SandboxProviderAllocationRequest, SandboxProviderDescriptor, SandboxProviderDestroyRequest,
    SandboxProviderHealth, SandboxProviderHealthStatus, SandboxProviderId, SandboxProviderKind,
    SandboxProviderReadiness, SandboxProviderResult, SandboxProviderStartRequest,
    SandboxProviderStopRequest, SandboxRuntimeBindingId, SandboxSessionId, SandboxWorkspaceId,
    TenantId,
};

const SANDBOX_POSTGRES_TEST_KEY_ID: &str = "sandbox-postgres-test-key";
const SANDBOX_POSTGRES_TEST_DATABASE_URL: &str = "SDKWORK_DATABASE_TEST_POSTGRES_URL";
const SANDBOX_WORKSPACE_DATABASE_URL: &str = "SDKWORK_DATABASE_URL";
const SANDBOX_POSTGRES_TEST_DATABASE_URL_MISMATCH: &str =
    "SDKWORK_DATABASE_URL must exactly match SDKWORK_DATABASE_TEST_POSTGRES_URL before destructive PostgreSQL tests";

fn validate_sandbox_postgres_test_database_url(
    sandbox_workspace_database_url: &str,
    sandbox_test_database_url: &str,
) -> Result<(), &'static str> {
    if sandbox_workspace_database_url == sandbox_test_database_url {
        Ok(())
    } else {
        Err(SANDBOX_POSTGRES_TEST_DATABASE_URL_MISMATCH)
    }
}

fn sandbox_postgres_test_pool_builder() -> PoolBuilder {
    let sandbox_test_database_url = std::env::var(SANDBOX_POSTGRES_TEST_DATABASE_URL)
        .unwrap_or_else(|_| panic!("{SANDBOX_POSTGRES_TEST_DATABASE_URL} must be set"));
    let sandbox_workspace_database_url = std::env::var(SANDBOX_WORKSPACE_DATABASE_URL)
        .unwrap_or_else(|_| panic!("{SANDBOX_WORKSPACE_DATABASE_URL} must be set"));
    validate_sandbox_postgres_test_database_url(
        &sandbox_workspace_database_url,
        &sandbox_test_database_url,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    PoolBuilder::from_env("SANDBOX_TEST")
        .unwrap_or_else(|error| panic!("sandbox test database config failed: {error}"))
}

#[test]
fn sandbox_postgres_destructive_test_requires_matching_non_echoing_database_urls() {
    assert_eq!(
        validate_sandbox_postgres_test_database_url("same-test-url", "same-test-url"),
        Ok(())
    );
    assert_eq!(
        validate_sandbox_postgres_test_database_url(
            "workspace-url-containing-secret",
            "test-url-containing-secret",
        ),
        Err(SANDBOX_POSTGRES_TEST_DATABASE_URL_MISMATCH)
    );
    assert!(!SANDBOX_POSTGRES_TEST_DATABASE_URL_MISMATCH.contains("containing-secret"));
}

struct SandboxHistoricalKeyPause {
    sandbox_entered: SyncSender<()>,
    sandbox_resume: Receiver<()>,
}

struct TestSandboxAllocationKeySource {
    sandbox_current_key_version: RwLock<u64>,
    sandbox_key_material_by_version: RwLock<BTreeMap<u64, Vec<u8>>>,
}

impl TestSandboxAllocationKeySource {
    fn with_v1() -> Self {
        Self {
            sandbox_current_key_version: RwLock::new(1),
            sandbox_key_material_by_version: RwLock::new(BTreeMap::from([(1, vec![17_u8; 32])])),
        }
    }

    fn rotate_to_v2(&self) {
        self.sandbox_key_material_by_version
            .write()
            .unwrap_or_else(|error| panic!("sandbox key material lock poisoned: {error}"))
            .insert(2, vec![23_u8; 32]);
        *self
            .sandbox_current_key_version
            .write()
            .unwrap_or_else(|error| panic!("sandbox current key lock poisoned: {error}")) = 2;
    }

    fn rotate_to_v3(&self) {
        self.sandbox_key_material_by_version
            .write()
            .unwrap_or_else(|error| panic!("sandbox key material lock poisoned: {error}"))
            .insert(3, vec![29_u8; 32]);
        *self
            .sandbox_current_key_version
            .write()
            .unwrap_or_else(|error| panic!("sandbox current key lock poisoned: {error}")) = 3;
    }

    fn sandbox_key(
        &self,
        sandbox_allocation_key_version: u64,
    ) -> Result<SandboxProviderAllocationKey, SandboxSessionRepositoryError> {
        let sandbox_key_material = self
            .sandbox_key_material_by_version
            .read()
            .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?
            .get(&sandbox_allocation_key_version)
            .cloned()
            .ok_or(SandboxSessionRepositoryError::ProtectionFailed)?;
        SandboxProviderAllocationKey::new(
            SANDBOX_POSTGRES_TEST_KEY_ID,
            sandbox_allocation_key_version,
            sandbox_key_material,
        )
    }
}

impl SandboxProviderAllocationKeySource for TestSandboxAllocationKeySource {
    fn current_sandbox_allocation_key(
        &self,
    ) -> Result<SandboxProviderAllocationKey, SandboxSessionRepositoryError> {
        let sandbox_current_key_version = *self
            .sandbox_current_key_version
            .read()
            .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?;
        self.sandbox_key(sandbox_current_key_version)
    }

    fn sandbox_allocation_key(
        &self,
        sandbox_allocation_key_id: &str,
        sandbox_allocation_key_version: u64,
    ) -> Result<SandboxProviderAllocationKey, SandboxSessionRepositoryError> {
        if sandbox_allocation_key_id != SANDBOX_POSTGRES_TEST_KEY_ID {
            return Err(SandboxSessionRepositoryError::ProtectionFailed);
        }
        self.sandbox_key(sandbox_allocation_key_version)
    }
}

struct TestSandboxAllocationProtector {
    sandbox_inner: SdkworkUtilsSandboxProviderAllocationProtector,
    sandbox_reencryption_pause: Mutex<Option<SandboxHistoricalKeyPause>>,
}

impl TestSandboxAllocationProtector {
    fn new(sandbox_allocation_key_source: Arc<TestSandboxAllocationKeySource>) -> Self {
        Self {
            sandbox_inner: SdkworkUtilsSandboxProviderAllocationProtector::new(
                sandbox_allocation_key_source,
            ),
            sandbox_reencryption_pause: Mutex::new(None),
        }
    }

    fn pause_next_sandbox_reencryption(&self) -> (Receiver<()>, SyncSender<()>) {
        let (sandbox_entered_sender, sandbox_entered_receiver) = sync_channel(1);
        let (sandbox_resume_sender, sandbox_resume_receiver) = sync_channel(1);
        let mut sandbox_pause = self
            .sandbox_reencryption_pause
            .lock()
            .unwrap_or_else(|error| panic!("sandbox re-encryption pause lock poisoned: {error}"));
        assert!(
            sandbox_pause.is_none(),
            "sandbox re-encryption pause is already armed"
        );
        *sandbox_pause = Some(SandboxHistoricalKeyPause {
            sandbox_entered: sandbox_entered_sender,
            sandbox_resume: sandbox_resume_receiver,
        });
        (sandbox_entered_receiver, sandbox_resume_sender)
    }
}

impl SandboxProviderAllocationProtector for TestSandboxAllocationProtector {
    fn current_sandbox_allocation_protection_version(
        &self,
    ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationProtectionVersion> {
        self.sandbox_inner
            .current_sandbox_allocation_protection_version()
    }

    fn protect_sandbox_allocation_reference(
        &self,
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
        sandbox_allocation_reference: &SandboxProviderAllocationRef,
    ) -> SandboxSessionRepositoryResult<SandboxProtectedProviderAllocationRef> {
        self.sandbox_inner.protect_sandbox_allocation_reference(
            sandbox_protection_context,
            sandbox_allocation_reference,
        )
    }

    fn restore_sandbox_allocation_reference(
        &self,
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
        sandbox_protected_allocation_reference: &SandboxProtectedProviderAllocationRef,
    ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationRef> {
        self.sandbox_inner.restore_sandbox_allocation_reference(
            sandbox_protection_context,
            sandbox_protected_allocation_reference,
        )
    }

    fn reencrypt_sandbox_allocation_reference(
        &self,
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
        sandbox_protected_allocation_reference: &SandboxProtectedProviderAllocationRef,
    ) -> SandboxSessionRepositoryResult<SandboxProtectedProviderAllocationRef> {
        let sandbox_pause = self
            .sandbox_reencryption_pause
            .lock()
            .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?
            .take();
        if let Some(sandbox_pause) = sandbox_pause {
            sandbox_pause
                .sandbox_entered
                .send(())
                .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?;
            sandbox_pause
                .sandbox_resume
                .recv_timeout(Duration::from_secs(10))
                .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?;
        }
        self.sandbox_inner.reencrypt_sandbox_allocation_reference(
            sandbox_protection_context,
            sandbox_protected_allocation_reference,
        )
    }
}

struct TestSandboxProvider {
    sandbox_provider_descriptor: SandboxProviderDescriptor,
}

impl TestSandboxProvider {
    fn new() -> Self {
        Self {
            sandbox_provider_descriptor: SandboxProviderDescriptor::new(
                parse_sandbox_provider_id("provider-postgres-test"),
                SandboxProviderKind::parse("test")
                    .unwrap_or_else(|error| panic!("invalid test sandbox provider kind: {error}")),
                [RuntimeCapability::Filesystem],
                IsolationAssurance::HostUser,
            ),
        }
    }
}

#[async_trait]
impl SandboxProvider for TestSandboxProvider {
    fn sandbox_provider_descriptor(&self) -> &SandboxProviderDescriptor {
        &self.sandbox_provider_descriptor
    }

    async fn sandbox_provider_health(&self) -> SandboxProviderResult<SandboxProviderHealth> {
        Ok(SandboxProviderHealth {
            sandbox_provider_health_status: SandboxProviderHealthStatus::Ready,
        })
    }

    async fn allocate(
        &self,
        sandbox_request: SandboxProviderAllocationRequest,
    ) -> SandboxProviderResult<SandboxProviderAllocation> {
        let sandbox_allocation_reference = SandboxProviderAllocationRef::new(format!(
            "allocation-{}-{}",
            sandbox_request.sandbox_runtime_binding_id, sandbox_request.sandbox_fencing_token
        ))
        .unwrap_or_else(|error| panic!("invalid test sandbox allocation reference: {error}"));
        Ok(SandboxProviderAllocation {
            sandbox_allocation_reference,
        })
    }

    async fn start(
        &self,
        _sandbox_request: SandboxProviderStartRequest,
    ) -> SandboxProviderResult<SandboxProviderReadiness> {
        Ok(SandboxProviderReadiness {
            sandbox_provider_ready: true,
            sandbox_policy_enforced: true,
            sandbox_workspace_attached: true,
        })
    }

    async fn stop(
        &self,
        _sandbox_request: SandboxProviderStopRequest,
    ) -> SandboxProviderResult<()> {
        Ok(())
    }

    async fn destroy(
        &self,
        _sandbox_request: SandboxProviderDestroyRequest,
    ) -> SandboxProviderResult<()> {
        Ok(())
    }
}

fn tenant_id(sandbox_value: &str) -> TenantId {
    TenantId::parse(sandbox_value).unwrap_or_else(|error| panic!("invalid test tenant id: {error}"))
}

fn parse_sandbox_workspace_id(sandbox_value: &str) -> SandboxWorkspaceId {
    SandboxWorkspaceId::parse(sandbox_value)
        .unwrap_or_else(|error| panic!("invalid test sandbox workspace id: {error}"))
}

fn parse_sandbox_session_id(sandbox_value: &str) -> SandboxSessionId {
    SandboxSessionId::parse(sandbox_value)
        .unwrap_or_else(|error| panic!("invalid test sandbox session id: {error}"))
}

fn parse_sandbox_provider_id(sandbox_value: &str) -> SandboxProviderId {
    SandboxProviderId::parse(sandbox_value)
        .unwrap_or_else(|error| panic!("invalid test sandbox provider id: {error}"))
}

fn sandbox_session_from_snapshot(
    tenant_id: TenantId,
    sandbox_session_id: SandboxSessionId,
    sandbox_session_state: SandboxSessionState,
    sandbox_runtime_binding: Option<SandboxRuntimeBindingRepositorySnapshot>,
    sandbox_operations: Vec<SandboxSessionOperationRepositorySnapshot>,
    sandbox_version: u64,
    sandbox_allocation_protector: &dyn SandboxProviderAllocationProtector,
) -> SandboxSession {
    SandboxSessionRepositorySnapshot::new(
        tenant_id,
        parse_sandbox_workspace_id("workspace-postgres-test"),
        sandbox_session_id,
        sandbox_session_state,
        BTreeSet::from([RuntimeCapability::Filesystem]),
        IsolationAssurance::HostUser,
        sandbox_runtime_binding,
        None,
        sandbox_operations,
        sandbox_version,
    )
    .restore(sandbox_allocation_protector)
    .unwrap_or_else(|error| panic!("test sandbox session restore failed: {error}"))
}

fn sandbox_session_with_version(
    sandbox_session: &SandboxSession,
    sandbox_version: u64,
    sandbox_allocation_protector: &dyn SandboxProviderAllocationProtector,
) -> SandboxSession {
    let sandbox_snapshot =
        SandboxSessionRepositorySnapshot::capture(sandbox_session, sandbox_allocation_protector)
            .unwrap_or_else(|error| panic!("test sandbox session capture failed: {error}"));
    SandboxSessionRepositorySnapshot::new(
        sandbox_snapshot.tenant_id().clone(),
        sandbox_snapshot.sandbox_workspace_id().clone(),
        sandbox_snapshot.sandbox_session_id().clone(),
        sandbox_snapshot.sandbox_session_state(),
        sandbox_snapshot.sandbox_required_capabilities().clone(),
        sandbox_snapshot.sandbox_minimum_assurance(),
        sandbox_snapshot.sandbox_runtime_binding().cloned(),
        sandbox_snapshot.sandbox_last_failure(),
        sandbox_snapshot.sandbox_operations().to_vec(),
        sandbox_version,
    )
    .restore(sandbox_allocation_protector)
    .unwrap_or_else(|error| panic!("versioned test sandbox session restore failed: {error}"))
}

fn sandbox_bound_session(
    tenant_id: TenantId,
    sandbox_session_id: SandboxSessionId,
    sandbox_runtime_binding_id: SandboxRuntimeBindingId,
    sandbox_private_allocation_reference: &str,
    sandbox_allocation_protector: &dyn SandboxProviderAllocationProtector,
) -> SandboxSession {
    let sandbox_protection_context = SandboxProviderAllocationProtectionContext::for_repository(
        tenant_id.clone(),
        sandbox_session_id.clone(),
        sandbox_runtime_binding_id.clone(),
    );
    let sandbox_allocation_reference =
        SandboxProviderAllocationRef::new(sandbox_private_allocation_reference)
            .unwrap_or_else(|error| panic!("invalid test sandbox allocation reference: {error}"));
    let sandbox_protected_allocation_reference = sandbox_allocation_protector
        .protect_sandbox_allocation_reference(
            &sandbox_protection_context,
            &sandbox_allocation_reference,
        )
        .unwrap_or_else(|error| panic!("test sandbox allocation protection failed: {error}"));
    sandbox_session_from_snapshot(
        tenant_id,
        sandbox_session_id,
        SandboxSessionState::Running,
        Some(SandboxRuntimeBindingRepositorySnapshot::new(
            SandboxId::generate(),
            sandbox_runtime_binding_id,
            parse_sandbox_provider_id("provider-postgres-test"),
            Some(sandbox_protected_allocation_reference),
        )),
        vec![
            SandboxSessionOperationRepositorySnapshot::new(
                OperationId::generate(),
                SandboxSessionOperationKind::Create,
                SandboxOperationOutcome::Succeeded,
            ),
            SandboxSessionOperationRepositorySnapshot::new(
                OperationId::generate(),
                SandboxSessionOperationKind::Start,
                SandboxOperationOutcome::Succeeded,
            ),
        ],
        0,
        sandbox_allocation_protector,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires SDKWORK_DATABASE_TEST_POSTGRES_URL and an initialized PostgreSQL database"]
async fn sandbox_postgres_repository_enforces_durable_lifecycle_contract() {
    let sandbox_database_pool = sandbox_postgres_test_pool_builder()
        .max_connections(4)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .build()
        .await
        .unwrap_or_else(|error| panic!("sandbox test database pool failed: {error}"));
    let sandbox_postgres_pool = sandbox_database_pool
        .as_postgres()
        .unwrap_or_else(|| panic!("sandbox test database must be PostgreSQL"));
    sqlx::raw_sql(
        "TRUNCATE TABLE sandbox_session_operation, sandbox_runtime_binding, \
         sandbox_session_lease, sandbox_session, sandbox_instance CASCADE",
    )
    .execute(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("sandbox test database cleanup failed: {error}"));

    let sandbox_allocation_key_source = Arc::new(TestSandboxAllocationKeySource::with_v1());
    let sandbox_test_allocation_protector = Arc::new(TestSandboxAllocationProtector::new(
        Arc::clone(&sandbox_allocation_key_source),
    ));
    let sandbox_allocation_protector: Arc<dyn SandboxProviderAllocationProtector> =
        sandbox_test_allocation_protector.clone();
    let sandbox_session_repository = Arc::new(
        SqlxSandboxSessionRepository::new(
            sandbox_database_pool.clone(),
            Arc::clone(&sandbox_allocation_protector),
        )
        .unwrap_or_else(|error| panic!("sandbox SQLx repository creation failed: {error}")),
    );

    let tenant_a = tenant_id("tenant-postgres-a");
    let tenant_b = tenant_id("tenant-postgres-b");
    let sandbox_session_id = parse_sandbox_session_id("session-postgres-roundtrip");
    let sandbox_runtime_binding_id = SandboxRuntimeBindingId::generate();
    let sandbox_allocation_reference =
        SandboxProviderAllocationRef::new("private-provider-allocation-postgres-roundtrip")
            .unwrap_or_else(|error| panic!("invalid test sandbox allocation reference: {error}"));
    let sandbox_protection_context = SandboxProviderAllocationProtectionContext::for_repository(
        tenant_a.clone(),
        sandbox_session_id.clone(),
        sandbox_runtime_binding_id.clone(),
    );
    let sandbox_protected_allocation_reference = sandbox_allocation_protector
        .protect_sandbox_allocation_reference(
            &sandbox_protection_context,
            &sandbox_allocation_reference,
        )
        .unwrap_or_else(|error| panic!("test sandbox allocation protection failed: {error}"));
    let sandbox_operation_id = OperationId::generate();
    let sandbox_session = sandbox_session_from_snapshot(
        tenant_a.clone(),
        sandbox_session_id.clone(),
        SandboxSessionState::Running,
        Some(SandboxRuntimeBindingRepositorySnapshot::new(
            SandboxId::generate(),
            sandbox_runtime_binding_id,
            parse_sandbox_provider_id("provider-postgres-test"),
            Some(sandbox_protected_allocation_reference),
        )),
        vec![
            SandboxSessionOperationRepositorySnapshot::new(
                sandbox_operation_id.clone(),
                SandboxSessionOperationKind::Create,
                SandboxOperationOutcome::Succeeded,
            ),
            SandboxSessionOperationRepositorySnapshot::new(
                OperationId::generate(),
                SandboxSessionOperationKind::Start,
                SandboxOperationOutcome::Succeeded,
            ),
        ],
        0,
        sandbox_allocation_protector.as_ref(),
    );
    sandbox_session_repository
        .insert_sandbox_session(sandbox_session.clone())
        .await
        .unwrap_or_else(|error| panic!("sandbox session insert failed: {error}"));

    let stored_sandbox_session = sandbox_session_repository
        .get_sandbox_session(&tenant_a, &sandbox_session_id)
        .await
        .unwrap_or_else(|error| panic!("sandbox session round-trip lookup failed: {error}"))
        .unwrap_or_else(|| panic!("sandbox session round-trip row must exist"));
    assert_eq!(stored_sandbox_session, sandbox_session);
    assert!(sandbox_session_repository
        .get_sandbox_session(&tenant_b, &sandbox_session_id)
        .await
        .unwrap_or_else(|error| panic!("cross-tenant sandbox session lookup failed: {error}"))
        .is_none());
    assert_eq!(
        sandbox_session_repository
            .find_by_sandbox_operation(&tenant_a, &sandbox_operation_id)
            .await
            .unwrap_or_else(|error| panic!("sandbox operation lookup failed: {error}"))
            .map(|sandbox_session| sandbox_session.sandbox_session_id().clone()),
        Some(sandbox_session_id.clone())
    );
    assert_eq!(
        sandbox_session_repository
            .list_sandbox_sessions_requiring_reconciliation(&tenant_a, None, 0)
            .await,
        Err(SandboxSessionRepositoryError::InvalidPageRequest)
    );

    for (sandbox_invalid_state, sandbox_invalid_failure) in [
        ("destroyed", None),
        ("starting", None),
        ("failed", Some("provider")),
    ] {
        sqlx::query(
            "UPDATE sandbox_session SET \
                sandbox_session_state = $3, sandbox_last_failure = $4 \
             WHERE tenant_id = $1 AND sandbox_session_id = $2",
        )
        .bind(tenant_a.as_str())
        .bind(sandbox_session_id.as_str())
        .bind(sandbox_invalid_state)
        .bind(sandbox_invalid_failure)
        .execute(sandbox_postgres_pool)
        .await
        .unwrap_or_else(|error| panic!("sandbox invalid-state fixture update failed: {error}"));
        assert_eq!(
            sandbox_session_repository
                .get_sandbox_session(&tenant_a, &sandbox_session_id)
                .await,
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
    }
    sqlx::query(
        "UPDATE sandbox_session SET \
            sandbox_session_state = 'running', sandbox_last_failure = NULL \
         WHERE tenant_id = $1 AND sandbox_session_id = $2",
    )
    .bind(tenant_a.as_str())
    .bind(sandbox_session_id.as_str())
    .execute(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("sandbox valid-state fixture restore failed: {error}"));
    assert_eq!(
        sandbox_session_repository
            .get_sandbox_session(&tenant_a, &sandbox_session_id)
            .await
            .unwrap_or_else(|error| panic!("restored sandbox session lookup failed: {error}")),
        Some(sandbox_session.clone())
    );

    let sandbox_allocation_metadata_before: (String, String, i64, i16) = sqlx::query_as(
        "SELECT sandbox_allocation_ciphertext, sandbox_allocation_key_id, \
                sandbox_allocation_key_version, sandbox_allocation_crypto_version \
         FROM sandbox_runtime_binding \
         WHERE tenant_id = $1 AND sandbox_session_id = $2",
    )
    .bind(tenant_a.as_str())
    .bind(sandbox_session_id.as_str())
    .fetch_one(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("sandbox allocation metadata lookup failed: {error}"));
    let sandbox_allocation_ciphertext = sandbox_allocation_metadata_before.0.clone();
    assert_ne!(
        sandbox_allocation_ciphertext,
        sandbox_allocation_reference.expose_to_provider()
    );
    assert!(
        !sandbox_allocation_ciphertext.contains(sandbox_allocation_reference.expose_to_provider())
    );

    let sandbox_unsafe_key_id_error = sqlx::query(
        "UPDATE sandbox_runtime_binding \
         SET sandbox_allocation_key_id = $3 \
         WHERE tenant_id = $1 AND sandbox_session_id = $2",
    )
    .bind(tenant_a.as_str())
    .bind(sandbox_session_id.as_str())
    .bind("unsafe\nkey-id")
    .execute(sandbox_postgres_pool)
    .await
    .expect_err("PostgreSQL must reject a non-printable sandbox allocation key id");
    let sqlx::Error::Database(sandbox_database_error) = sandbox_unsafe_key_id_error else {
        panic!("unsafe sandbox allocation key id returned a non-database error");
    };
    assert_eq!(sandbox_database_error.code().as_deref(), Some("23514"));
    assert_eq!(
        sandbox_database_error.constraint(),
        Some("ck_sandbox_runtime_binding_allocation_metadata")
    );

    let sandbox_allocation_metadata_after: (String, String, i64, i16) = sqlx::query_as(
        "SELECT sandbox_allocation_ciphertext, sandbox_allocation_key_id, \
                sandbox_allocation_key_version, sandbox_allocation_crypto_version \
         FROM sandbox_runtime_binding \
         WHERE tenant_id = $1 AND sandbox_session_id = $2",
    )
    .bind(tenant_a.as_str())
    .bind(sandbox_session_id.as_str())
    .fetch_one(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("sandbox allocation metadata re-read failed: {error}"));
    assert_eq!(
        sandbox_allocation_metadata_after,
        sandbox_allocation_metadata_before
    );

    let conflicting_sandbox_session = sandbox_session_from_snapshot(
        tenant_a.clone(),
        parse_sandbox_session_id("session-postgres-conflict"),
        SandboxSessionState::Created,
        None,
        vec![SandboxSessionOperationRepositorySnapshot::new(
            sandbox_operation_id,
            SandboxSessionOperationKind::Create,
            SandboxOperationOutcome::Succeeded,
        )],
        0,
        sandbox_allocation_protector.as_ref(),
    );
    assert_eq!(
        sandbox_session_repository
            .insert_sandbox_session(conflicting_sandbox_session)
            .await,
        Err(SandboxSessionRepositoryError::DuplicateOperation)
    );

    let competing_sandbox_session_id = parse_sandbox_session_id("session-postgres-competing");
    let competing_sandbox_session = sandbox_session_from_snapshot(
        tenant_a.clone(),
        competing_sandbox_session_id.clone(),
        SandboxSessionState::Created,
        None,
        vec![SandboxSessionOperationRepositorySnapshot::new(
            OperationId::generate(),
            SandboxSessionOperationKind::Create,
            SandboxOperationOutcome::Succeeded,
        )],
        0,
        sandbox_allocation_protector.as_ref(),
    );
    sandbox_session_repository
        .insert_sandbox_session(competing_sandbox_session)
        .await
        .unwrap_or_else(|error| panic!("competing sandbox session insert failed: {error}"));
    let competing_first_sandbox_lease_owner_id = SandboxLeaseOwnerId::generate();
    let competing_second_sandbox_lease_owner_id = SandboxLeaseOwnerId::generate();
    let (competing_first_sandbox_lease, competing_second_sandbox_lease) = tokio::join!(
        sandbox_session_repository.acquire_sandbox_session_lease(
            &tenant_a,
            &competing_sandbox_session_id,
            &competing_first_sandbox_lease_owner_id,
            Duration::from_secs(30),
        ),
        sandbox_session_repository.acquire_sandbox_session_lease(
            &tenant_a,
            &competing_sandbox_session_id,
            &competing_second_sandbox_lease_owner_id,
            Duration::from_secs(30),
        )
    );
    let competing_first_sandbox_lease = competing_first_sandbox_lease
        .unwrap_or_else(|error| panic!("first concurrent sandbox lease failed: {error}"));
    let competing_second_sandbox_lease = competing_second_sandbox_lease
        .unwrap_or_else(|error| panic!("second concurrent sandbox lease failed: {error}"));
    let winning_competing_sandbox_lease = match (
        competing_first_sandbox_lease,
        competing_second_sandbox_lease,
    ) {
        (Some(winning_competing_sandbox_lease), None)
        | (None, Some(winning_competing_sandbox_lease)) => winning_competing_sandbox_lease,
        _ => panic!("exactly one concurrent sandbox lease owner must win"),
    };
    assert_eq!(
        winning_competing_sandbox_lease
            .sandbox_fencing_token()
            .value(),
        1
    );
    assert!(sandbox_session_repository
        .release_sandbox_session_lease(&winning_competing_sandbox_lease)
        .await
        .unwrap_or_else(|error| panic!("concurrent sandbox lease release failed: {error}")));
    sqlx::query(
        "UPDATE sandbox_session_lease SET sandbox_fencing_token = 9223372036854775807 \
         WHERE tenant_id = $1 AND sandbox_session_id = $2",
    )
    .bind(tenant_a.as_str())
    .bind(competing_sandbox_session_id.as_str())
    .execute(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("sandbox fencing token saturation failed: {error}"));
    assert_eq!(
        sandbox_session_repository
            .acquire_sandbox_session_lease(
                &tenant_a,
                &competing_sandbox_session_id,
                &competing_first_sandbox_lease_owner_id,
                Duration::from_secs(30),
            )
            .await,
        Err(SandboxSessionRepositoryError::LeaseConflict)
    );

    let first_sandbox_lease_owner_id = SandboxLeaseOwnerId::generate();
    let second_sandbox_lease_owner_id = SandboxLeaseOwnerId::generate();
    let first_sandbox_session_lease = sandbox_session_repository
        .acquire_sandbox_session_lease(
            &tenant_a,
            &sandbox_session_id,
            &first_sandbox_lease_owner_id,
            Duration::from_secs(30),
        )
        .await
        .unwrap_or_else(|error| panic!("first sandbox lease acquisition failed: {error}"))
        .unwrap_or_else(|| panic!("first sandbox lease must be acquired"));
    assert_eq!(
        first_sandbox_session_lease.sandbox_fencing_token().value(),
        1
    );
    assert!(sandbox_session_repository
        .acquire_sandbox_session_lease(
            &tenant_a,
            &sandbox_session_id,
            &second_sandbox_lease_owner_id,
            Duration::from_secs(30),
        )
        .await
        .unwrap_or_else(|error| panic!("competing sandbox lease lookup failed: {error}"))
        .is_none());

    let versioned_sandbox_session = sandbox_session_with_version(
        &stored_sandbox_session,
        1,
        sandbox_allocation_protector.as_ref(),
    );
    sandbox_session_repository
        .save_sandbox_session(
            versioned_sandbox_session.clone(),
            0,
            &first_sandbox_session_lease,
        )
        .await
        .unwrap_or_else(|error| panic!("sandbox version CAS save failed: {error}"));
    assert_eq!(
        sandbox_session_repository
            .save_sandbox_session(
                versioned_sandbox_session.clone(),
                0,
                &first_sandbox_session_lease,
            )
            .await,
        Err(SandboxSessionRepositoryError::VersionConflict)
    );
    assert!(sandbox_session_repository
        .release_sandbox_session_lease(&first_sandbox_session_lease)
        .await
        .unwrap_or_else(|error| panic!("first sandbox lease release failed: {error}")));

    let second_sandbox_session_lease = sandbox_session_repository
        .acquire_sandbox_session_lease(
            &tenant_a,
            &sandbox_session_id,
            &second_sandbox_lease_owner_id,
            Duration::from_millis(5),
        )
        .await
        .unwrap_or_else(|error| panic!("second sandbox lease acquisition failed: {error}"))
        .unwrap_or_else(|| panic!("second sandbox lease must be acquired"));
    assert_eq!(
        second_sandbox_session_lease.sandbox_fencing_token().value(),
        2
    );
    tokio::time::sleep(Duration::from_millis(15)).await;
    let takeover_sandbox_session_lease = sandbox_session_repository
        .acquire_sandbox_session_lease(
            &tenant_a,
            &sandbox_session_id,
            &first_sandbox_lease_owner_id,
            Duration::from_secs(30),
        )
        .await
        .unwrap_or_else(|error| panic!("takeover sandbox lease acquisition failed: {error}"))
        .unwrap_or_else(|| panic!("takeover sandbox lease must be acquired"));
    assert_eq!(
        takeover_sandbox_session_lease
            .sandbox_fencing_token()
            .value(),
        3
    );
    assert!(sandbox_session_repository
        .renew_sandbox_session_lease(&second_sandbox_session_lease, Duration::from_secs(30),)
        .await
        .unwrap_or_else(|error| panic!("stale sandbox lease renewal failed: {error}"))
        .is_none());
    assert!(!sandbox_session_repository
        .release_sandbox_session_lease(&second_sandbox_session_lease)
        .await
        .unwrap_or_else(|error| panic!("stale sandbox lease release failed: {error}")));
    assert_eq!(
        sandbox_session_repository
            .save_sandbox_session(versioned_sandbox_session, 0, &second_sandbox_session_lease,)
            .await,
        Err(SandboxSessionRepositoryError::LeaseConflict)
    );
    assert!(sandbox_session_repository
        .release_sandbox_session_lease(&takeover_sandbox_session_lease)
        .await
        .unwrap_or_else(|error| panic!("takeover sandbox lease release failed: {error}")));

    let sandbox_rotation_session_id = parse_sandbox_session_id("session-postgres-rotation-a-1");
    let sandbox_rotation_expected_session = sandbox_bound_session(
        tenant_a.clone(),
        sandbox_rotation_session_id.clone(),
        SandboxRuntimeBindingId::parse("binding-postgres-rotation-a-1")
            .unwrap_or_else(|error| panic!("invalid rotation binding: {error}")),
        "private-provider-allocation-rotation-a-1",
        sandbox_allocation_protector.as_ref(),
    );
    sandbox_session_repository
        .insert_sandbox_session(sandbox_rotation_expected_session.clone())
        .await
        .unwrap_or_else(|error| panic!("first rotation sandbox session insert failed: {error}"));
    sandbox_session_repository
        .insert_sandbox_session(sandbox_bound_session(
            tenant_a.clone(),
            parse_sandbox_session_id("session-postgres-rotation-a-2"),
            SandboxRuntimeBindingId::parse("binding-postgres-rotation-a-2")
                .unwrap_or_else(|error| panic!("invalid rotation binding: {error}")),
            "private-provider-allocation-rotation-a-2",
            sandbox_allocation_protector.as_ref(),
        ))
        .await
        .unwrap_or_else(|error| panic!("second rotation sandbox session insert failed: {error}"));
    sandbox_session_repository
        .insert_sandbox_session(sandbox_bound_session(
            tenant_b.clone(),
            parse_sandbox_session_id("session-postgres-rotation-b-1"),
            SandboxRuntimeBindingId::parse("binding-postgres-rotation-b-1")
                .unwrap_or_else(|error| panic!("invalid rotation binding: {error}")),
            "private-provider-allocation-rotation-b-1",
            sandbox_allocation_protector.as_ref(),
        ))
        .await
        .unwrap_or_else(|error| panic!("cross-tenant rotation session insert failed: {error}"));
    let sandbox_conflict_tenant = tenant_id("tenant-postgres-rotation-conflict");
    let sandbox_conflict_session_id =
        parse_sandbox_session_id("session-postgres-rotation-conflict");
    sandbox_session_repository
        .insert_sandbox_session(sandbox_bound_session(
            sandbox_conflict_tenant.clone(),
            sandbox_conflict_session_id.clone(),
            SandboxRuntimeBindingId::parse("binding-postgres-rotation-conflict")
                .unwrap_or_else(|error| panic!("invalid rotation binding: {error}")),
            "private-provider-allocation-rotation-conflict",
            sandbox_allocation_protector.as_ref(),
        ))
        .await
        .unwrap_or_else(|error| panic!("conflict rotation session insert failed: {error}"));
    let sandbox_session_cas_tenant = tenant_id("tenant-postgres-rotation-session-cas");
    let sandbox_session_cas_original_session_id =
        parse_sandbox_session_id("session-postgres-rotation-session-cas-original");
    let sandbox_session_cas_replacement_session_id =
        parse_sandbox_session_id("session-postgres-rotation-session-cas-replacement");
    let sandbox_session_cas_runtime_binding_id =
        SandboxRuntimeBindingId::parse("binding-postgres-rotation-session-cas")
            .unwrap_or_else(|error| panic!("invalid session CAS binding: {error}"));
    sandbox_session_repository
        .insert_sandbox_session(sandbox_bound_session(
            sandbox_session_cas_tenant.clone(),
            sandbox_session_cas_original_session_id.clone(),
            sandbox_session_cas_runtime_binding_id.clone(),
            "private-provider-allocation-rotation-session-cas",
            sandbox_allocation_protector.as_ref(),
        ))
        .await
        .unwrap_or_else(|error| panic!("session CAS rotation session insert failed: {error}"));
    sandbox_session_repository
        .insert_sandbox_session(sandbox_session_from_snapshot(
            sandbox_session_cas_tenant.clone(),
            sandbox_session_cas_replacement_session_id.clone(),
            SandboxSessionState::Created,
            None,
            vec![SandboxSessionOperationRepositorySnapshot::new(
                OperationId::generate(),
                SandboxSessionOperationKind::Create,
                SandboxOperationOutcome::Succeeded,
            )],
            0,
            sandbox_allocation_protector.as_ref(),
        ))
        .await
        .unwrap_or_else(|error| panic!("session CAS replacement session insert failed: {error}"));
    let sandbox_version_drift_tenant = tenant_id("tenant-postgres-rotation-version-drift");
    let sandbox_version_drift_session_id =
        parse_sandbox_session_id("session-postgres-rotation-version-drift");
    sandbox_session_repository
        .insert_sandbox_session(sandbox_bound_session(
            sandbox_version_drift_tenant.clone(),
            sandbox_version_drift_session_id.clone(),
            SandboxRuntimeBindingId::parse("binding-postgres-rotation-version-drift")
                .unwrap_or_else(|error| panic!("invalid version drift binding: {error}")),
            "private-provider-allocation-rotation-version-drift",
            sandbox_allocation_protector.as_ref(),
        ))
        .await
        .unwrap_or_else(|error| panic!("version drift rotation session insert failed: {error}"));

    sandbox_allocation_key_source.rotate_to_v2();
    sandbox_session_repository
        .insert_sandbox_session(sandbox_bound_session(
            tenant_a.clone(),
            parse_sandbox_session_id("session-postgres-rotation-current"),
            SandboxRuntimeBindingId::parse("binding-postgres-rotation-current")
                .unwrap_or_else(|error| panic!("invalid current binding: {error}")),
            "private-provider-allocation-rotation-current",
            sandbox_allocation_protector.as_ref(),
        ))
        .await
        .unwrap_or_else(|error| panic!("current-key sandbox session insert failed: {error}"));

    assert_eq!(
        sandbox_session_repository
            .reencrypt_sandbox_provider_allocation_references_page(&tenant_a, None, 0)
            .await,
        Err(SandboxSessionRepositoryError::InvalidPageRequest)
    );
    assert_eq!(
        sandbox_session_repository
            .reencrypt_sandbox_provider_allocation_references_page(&tenant_a, None, 201)
            .await,
        Err(SandboxSessionRepositoryError::InvalidPageRequest)
    );

    let mut sandbox_rotation_cursor = None;
    let mut sandbox_rotation_page_count = 0;
    let mut sandbox_rotation_scanned_count = 0;
    let mut sandbox_rotation_reencrypted_count = 0;
    loop {
        let sandbox_rotation_page = sandbox_session_repository
            .reencrypt_sandbox_provider_allocation_references_page(
                &tenant_a,
                sandbox_rotation_cursor.as_ref(),
                1,
            )
            .await
            .unwrap_or_else(|error| panic!("sandbox allocation rotation page failed: {error}"));
        let sandbox_rotation_page_debug = format!("{sandbox_rotation_page:?}");
        assert!(!sandbox_rotation_page_debug.contains("private-provider-allocation"));
        assert!(!sandbox_rotation_page_debug.contains(&sandbox_allocation_ciphertext));
        sandbox_rotation_page_count += 1;
        sandbox_rotation_scanned_count += sandbox_rotation_page.sandbox_scanned_count();
        sandbox_rotation_reencrypted_count += sandbox_rotation_page.sandbox_reencrypted_count();
        assert_eq!(sandbox_rotation_page.sandbox_conflict_count(), 0);
        assert!(
            sandbox_rotation_page_count <= 4,
            "sandbox rotation cursor did not converge"
        );
        sandbox_rotation_cursor = sandbox_rotation_page
            .sandbox_next_runtime_binding_id()
            .cloned();
        if sandbox_rotation_cursor.is_none() {
            break;
        }
    }
    assert_eq!(sandbox_rotation_page_count, 3);
    assert_eq!(sandbox_rotation_scanned_count, 3);
    assert_eq!(sandbox_rotation_reencrypted_count, 3);
    // The settlement loop must confirm the sweep in one clean pass once
    // every row is already on the current key.
    let sandbox_settlement = sandbox_session_repository
        .reencrypt_sandbox_provider_allocation_references_until_settled(&tenant_a, 2)
        .await
        .unwrap_or_else(|error| panic!("sandbox allocation settlement failed: {error}"));
    assert_eq!(sandbox_settlement.sandbox_passes(), 1);
    assert_eq!(sandbox_settlement.sandbox_scanned_count(), 0);
    assert_eq!(sandbox_settlement.sandbox_reencrypted_count(), 0);

    let sandbox_tenant_a_key_versions: Vec<i64> = sqlx::query_scalar(
        "SELECT sandbox_allocation_key_version FROM sandbox_runtime_binding \
         WHERE tenant_id = $1 AND sandbox_allocation_ciphertext IS NOT NULL \
         ORDER BY sandbox_runtime_binding_id",
    )
    .bind(tenant_a.as_str())
    .fetch_all(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("tenant A key version lookup failed: {error}"));
    assert_eq!(sandbox_tenant_a_key_versions, vec![2, 2, 2, 2]);
    let sandbox_tenant_b_key_version: i64 = sqlx::query_scalar(
        "SELECT sandbox_allocation_key_version FROM sandbox_runtime_binding \
         WHERE tenant_id = $1 AND sandbox_allocation_ciphertext IS NOT NULL",
    )
    .bind(tenant_b.as_str())
    .fetch_one(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("tenant B key version lookup failed: {error}"));
    assert_eq!(sandbox_tenant_b_key_version, 1);

    let sandbox_tenant_b_rotation_page = sandbox_session_repository
        .reencrypt_sandbox_provider_allocation_references_page(&tenant_b, None, 20)
        .await
        .unwrap_or_else(|error| panic!("tenant B allocation rotation failed: {error}"));
    assert_eq!(sandbox_tenant_b_rotation_page.sandbox_scanned_count(), 1);
    assert_eq!(
        sandbox_tenant_b_rotation_page.sandbox_reencrypted_count(),
        1
    );
    assert_eq!(sandbox_tenant_b_rotation_page.sandbox_conflict_count(), 0);
    assert!(sandbox_tenant_b_rotation_page
        .sandbox_next_runtime_binding_id()
        .is_none());
    let sandbox_tenant_a_second_scan = sandbox_session_repository
        .reencrypt_sandbox_provider_allocation_references_page(&tenant_a, None, 200)
        .await
        .unwrap_or_else(|error| panic!("tenant A second allocation scan failed: {error}"));
    assert_eq!(sandbox_tenant_a_second_scan.sandbox_scanned_count(), 0);
    assert_eq!(sandbox_tenant_a_second_scan.sandbox_reencrypted_count(), 0);

    let sandbox_conflict_session = sandbox_session_repository
        .get_sandbox_session(&sandbox_conflict_tenant, &sandbox_conflict_session_id)
        .await
        .unwrap_or_else(|error| panic!("conflict sandbox session lookup failed: {error}"))
        .unwrap_or_else(|| panic!("conflict sandbox session must exist"));
    let sandbox_conflict_versioned_session = sandbox_session_with_version(
        &sandbox_conflict_session,
        1,
        sandbox_allocation_protector.as_ref(),
    );
    let sandbox_conflict_lease = sandbox_session_repository
        .acquire_sandbox_session_lease(
            &sandbox_conflict_tenant,
            &sandbox_conflict_session_id,
            &SandboxLeaseOwnerId::generate(),
            Duration::from_secs(30),
        )
        .await
        .unwrap_or_else(|error| panic!("conflict sandbox lease failed: {error}"))
        .unwrap_or_else(|| panic!("conflict sandbox lease must be acquired"));
    let (sandbox_historical_key_entered, sandbox_historical_key_resume) =
        sandbox_test_allocation_protector.pause_next_sandbox_reencryption();
    let sandbox_conflict_repository = Arc::clone(&sandbox_session_repository);
    let sandbox_conflict_tenant_for_task = sandbox_conflict_tenant.clone();
    let sandbox_reencryption_task = tokio::spawn(async move {
        sandbox_conflict_repository
            .reencrypt_sandbox_provider_allocation_references_page(
                &sandbox_conflict_tenant_for_task,
                None,
                20,
            )
            .await
    });
    let sandbox_lifecycle_repository = Arc::clone(&sandbox_session_repository);
    let sandbox_lifecycle_database_pool = sandbox_database_pool.clone();
    let sandbox_lifecycle_tenant = sandbox_conflict_tenant.clone();
    let sandbox_lifecycle_session_id = sandbox_conflict_session_id.clone();
    let sandbox_lifecycle_versioned_session = sandbox_conflict_versioned_session.clone();
    let sandbox_lifecycle_lease = sandbox_conflict_lease.clone();
    let sandbox_lifecycle_task = tokio::spawn(async move {
        tokio::task::spawn_blocking(move || {
            sandbox_historical_key_entered
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|error| panic!("historical key lookup did not pause: {error}"));
        })
        .await
        .unwrap_or_else(|error| panic!("historical key pause task failed: {error}"));
        sandbox_lifecycle_repository
            .save_sandbox_session(
                sandbox_lifecycle_versioned_session,
                0,
                &sandbox_lifecycle_lease,
            )
            .await
            .unwrap_or_else(|error| panic!("concurrent lifecycle save failed: {error}"));
        let sandbox_lifecycle_postgres_pool = sandbox_lifecycle_database_pool
            .as_postgres()
            .unwrap_or_else(|| panic!("sandbox test database must remain PostgreSQL"));
        let sandbox_lifecycle_ciphertext: String = sqlx::query_scalar(
            "SELECT sandbox_allocation_ciphertext FROM sandbox_runtime_binding \
             WHERE tenant_id = $1 AND sandbox_session_id = $2",
        )
        .bind(sandbox_lifecycle_tenant.as_str())
        .bind(sandbox_lifecycle_session_id.as_str())
        .fetch_one(sandbox_lifecycle_postgres_pool)
        .await
        .unwrap_or_else(|error| panic!("lifecycle ciphertext lookup failed: {error}"));
        sandbox_historical_key_resume
            .send(())
            .unwrap_or_else(|error| panic!("historical key lookup did not resume: {error}"));
        sandbox_lifecycle_ciphertext
    });
    let (sandbox_conflict_rotation_page, sandbox_lifecycle_ciphertext) = tokio::join!(
        async {
            sandbox_reencryption_task
                .await
                .unwrap_or_else(|error| panic!("sandbox re-encryption task failed: {error}"))
                .unwrap_or_else(|error| panic!("sandbox conflict rotation failed: {error}"))
        },
        async {
            sandbox_lifecycle_task
                .await
                .unwrap_or_else(|error| panic!("sandbox lifecycle task failed: {error}"))
        }
    );
    assert_eq!(sandbox_conflict_rotation_page.sandbox_scanned_count(), 1);
    assert_eq!(
        sandbox_conflict_rotation_page.sandbox_reencrypted_count(),
        0
    );
    assert_eq!(sandbox_conflict_rotation_page.sandbox_conflict_count(), 1);
    let sandbox_ciphertext_after_conflict: String = sqlx::query_scalar(
        "SELECT sandbox_allocation_ciphertext FROM sandbox_runtime_binding \
         WHERE tenant_id = $1 AND sandbox_session_id = $2",
    )
    .bind(sandbox_conflict_tenant.as_str())
    .bind(sandbox_conflict_session_id.as_str())
    .fetch_one(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("post-conflict ciphertext lookup failed: {error}"));
    assert_eq!(
        sandbox_ciphertext_after_conflict,
        sandbox_lifecycle_ciphertext
    );
    let sandbox_conflict_second_scan = sandbox_session_repository
        .reencrypt_sandbox_provider_allocation_references_page(&sandbox_conflict_tenant, None, 20)
        .await
        .unwrap_or_else(|error| panic!("conflict tenant second scan failed: {error}"));
    assert_eq!(sandbox_conflict_second_scan.sandbox_scanned_count(), 0);
    assert!(sandbox_session_repository
        .release_sandbox_session_lease(&sandbox_conflict_lease)
        .await
        .unwrap_or_else(|error| panic!("conflict sandbox lease release failed: {error}")));

    let sandbox_session_cas_ciphertext_before: String = sqlx::query_scalar(
        "SELECT sandbox_allocation_ciphertext FROM sandbox_runtime_binding \
         WHERE tenant_id = $1 AND sandbox_runtime_binding_id = $2",
    )
    .bind(sandbox_session_cas_tenant.as_str())
    .bind(sandbox_session_cas_runtime_binding_id.as_str())
    .fetch_one(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("session CAS ciphertext lookup failed: {error}"));
    let sandbox_session_cas_database_pool = sandbox_postgres_test_pool_builder()
        .max_connections(1)
        .build()
        .await
        .unwrap_or_else(|error| panic!("session CAS database pool failed: {error}"));
    let (sandbox_session_cas_entered, sandbox_session_cas_resume) =
        sandbox_test_allocation_protector.pause_next_sandbox_reencryption();
    let sandbox_session_cas_repository = Arc::clone(&sandbox_session_repository);
    let sandbox_session_cas_tenant_for_task = sandbox_session_cas_tenant.clone();
    let sandbox_session_cas_task = tokio::spawn(async move {
        sandbox_session_cas_repository
            .reencrypt_sandbox_provider_allocation_references_page(
                &sandbox_session_cas_tenant_for_task,
                None,
                20,
            )
            .await
    });
    let sandbox_session_cas_tenant_for_update = sandbox_session_cas_tenant.clone();
    let sandbox_session_cas_runtime_binding_id_for_update =
        sandbox_session_cas_runtime_binding_id.clone();
    let sandbox_session_cas_replacement_session_id_for_update =
        sandbox_session_cas_replacement_session_id.clone();
    let sandbox_session_cas_update_task = tokio::spawn(async move {
        tokio::task::spawn_blocking(move || {
            sandbox_session_cas_entered
                .recv_timeout(Duration::from_secs(10))
                .unwrap_or_else(|error| panic!("session CAS re-encryption did not pause: {error}"));
        })
        .await
        .unwrap_or_else(|error| panic!("session CAS pause task failed: {error}"));
        let sandbox_session_cas_postgres_pool = sandbox_session_cas_database_pool
            .as_postgres()
            .unwrap_or_else(|| panic!("session CAS database must use PostgreSQL"));
        sqlx::query(
            "UPDATE sandbox_runtime_binding SET sandbox_session_id = $3 \
             WHERE tenant_id = $1 AND sandbox_runtime_binding_id = $2",
        )
        .bind(sandbox_session_cas_tenant_for_update.as_str())
        .bind(sandbox_session_cas_runtime_binding_id_for_update.as_str())
        .bind(sandbox_session_cas_replacement_session_id_for_update.as_str())
        .execute(sandbox_session_cas_postgres_pool)
        .await
        .unwrap_or_else(|error| panic!("session CAS concurrent replacement failed: {error}"));
        sandbox_session_cas_resume
            .send(())
            .unwrap_or_else(|error| panic!("session CAS re-encryption did not resume: {error}"));
    });
    let (sandbox_session_cas_page, ()) = tokio::join!(
        async {
            sandbox_session_cas_task
                .await
                .unwrap_or_else(|error| panic!("session CAS re-encryption task failed: {error}"))
                .unwrap_or_else(|error| panic!("session CAS re-encryption failed: {error}"))
        },
        async {
            sandbox_session_cas_update_task
                .await
                .unwrap_or_else(|error| panic!("session CAS update task failed: {error}"))
        }
    );
    assert_eq!(sandbox_session_cas_page.sandbox_scanned_count(), 1);
    assert_eq!(sandbox_session_cas_page.sandbox_reencrypted_count(), 0);
    assert_eq!(sandbox_session_cas_page.sandbox_conflict_count(), 1);
    let sandbox_session_cas_ciphertext_after: String = sqlx::query_scalar(
        "SELECT sandbox_allocation_ciphertext FROM sandbox_runtime_binding \
         WHERE tenant_id = $1 AND sandbox_runtime_binding_id = $2",
    )
    .bind(sandbox_session_cas_tenant.as_str())
    .bind(sandbox_session_cas_runtime_binding_id.as_str())
    .fetch_one(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("session CAS post-conflict lookup failed: {error}"));
    assert_eq!(
        sandbox_session_cas_ciphertext_after,
        sandbox_session_cas_ciphertext_before
    );
    sqlx::query(
        "UPDATE sandbox_runtime_binding SET sandbox_session_id = $3 \
         WHERE tenant_id = $1 AND sandbox_runtime_binding_id = $2",
    )
    .bind(sandbox_session_cas_tenant.as_str())
    .bind(sandbox_session_cas_runtime_binding_id.as_str())
    .bind(sandbox_session_cas_original_session_id.as_str())
    .execute(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("session CAS test cleanup failed: {error}"));
    let sandbox_session_cas_retry_page = sandbox_session_repository
        .reencrypt_sandbox_provider_allocation_references_page(
            &sandbox_session_cas_tenant,
            None,
            20,
        )
        .await
        .unwrap_or_else(|error| panic!("session CAS retry failed: {error}"));
    assert_eq!(sandbox_session_cas_retry_page.sandbox_scanned_count(), 1);
    assert_eq!(
        sandbox_session_cas_retry_page.sandbox_reencrypted_count(),
        1
    );
    assert_eq!(sandbox_session_cas_retry_page.sandbox_conflict_count(), 0);

    let sandbox_version_drift_ciphertext_before: String = sqlx::query_scalar(
        "SELECT sandbox_allocation_ciphertext FROM sandbox_runtime_binding \
         WHERE tenant_id = $1 AND sandbox_session_id = $2",
    )
    .bind(sandbox_version_drift_tenant.as_str())
    .bind(sandbox_version_drift_session_id.as_str())
    .fetch_one(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("version drift ciphertext lookup failed: {error}"));
    let (sandbox_version_drift_entered, sandbox_version_drift_resume) =
        sandbox_test_allocation_protector.pause_next_sandbox_reencryption();
    let sandbox_version_drift_repository = Arc::clone(&sandbox_session_repository);
    let sandbox_version_drift_tenant_for_task = sandbox_version_drift_tenant.clone();
    let sandbox_version_drift_task = tokio::spawn(async move {
        sandbox_version_drift_repository
            .reencrypt_sandbox_provider_allocation_references_page(
                &sandbox_version_drift_tenant_for_task,
                None,
                20,
            )
            .await
    });
    tokio::task::spawn_blocking(move || {
        sandbox_version_drift_entered
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|error| panic!("version drift re-encryption did not pause: {error}"));
    })
    .await
    .unwrap_or_else(|error| panic!("version drift pause task failed: {error}"));
    sandbox_allocation_key_source.rotate_to_v3();
    sandbox_version_drift_resume
        .send(())
        .unwrap_or_else(|error| panic!("version drift re-encryption did not resume: {error}"));
    assert_eq!(
        sandbox_version_drift_task
            .await
            .unwrap_or_else(|error| panic!("version drift task failed: {error}")),
        Err(SandboxSessionRepositoryError::ProtectionFailed)
    );
    let sandbox_version_drift_metadata_after_failure: (String, i64) = sqlx::query_as(
        "SELECT sandbox_allocation_ciphertext, sandbox_allocation_key_version \
         FROM sandbox_runtime_binding \
         WHERE tenant_id = $1 AND sandbox_session_id = $2",
    )
    .bind(sandbox_version_drift_tenant.as_str())
    .bind(sandbox_version_drift_session_id.as_str())
    .fetch_one(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("version drift metadata lookup failed: {error}"));
    assert_eq!(
        sandbox_version_drift_metadata_after_failure,
        (sandbox_version_drift_ciphertext_before, 1)
    );

    let sandbox_version_drift_retry_page = sandbox_session_repository
        .reencrypt_sandbox_provider_allocation_references_page(
            &sandbox_version_drift_tenant,
            None,
            20,
        )
        .await
        .unwrap_or_else(|error| panic!("version drift retry failed: {error}"));
    assert_eq!(sandbox_version_drift_retry_page.sandbox_scanned_count(), 1);
    assert_eq!(
        sandbox_version_drift_retry_page.sandbox_reencrypted_count(),
        1
    );
    assert_eq!(sandbox_version_drift_retry_page.sandbox_conflict_count(), 0);
    let sandbox_version_after_retry: i64 = sqlx::query_scalar(
        "SELECT sandbox_allocation_key_version FROM sandbox_runtime_binding \
         WHERE tenant_id = $1 AND sandbox_session_id = $2",
    )
    .bind(sandbox_version_drift_tenant.as_str())
    .bind(sandbox_version_drift_session_id.as_str())
    .fetch_one(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("version drift retry version lookup failed: {error}"));
    assert_eq!(sandbox_version_after_retry, 3);

    let transient_sandbox_session_id = parse_sandbox_session_id("session-postgres-reconcile");
    let transient_sandbox_session = sandbox_session_from_snapshot(
        tenant_a.clone(),
        transient_sandbox_session_id.clone(),
        SandboxSessionState::Starting,
        Some(SandboxRuntimeBindingRepositorySnapshot::new(
            SandboxId::generate(),
            SandboxRuntimeBindingId::generate(),
            parse_sandbox_provider_id("provider-postgres-test"),
            None,
        )),
        vec![
            SandboxSessionOperationRepositorySnapshot::new(
                OperationId::generate(),
                SandboxSessionOperationKind::Create,
                SandboxOperationOutcome::Succeeded,
            ),
            SandboxSessionOperationRepositorySnapshot::new(
                OperationId::generate(),
                SandboxSessionOperationKind::Start,
                SandboxOperationOutcome::InProgress,
            ),
        ],
        0,
        sandbox_allocation_protector.as_ref(),
    );
    sandbox_session_repository
        .insert_sandbox_session(transient_sandbox_session)
        .await
        .unwrap_or_else(|error| panic!("transient sandbox session insert failed: {error}"));
    let restarted_sandbox_session_repository = Arc::new(
        SqlxSandboxSessionRepository::new(
            sandbox_database_pool.clone(),
            Arc::clone(&sandbox_allocation_protector),
        )
        .unwrap_or_else(|error| panic!("restarted sandbox repository creation failed: {error}")),
    );
    assert_eq!(
        restarted_sandbox_session_repository
            .get_sandbox_session(&tenant_a, &sandbox_rotation_session_id)
            .await
            .unwrap_or_else(|error| panic!("restarted rotation session lookup failed: {error}")),
        Some(sandbox_rotation_expected_session)
    );
    assert_eq!(
        restarted_sandbox_session_repository
            .get_sandbox_session(&sandbox_conflict_tenant, &sandbox_conflict_session_id)
            .await
            .unwrap_or_else(|error| panic!("restarted conflict session lookup failed: {error}")),
        Some(sandbox_conflict_versioned_session)
    );
    let sandbox_lifecycle_service =
        sdkwork_intelligence_sandbox_service::SandboxLifecycleService::new(
            restarted_sandbox_session_repository,
            vec![Arc::new(TestSandboxProvider::new())],
        )
        .unwrap_or_else(|error| panic!("restarted sandbox lifecycle service failed: {error}"));
    let sandbox_reconciliation_page = sandbox_lifecycle_service
        .reconcile_sandbox_sessions(&tenant_a, None, 20)
        .await
        .unwrap_or_else(|error| panic!("sandbox PostgreSQL reconciliation failed: {error}"));
    assert_eq!(sandbox_reconciliation_page.sandbox_items().len(), 1);
    assert_eq!(
        sandbox_reconciliation_page.sandbox_items()[0].sandbox_reconciliation_outcome(),
        SandboxSessionReconciliationOutcome::Reconciled
    );
    assert_eq!(
        sandbox_session_repository
            .get_sandbox_session(&tenant_a, &transient_sandbox_session_id)
            .await
            .unwrap_or_else(|error| panic!("reconciled sandbox session lookup failed: {error}"))
            .map(|sandbox_session| sandbox_session.sandbox_session_state()),
        Some(SandboxSessionState::Running)
    );

    let sandbox_query_plan: serde_json::Value = sqlx::query_scalar(
        "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) \
         SELECT tenant_id, sandbox_session_id FROM sandbox_session \
         WHERE tenant_id = $1 AND sandbox_session_state = 'starting' \
           AND sandbox_session_id > $2 ORDER BY sandbox_session_id LIMIT 20",
    )
    .bind(tenant_a.as_str())
    .bind("")
    .fetch_one(sandbox_postgres_pool)
    .await
    .unwrap_or_else(|error| panic!("sandbox reconciliation query plan failed: {error}"));
    assert!(sandbox_query_plan.is_array());

    // Instance-registry section: the console listing is the hottest read and
    // its keyset seek compares the cursor against the stored microsecond
    // `created_at`. A second-truncated cursor would move the page boundary
    // earlier and silently skip rows created later within the same second
    // (`PAGINATION_SPEC.md` sections 5-6), so this section enumerates a
    // same-second batch through two-row pages and asserts the canonical
    // six-digit fraction survives every read path.
    sqlx::raw_sql("TRUNCATE TABLE sandbox_instance")
        .execute(sandbox_postgres_pool)
        .await
        .unwrap_or_else(|error| panic!("sandbox instance cleanup failed: {error}"));
    let sandbox_instance_repository =
        SqlxSandboxInstanceRepository::new(sandbox_database_pool.clone())
            .unwrap_or_else(|error| panic!("sandbox instance repository creation failed: {error}"));
    let sandbox_listing_owner = SandboxInstanceOwnerId::parse("owner-instance-listing")
        .unwrap_or_else(|error| panic!("invalid test sandbox instance owner id: {error}"));
    let mut sandbox_expected_listing_ids = Vec::new();
    for sandbox_ordinal in 0..6 {
        let sandbox_instance = SandboxInstance::request(CreateSandboxInstanceCommand {
            tenant_id: tenant_a.clone(),
            sandbox_instance_owner_id: sandbox_listing_owner.clone(),
            sandbox_instance_name: format!("listing-{sandbox_ordinal}"),
            sandbox_instance_profile: SandboxInstanceProfile::Standard,
            sandbox_instance_base_image: "sdkwork/sandbox:0.1.0".to_owned(),
            sandbox_instance_vcpu_count: 2,
            sandbox_instance_memory_mb: 4_096,
            sandbox_instance_disk_mb: 20_480,
            sandbox_instance_required_capabilities: BTreeSet::new(),
            sandbox_instance_minimum_assurance: IsolationAssurance::HostUser,
            sandbox_instance_auto_start: false,
            sandbox_instance_expires_at: None,
            sandbox_workspace_id: None,
        })
        .unwrap_or_else(|error| panic!("sandbox instance request failed: {error}"));
        let sandbox_stored_instance = sandbox_instance_repository
            .insert_sandbox_instance(&sandbox_instance)
            .await
            .unwrap_or_else(|error| panic!("sandbox instance insert failed: {error}"));
        let sandbox_stored_created_at = sandbox_stored_instance
            .created_at()
            .unwrap_or_else(|| panic!("stored sandbox instance must carry created_at"));
        assert_eq!(
            27,
            sandbox_stored_created_at.len(),
            "created_at must re-emit with the six-digit fraction: {sandbox_stored_created_at}"
        );
        assert!(
            sandbox_stored_created_at.as_bytes()[19] == b'.',
            "created_at must carry a fraction, not a second-truncated shape: \
             {sandbox_stored_created_at}"
        );
        sandbox_expected_listing_ids.push(
            sandbox_stored_instance
                .sandbox_instance_id()
                .as_str()
                .to_owned(),
        );
    }
    let mut sandbox_visited_listing_ids = Vec::new();
    let mut sandbox_listing_cursor: Option<SandboxInstanceListCursor> = None;
    loop {
        let sandbox_listing_page = sandbox_instance_repository
            .list_sandbox_instances(&tenant_a, None, None, sandbox_listing_cursor.as_ref(), 2)
            .await
            .unwrap_or_else(|error| panic!("sandbox instance listing failed: {error}"));
        for sandbox_instance in &sandbox_listing_page.items {
            sandbox_visited_listing_ids
                .push(sandbox_instance.sandbox_instance_id().as_str().to_owned());
        }
        match sandbox_listing_page.next_cursor {
            Some(sandbox_next_cursor) => sandbox_listing_cursor = Some(sandbox_next_cursor),
            None => break,
        }
    }
    let mut sandbox_sorted_expected = sandbox_expected_listing_ids.clone();
    sandbox_sorted_expected.sort();
    let mut sandbox_sorted_visited = sandbox_visited_listing_ids.clone();
    sandbox_sorted_visited.sort();
    assert_eq!(
        sandbox_sorted_expected, sandbox_sorted_visited,
        "cursor paging must visit every same-second instance exactly once"
    );
    assert_eq!(6, sandbox_visited_listing_ids.len());

    sandbox_database_pool.close().await;
}
