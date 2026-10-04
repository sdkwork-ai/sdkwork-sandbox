//! Merged route manifest for the composed Sandbox contribution.
//!
//! The composed owner manifest is the union of both served faces — the
//! dual-token app-api console routes and the ingress-token internal-api
//! routes — computed once and leaked `'static` because `HttpRoute` is a
//! copy-on-reference inventory type (`ADR-20261004`).

use std::sync::OnceLock;

use sdkwork_web_core::{HttpRoute, HttpRouteManifest};

/// The route manifest the composed `sdkwork-sandbox` contribution publishes.
#[must_use]
pub fn sandbox_api_route_manifest() -> HttpRouteManifest {
    static ROUTES: OnceLock<&'static [HttpRoute]> = OnceLock::new();
    let routes = ROUTES.get_or_init(|| {
        let mut routes = sdkwork_routes_sandbox_app_api::app_route_manifest()
            .routes()
            .to_vec();
        routes.extend_from_slice(
            sdkwork_routes_sandbox_internal_api::gateway_route_manifest().routes(),
        );
        Box::leak(routes.into_boxed_slice())
    });
    HttpRouteManifest::new(routes)
}
