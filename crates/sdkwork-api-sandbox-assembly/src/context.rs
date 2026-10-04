//! Domain context projection for both composed Sandbox surfaces.
//!
//! One injector serves the composed contribution, branching on the resolved
//! API surface: the dual-token app-api face gets the caller-scoped
//! [`SandboxAppRequestContext`], and the ingress-token internal-api face gets
//! the internal context projected by its own route crate. A surface this
//! crate does not own receives no extension, so a handler can never read
//! another surface's identity shape (`WEB_FRAMEWORK_SPEC.md`, `ADR-20261004`).

use axum::extract::Request;
use sdkwork_routes_sandbox_app_api::SandboxAppRequestContext;
use sdkwork_web_core::{DomainContextInjector, WebApiSurface, WebRequestContext};

/// Projects the verified web context into the per-surface Sandbox request
/// context.
#[derive(Clone, Default)]
pub struct SandboxDomainContextInjector;

impl DomainContextInjector for SandboxDomainContextInjector {
    fn inject(&self, request: &mut Request, context: &WebRequestContext) {
        match context.api_surface {
            WebApiSurface::AppApi => {
                if let Some(app_context) = sandbox_app_context_from_web_request(context) {
                    request.extensions_mut().insert(app_context);
                }
            }
            WebApiSurface::InternalApi => {
                // The internal-api projection is owned by its route crate, so
                // the composed injector reuses it verbatim instead of forking
                // the principal shape.
                sdkwork_routes_sandbox_internal_api::SandboxInternalContextInjector
                    .inject(request, context);
            }
            _ => {}
        }
    }
}

fn sandbox_app_context_from_web_request(
    context: &WebRequestContext,
) -> Option<SandboxAppRequestContext> {
    let principal = context.principal.as_ref()?;
    // Both ids must be present and positive: there is no anonymous or partial
    // shape for the app-api face to interpret, so a principal without a usable
    // owner receives no extension and the handlers fail closed.
    let tenant_id = positive_id(principal.tenant_id())?;
    let actor_id = positive_id(principal.user_id())?;
    Some(SandboxAppRequestContext {
        tenant_id,
        actor_id,
    })
}

fn positive_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = trimmed.parse::<u64>().ok().filter(|id| *id > 0)?;
    Some(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_id_keeps_only_positive_decimal_principals() {
        assert_eq!(Some("7".to_owned()), positive_id(" 7 "));
        assert_eq!(None, positive_id("0"));
        assert_eq!(None, positive_id("-3"));
        assert_eq!(None, positive_id(""));
        assert_eq!(None, positive_id("tenant-instance"));
    }
}
