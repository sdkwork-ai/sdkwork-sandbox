//! API assembly bootstrap for sdkwork-sandbox.
//!
//! The assembly exports the indivisible `ApiAssemblyContribution` contract
//! (`API_ASSEMBLY_SPEC.md` section 4). A composing gateway supplies its
//! process-shared PostgreSQL pool through [`assemble_api_router_with_pool`]; a
//! standalone mount resolves its own through [`assemble_api_router`].

use sdkwork_database_sqlx::DatabasePool;
use sdkwork_intelligence_sandbox_repository_sqlx::SqlxSandboxInstanceRepository;
use sdkwork_intelligence_sandbox_service::SandboxInstanceService;
use sdkwork_routes_sandbox_internal_api::{AppState, SandboxInternalContextInjector};
use sdkwork_sandbox_database_host::{
    bootstrap_sandbox_database, bootstrap_sandbox_database_from_env, SandboxDatabaseHostError,
};
use sdkwork_web_bootstrap::{ApiAssemblyContribution, PgPoolReadinessCheck, WebModule};
use sdkwork_web_core::HttpRouteManifest;
use sqlx::PgPool;
use std::error::Error as StdError;
use std::sync::Arc;
use thiserror::Error;

/// Indivisible host-neutral API assembly contribution (web-bootstrap contract).
pub type ApiAssembly = ApiAssemblyContribution;

/// Typed assembly failure. Every variant preserves the underlying error as its
/// source.
#[derive(Debug, Error)]
pub enum SandboxAssemblyError {
    #[error("sandbox runtime requires a postgres database pool")]
    PostgresPoolRequired,
    #[error("build sandbox instance repository failed")]
    Repository(#[source] Box<dyn StdError + Send + Sync>),
    #[error("assemble sandbox api contribution failed: {0}")]
    Contribution(String),
    #[error(transparent)]
    DatabaseHost(#[from] SandboxDatabaseHostError),
}

fn boxed<E>(error: E) -> Box<dyn StdError + Send + Sync>
where
    E: StdError + Send + Sync + 'static,
{
    Box::new(error)
}

struct SandboxRuntime {
    service: Arc<SandboxInstanceService<SqlxSandboxInstanceRepository>>,
    pool: PgPool,
}

impl SandboxRuntime {
    fn from_pool(pool: DatabasePool) -> Result<Self, SandboxAssemblyError> {
        let postgres_pool = pool
            .as_postgres()
            .cloned()
            .ok_or(SandboxAssemblyError::PostgresPoolRequired)?;
        let repository = SqlxSandboxInstanceRepository::new(pool)
            .map_err(|error| SandboxAssemblyError::Repository(boxed(error)))?;
        Ok(Self {
            service: Arc::new(SandboxInstanceService::new(Arc::new(repository))),
            pool: postgres_pool,
        })
    }

    /// Builds the contribution, so the caller also receives the manifest
    /// validation verdict (unknown owner prefix, auth-shape mismatch, an empty
    /// public path set) instead of a partially wired assembly.
    fn assemble(self) -> Result<ApiAssembly, SandboxAssemblyError> {
        let router = sdkwork_routes_sandbox_internal_api::assembly_business_router(AppState {
            service: self.service,
            readiness: None,
        });
        let route_manifest = HttpRouteManifest::from_owned_routes(
            sdkwork_routes_sandbox_internal_api::gateway_route_manifest()
                .routes()
                .to_vec(),
        );
        ApiAssemblyContribution::from_manifest(
            "sdkwork-sandbox",
            "SDKWork Sandbox API",
            router,
            route_manifest,
            vec![Arc::new(SandboxInternalContextInjector)],
            Arc::new(PgPoolReadinessCheck::new(self.pool)),
        )
        .map_err(SandboxAssemblyError::Contribution)
    }
}

/// Assembles the contribution against a caller-provided database pool, so a
/// composing gateway shares its process-wide PostgreSQL pool.
///
/// # Errors
///
/// Returns [`SandboxAssemblyError`] for the database lifecycle, repository
/// construction or manifest error.
pub async fn assemble_api_router_with_pool(
    pool: DatabasePool,
) -> Result<ApiAssembly, SandboxAssemblyError> {
    let host = bootstrap_sandbox_database(pool).await?;
    SandboxRuntime::from_pool(host.pool().clone()).and_then(SandboxRuntime::assemble)
}

/// Assembles the contribution from environment-resolved configuration.
///
/// # Errors
///
/// Returns [`SandboxAssemblyError`] for the database lifecycle, repository
/// construction or manifest error.
pub async fn assemble_api_router() -> Result<ApiAssembly, SandboxAssemblyError> {
    let host = bootstrap_sandbox_database_from_env().await?;
    SandboxRuntime::from_pool(host.pool().clone()).and_then(SandboxRuntime::assemble)
}

/// Runs the Sandbox-owned database lifecycle without constructing HTTP routes.
///
/// # Errors
///
/// Returns [`SandboxDatabaseHostError`] for the database lifecycle error.
pub async fn bootstrap_database_from_env() -> Result<(), SandboxDatabaseHostError> {
    bootstrap_sandbox_database_from_env().await.map(|_| ())
}

/// Canonical Web Module definition for this application
/// (`API_ASSEMBLY_SPEC.md` section 4.1.1): the complete HTTP surface — every
/// route and manifest document of this owner — as one installable module.
///
/// # Errors
///
/// Returns the error propagated from [`assemble_api_router`].
pub async fn web_module() -> Result<WebModule, SandboxAssemblyError> {
    Ok(WebModule::from_contribution(assemble_api_router().await?))
}

/// Same as [`web_module`] but composed on a process-shared database pool.
///
/// # Errors
///
/// Returns the error propagated from [`assemble_api_router_with_pool`].
pub async fn web_module_with_pool(pool: DatabasePool) -> Result<WebModule, SandboxAssemblyError> {
    Ok(WebModule::from_contribution(
        assemble_api_router_with_pool(pool).await?,
    ))
}
