//! API assembly bootstrap for sdkwork-sandbox.
//!
//! The assembly exports the indivisible `ApiAssemblyContribution` contract
//! (`API_ASSEMBLY_SPEC.md` section 4). A composing gateway supplies its
//! process-shared PostgreSQL pool through [`assemble_api_router_with_pool`]; a
//! standalone mount resolves its own through [`assemble_api_router`].
//!
//! The contribution carries both served faces as one owner surface set
//! (`REQ-2026-0030`, `ADR-20261004`): the dual-token app-api console face and
//! the ingress-token internal-api face, with one surface-branching domain
//! context injector and one merged route manifest. A second contribution for
//! the same owner would fail composition at the edge, so the merge happens
//! here and nowhere else.

use axum::Router;
use sdkwork_database_sqlx::DatabasePool;
use sdkwork_intelligence_sandbox_repository_sqlx::SqlxSandboxInstanceRepository;
use sdkwork_intelligence_sandbox_service::SandboxInstanceService;
use sdkwork_routes_sandbox_internal_api::AppState;
use sdkwork_sandbox_database_host::{
    bootstrap_sandbox_database, bootstrap_sandbox_database_from_env, SandboxDatabaseHostError,
};
use sdkwork_web_bootstrap::{
    ApiAssemblyContribution, CompositeReadinessCheck, PgPoolReadinessCheck, WebModule,
};
use sqlx::PgPool;
use std::error::Error as StdError;
use std::sync::Arc;
use thiserror::Error;

use crate::context::SandboxDomainContextInjector;
use crate::readiness::SandboxDrainGate;
use crate::route_manifest::sandbox_api_route_manifest;

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
    ///
    /// Readiness composes the pool probe with the caller's drain gate, so a
    /// gracefully draining instance fails `/readyz` and load balancers stop
    /// routing to it while its last in-flight requests finish.
    fn assemble(
        self,
        sandbox_drain_gate: &SandboxDrainGate,
    ) -> Result<ApiAssembly, SandboxAssemblyError> {
        // Both faces share one service instance; each route crate shapes it
        // into its own `AppState`, so neither face can read the other's
        // request context type.
        let app_router = sdkwork_routes_sandbox_app_api::assembly_business_router(
            sdkwork_routes_sandbox_app_api::AppState {
                service: self.service.clone(),
            },
        );
        let internal_router =
            sdkwork_routes_sandbox_internal_api::assembly_business_router(AppState {
                service: self.service,
                readiness: None,
            });
        let route_manifest = sandbox_api_route_manifest();
        ApiAssemblyContribution::from_manifest(
            "sdkwork-sandbox",
            "SDKWork Sandbox API",
            Router::new().merge(app_router).merge(internal_router),
            route_manifest,
            vec![Arc::new(SandboxDomainContextInjector)],
            Arc::new(
                CompositeReadinessCheck::new(vec![Arc::new(PgPoolReadinessCheck::new(self.pool))])
                    .push(Arc::new(sandbox_drain_gate.clone())),
            ),
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
    assemble_api_router_with_pool_and_drain(pool, &SandboxDrainGate::new()).await
}

/// Same as [`assemble_api_router_with_pool`] with an explicit drain gate: the
/// gateway flips it on shutdown so readiness fails while the drain finishes.
///
/// # Errors
///
/// Returns [`SandboxAssemblyError`] for the database lifecycle, repository
/// construction or manifest error.
pub async fn assemble_api_router_with_pool_and_drain(
    pool: DatabasePool,
    sandbox_drain_gate: &SandboxDrainGate,
) -> Result<ApiAssembly, SandboxAssemblyError> {
    let host = bootstrap_sandbox_database(pool).await?;
    SandboxRuntime::from_pool(host.pool().clone())
        .and_then(|runtime| runtime.assemble(sandbox_drain_gate))
}

/// Assembles the contribution from environment-resolved configuration.
///
/// # Errors
///
/// Returns [`SandboxAssemblyError`] for the database lifecycle, repository
/// construction or manifest error.
pub async fn assemble_api_router() -> Result<ApiAssembly, SandboxAssemblyError> {
    assemble_api_router_with_drain(&SandboxDrainGate::new()).await
}

/// Same as [`assemble_api_router`] with an explicit drain gate (see
/// [`assemble_api_router_with_pool_and_drain`]).
///
/// # Errors
///
/// Returns [`SandboxAssemblyError`] for the database lifecycle, repository
/// construction or manifest error.
pub async fn assemble_api_router_with_drain(
    sandbox_drain_gate: &SandboxDrainGate,
) -> Result<ApiAssembly, SandboxAssemblyError> {
    let host = bootstrap_sandbox_database_from_env().await?;
    SandboxRuntime::from_pool(host.pool().clone())
        .and_then(|runtime| runtime.assemble(sandbox_drain_gate))
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

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_web_contract::{route_inventory_from_openapi, route_inventory_from_routes};
    use sdkwork_web_core::HttpRouteManifest;

    #[test]
    fn combined_manifest_openapi_and_permission_inventories_match() {
        let manifest = sandbox_api_route_manifest();
        let openapi =
            sdkwork_web_contract::build_openapi_document("SDKWork Sandbox API", manifest.routes());
        assert_eq!(
            route_inventory_from_routes(manifest.routes()),
            route_inventory_from_openapi(&openapi)
                .expect("valid combined Sandbox OpenAPI inventory")
        );
        assert_permission_catalog_matches(&manifest);
    }

    #[test]
    fn app_api_manifest_openapi_and_permission_inventories_match() {
        let manifest = sdkwork_routes_sandbox_app_api::app_route_manifest();
        let openapi = sdkwork_web_contract::build_openapi_document(
            "SDKWork Sandbox App API",
            manifest.routes(),
        );
        assert_eq!(
            route_inventory_from_routes(manifest.routes()),
            route_inventory_from_openapi(&openapi)
                .expect("valid Sandbox App API OpenAPI inventory")
        );
        assert_permission_catalog_matches(&manifest);
    }

    #[test]
    fn composed_manifest_carries_both_surfaces_exactly_once() {
        let manifest = sandbox_api_route_manifest();
        let mut app_routes = 0;
        let mut internal_routes = 0;
        for route in manifest.routes() {
            if route.path.starts_with("/app/v3/api/sandbox/") {
                app_routes += 1;
            } else if route
                .path
                .starts_with("/internal/v3/api/intelligence/sandbox/")
            {
                internal_routes += 1;
            }
        }
        assert_eq!(5, app_routes, "the app-api face serves five routes");
        assert_eq!(
            5, internal_routes,
            "the internal-api face serves five routes"
        );
        assert_eq!(
            10,
            manifest.routes().len(),
            "the merged manifest carries both faces and nothing else"
        );
    }

    #[test]
    fn composed_manifest_passes_surface_auth_validation() {
        // The composed contribution mixes face auth shapes by design: each
        // route is validated against its own path's surface, so the app-api
        // face must stay dual-token while the internal-api face stays
        // ingress-token. This is the same check `from_manifest` runs at
        // composition time; a regression here fails the edge at startup.
        let manifest = sandbox_api_route_manifest();
        manifest
            .validate_route_auth_for_surfaces(
                &sdkwork_web_core::WebRequestContextProfile::default(),
            )
            .expect("both composed faces must satisfy their surface auth contracts");
    }

    fn assert_permission_catalog_matches(manifest: &HttpRouteManifest) {
        let mut expected = manifest
            .routes()
            .iter()
            .flat_map(|route| {
                route
                    .required_permission
                    .into_iter()
                    .chain(route.alternate_permissions.into_iter().flatten().copied())
            })
            .collect::<Vec<_>>();
        expected.sort_unstable();
        expected.dedup();
        assert_eq!(
            expected,
            sdkwork_web_bootstrap::permission_catalog(manifest.routes())
        );
    }
}
