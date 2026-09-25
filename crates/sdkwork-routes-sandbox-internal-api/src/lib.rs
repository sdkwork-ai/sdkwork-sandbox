#![forbid(unsafe_code)]
//! SDKWork Sandbox internal-api surface
//! (`INTERNAL_API_SPEC.md`; `TECH_ARCHITECTURE.md` section 5).
//!
//! The application-local control slice: a verified internal caller applies for
//! a sandbox instance, lists the tenant's instances, reads one, updates it, and
//! retires it. The surface is a pure HTTP adapter over
//! [`sdkwork_intelligence_sandbox_service::SandboxInstanceService`]: it owns the
//! wire vocabulary, the response envelope and the error taxonomy, and nothing
//! else. All business routes are ingress-token gated; the only public paths are
//! the infrastructure probes.

use axum::{extract::State, routing::get, Router};
use sdkwork_intelligence_sandbox_service::{SandboxInstanceRepository, SandboxInstanceService};
use sdkwork_web_bootstrap::ReadinessCheck;
use sdkwork_web_core::HttpRouteManifest;
use std::sync::Arc;

mod errors;
mod handlers;
pub mod health;
pub mod http_route_manifest;
mod paths;
mod payloads;
mod ports;
mod response;
mod web_bootstrap;

pub use errors::SandboxApiError;
pub use handlers::{
    create_sandbox_instance, delete_sandbox_instance, list_sandbox_instances,
    retrieve_sandbox_instance, update_sandbox_instance,
};
pub use health::livez;
pub use http_route_manifest::internal_route_manifest;
pub use payloads::{
    encode_sandbox_instance_cursor, CreateSandboxInstanceRequest, SandboxInstanceListQuery,
    SandboxInstanceView, UpdateSandboxInstanceRequest,
};
pub use ports::SandboxInternalRequestContext;
pub use response::{cursor_page_data, finish_api_json, item_data, ApiResult};
pub use web_bootstrap::{
    sandbox_public_path_prefixes, wrap_router_with_web_framework,
    wrap_router_with_web_framework_from_env, SandboxInternalContextInjector,
};

/// Shared state for every Sandbox internal-api handler.
///
/// There is no default tenant: a request without a verified tenant context is
/// rejected by the handlers, never re-scoped.
#[derive(Clone)]
pub struct AppState<R: SandboxInstanceRepository> {
    /// CRUD over the instance registry.
    pub service: Arc<SandboxInstanceService<R>>,
    /// Readiness probe for a standalone mount; the composing gateway owns its
    /// own aggregation and leaves this `None`.
    pub readiness: Option<Arc<dyn ReadinessCheck>>,
}

pub(crate) fn business_routes<R>() -> Router<AppState<R>>
where
    R: SandboxInstanceRepository + Clone + Send + Sync + 'static,
{
    Router::new()
        .route(
            paths::SANDBOX_INSTANCES,
            get(list_sandbox_instances::<R>).post(create_sandbox_instance::<R>),
        )
        .route(
            paths::SANDBOX_INSTANCE,
            get(retrieve_sandbox_instance::<R>)
                .patch(update_sandbox_instance::<R>)
                .delete(delete_sandbox_instance::<R>),
        )
}

/// The business routes of this surface, for the composing assembly only.
///
/// The assembly hands this router to [`sdkwork_web_bootstrap::ApiAssemblyContribution`]
/// together with [`internal_route_manifest`], and the composing gateway then
/// applies the web framework layer that enforces ingress-token auth. Handlers
/// fail closed without a verified tenant context, so an un-wrapped mount cannot
/// serve tenant data.
pub fn assembly_business_router<R>(state: AppState<R>) -> Router
where
    R: SandboxInstanceRepository + Clone + Send + Sync + 'static,
{
    business_routes::<R>().with_state(state)
}

/// Routes plus the infrastructure probes of a standalone mount, wrapped in the
/// web framework layer (ingress-token gate applied).
pub async fn standalone_router<R>(
    state: AppState<R>,
    resolver: sdkwork_iam_web_adapter::IamWebRequestContextResolver,
) -> Router
where
    R: SandboxInstanceRepository + Clone + Send + Sync + 'static,
{
    let router = Router::new()
        .route(paths::LIVEZ, get(health::livez))
        .route(paths::READYZ, get(readyz::<R>))
        .route(paths::HEALTHZ, get(healthz::<R>))
        .merge(business_routes::<R>())
        .with_state(state);
    wrap_router_with_web_framework(resolver, router)
}

/// Builds the standalone router with a PostgreSQL-backed readiness probe,
/// resolving the IAM context resolver from the environment.
pub async fn build_standalone_router_from_env<R>(
    service: Arc<SandboxInstanceService<R>>,
    readiness: Option<Arc<dyn ReadinessCheck>>,
) -> Router
where
    R: SandboxInstanceRepository + Clone + Send + Sync + 'static,
{
    let resolver = sdkwork_iam_web_adapter::iam_web_request_context_resolver_from_env().await;
    standalone_router(AppState { service, readiness }, resolver).await
}

/// The route manifest this surface publishes to the composing assembly.
#[must_use]
pub fn gateway_route_manifest() -> HttpRouteManifest {
    internal_route_manifest()
}

async fn readyz<R>(State(state): State<AppState<R>>) -> axum::response::Response
where
    R: SandboxInstanceRepository + Clone + Send + Sync + 'static,
{
    health::readyz(state.readiness.clone()).await
}

async fn healthz<R>(State(state): State<AppState<R>>) -> axum::response::Response
where
    R: SandboxInstanceRepository + Clone + Send + Sync + 'static,
{
    health::healthz(state.readiness.clone()).await
}
