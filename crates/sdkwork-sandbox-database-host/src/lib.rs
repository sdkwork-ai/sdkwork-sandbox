#![forbid(unsafe_code)]
//! Sandbox database lifecycle bootstrap.
//!
//! `DATABASE_FRAMEWORK_SPEC.md` section 4 gives the owning application the
//! responsibility for its own schema lifecycle: the HTTP assembly never runs
//! `db:migrate` at serve time, so the migration closure is applied through the
//! explicit `pnpm db:*` entry points and observed here as drift.
//!
//! The same module composes in-process under the Web Server edge, so a schema
//! drift must fail readiness rather than surface as a request-time error on a
//! surface the composition layer already declared served
//! (`DATABASE_FRAMEWORK_SPEC.md` section 4.4.1).

use std::error::Error as StdError;
use std::path::PathBuf;
use std::sync::Arc;

use sdkwork_database_config::DatabaseConfig;
use sdkwork_database_drift::DriftEngine;
use sdkwork_database_lifecycle::{lifecycle_options_from_env, LifecycleOrchestrator};
use sdkwork_database_spi::{DatabaseAssetProvider, DatabaseManifest, DefaultDatabaseModule};
use sdkwork_database_sqlx::{create_pool_from_config, DatabasePool};
use thiserror::Error;

/// Module id used by the database framework manifests.
pub const MODULE_ID: &str = "sandbox";

/// Environment key overriding the application root the database module is read
/// from; defaults to the repository the crate is compiled inside.
pub const APP_ROOT_ENV: &str = "SDKWORK_SANDBOX_APP_ROOT";

/// Typed failure of the Sandbox database lifecycle bootstrap. Every variant
/// preserves the underlying error as its source.
#[derive(Debug, Error)]
pub enum SandboxDatabaseHostError {
    #[error("load sandbox database module failed")]
    LoadModule(#[source] Box<dyn StdError + Send + Sync>),
    #[error("read sandbox database manifest failed")]
    ReadManifest(#[source] Box<dyn StdError + Send + Sync>),
    #[error("read sandbox database config failed")]
    ReadConfig(#[source] Box<dyn StdError + Send + Sync>),
    #[error("create sandbox database pool failed")]
    CreatePool(#[source] Box<dyn StdError + Send + Sync>),
    #[error("sandbox database init failed")]
    Init(#[source] Box<dyn StdError + Send + Sync>),
    #[error("sandbox database migrate failed")]
    Migrate(#[source] Box<dyn StdError + Send + Sync>),
    #[error("sandbox database drift check failed")]
    DriftCheck(#[source] Box<dyn StdError + Send + Sync>),
    #[error(
        "sandbox database schema drift detected ({error_count} error(s)): {details}. \
         Run `pnpm db:migrate` and then `pnpm db:drift:check`"
    )]
    SchemaDrift { error_count: u32, details: String },
}

pub struct SandboxDatabaseHost {
    pool: DatabasePool,
    module: Arc<DefaultDatabaseModule>,
}

impl SandboxDatabaseHost {
    #[must_use]
    pub fn pool(&self) -> &DatabasePool {
        &self.pool
    }

    #[must_use]
    pub fn postgres_pool(&self) -> Option<sqlx::PgPool> {
        self.pool.as_postgres().cloned()
    }

    #[must_use]
    pub fn module(&self) -> Arc<DefaultDatabaseModule> {
        self.module.clone()
    }
}

fn boxed<E>(error: E) -> Box<dyn StdError + Send + Sync>
where
    E: StdError + Send + Sync + 'static,
{
    Box::new(error)
}

/// Runs the Sandbox module's lifecycle against a caller-supplied pool.
///
/// # Errors
///
/// Returns [`SandboxDatabaseHostError`] when the module cannot be loaded, the
/// lifecycle fails, or the schema drifts from the contract with `error`-severity
/// diffs.
pub async fn bootstrap_sandbox_database(
    pool: DatabasePool,
) -> Result<SandboxDatabaseHost, SandboxDatabaseHostError> {
    let app_root = resolve_app_root();
    let module = Arc::new(
        DefaultDatabaseModule::from_app_root(&app_root)
            .map_err(|error| SandboxDatabaseHostError::LoadModule(boxed(error)))?,
    );
    let manifest = DatabaseManifest::from_file(module.manifest_path())
        .map_err(|error| SandboxDatabaseHostError::ReadManifest(boxed(error)))?;
    let options = lifecycle_options_from_env("SANDBOX", &manifest);
    let orchestrator =
        LifecycleOrchestrator::new(pool.clone(), module.clone()).with_applied_by("sdkwork-sandbox");

    orchestrator
        .init()
        .await
        .map_err(|error| SandboxDatabaseHostError::Init(boxed(error)))?;

    if options.auto_migrate {
        orchestrator
            .migrate()
            .await
            .map_err(|error| SandboxDatabaseHostError::Migrate(boxed(error)))?;
    }

    let drift = DriftEngine::new(pool.clone(), module.clone())
        .analyze()
        .await
        .map_err(|error| SandboxDatabaseHostError::DriftCheck(boxed(error)))?;
    if drift.summary.error > 0 {
        let details = drift
            .diffs
            .iter()
            .filter(|diff| diff.severity == "error")
            .take(5)
            .map(|diff| diff.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(SandboxDatabaseHostError::SchemaDrift {
            error_count: drift.summary.error,
            details,
        });
    }

    Ok(SandboxDatabaseHost { pool, module })
}

/// Loads the workspace database profile from the environment and bootstraps.
///
/// # Errors
///
/// Returns [`SandboxDatabaseHostError`] when the configuration, pool creation,
/// or the lifecycle/drift checks fail.
pub async fn bootstrap_sandbox_database_from_env(
) -> Result<SandboxDatabaseHost, SandboxDatabaseHostError> {
    let _ = dotenvy::dotenv();
    let config = DatabaseConfig::from_env("SANDBOX")
        .map_err(|error| SandboxDatabaseHostError::ReadConfig(boxed(error)))?;
    let pool = create_pool_from_config(config)
        .await
        .map_err(|error| SandboxDatabaseHostError::CreatePool(boxed(error)))?;
    bootstrap_sandbox_database(pool).await
}

fn resolve_app_root() -> PathBuf {
    std::env::var(APP_ROOT_ENV).map_or_else(
        |_| {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let candidate = manifest_dir.join("../..");
            candidate.canonicalize().unwrap_or(candidate)
        },
        PathBuf::from,
    )
}
