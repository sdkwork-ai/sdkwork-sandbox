//! Route manifest for the Sandbox internal-api surface
//! (`INTERNAL_API_SPEC.md` sections 2, 4 and 6; `API_SPEC.md` sections 5-7).
//!
//! Every route is an ingress-token route: the tenant and the caller identity
//! come from the verified `WebRequestContext` (the ingress token resolved on
//! the application host), never from a client-writable header or body field
//! (`API_SPEC.md` section 12, `INTERNAL_API_SPEC.md` section 4). Protected
//! internal-api operations are operator-trusted, so no per-route IAM
//! permission code is declared — the surface gate is the ingress token itself.

use sdkwork_web_core::{HttpMethod, HttpRoute, HttpRouteManifest};

use crate::paths;

const fn sandbox_route(
    method: HttpMethod,
    path: &'static str,
    operation_id: &'static str,
) -> HttpRoute {
    HttpRoute::ingress_token(method, path, "sandbox", operation_id)
}

const HTTP_ROUTES: &[HttpRoute] = &[
    sandbox_route(
        HttpMethod::Get,
        paths::SANDBOX_INSTANCES,
        "sandboxInstances.list",
    ),
    sandbox_route(
        HttpMethod::Post,
        paths::SANDBOX_INSTANCES,
        "sandboxInstances.create",
    ),
    sandbox_route(
        HttpMethod::Get,
        paths::SANDBOX_INSTANCE,
        "sandboxInstances.retrieve",
    ),
    sandbox_route(
        HttpMethod::Patch,
        paths::SANDBOX_INSTANCE,
        "sandboxInstances.update",
    ),
    sandbox_route(
        HttpMethod::Delete,
        paths::SANDBOX_INSTANCE,
        "sandboxInstances.delete",
    ),
];

#[must_use]
pub fn internal_route_manifest() -> HttpRouteManifest {
    HttpRouteManifest::new(HTTP_ROUTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_declared_route_is_ingress_token_gated() {
        let manifest = internal_route_manifest();
        assert_eq!(
            HTTP_ROUTES.len(),
            manifest.routes().len(),
            "manifest must expose exactly the declared routes"
        );
        for route in manifest.routes() {
            assert!(
                route.auth.is_ingress_token_credential_mode(),
                "route {} must require ingress-token auth",
                route.operation_id
            );
            assert!(
                route.required_permission.is_none(),
                "internal-api routes are operator-trusted and must not declare an IAM permission",
            );
            assert!(
                !manifest.is_public_route("GET", route.path),
                "route {} must not be public",
                route.operation_id
            );
        }
    }

    #[test]
    fn every_route_is_mounted_under_the_locked_internal_prefix() {
        for route in HTTP_ROUTES {
            assert!(
                route
                    .path
                    .starts_with("/internal/v3/api/intelligence/sandbox/"),
                "route {} must stay under the locked /internal/v3/api prefix",
                route.operation_id
            );
        }
    }

    #[test]
    fn operation_ids_are_globally_unique_and_do_not_repeat_the_tag() {
        let mut seen = std::collections::BTreeSet::new();
        for route in HTTP_ROUTES {
            assert!(
                seen.insert(route.operation_id),
                "duplicate operationId {}",
                route.operation_id
            );
            assert_eq!(
                "sandbox", route.tag,
                "route {} must carry the canonical tag",
                route.operation_id
            );
            let resource_root = route.operation_id.split('.').next().unwrap_or_default();
            assert_eq!(
                "sandboxInstances", resource_root,
                "operationId {} must open with the resource root",
                route.operation_id
            );
            assert_ne!(
                route.tag, resource_root,
                "operationId {} must not repeat the tag (API_SPEC section 7.1)",
                route.operation_id
            );
        }
    }
}
