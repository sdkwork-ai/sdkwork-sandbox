//! Typed per-request context injected for Sandbox internal-api handlers.
//!
//! Handlers never re-parse credentials or identity headers; the framework
//! resolves them once into a [`WebRequestContext`](sdkwork_web_core::WebRequestContext)
//! and this projection carries only the tenant and caller the sandbox service
//! needs (`INTERNAL_API_SPEC.md` section 4, `API_SPEC.md` section 12,
//! `WEB_FRAMEWORK_SPEC.md`).

/// Tenant and caller identity for one Sandbox internal-api request.
#[derive(Debug, Clone)]
pub struct SandboxInternalRequestContext {
    /// Tenant the request is scoped to, from the verified ingress context.
    pub tenant_id: String,
    /// Verified caller subject, when the ingress context carries one.
    pub actor_id: Option<String>,
    /// Organization scope, when the ingress context was organization-scoped.
    pub organization_id: Option<String>,
    /// Session identity, when the ingress context carries one.
    pub session_id: Option<String>,
}
