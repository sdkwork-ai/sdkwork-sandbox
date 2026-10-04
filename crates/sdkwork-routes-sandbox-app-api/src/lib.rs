#![forbid(unsafe_code)]
//! SDKWork Sandbox app-api console surface
//! (`API_SPEC.md`; `REQ-2026-0030`, `ADR-20261004`).
//!
//! The browser-facing half of the sandbox instance registry: a signed-in
//! console principal lists, provisions, reads, updates, and retires the
//! instances it owns. The surface is a pure HTTP adapter over
//! [`sdkwork_intelligence_sandbox_service::SandboxInstanceService`]: it owns
//! the wire vocabulary, the response envelope and the error taxonomy, and
//! nothing else. Every business route is IAM dual-token gated, the tenant and
//! the owner are derived server-side from the verified principal
//! (`API_SPEC.md` section 12), and the collection listing is always narrowed
//! to the calling owner — the tenant-wide inventory stays the internal-api
//! face's job (`sdkwork-routes-sandbox-internal-api`), never this surface's.

use std::sync::Arc;

use axum::{routing::get, Router};
use sdkwork_intelligence_sandbox_service::{SandboxInstanceRepository, SandboxInstanceService};
use sdkwork_web_core::HttpRouteManifest;

mod errors;
mod handlers;
mod http_route_manifest;
mod paths;
mod payloads;
mod ports;
mod response;

pub use errors::SandboxAppApiError;
pub use handlers::{
    create_sandbox_instance, delete_sandbox_instance, list_sandbox_instances,
    retrieve_sandbox_instance, update_sandbox_instance,
};
pub use http_route_manifest::{
    PERM_INSTANCES_CREATE, PERM_INSTANCES_DELETE, PERM_INSTANCES_READ, PERM_INSTANCES_UPDATE,
};
pub use paths::{SANDBOX_INSTANCE, SANDBOX_INSTANCES};
pub use payloads::{
    CreateSandboxInstanceRequest, SandboxInstanceListQuery, SandboxInstanceView,
    UpdateSandboxInstanceRequest, ValidatedOffsetPage,
};
pub use ports::SandboxAppRequestContext;
pub use response::{finish_api_json, offset_page_data, ApiResult};

/// Shared state for every Sandbox app-api handler.
///
/// There is no default tenant and no default owner: both are projected from
/// the verified principal by the composing assembly's domain context injector
/// ([`SandboxAppRequestContext`]), and a handler that receives none fails
/// closed rather than re-scoping.
#[derive(Clone)]
pub struct AppState<R: SandboxInstanceRepository> {
    /// CRUD over the instance registry.
    pub service: Arc<SandboxInstanceService<R>>,
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
/// The assembly hands this router to
/// [`sdkwork_web_bootstrap::ApiAssemblyContribution`] together with
/// [`app_route_manifest`], and the composing gateway then applies the web
/// framework layer that enforces dual-token auth. Handlers fail closed
/// without a verified principal extension, so an un-wrapped mount cannot
/// serve tenant data.
pub fn assembly_business_router<R>(state: AppState<R>) -> Router
where
    R: SandboxInstanceRepository + Clone + Send + Sync + 'static,
{
    business_routes::<R>().with_state(state)
}

/// The route manifest this surface publishes to the composing assembly.
#[must_use]
pub fn app_route_manifest() -> HttpRouteManifest {
    http_route_manifest::app_route_manifest()
}
