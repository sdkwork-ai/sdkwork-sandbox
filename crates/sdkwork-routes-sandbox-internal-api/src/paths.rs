//! Canonical route paths for the Sandbox internal-api surface
//! (`INTERNAL_API_SPEC.md` sections 2 and 6).
//!
//! The surface is locked to the `/internal/v3/api` prefix and the domain
//! segments follow `DOMAIN_SPEC.md` (`/internal/v3/api/intelligence/sandbox/*`),
//! so the operationId resource segment is `sandboxInstances`
//! (`API_SPEC.md` section 7.3).

/// Liveness probe. Infrastructure paths are owned by the composing gateway, but
/// a standalone mount still answers them (WEB_FRAMEWORK_SPEC health contract).
pub const LIVEZ: &str = "/livez";
/// Readiness probe.
pub const READYZ: &str = "/readyz";
/// Health probe.
pub const HEALTHZ: &str = "/healthz";

/// `GET` / `POST` on the tenant's sandbox instance collection.
pub const SANDBOX_INSTANCES: &str = "/internal/v3/api/intelligence/sandbox/sandbox_instances";
/// `GET` / `PATCH` / `DELETE` on one sandbox instance.
pub const SANDBOX_INSTANCE: &str =
    "/internal/v3/api/intelligence/sandbox/sandbox_instances/{sandboxInstanceId}";
