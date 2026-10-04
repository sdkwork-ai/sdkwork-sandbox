//! Route manifest for the Sandbox app-api surface
//! (`API_SPEC.md` sections 4.5, 5-7 and 14; `REQ-2026-0030`,
//! `ADR-20261004`).
//!
//! Every route is a dual-token route: the tenant and the caller come from the
//! verified IAM principal the composing gateway resolves
//! (`API_SPEC.md` section 12), never from a client-writable header or body
//! field. Each route carries the IAM permission code that federated catalog
//! roles grant for it (`specs/iam.module.manifest.json`); on app-api surfaces
//! the platform's surface-authorization tiers keep the manifest gate
//! observability-only and let the service layer's self-scoping decide access
//! (`PERMISSION_STANDARD_SPEC` surface tiers).

use sdkwork_web_core::{HttpMethod, HttpRoute, HttpRouteManifest};

use crate::paths;

/// `GET`/`PATCH` reads over the caller's own instances.
pub const PERM_INSTANCES_READ: &str = "sandbox.instances.read";
/// `POST` provisioning under the caller's own identity.
pub const PERM_INSTANCES_CREATE: &str = "sandbox.instances.create";
/// `PATCH` updates of the caller's own instances.
pub const PERM_INSTANCES_UPDATE: &str = "sandbox.instances.update";
/// `DELETE` retirement of the caller's own instances.
pub const PERM_INSTANCES_DELETE: &str = "sandbox.instances.delete";

const fn sandbox_app_route(
    method: HttpMethod,
    path: &'static str,
    operation_id: &'static str,
    permission: &'static str,
) -> HttpRoute {
    HttpRoute::dual_token(method, path, "sandbox", operation_id)
        .with_required_permission(permission)
}

const HTTP_ROUTES: &[HttpRoute] = &[
    sandbox_app_route(
        HttpMethod::Get,
        paths::SANDBOX_INSTANCES,
        "sandboxInstances.list",
        PERM_INSTANCES_READ,
    ),
    sandbox_app_route(
        HttpMethod::Post,
        paths::SANDBOX_INSTANCES,
        "sandboxInstances.create",
        PERM_INSTANCES_CREATE,
    ),
    sandbox_app_route(
        HttpMethod::Get,
        paths::SANDBOX_INSTANCE,
        "sandboxInstances.retrieve",
        PERM_INSTANCES_READ,
    ),
    sandbox_app_route(
        HttpMethod::Patch,
        paths::SANDBOX_INSTANCE,
        "sandboxInstances.update",
        PERM_INSTANCES_UPDATE,
    ),
    sandbox_app_route(
        HttpMethod::Delete,
        paths::SANDBOX_INSTANCE,
        "sandboxInstances.delete",
        PERM_INSTANCES_DELETE,
    ),
];

#[must_use]
pub fn app_route_manifest() -> HttpRouteManifest {
    HttpRouteManifest::new(HTTP_ROUTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_web_core::RouteAuth;

    #[test]
    fn every_declared_route_is_dual_token_gated_with_its_permission() {
        let manifest = app_route_manifest();
        assert_eq!(
            HTTP_ROUTES.len(),
            manifest.routes().len(),
            "manifest must expose exactly the declared routes"
        );
        for route in manifest.routes() {
            assert!(
                route.auth == RouteAuth::DualToken,
                "route {} must require dual-token auth",
                route.operation_id
            );
            assert!(
                route
                    .required_permission
                    .is_some_and(|permission| { permission.starts_with("sandbox.instances.") }),
                "route {} must declare a federated sandbox.instances permission",
                route.operation_id
            );
            assert!(
                !manifest.is_public_route("GET", route.path),
                "route {} must not be public",
                route.operation_id
            );
        }
    }

    #[test]
    fn every_route_is_mounted_under_the_locked_app_prefix() {
        for route in HTTP_ROUTES {
            assert!(
                route.path.starts_with("/app/v3/api/sandbox/"),
                "route {} must stay under the locked /app/v3/api prefix",
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

    #[test]
    fn permission_codes_match_the_federated_catalog() {
        // `specs/iam.module.manifest.json` owns exactly these four codes
        // (`REQ-2026-0030`); a code that drifts here would be grantable in
        // the catalog yet unenforceable on the manifest, or vice versa.
        let declared: std::collections::BTreeSet<_> = HTTP_ROUTES
            .iter()
            .filter_map(|route| route.required_permission)
            .collect();
        assert_eq!(
            std::collections::BTreeSet::from([
                PERM_INSTANCES_READ,
                PERM_INSTANCES_CREATE,
                PERM_INSTANCES_UPDATE,
                PERM_INSTANCES_DELETE,
            ]),
            declared
        );
    }
}
