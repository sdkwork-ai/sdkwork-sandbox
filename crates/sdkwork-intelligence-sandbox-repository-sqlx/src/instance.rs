//! PostgreSQL adapter for the console-facing Sandbox Instance registry.
//!
//! All values are bound; only fixed clauses are concatenated. Timestamps are
//! projected with `to_char(... AT TIME ZONE 'UTC')` so the adapter does not
//! depend on a Rust datetime feature that the workspace `sqlx` dependency does
//! not enable, and the re-emitted string is exactly the RFC 3339 shape the wire
//! contract and the service validator both accept.

use async_trait::async_trait;
use sdkwork_database_sqlx::DatabasePool;
use sdkwork_intelligence_sandbox_service::{
    SandboxInstance, SandboxInstanceListCursor, SandboxInstanceListPage, SandboxInstanceProfile,
    SandboxInstanceRepository, SandboxInstanceRepositoryError, SandboxInstanceRepositoryResult,
    SandboxInstanceState,
};
use sdkwork_sandbox_provider_spi::{
    SandboxInstanceId, SandboxInstanceOwnerId, SandboxWorkspaceId, TenantId,
};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

use crate::codec::{
    parse_sandbox_isolation_assurance, parse_sandbox_runtime_capabilities,
    parse_sandbox_session_failure, sandbox_isolation_assurance_value,
    sandbox_runtime_capabilities_value, sandbox_session_failure_value,
};

/// `SQLSTATE` for a unique-constraint violation.
const SQLSTATE_UNIQUE_VIOLATION: &str = "23505";
/// `SQLSTATE` for an invalid text representation (a malformed cast input).
const SQLSTATE_INVALID_TEXT_REPRESENTATION: &str = "22P02";

/// Projection shared by every read path, so a column added to one read cannot
/// silently drift from the others.
const SANDBOX_INSTANCE_COLUMNS: &str = "tenant_id, \
     sandbox_instance_id, \
     sandbox_instance_owner_id, \
     sandbox_instance_name, \
     sandbox_instance_state, \
     sandbox_instance_profile, \
     sandbox_instance_base_image, \
     sandbox_instance_vcpu_count, \
     sandbox_instance_memory_mb, \
     sandbox_instance_disk_mb, \
     sandbox_instance_required_capabilities, \
     sandbox_instance_minimum_assurance, \
     sandbox_instance_auto_start, \
     to_char(sandbox_instance_expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') \
        AS sandbox_instance_expires_at, \
     sandbox_workspace_id, \
     sandbox_instance_last_failure, \
     version, \
     to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
     to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

/// PostgreSQL-backed [`SandboxInstanceRepository`]. Cheap to clone: the wrapped
/// pool is itself a handle, so every clone shares one connection pool.
#[derive(Clone)]
pub struct SqlxSandboxInstanceRepository {
    sandbox_database_pool: DatabasePool,
}

impl SqlxSandboxInstanceRepository {
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceRepositoryError::UnsupportedDatabaseEngine`]
    /// when the pool is not PostgreSQL.
    pub fn new(sandbox_database_pool: DatabasePool) -> SandboxInstanceRepositoryResult<Self> {
        if sandbox_database_pool.as_postgres().is_none() {
            return Err(SandboxInstanceRepositoryError::UnsupportedDatabaseEngine);
        }
        Ok(Self {
            sandbox_database_pool,
        })
    }

    fn sandbox_postgres_pool(&self) -> SandboxInstanceRepositoryResult<&PgPool> {
        self.sandbox_database_pool
            .as_postgres()
            .ok_or(SandboxInstanceRepositoryError::UnsupportedDatabaseEngine)
    }

    fn map_sandbox_sqlx_error(error: sqlx::Error) -> SandboxInstanceRepositoryError {
        match &error {
            sqlx::Error::Database(database_error) => match database_error.code().as_deref() {
                Some(SQLSTATE_UNIQUE_VIOLATION) => SandboxInstanceRepositoryError::DuplicateName,
                // A cast failure can only come from a timestamp-shaped bind
                // (the cursor timestamp), which is a malformed page request.
                Some(SQLSTATE_INVALID_TEXT_REPRESENTATION) => {
                    SandboxInstanceRepositoryError::InvalidPageRequest
                }
                _ => SandboxInstanceRepositoryError::Unavailable,
            },
            sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed => {
                SandboxInstanceRepositoryError::Unavailable
            }
            _ => SandboxInstanceRepositoryError::Unavailable,
        }
    }

    fn read_sandbox_instance(
        sandbox_instance_row: &PgRow,
    ) -> SandboxInstanceRepositoryResult<SandboxInstance> {
        let invalid = || SandboxInstanceRepositoryError::InvalidStoredData;
        let tenant_id = sandbox_instance_row
            .try_get::<String, _>("tenant_id")
            .map_err(|_| invalid())?;
        let sandbox_instance_id = sandbox_instance_row
            .try_get::<String, _>("sandbox_instance_id")
            .map_err(|_| invalid())?;
        let sandbox_instance_owner_id = sandbox_instance_row
            .try_get::<String, _>("sandbox_instance_owner_id")
            .map_err(|_| invalid())?;
        let sandbox_instance_name = sandbox_instance_row
            .try_get::<String, _>("sandbox_instance_name")
            .map_err(|_| invalid())?;
        let sandbox_instance_state = sandbox_instance_row
            .try_get::<String, _>("sandbox_instance_state")
            .map_err(|_| invalid())?;
        let sandbox_instance_profile = sandbox_instance_row
            .try_get::<String, _>("sandbox_instance_profile")
            .map_err(|_| invalid())?;
        let sandbox_instance_base_image = sandbox_instance_row
            .try_get::<String, _>("sandbox_instance_base_image")
            .map_err(|_| invalid())?;
        let sandbox_instance_vcpu_count = sandbox_instance_row
            .try_get::<i32, _>("sandbox_instance_vcpu_count")
            .map_err(|_| invalid())?;
        let sandbox_instance_memory_mb = sandbox_instance_row
            .try_get::<i32, _>("sandbox_instance_memory_mb")
            .map_err(|_| invalid())?;
        let sandbox_instance_disk_mb = sandbox_instance_row
            .try_get::<i32, _>("sandbox_instance_disk_mb")
            .map_err(|_| invalid())?;
        let sandbox_instance_required_capabilities = sandbox_instance_row
            .try_get::<serde_json::Value, _>("sandbox_instance_required_capabilities")
            .map_err(|_| invalid())?;
        let sandbox_instance_minimum_assurance = sandbox_instance_row
            .try_get::<String, _>("sandbox_instance_minimum_assurance")
            .map_err(|_| invalid())?;
        let sandbox_instance_auto_start = sandbox_instance_row
            .try_get::<bool, _>("sandbox_instance_auto_start")
            .map_err(|_| invalid())?;
        let sandbox_instance_expires_at = sandbox_instance_row
            .try_get::<Option<String>, _>("sandbox_instance_expires_at")
            .map_err(|_| invalid())?;
        let sandbox_workspace_id = sandbox_instance_row
            .try_get::<Option<String>, _>("sandbox_workspace_id")
            .map_err(|_| invalid())?;
        let sandbox_instance_last_failure = sandbox_instance_row
            .try_get::<Option<String>, _>("sandbox_instance_last_failure")
            .map_err(|_| invalid())?;
        let sandbox_version = sandbox_instance_row
            .try_get::<i64, _>("version")
            .map_err(|_| invalid())?;
        let created_at = sandbox_instance_row
            .try_get::<String, _>("created_at")
            .map_err(|_| invalid())?;
        let updated_at = sandbox_instance_row
            .try_get::<String, _>("updated_at")
            .map_err(|_| invalid())?;

        let sandbox_instance_vcpu_count =
            u32::try_from(sandbox_instance_vcpu_count).map_err(|_| invalid())?;
        let sandbox_instance_memory_mb =
            u32::try_from(sandbox_instance_memory_mb).map_err(|_| invalid())?;
        let sandbox_instance_disk_mb =
            u32::try_from(sandbox_instance_disk_mb).map_err(|_| invalid())?;
        let sandbox_version = u64::try_from(sandbox_version).map_err(|_| invalid())?;

        Ok(SandboxInstance::restore(
            TenantId::parse(tenant_id).map_err(|_| invalid())?,
            SandboxInstanceId::parse(sandbox_instance_id).map_err(|_| invalid())?,
            SandboxInstanceOwnerId::parse(sandbox_instance_owner_id).map_err(|_| invalid())?,
            sandbox_instance_name,
            SandboxInstanceState::parse(&sandbox_instance_state).ok_or_else(invalid)?,
            SandboxInstanceProfile::parse(&sandbox_instance_profile).ok_or_else(invalid)?,
            sandbox_instance_base_image,
            sandbox_instance_vcpu_count,
            sandbox_instance_memory_mb,
            sandbox_instance_disk_mb,
            parse_sandbox_runtime_capabilities(sandbox_instance_required_capabilities)
                .map_err(|_| invalid())?,
            parse_sandbox_isolation_assurance(&sandbox_instance_minimum_assurance)
                .map_err(|_| invalid())?,
            sandbox_instance_auto_start,
            sandbox_instance_expires_at,
            sandbox_workspace_id
                .map(|value| SandboxWorkspaceId::parse(value).map_err(|_| invalid()))
                .transpose()?,
            sandbox_instance_last_failure
                .map(|value| parse_sandbox_session_failure(&value).map_err(|_| invalid()))
                .transpose()?,
            sandbox_version,
            Some(created_at),
            Some(updated_at),
        ))
    }
}

/// The `expires_at` value to bind, already narrowed to `None | Some(text)`.
fn sandbox_instance_expires_at_bind(sandbox_instance: &SandboxInstance) -> Option<&str> {
    sandbox_instance.sandbox_instance_expires_at()
}

#[async_trait]
impl SandboxInstanceRepository for SqlxSandboxInstanceRepository {
    async fn insert_sandbox_instance(
        &self,
        sandbox_instance: &SandboxInstance,
    ) -> SandboxInstanceRepositoryResult<SandboxInstance> {
        let sql = format!(
            "INSERT INTO sandbox_instance (\
                tenant_id, sandbox_instance_id, sandbox_instance_owner_id, \
                sandbox_instance_name, sandbox_instance_state, sandbox_instance_profile, \
                sandbox_instance_base_image, sandbox_instance_vcpu_count, \
                sandbox_instance_memory_mb, sandbox_instance_disk_mb, \
                sandbox_instance_required_capabilities, sandbox_instance_minimum_assurance, \
                sandbox_instance_auto_start, sandbox_instance_expires_at, sandbox_workspace_id, \
                version\
             ) VALUES (\
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, \
                CAST($14 AS TIMESTAMPTZ), $15, $16\
             ) RETURNING {SANDBOX_INSTANCE_COLUMNS}"
        );
        let row = sqlx::query(audited_sql(&sql))
            .bind(sandbox_instance.tenant_id().as_str())
            .bind(sandbox_instance.sandbox_instance_id().as_str())
            .bind(sandbox_instance.sandbox_instance_owner_id().as_str())
            .bind(sandbox_instance.sandbox_instance_name())
            .bind(sandbox_instance.sandbox_instance_state().as_str())
            .bind(sandbox_instance.sandbox_instance_profile().as_str())
            .bind(sandbox_instance.sandbox_instance_base_image())
            .bind(
                i32::try_from(sandbox_instance.sandbox_instance_vcpu_count())
                    .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
            )
            .bind(
                i32::try_from(sandbox_instance.sandbox_instance_memory_mb())
                    .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
            )
            .bind(
                i32::try_from(sandbox_instance.sandbox_instance_disk_mb())
                    .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
            )
            .bind(sandbox_runtime_capabilities_value(
                sandbox_instance.sandbox_instance_required_capabilities(),
            ))
            .bind(sandbox_isolation_assurance_value(
                sandbox_instance.sandbox_instance_minimum_assurance(),
            ))
            .bind(sandbox_instance.sandbox_instance_auto_start())
            .bind(sandbox_instance_expires_at_bind(sandbox_instance))
            .bind(
                sandbox_instance
                    .sandbox_workspace_id()
                    .map(SandboxWorkspaceId::as_str),
            )
            .bind(
                i64::try_from(sandbox_instance.sandbox_version())
                    .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
            )
            .fetch_one(self.sandbox_postgres_pool()?)
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        Self::read_sandbox_instance(&row)
    }

    async fn list_sandbox_instances(
        &self,
        tenant_id: &TenantId,
        sandbox_instance_owner_id: Option<&SandboxInstanceOwnerId>,
        sandbox_instance_state: Option<SandboxInstanceState>,
        cursor: Option<&SandboxInstanceListCursor>,
        page_size: u32,
    ) -> SandboxInstanceRepositoryResult<SandboxInstanceListPage> {
        if page_size == 0 || page_size > 200 {
            return Err(SandboxInstanceRepositoryError::InvalidPageRequest);
        }
        let owner_filter = sandbox_instance_owner_id.map(SandboxInstanceOwnerId::as_str);
        let state_filter = sandbox_instance_state.map(SandboxInstanceState::as_str);
        // Seek pagination on `(created_at, sandbox_instance_id)` descending
        // (`DATABASE_SPEC.md` section 20.5): one indexed scan of at most
        // `page_size + 1` rows, no OFFSET and no `COUNT(*)`. The extra row only
        // detects `has_more` and is dropped before mapping.
        let sql = format!(
            "SELECT {SANDBOX_INSTANCE_COLUMNS} FROM sandbox_instance \
             WHERE tenant_id = $1 \
               AND ($2::TEXT IS NULL OR sandbox_instance_owner_id = $2) \
               AND ($3::TEXT IS NULL OR sandbox_instance_state = $3) \
               AND ($4::TIMESTAMPTZ IS NULL \
                    OR (created_at, sandbox_instance_id) < (CAST($4 AS TIMESTAMPTZ), $5)) \
             ORDER BY created_at DESC, sandbox_instance_id DESC \
             LIMIT $6"
        );
        let rows = sqlx::query(audited_sql(&sql))
            .bind(tenant_id.as_str())
            .bind(owner_filter)
            .bind(state_filter)
            .bind(cursor.map(SandboxInstanceListCursor::created_at))
            .bind(cursor.map(|value| value.sandbox_instance_id().as_str()))
            .bind(i64::from(page_size) + 1)
            .fetch_all(self.sandbox_postgres_pool()?)
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        let has_more = rows.len() > page_size as usize;
        let window = if has_more {
            &rows[..page_size as usize]
        } else {
            &rows[..]
        };
        let next_cursor = if has_more {
            let last = window
                .last()
                .ok_or(SandboxInstanceRepositoryError::InvalidStoredData)?;
            let created_at = last
                .try_get::<String, _>("created_at")
                .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?;
            let sandbox_instance_id = last
                .try_get::<String, _>("sandbox_instance_id")
                .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?;
            Some(
                SandboxInstanceListCursor::new(
                    created_at,
                    SandboxInstanceId::parse(sandbox_instance_id)
                        .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
                )
                .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
            )
        } else {
            None
        };
        let items = window
            .iter()
            .map(Self::read_sandbox_instance)
            .collect::<SandboxInstanceRepositoryResult<Vec<SandboxInstance>>>()?;
        Ok(SandboxInstanceListPage { items, next_cursor })
    }

    async fn get_sandbox_instance(
        &self,
        tenant_id: &TenantId,
        sandbox_instance_id: &SandboxInstanceId,
    ) -> SandboxInstanceRepositoryResult<Option<SandboxInstance>> {
        let sql = format!(
            "SELECT {SANDBOX_INSTANCE_COLUMNS} FROM sandbox_instance \
             WHERE tenant_id = $1 AND sandbox_instance_id = $2"
        );
        let row = sqlx::query(audited_sql(&sql))
            .bind(tenant_id.as_str())
            .bind(sandbox_instance_id.as_str())
            .fetch_optional(self.sandbox_postgres_pool()?)
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;
        row.as_ref().map(Self::read_sandbox_instance).transpose()
    }

    async fn save_sandbox_instance(
        &self,
        sandbox_instance: &SandboxInstance,
        expected_sandbox_version: u64,
    ) -> SandboxInstanceRepositoryResult<bool> {
        let result = sqlx::query(
            "UPDATE sandbox_instance SET \
                sandbox_instance_name = $3, \
                sandbox_instance_state = $4, \
                sandbox_instance_profile = $5, \
                sandbox_instance_vcpu_count = $6, \
                sandbox_instance_memory_mb = $7, \
                sandbox_instance_disk_mb = $8, \
                sandbox_instance_auto_start = $9, \
                sandbox_instance_expires_at = CAST($10 AS TIMESTAMPTZ), \
                sandbox_workspace_id = $11, \
                sandbox_instance_last_failure = $12, \
                version = version + 1, \
                updated_at = CURRENT_TIMESTAMP \
             WHERE tenant_id = $1 AND sandbox_instance_id = $2 AND version = $13",
        )
        .bind(sandbox_instance.tenant_id().as_str())
        .bind(sandbox_instance.sandbox_instance_id().as_str())
        .bind(sandbox_instance.sandbox_instance_name())
        .bind(sandbox_instance.sandbox_instance_state().as_str())
        .bind(sandbox_instance.sandbox_instance_profile().as_str())
        .bind(
            i32::try_from(sandbox_instance.sandbox_instance_vcpu_count())
                .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
        )
        .bind(
            i32::try_from(sandbox_instance.sandbox_instance_memory_mb())
                .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
        )
        .bind(
            i32::try_from(sandbox_instance.sandbox_instance_disk_mb())
                .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
        )
        .bind(sandbox_instance.sandbox_instance_auto_start())
        .bind(sandbox_instance_expires_at_bind(sandbox_instance))
        .bind(
            sandbox_instance
                .sandbox_workspace_id()
                .map(SandboxWorkspaceId::as_str),
        )
        .bind(
            sandbox_instance
                .sandbox_instance_last_failure()
                .map(sandbox_session_failure_value),
        )
        .bind(
            i64::try_from(expected_sandbox_version)
                .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
        )
        .execute(self.sandbox_postgres_pool()?)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn delete_sandbox_instance(
        &self,
        tenant_id: &TenantId,
        sandbox_instance_id: &SandboxInstanceId,
        expected_sandbox_version: u64,
    ) -> SandboxInstanceRepositoryResult<bool> {
        let result = sqlx::query(
            "DELETE FROM sandbox_instance \
             WHERE tenant_id = $1 AND sandbox_instance_id = $2 AND version = $3",
        )
        .bind(tenant_id.as_str())
        .bind(sandbox_instance_id.as_str())
        .bind(
            i64::try_from(expected_sandbox_version)
                .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
        )
        .execute(self.sandbox_postgres_pool()?)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;
        Ok(result.rows_affected() == 1)
    }
}

/// Marks a dynamically assembled statement as audited for sqlx 0.9's
/// compile-time injection check (`SqlSafeStr`).
///
/// Every instance statement is assembled exclusively from the fixed clauses in
/// this file and `SANDBOX_INSTANCE_COLUMNS`; each request-controlled value
/// enters through a `$N` bind parameter. Never interpolate a value derived from
/// request input into the SQL text.
fn audited_sql(sql: &str) -> sqlx::AssertSqlSafe<&str> {
    sqlx::AssertSqlSafe(sql)
}
