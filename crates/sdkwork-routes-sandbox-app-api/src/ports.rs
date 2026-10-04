//! Typed per-request context injected for Sandbox app-api handlers.
//!
//! Handlers never re-parse credentials or identity headers; the framework
//! resolves the IAM dual token once into a
//! [`WebRequestContext`](sdkwork_web_core::WebRequestContext) and this
//! projection carries only the tenant and caller the sandbox service needs
//! (`API_SPEC.md` section 12, `WEB_FRAMEWORK_SPEC.md`, `REQ-2026-0030`).

/// Tenant and owner identity for one Sandbox app-api request.
///
/// Both fields are the verified principal's positive ids, projected by the
/// composing assembly only when both are present — there is no anonymous or
/// partial shape for this surface to interpret.
#[derive(Debug, Clone)]
pub struct SandboxAppRequestContext {
    /// Tenant the request is scoped to, from the verified principal.
    pub tenant_id: String,
    /// Verified caller subject; also the instance owner for every operation
    /// on this face, so one account cannot address another account's rows.
    pub actor_id: String,
}
