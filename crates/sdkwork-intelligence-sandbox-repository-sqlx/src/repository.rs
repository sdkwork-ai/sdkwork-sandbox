use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sdkwork_database_sqlx::DatabasePool;
use sdkwork_intelligence_sandbox_service::{
    SandboxProtectedProviderAllocationRef, SandboxProviderAllocationProtector,
    SandboxRuntimeBindingRepositorySnapshot, SandboxSession, SandboxSessionLease,
    SandboxSessionOperationRepositorySnapshot, SandboxSessionRepository,
    SandboxSessionRepositoryError, SandboxSessionRepositoryResult,
    SandboxSessionRepositorySnapshot,
};
use sdkwork_sandbox_provider_spi::{
    OperationId, SandboxFencingToken, SandboxId, SandboxLeaseOwnerId, SandboxProviderId,
    SandboxRuntimeBindingId, SandboxSessionId, SandboxWorkspaceId, TenantId,
};
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, PgPool, Row};

use crate::codec::{
    parse_sandbox_isolation_assurance, parse_sandbox_operation_kind,
    parse_sandbox_operation_outcome, parse_sandbox_runtime_capabilities,
    parse_sandbox_session_failure, parse_sandbox_session_state, sandbox_isolation_assurance_value,
    sandbox_operation_kind_value, sandbox_operation_outcome_values,
    sandbox_runtime_capabilities_value, sandbox_session_failure_value, sandbox_session_state_value,
};

/// Maximum lifecycle operations loaded for one sandbox session.
///
/// Safety bound until REQ-2026-0020 (bounded lifecycle history and idempotency
/// retention) authorizes a retention policy. A persisted session whose
/// operation history exceeds this bound fails closed instead of loading an
/// unbounded row set into process memory, keeping repository reads bounded.
pub const MAX_SANDBOX_SESSION_OPERATIONS: usize = 10_000;

/// Statement timeout applied to every sandbox repository transaction,
/// matching the authoritative-server baseline header contract
/// (`database/ddl/baseline/postgres/0001_sandbox_baseline.sql`).
const SANDBOX_DATABASE_STATEMENT_TIMEOUT: &str = "30s";

/// Lock timeout applied to every sandbox repository transaction, matching the
/// authoritative-server baseline header contract.
const SANDBOX_DATABASE_LOCK_TIMEOUT: &str = "2s";

pub struct SqlxSandboxSessionRepository {
    sandbox_database_pool: DatabasePool,
    sandbox_allocation_protector: Arc<dyn SandboxProviderAllocationProtector>,
}

impl SqlxSandboxSessionRepository {
    pub fn new(
        sandbox_database_pool: DatabasePool,
        sandbox_allocation_protector: Arc<dyn SandboxProviderAllocationProtector>,
    ) -> SandboxSessionRepositoryResult<Self> {
        if sandbox_database_pool.as_postgres().is_none() {
            return Err(SandboxSessionRepositoryError::UnsupportedDatabaseEngine);
        }
        Ok(Self {
            sandbox_database_pool,
            sandbox_allocation_protector,
        })
    }

    pub(crate) fn sandbox_postgres_pool(&self) -> SandboxSessionRepositoryResult<&PgPool> {
        self.sandbox_database_pool
            .as_postgres()
            .ok_or(SandboxSessionRepositoryError::UnsupportedDatabaseEngine)
    }

    pub(crate) fn map_sandbox_sqlx_error(error: sqlx::Error) -> SandboxSessionRepositoryError {
        let sqlx::Error::Database(database_error) = &error else {
            return SandboxSessionRepositoryError::Unavailable;
        };
        match database_error.code().as_deref() {
            Some("23505") => {
                if database_error.constraint() == Some("pk_sandbox_session_operation") {
                    SandboxSessionRepositoryError::DuplicateOperation
                } else if database_error.constraint()
                    == Some("uk_sandbox_session_operation_sequence")
                {
                    SandboxSessionRepositoryError::InvalidStoredData
                } else {
                    SandboxSessionRepositoryError::VersionConflict
                }
            }
            Some("23502" | "23503" | "23514") => SandboxSessionRepositoryError::InvalidStoredData,
            Some("40001" | "40P01" | "55P03" | "57014") => {
                SandboxSessionRepositoryError::Unavailable
            }
            _ => SandboxSessionRepositoryError::Unavailable,
        }
    }

    pub(crate) fn sandbox_allocation_protector(&self) -> &dyn SandboxProviderAllocationProtector {
        self.sandbox_allocation_protector.as_ref()
    }

    fn sandbox_version_to_i64(sandbox_version: u64) -> SandboxSessionRepositoryResult<i64> {
        i64::try_from(sandbox_version).map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)
    }

    fn sandbox_version_from_i64(sandbox_version: i64) -> SandboxSessionRepositoryResult<u64> {
        u64::try_from(sandbox_version).map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)
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

    fn sandbox_session_lease_from_row(
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
        sandbox_lease_owner_id: &SandboxLeaseOwnerId,
        sandbox_lease_row: &sqlx::postgres::PgRow,
    ) -> SandboxSessionRepositoryResult<SandboxSessionLease> {
        let sandbox_fencing_token: i64 = sandbox_lease_row
            .try_get("sandbox_fencing_token")
            .map_err(Self::map_sandbox_sqlx_error)?;
        let sandbox_lease_expires_at_unix_millis: i64 = sandbox_lease_row
            .try_get("sandbox_lease_expires_at_unix_millis")
            .map_err(Self::map_sandbox_sqlx_error)?;
        SandboxSessionLease::new(
            tenant_id.clone(),
            sandbox_session_id.clone(),
            sandbox_lease_owner_id.clone(),
            SandboxFencingToken::new(
                u64::try_from(sandbox_fencing_token)
                    .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
            )
            .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
            sandbox_lease_expires_at_unix_millis,
        )
    }

    async fn lock_valid_sandbox_session_lease(
        sandbox_connection: &mut PgConnection,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<()> {
        let sandbox_lease_is_valid: Option<i32> = sqlx::query_scalar(
            "SELECT 1 \
             FROM sandbox_session_lease \
             WHERE tenant_id = $1 \
               AND sandbox_session_id = $2 \
               AND sandbox_lease_owner_id = $3 \
               AND sandbox_fencing_token = $4 \
               AND sandbox_lease_expires_at > CURRENT_TIMESTAMP \
             FOR UPDATE",
        )
        .bind(sandbox_session_lease.tenant_id().as_str())
        .bind(sandbox_session_lease.sandbox_session_id().as_str())
        .bind(sandbox_session_lease.sandbox_lease_owner_id().as_str())
        .bind(
            i64::try_from(sandbox_session_lease.sandbox_fencing_token().value())
                .map_err(|_| SandboxSessionRepositoryError::LeaseConflict)?,
        )
        .fetch_optional(&mut *sandbox_connection)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        if sandbox_lease_is_valid.is_none() {
            return Err(SandboxSessionRepositoryError::LeaseConflict);
        }
        Ok(())
    }

    async fn enforce_sandbox_transaction_timeouts(
        sandbox_connection: &mut PgConnection,
    ) -> SandboxSessionRepositoryResult<()> {
        sqlx::query("SET LOCAL statement_timeout = $1")
            .bind(SANDBOX_DATABASE_STATEMENT_TIMEOUT)
            .execute(&mut *sandbox_connection)
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        sqlx::query("SET LOCAL lock_timeout = $1")
            .bind(SANDBOX_DATABASE_LOCK_TIMEOUT)
            .execute(&mut *sandbox_connection)
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        Ok(())
    }

    fn parse_sandbox_runtime_binding_row(
        sandbox_runtime_binding_row: &PgRow,
    ) -> SandboxSessionRepositoryResult<SandboxRuntimeBindingRepositorySnapshot> {
        let sandbox_id: String = sandbox_runtime_binding_row
            .try_get("sandbox_id")
            .map_err(Self::map_sandbox_sqlx_error)?;
        let sandbox_runtime_binding_id: String = sandbox_runtime_binding_row
            .try_get("sandbox_runtime_binding_id")
            .map_err(Self::map_sandbox_sqlx_error)?;
        let sandbox_provider_id: String = sandbox_runtime_binding_row
            .try_get("sandbox_provider_id")
            .map_err(Self::map_sandbox_sqlx_error)?;
        let sandbox_allocation_ciphertext: Option<String> = sandbox_runtime_binding_row
            .try_get("sandbox_allocation_ciphertext")
            .map_err(Self::map_sandbox_sqlx_error)?;
        let sandbox_allocation_key_id: Option<String> = sandbox_runtime_binding_row
            .try_get("sandbox_allocation_key_id")
            .map_err(Self::map_sandbox_sqlx_error)?;
        let sandbox_allocation_key_version: Option<i64> = sandbox_runtime_binding_row
            .try_get("sandbox_allocation_key_version")
            .map_err(Self::map_sandbox_sqlx_error)?;
        let sandbox_allocation_crypto_version: Option<i16> = sandbox_runtime_binding_row
            .try_get("sandbox_allocation_crypto_version")
            .map_err(Self::map_sandbox_sqlx_error)?;
        let sandbox_protected_allocation_reference = match (
            sandbox_allocation_ciphertext,
            sandbox_allocation_key_id,
            sandbox_allocation_key_version,
            sandbox_allocation_crypto_version,
        ) {
            (None, None, None, None) => None,
            (
                Some(sandbox_allocation_ciphertext),
                Some(sandbox_allocation_key_id),
                Some(sandbox_allocation_key_version),
                Some(sandbox_allocation_crypto_version),
            ) => Some(SandboxProtectedProviderAllocationRef::new(
                sandbox_allocation_ciphertext,
                sandbox_allocation_key_id,
                u64::try_from(sandbox_allocation_key_version)
                    .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
                u16::try_from(sandbox_allocation_crypto_version)
                    .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
            )?),
            _ => return Err(SandboxSessionRepositoryError::InvalidStoredData),
        };
        Ok(SandboxRuntimeBindingRepositorySnapshot::new(
            SandboxId::parse(sandbox_id)
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
            SandboxRuntimeBindingId::parse(sandbox_runtime_binding_id)
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
            SandboxProviderId::parse(sandbox_provider_id)
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
            sandbox_protected_allocation_reference,
        ))
    }

    /// Hydrates the requested sandbox sessions with at most three queries and
    /// bounded memory: one query per sandbox_session / sandbox_runtime_binding /
    /// sandbox_session_operation row family.
    ///
    /// Operation history is window-numbered per session and capped at
    /// `MAX_SANDBOX_SESSION_OPERATIONS + 1` rows; a session whose history
    /// exceeds the safety bound fails closed instead of loading an unbounded
    /// row set into process memory.
    async fn load_sandbox_session_snapshots(
        sandbox_connection: &mut PgConnection,
        tenant_id: &TenantId,
        sandbox_session_ids: &[SandboxSessionId],
    ) -> SandboxSessionRepositoryResult<Vec<SandboxSessionRepositorySnapshot>> {
        if sandbox_session_ids.is_empty() {
            return Ok(Vec::new());
        }
        let sandbox_session_id_values: Vec<&str> = sandbox_session_ids
            .iter()
            .map(SandboxSessionId::as_str)
            .collect();
        let sandbox_session_id_values: &[&str] = &sandbox_session_id_values;
        let sandbox_operations_window_limit = i64::try_from(MAX_SANDBOX_SESSION_OPERATIONS + 1)
            .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?;

        let sandbox_session_rows = sqlx::query(
            "SELECT tenant_id, sandbox_workspace_id, sandbox_session_id, \
                    sandbox_session_state, sandbox_required_capabilities, \
                    sandbox_minimum_assurance, sandbox_last_failure, version \
             FROM sandbox_session \
             WHERE tenant_id = $1 AND sandbox_session_id = ANY($2)",
        )
        .bind(tenant_id.as_str())
        .bind(sandbox_session_id_values)
        .fetch_all(&mut *sandbox_connection)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        let mut sandbox_session_rows_by_id: BTreeMap<String, PgRow> = BTreeMap::new();
        for sandbox_session_row in sandbox_session_rows {
            let stored_sandbox_session_id: String = sandbox_session_row
                .try_get("sandbox_session_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            if sandbox_session_rows_by_id
                .insert(stored_sandbox_session_id, sandbox_session_row)
                .is_some()
            {
                return Err(SandboxSessionRepositoryError::InvalidStoredData);
            }
        }

        let sandbox_runtime_binding_rows = sqlx::query(
            "SELECT tenant_id, sandbox_session_id, sandbox_id, sandbox_runtime_binding_id, \
                    sandbox_provider_id, sandbox_allocation_ciphertext, \
                    sandbox_allocation_key_id, sandbox_allocation_key_version, \
                    sandbox_allocation_crypto_version \
             FROM sandbox_runtime_binding \
             WHERE tenant_id = $1 AND sandbox_session_id = ANY($2)",
        )
        .bind(tenant_id.as_str())
        .bind(sandbox_session_id_values)
        .fetch_all(&mut *sandbox_connection)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        let mut sandbox_runtime_bindings_by_id: BTreeMap<
            String,
            SandboxRuntimeBindingRepositorySnapshot,
        > = BTreeMap::new();
        for sandbox_runtime_binding_row in sandbox_runtime_binding_rows {
            let stored_sandbox_session_id: String = sandbox_runtime_binding_row
                .try_get("sandbox_session_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            if sandbox_runtime_bindings_by_id
                .insert(
                    stored_sandbox_session_id,
                    Self::parse_sandbox_runtime_binding_row(&sandbox_runtime_binding_row)?,
                )
                .is_some()
            {
                return Err(SandboxSessionRepositoryError::InvalidStoredData);
            }
        }

        let sandbox_operation_rows = sqlx::query(
            "SELECT sandbox_operation_id, sandbox_session_id, sandbox_operation_sequence, \
                    sandbox_operation_kind, sandbox_operation_outcome, \
                    sandbox_session_failure, sandbox_operation_row_number \
             FROM ( \
                SELECT tenant_id, sandbox_session_id, sandbox_operation_id, \
                       sandbox_operation_sequence, sandbox_operation_kind, \
                       sandbox_operation_outcome, sandbox_session_failure, \
                       ROW_NUMBER() OVER (PARTITION BY tenant_id, sandbox_session_id \
                                          ORDER BY sandbox_operation_sequence) \
                           AS sandbox_operation_row_number \
                FROM sandbox_session_operation \
                WHERE tenant_id = $1 AND sandbox_session_id = ANY($2) \
             ) AS sandbox_operation_window \
             WHERE sandbox_operation_window.sandbox_operation_row_number <= $3 \
             ORDER BY sandbox_session_id, sandbox_operation_row_number",
        )
        .bind(tenant_id.as_str())
        .bind(sandbox_session_id_values)
        .bind(sandbox_operations_window_limit)
        .fetch_all(&mut *sandbox_connection)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        let max_sandbox_operations = i64::try_from(MAX_SANDBOX_SESSION_OPERATIONS)
            .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?;
        let mut sandbox_operations_by_id: BTreeMap<
            String,
            Vec<(i64, SandboxSessionOperationRepositorySnapshot)>,
        > = BTreeMap::new();
        for sandbox_operation_row in sandbox_operation_rows {
            let stored_sandbox_session_id: String = sandbox_operation_row
                .try_get("sandbox_session_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_operation_sequence: i64 = sandbox_operation_row
                .try_get("sandbox_operation_sequence")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_operation_row_number: i64 = sandbox_operation_row
                .try_get("sandbox_operation_row_number")
                .map_err(Self::map_sandbox_sqlx_error)?;
            if sandbox_operation_row_number > max_sandbox_operations {
                return Err(SandboxSessionRepositoryError::InvalidStoredData);
            }
            let sandbox_operation_id: String = sandbox_operation_row
                .try_get("sandbox_operation_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_operation_kind: String = sandbox_operation_row
                .try_get("sandbox_operation_kind")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_operation_outcome: String = sandbox_operation_row
                .try_get("sandbox_operation_outcome")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_session_failure: Option<String> = sandbox_operation_row
                .try_get("sandbox_session_failure")
                .map_err(Self::map_sandbox_sqlx_error)?;
            sandbox_operations_by_id
                .entry(stored_sandbox_session_id)
                .or_default()
                .push((
                    sandbox_operation_sequence,
                    SandboxSessionOperationRepositorySnapshot::new(
                        OperationId::parse(sandbox_operation_id)
                            .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
                        parse_sandbox_operation_kind(&sandbox_operation_kind)?,
                        parse_sandbox_operation_outcome(
                            &sandbox_operation_outcome,
                            sandbox_session_failure.as_deref(),
                        )?,
                    ),
                ));
        }

        let mut sandbox_snapshots = Vec::with_capacity(sandbox_session_ids.len());
        for sandbox_session_id in sandbox_session_ids {
            let Some(sandbox_session_row) =
                sandbox_session_rows_by_id.remove(sandbox_session_id.as_str())
            else {
                continue;
            };
            let stored_tenant_id: String = sandbox_session_row
                .try_get("tenant_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let stored_sandbox_workspace_id: String = sandbox_session_row
                .try_get("sandbox_workspace_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let stored_sandbox_session_id: String = sandbox_session_row
                .try_get("sandbox_session_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_session_state: String = sandbox_session_row
                .try_get("sandbox_session_state")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_required_capabilities: serde_json::Value = sandbox_session_row
                .try_get("sandbox_required_capabilities")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_minimum_assurance: String = sandbox_session_row
                .try_get("sandbox_minimum_assurance")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_last_failure: Option<String> = sandbox_session_row
                .try_get("sandbox_last_failure")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_version: i64 = sandbox_session_row
                .try_get("version")
                .map_err(Self::map_sandbox_sqlx_error)?;

            let stored_tenant_id = TenantId::parse(stored_tenant_id)
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?;
            let stored_sandbox_workspace_id =
                SandboxWorkspaceId::parse(stored_sandbox_workspace_id)
                    .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?;
            let stored_sandbox_session_id = SandboxSessionId::parse(stored_sandbox_session_id)
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?;
            if stored_tenant_id != *tenant_id || stored_sandbox_session_id != *sandbox_session_id {
                return Err(SandboxSessionRepositoryError::InvalidStoredData);
            }

            let sandbox_operations = sandbox_operations_by_id
                .remove(sandbox_session_id.as_str())
                .ok_or(SandboxSessionRepositoryError::InvalidStoredData)?;
            for (expected_sandbox_operation_sequence, (sandbox_operation_sequence, _)) in
                sandbox_operations.iter().enumerate()
            {
                if *sandbox_operation_sequence
                    != i64::try_from(expected_sandbox_operation_sequence)
                        .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?
                {
                    return Err(SandboxSessionRepositoryError::InvalidStoredData);
                }
            }
            let sandbox_operations: Vec<SandboxSessionOperationRepositorySnapshot> =
                sandbox_operations
                    .into_iter()
                    .map(|(_, sandbox_operation)| sandbox_operation)
                    .collect();

            sandbox_snapshots.push(SandboxSessionRepositorySnapshot::new(
                stored_tenant_id,
                stored_sandbox_workspace_id,
                stored_sandbox_session_id,
                parse_sandbox_session_state(&sandbox_session_state)?,
                parse_sandbox_runtime_capabilities(sandbox_required_capabilities)?,
                parse_sandbox_isolation_assurance(&sandbox_minimum_assurance)?,
                sandbox_runtime_bindings_by_id.remove(sandbox_session_id.as_str()),
                sandbox_last_failure
                    .as_deref()
                    .map(parse_sandbox_session_failure)
                    .transpose()?,
                sandbox_operations,
                Self::sandbox_version_from_i64(sandbox_version)?,
            ));
        }
        Ok(sandbox_snapshots)
    }

    async fn load_sandbox_session_snapshot(
        sandbox_connection: &mut PgConnection,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSessionRepositorySnapshot>> {
        let mut sandbox_snapshots = Self::load_sandbox_session_snapshots(
            sandbox_connection,
            tenant_id,
            std::slice::from_ref(sandbox_session_id),
        )
        .await?;
        Ok(sandbox_snapshots.pop())
    }

    async fn insert_sandbox_operations(
        sandbox_connection: &mut PgConnection,
        sandbox_session_snapshot: &SandboxSessionRepositorySnapshot,
        sandbox_allow_update: bool,
    ) -> SandboxSessionRepositoryResult<()> {
        for (sandbox_operation_sequence, sandbox_operation) in sandbox_session_snapshot
            .sandbox_operations()
            .iter()
            .enumerate()
        {
            let sandbox_operation_sequence = i64::try_from(sandbox_operation_sequence)
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?;
            let (sandbox_operation_outcome, sandbox_session_failure) =
                sandbox_operation_outcome_values(sandbox_operation.sandbox_operation_outcome());
            let sandbox_result = if sandbox_allow_update {
                sqlx::query(
                    "INSERT INTO sandbox_session_operation AS existing (\
                        tenant_id, sandbox_operation_id, sandbox_session_id, \
                        sandbox_operation_sequence, sandbox_operation_kind, \
                        sandbox_operation_outcome, sandbox_session_failure\
                     ) VALUES ($1, $2, $3, $4, $5, $6, $7) \
                     ON CONFLICT (tenant_id, sandbox_operation_id) DO UPDATE SET \
                        sandbox_operation_outcome = EXCLUDED.sandbox_operation_outcome, \
                        sandbox_session_failure = EXCLUDED.sandbox_session_failure, \
                        updated_at = CURRENT_TIMESTAMP \
                     WHERE existing.sandbox_session_id = EXCLUDED.sandbox_session_id \
                       AND existing.sandbox_operation_sequence = \
                           EXCLUDED.sandbox_operation_sequence \
                       AND existing.sandbox_operation_kind = EXCLUDED.sandbox_operation_kind",
                )
                .bind(sandbox_session_snapshot.tenant_id().as_str())
                .bind(sandbox_operation.sandbox_operation_id().as_str())
                .bind(sandbox_session_snapshot.sandbox_session_id().as_str())
                .bind(sandbox_operation_sequence)
                .bind(sandbox_operation_kind_value(
                    sandbox_operation.sandbox_operation_kind(),
                ))
                .bind(sandbox_operation_outcome)
                .bind(sandbox_session_failure)
                .execute(&mut *sandbox_connection)
                .await
            } else {
                sqlx::query(
                    "INSERT INTO sandbox_session_operation (\
                        tenant_id, sandbox_operation_id, sandbox_session_id, \
                        sandbox_operation_sequence, sandbox_operation_kind, \
                        sandbox_operation_outcome, sandbox_session_failure\
                     ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(sandbox_session_snapshot.tenant_id().as_str())
                .bind(sandbox_operation.sandbox_operation_id().as_str())
                .bind(sandbox_session_snapshot.sandbox_session_id().as_str())
                .bind(sandbox_operation_sequence)
                .bind(sandbox_operation_kind_value(
                    sandbox_operation.sandbox_operation_kind(),
                ))
                .bind(sandbox_operation_outcome)
                .bind(sandbox_session_failure)
                .execute(&mut *sandbox_connection)
                .await
            }
            .map_err(Self::map_sandbox_sqlx_error)?;
            if sandbox_result.rows_affected() != 1 {
                return Err(SandboxSessionRepositoryError::DuplicateOperation);
            }
        }
        Ok(())
    }

    async fn sync_sandbox_runtime_binding(
        sandbox_connection: &mut PgConnection,
        sandbox_session_snapshot: &SandboxSessionRepositorySnapshot,
    ) -> SandboxSessionRepositoryResult<()> {
        let Some(sandbox_runtime_binding) = sandbox_session_snapshot.sandbox_runtime_binding()
        else {
            sqlx::query(
                "DELETE FROM sandbox_runtime_binding \
                 WHERE tenant_id = $1 AND sandbox_session_id = $2",
            )
            .bind(sandbox_session_snapshot.tenant_id().as_str())
            .bind(sandbox_session_snapshot.sandbox_session_id().as_str())
            .execute(&mut *sandbox_connection)
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
            return Ok(());
        };
        let sandbox_protected_allocation_reference =
            sandbox_runtime_binding.sandbox_protected_allocation_reference();
        let sandbox_allocation_key_version = sandbox_protected_allocation_reference
            .map(|sandbox_protected_allocation_reference| {
                i64::try_from(
                    sandbox_protected_allocation_reference.sandbox_allocation_key_version(),
                )
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)
            })
            .transpose()?;
        let sandbox_allocation_crypto_version = sandbox_protected_allocation_reference
            .map(|sandbox_protected_allocation_reference| {
                i16::try_from(
                    sandbox_protected_allocation_reference.sandbox_allocation_crypto_version(),
                )
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)
            })
            .transpose()?;
        sqlx::query(
            "INSERT INTO sandbox_runtime_binding AS existing (\
                tenant_id, sandbox_runtime_binding_id, sandbox_session_id, sandbox_id, \
                sandbox_provider_id, sandbox_allocation_ciphertext, \
                sandbox_allocation_key_id, sandbox_allocation_key_version, \
                sandbox_allocation_crypto_version\
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (tenant_id, sandbox_session_id) DO UPDATE SET \
                sandbox_runtime_binding_id = EXCLUDED.sandbox_runtime_binding_id, \
                sandbox_id = EXCLUDED.sandbox_id, \
                sandbox_provider_id = EXCLUDED.sandbox_provider_id, \
                sandbox_allocation_ciphertext = EXCLUDED.sandbox_allocation_ciphertext, \
                sandbox_allocation_key_id = EXCLUDED.sandbox_allocation_key_id, \
                sandbox_allocation_key_version = EXCLUDED.sandbox_allocation_key_version, \
                sandbox_allocation_crypto_version = EXCLUDED.sandbox_allocation_crypto_version, \
                updated_at = CURRENT_TIMESTAMP \
             WHERE existing.tenant_id = EXCLUDED.tenant_id",
        )
        .bind(sandbox_session_snapshot.tenant_id().as_str())
        .bind(
            sandbox_runtime_binding
                .sandbox_runtime_binding_id()
                .as_str(),
        )
        .bind(sandbox_session_snapshot.sandbox_session_id().as_str())
        .bind(sandbox_runtime_binding.sandbox_id().as_str())
        .bind(sandbox_runtime_binding.sandbox_provider_id().as_str())
        .bind(sandbox_protected_allocation_reference.map(
            |sandbox_protected_allocation_reference| {
                sandbox_protected_allocation_reference.sandbox_allocation_ciphertext()
            },
        ))
        .bind(sandbox_protected_allocation_reference.map(
            |sandbox_protected_allocation_reference| {
                sandbox_protected_allocation_reference.sandbox_allocation_key_id()
            },
        ))
        .bind(sandbox_allocation_key_version)
        .bind(sandbox_allocation_crypto_version)
        .execute(&mut *sandbox_connection)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        Ok(())
    }

    async fn ensure_sandbox_session_lease_row(
        sandbox_connection: &mut PgConnection,
        sandbox_session_snapshot: &SandboxSessionRepositorySnapshot,
    ) -> SandboxSessionRepositoryResult<()> {
        sqlx::query(
            "INSERT INTO sandbox_session_lease (tenant_id, sandbox_session_id) \
             VALUES ($1, $2) \
             ON CONFLICT (tenant_id, sandbox_session_id) DO NOTHING",
        )
        .bind(sandbox_session_snapshot.tenant_id().as_str())
        .bind(sandbox_session_snapshot.sandbox_session_id().as_str())
        .execute(&mut *sandbox_connection)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        Ok(())
    }

    async fn read_sandbox_session(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>> {
        let mut sandbox_transaction = self
            .sandbox_postgres_pool()?
            .begin()
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *sandbox_transaction)
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        Self::enforce_sandbox_transaction_timeouts(&mut sandbox_transaction).await?;
        let sandbox_snapshot = Self::load_sandbox_session_snapshot(
            &mut sandbox_transaction,
            tenant_id,
            sandbox_session_id,
        )
        .await?;
        sandbox_transaction
            .commit()
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        sandbox_snapshot
            .map(|sandbox_snapshot| {
                sandbox_snapshot.restore(self.sandbox_allocation_protector.as_ref())
            })
            .transpose()
    }
}

#[async_trait]
impl SandboxSessionRepository for SqlxSandboxSessionRepository {
    async fn find_by_sandbox_operation(
        &self,
        tenant_id: &TenantId,
        sandbox_operation_id: &OperationId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>> {
        let sandbox_session_id: Option<String> = sqlx::query_scalar(
            "SELECT sandbox_session_id \
             FROM sandbox_session_operation \
             WHERE tenant_id = $1 AND sandbox_operation_id = $2",
        )
        .bind(tenant_id.as_str())
        .bind(sandbox_operation_id.as_str())
        .fetch_optional(self.sandbox_postgres_pool()?)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        let Some(sandbox_session_id) = sandbox_session_id else {
            return Ok(None);
        };
        let sandbox_session_id = SandboxSessionId::parse(sandbox_session_id)
            .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?;
        self.read_sandbox_session(tenant_id, &sandbox_session_id)
            .await
    }

    async fn get_sandbox_session(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>> {
        self.read_sandbox_session(tenant_id, sandbox_session_id)
            .await
    }

    async fn insert_sandbox_session(
        &self,
        sandbox_session: SandboxSession,
    ) -> SandboxSessionRepositoryResult<()> {
        let sandbox_session_snapshot = SandboxSessionRepositorySnapshot::capture(
            &sandbox_session,
            self.sandbox_allocation_protector.as_ref(),
        )?;
        let mut sandbox_transaction = self
            .sandbox_postgres_pool()?
            .begin()
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        Self::enforce_sandbox_transaction_timeouts(&mut sandbox_transaction).await?;
        sqlx::query(
            "INSERT INTO sandbox_session (\
                tenant_id, sandbox_session_id, sandbox_workspace_id, \
                sandbox_session_state, sandbox_required_capabilities, \
                sandbox_minimum_assurance, sandbox_last_failure, version\
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(sandbox_session_snapshot.tenant_id().as_str())
        .bind(sandbox_session_snapshot.sandbox_session_id().as_str())
        .bind(sandbox_session_snapshot.sandbox_workspace_id().as_str())
        .bind(sandbox_session_state_value(
            sandbox_session_snapshot.sandbox_session_state(),
        ))
        .bind(sandbox_runtime_capabilities_value(
            sandbox_session_snapshot.sandbox_required_capabilities(),
        ))
        .bind(sandbox_isolation_assurance_value(
            sandbox_session_snapshot.sandbox_minimum_assurance(),
        ))
        .bind(
            sandbox_session_snapshot
                .sandbox_last_failure()
                .map(sandbox_session_failure_value),
        )
        .bind(Self::sandbox_version_to_i64(
            sandbox_session_snapshot.sandbox_version(),
        )?)
        .execute(&mut *sandbox_transaction)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        Self::insert_sandbox_operations(&mut sandbox_transaction, &sandbox_session_snapshot, false)
            .await?;
        Self::sync_sandbox_runtime_binding(&mut sandbox_transaction, &sandbox_session_snapshot)
            .await?;
        Self::ensure_sandbox_session_lease_row(&mut sandbox_transaction, &sandbox_session_snapshot)
            .await?;
        sandbox_transaction
            .commit()
            .await
            .map_err(Self::map_sandbox_sqlx_error)
    }

    async fn save_sandbox_session(
        &self,
        sandbox_session: SandboxSession,
        expected_sandbox_version: u64,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<()> {
        let next_sandbox_version = expected_sandbox_version
            .checked_add(1)
            .ok_or(SandboxSessionRepositoryError::VersionConflict)?;
        if sandbox_session.sandbox_version() != next_sandbox_version {
            return Err(SandboxSessionRepositoryError::VersionConflict);
        }
        let sandbox_session_snapshot = SandboxSessionRepositorySnapshot::capture(
            &sandbox_session,
            self.sandbox_allocation_protector.as_ref(),
        )?;
        let mut sandbox_transaction = self
            .sandbox_postgres_pool()?
            .begin()
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        if sandbox_session_snapshot.tenant_id() != sandbox_session_lease.tenant_id()
            || sandbox_session_snapshot.sandbox_session_id()
                != sandbox_session_lease.sandbox_session_id()
        {
            return Err(SandboxSessionRepositoryError::LeaseConflict);
        }
        Self::enforce_sandbox_transaction_timeouts(&mut sandbox_transaction).await?;
        Self::lock_valid_sandbox_session_lease(&mut sandbox_transaction, sandbox_session_lease)
            .await?;
        let sandbox_update_result = sqlx::query(
            "UPDATE sandbox_session SET \
                sandbox_workspace_id = $3, \
                sandbox_session_state = $4, \
                sandbox_required_capabilities = $5, \
                sandbox_minimum_assurance = $6, \
                sandbox_last_failure = $7, \
                version = $8, \
                updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = $1 AND sandbox_session_id = $2 AND version = $9",
        )
        .bind(sandbox_session_snapshot.tenant_id().as_str())
        .bind(sandbox_session_snapshot.sandbox_session_id().as_str())
        .bind(sandbox_session_snapshot.sandbox_workspace_id().as_str())
        .bind(sandbox_session_state_value(
            sandbox_session_snapshot.sandbox_session_state(),
        ))
        .bind(sandbox_runtime_capabilities_value(
            sandbox_session_snapshot.sandbox_required_capabilities(),
        ))
        .bind(sandbox_isolation_assurance_value(
            sandbox_session_snapshot.sandbox_minimum_assurance(),
        ))
        .bind(
            sandbox_session_snapshot
                .sandbox_last_failure()
                .map(sandbox_session_failure_value),
        )
        .bind(Self::sandbox_version_to_i64(
            sandbox_session_snapshot.sandbox_version(),
        )?)
        .bind(Self::sandbox_version_to_i64(expected_sandbox_version)?)
        .execute(&mut *sandbox_transaction)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        if sandbox_update_result.rows_affected() != 1 {
            let sandbox_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(\
                    SELECT 1 FROM sandbox_session \
                    WHERE tenant_id = $1 AND sandbox_session_id = $2\
                 )",
            )
            .bind(sandbox_session_snapshot.tenant_id().as_str())
            .bind(sandbox_session_snapshot.sandbox_session_id().as_str())
            .fetch_one(&mut *sandbox_transaction)
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
            return if sandbox_exists {
                Err(SandboxSessionRepositoryError::VersionConflict)
            } else {
                Err(SandboxSessionRepositoryError::NotFound)
            };
        }
        Self::insert_sandbox_operations(&mut sandbox_transaction, &sandbox_session_snapshot, true)
            .await?;
        Self::sync_sandbox_runtime_binding(&mut sandbox_transaction, &sandbox_session_snapshot)
            .await?;
        Self::ensure_sandbox_session_lease_row(&mut sandbox_transaction, &sandbox_session_snapshot)
            .await?;
        sandbox_transaction
            .commit()
            .await
            .map_err(Self::map_sandbox_sqlx_error)
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
        let sandbox_lease_row = sqlx::query(
            "INSERT INTO sandbox_session_lease AS existing (\
                tenant_id, sandbox_session_id, sandbox_lease_owner_id, \
                sandbox_lease_expires_at, sandbox_fencing_token\
             ) \
             SELECT $1, $2, $3, \
                    CURRENT_TIMESTAMP + make_interval(secs => $4::double precision / 1000.0), \
                    1 \
             FROM sandbox_session \
             WHERE tenant_id = $1 AND sandbox_session_id = $2 \
             ON CONFLICT (tenant_id, sandbox_session_id) DO UPDATE SET \
                sandbox_lease_owner_id = EXCLUDED.sandbox_lease_owner_id, \
                sandbox_lease_expires_at = EXCLUDED.sandbox_lease_expires_at, \
                sandbox_fencing_token = existing.sandbox_fencing_token + 1, \
                updated_at = CURRENT_TIMESTAMP \
             WHERE (existing.sandbox_lease_owner_id IS NULL \
                    OR existing.sandbox_lease_expires_at <= CURRENT_TIMESTAMP) \
               AND existing.sandbox_fencing_token < 9223372036854775807 \
             RETURNING sandbox_fencing_token, \
                (EXTRACT(EPOCH FROM sandbox_lease_expires_at) * 1000)::BIGINT \
                    AS sandbox_lease_expires_at_unix_millis",
        )
        .bind(tenant_id.as_str())
        .bind(sandbox_session_id.as_str())
        .bind(sandbox_lease_owner_id.as_str())
        .bind(sandbox_lease_duration_millis)
        .fetch_optional(self.sandbox_postgres_pool()?)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        if let Some(sandbox_lease_row) = sandbox_lease_row {
            return Self::sandbox_session_lease_from_row(
                tenant_id,
                sandbox_session_id,
                sandbox_lease_owner_id,
                &sandbox_lease_row,
            )
            .map(Some);
        }
        let sandbox_lease_status: Option<(Option<i64>, bool)> = sqlx::query_as(
            "SELECT sandbox_session_lease.sandbox_fencing_token, \
                    (sandbox_session_lease.sandbox_lease_owner_id IS NULL \
                     OR sandbox_session_lease.sandbox_lease_expires_at <= CURRENT_TIMESTAMP) \
                        AS sandbox_lease_is_available \
             FROM sandbox_session \
             LEFT JOIN sandbox_session_lease \
               ON sandbox_session_lease.tenant_id = sandbox_session.tenant_id \
              AND sandbox_session_lease.sandbox_session_id = sandbox_session.sandbox_session_id \
             WHERE sandbox_session.tenant_id = $1 \
               AND sandbox_session.sandbox_session_id = $2",
        )
        .bind(tenant_id.as_str())
        .bind(sandbox_session_id.as_str())
        .fetch_optional(self.sandbox_postgres_pool()?)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        match sandbox_lease_status {
            None => Err(SandboxSessionRepositoryError::NotFound),
            Some((None, _)) => Err(SandboxSessionRepositoryError::InvalidStoredData),
            Some((Some(sandbox_fencing_token), true)) if sandbox_fencing_token == i64::MAX => {
                Err(SandboxSessionRepositoryError::LeaseConflict)
            }
            Some((Some(_), _)) => Ok(None),
        }
    }

    async fn renew_sandbox_session_lease(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
        sandbox_lease_duration: Duration,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSessionLease>> {
        let sandbox_lease_duration_millis =
            Self::sandbox_lease_duration_millis(sandbox_lease_duration)?;
        let sandbox_lease_row = sqlx::query(
            "UPDATE sandbox_session_lease SET \
                sandbox_lease_expires_at = \
                    CURRENT_TIMESTAMP + make_interval(secs => $5::double precision / 1000.0), \
                updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = $1 \
               AND sandbox_session_id = $2 \
               AND sandbox_lease_owner_id = $3 \
               AND sandbox_fencing_token = $4 \
               AND sandbox_lease_expires_at > CURRENT_TIMESTAMP \
             RETURNING sandbox_fencing_token, \
                (EXTRACT(EPOCH FROM sandbox_lease_expires_at) * 1000)::BIGINT \
                    AS sandbox_lease_expires_at_unix_millis",
        )
        .bind(sandbox_session_lease.tenant_id().as_str())
        .bind(sandbox_session_lease.sandbox_session_id().as_str())
        .bind(sandbox_session_lease.sandbox_lease_owner_id().as_str())
        .bind(
            i64::try_from(sandbox_session_lease.sandbox_fencing_token().value())
                .map_err(|_| SandboxSessionRepositoryError::LeaseConflict)?,
        )
        .bind(sandbox_lease_duration_millis)
        .fetch_optional(self.sandbox_postgres_pool()?)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        sandbox_lease_row
            .map(|sandbox_lease_row| {
                Self::sandbox_session_lease_from_row(
                    sandbox_session_lease.tenant_id(),
                    sandbox_session_lease.sandbox_session_id(),
                    sandbox_session_lease.sandbox_lease_owner_id(),
                    &sandbox_lease_row,
                )
            })
            .transpose()
    }

    async fn release_sandbox_session_lease(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<bool> {
        let sandbox_release_result = sqlx::query(
            "UPDATE sandbox_session_lease SET \
                sandbox_lease_owner_id = NULL, \
                sandbox_lease_expires_at = NULL, \
                updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = $1 \
               AND sandbox_session_id = $2 \
               AND sandbox_lease_owner_id = $3 \
               AND sandbox_fencing_token = $4",
        )
        .bind(sandbox_session_lease.tenant_id().as_str())
        .bind(sandbox_session_lease.sandbox_session_id().as_str())
        .bind(sandbox_session_lease.sandbox_lease_owner_id().as_str())
        .bind(
            i64::try_from(sandbox_session_lease.sandbox_fencing_token().value())
                .map_err(|_| SandboxSessionRepositoryError::LeaseConflict)?,
        )
        .execute(self.sandbox_postgres_pool()?)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        Ok(sandbox_release_result.rows_affected() == 1)
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
        let sandbox_session_id_values: Vec<String> = sqlx::query_scalar(
            "SELECT sandbox_session_id \
             FROM sandbox_session \
             WHERE tenant_id = $1 \
               AND sandbox_session_state IN ('starting', 'stopping', 'destroying') \
               AND ($2::TEXT IS NULL OR sandbox_session_id > $2) \
             ORDER BY sandbox_session_id \
             LIMIT $3",
        )
        .bind(tenant_id.as_str())
        .bind(after_sandbox_session_id.map(SandboxSessionId::as_str))
        .bind(i64::from(sandbox_page_size))
        .fetch_all(self.sandbox_postgres_pool()?)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        let sandbox_session_ids: Vec<SandboxSessionId> = sandbox_session_id_values
            .into_iter()
            .map(|sandbox_session_id| {
                SandboxSessionId::parse(sandbox_session_id)
                    .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)
            })
            .collect::<SandboxSessionRepositoryResult<_>>()?;
        let mut sandbox_transaction = self
            .sandbox_postgres_pool()?
            .begin()
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *sandbox_transaction)
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        Self::enforce_sandbox_transaction_timeouts(&mut sandbox_transaction).await?;
        let sandbox_snapshots = Self::load_sandbox_session_snapshots(
            &mut sandbox_transaction,
            tenant_id,
            &sandbox_session_ids,
        )
        .await?;
        sandbox_transaction
            .commit()
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        let mut sandbox_sessions = Vec::with_capacity(sandbox_snapshots.len());
        for sandbox_snapshot in sandbox_snapshots {
            sandbox_sessions
                .push(sandbox_snapshot.restore(self.sandbox_allocation_protector.as_ref())?);
        }
        Ok(sandbox_sessions)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::error::Error as StdError;
    use std::fmt;

    use sqlx::error::{DatabaseError, ErrorKind};

    use super::SqlxSandboxSessionRepository;
    use crate::repository::SandboxSessionRepositoryError;

    struct TestSandboxDatabaseError {
        sandbox_sqlstate_code: Option<String>,
        sandbox_constraint_name: Option<String>,
    }

    impl TestSandboxDatabaseError {
        fn with_code(sandbox_sqlstate_code: &str) -> Self {
            Self {
                sandbox_sqlstate_code: Some(sandbox_sqlstate_code.to_owned()),
                sandbox_constraint_name: None,
            }
        }

        fn with_constraint(sandbox_sqlstate_code: &str, sandbox_constraint_name: &str) -> Self {
            Self {
                sandbox_sqlstate_code: Some(sandbox_sqlstate_code.to_owned()),
                sandbox_constraint_name: Some(sandbox_constraint_name.to_owned()),
            }
        }
    }

    impl fmt::Debug for TestSandboxDatabaseError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("TestSandboxDatabaseError")
        }
    }

    impl fmt::Display for TestSandboxDatabaseError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("test sandbox database error")
        }
    }

    impl StdError for TestSandboxDatabaseError {}

    impl DatabaseError for TestSandboxDatabaseError {
        fn message(&self) -> &str {
            "test sandbox database error"
        }

        fn code(&self) -> Option<Cow<'_, str>> {
            self.sandbox_sqlstate_code.as_deref().map(Cow::Borrowed)
        }

        fn constraint(&self) -> Option<&str> {
            self.sandbox_constraint_name.as_deref()
        }

        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }

        fn as_error(&self) -> &(dyn StdError + Send + Sync + 'static) {
            self
        }

        fn as_error_mut(&mut self) -> &mut (dyn StdError + Send + Sync + 'static) {
            self
        }

        fn into_error(self: Box<Self>) -> Box<dyn StdError + Send + Sync + 'static> {
            self
        }
    }

    fn sandbox_database_error(sandbox_database_error: TestSandboxDatabaseError) -> sqlx::Error {
        sqlx::Error::Database(Box::new(sandbox_database_error))
    }

    #[test]
    fn sandbox_sqlx_error_mapping_classifies_unique_violations_by_constraint() {
        assert_eq!(
            SqlxSandboxSessionRepository::map_sandbox_sqlx_error(sandbox_database_error(
                TestSandboxDatabaseError::with_constraint("23505", "pk_sandbox_session_operation"),
            )),
            SandboxSessionRepositoryError::DuplicateOperation
        );
        assert_eq!(
            SqlxSandboxSessionRepository::map_sandbox_sqlx_error(sandbox_database_error(
                TestSandboxDatabaseError::with_constraint(
                    "23505",
                    "uk_sandbox_session_operation_sequence",
                ),
            )),
            SandboxSessionRepositoryError::InvalidStoredData
        );
        assert_eq!(
            SqlxSandboxSessionRepository::map_sandbox_sqlx_error(sandbox_database_error(
                TestSandboxDatabaseError::with_constraint("23505", "pk_sandbox_session"),
            )),
            SandboxSessionRepositoryError::VersionConflict
        );
        assert_eq!(
            SqlxSandboxSessionRepository::map_sandbox_sqlx_error(sandbox_database_error(
                TestSandboxDatabaseError::with_code("23505"),
            )),
            SandboxSessionRepositoryError::VersionConflict
        );
    }

    #[test]
    fn sandbox_sqlx_error_mapping_classifies_integrity_and_transient_codes() {
        for sandbox_integrity_code in ["23502", "23503", "23514"] {
            assert_eq!(
                SqlxSandboxSessionRepository::map_sandbox_sqlx_error(sandbox_database_error(
                    TestSandboxDatabaseError::with_code(sandbox_integrity_code),
                )),
                SandboxSessionRepositoryError::InvalidStoredData
            );
        }
        for sandbox_transient_code in ["40001", "40P01", "55P03", "57014"] {
            assert_eq!(
                SqlxSandboxSessionRepository::map_sandbox_sqlx_error(sandbox_database_error(
                    TestSandboxDatabaseError::with_code(sandbox_transient_code),
                )),
                SandboxSessionRepositoryError::Unavailable
            );
        }
        assert_eq!(
            SqlxSandboxSessionRepository::map_sandbox_sqlx_error(sandbox_database_error(
                TestSandboxDatabaseError::with_code("42P01"),
            )),
            SandboxSessionRepositoryError::Unavailable
        );
    }

    #[test]
    fn sandbox_sqlx_error_mapping_maps_connection_failures_to_unavailable() {
        assert_eq!(
            SqlxSandboxSessionRepository::map_sandbox_sqlx_error(sqlx::Error::Io(
                std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "connection refused"),
            )),
            SandboxSessionRepositoryError::Unavailable
        );
        assert_eq!(
            SqlxSandboxSessionRepository::map_sandbox_sqlx_error(sqlx::Error::PoolTimedOut),
            SandboxSessionRepositoryError::Unavailable
        );
    }
}
