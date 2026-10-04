//! Canonical route paths for the Sandbox app-api console surface
//! (`API_SPEC.md` sections 7 and 14; `REQ-2026-0030`).
//!
//! The surface is locked to the `/app/v3/api` prefix with the module's
//! application-code segment (`/app/v3/api/sandbox/*`), matching every
//! console-called dependency module on the Web Server edge, so the
//! operationId resource segment is `sandboxInstances`
//! (`API_SPEC.md` section 7.3).

/// `GET` / `POST` on the signed-in principal's sandbox instance collection.
pub const SANDBOX_INSTANCES: &str = "/app/v3/api/sandbox/sandbox_instances";
/// `GET` / `PATCH` / `DELETE` on one of the principal's sandbox instances.
pub const SANDBOX_INSTANCE: &str = "/app/v3/api/sandbox/sandbox_instances/{sandboxInstanceId}";
