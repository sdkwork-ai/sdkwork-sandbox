//! Web Framework wiring for the Sandbox internal-api router.
//!
//! The layer resolves the typed [`WebRequestContext`] once, enforces the route
//! manifest's ingress-token authentication, and projects the identity the
//! sandbox service needs into a request extension (`WEB_FRAMEWORK_SPEC.md`,
//! `INTERNAL_API_SPEC.md` sections 4 and 8).

use std::sync::Arc;

use axum::Router;
use sdkwork_iam_web_adapter::IamWebRequestContextResolver;
use sdkwork_web_axum::{with_web_request_context, WebFrameworkLayer};
use sdkwork_web_core::{
    DefaultRateLimitPolicyResolver, DomainContextInjector, WebRequestContext,
    WebRequestContextProfile,
};

use crate::http_route_manifest::internal_route_manifest;
use crate::paths;
use crate::ports::SandboxInternalRequestContext;

/// Routes that must answer before a principal exists.
#[must_use]
pub fn sandbox_public_path_prefixes() -> Vec<String> {
    vec![
        paths::LIVEZ.to_owned(),
        paths::READYZ.to_owned(),
        paths::HEALTHZ.to_owned(),
    ]
}

/// Projects the verified web context into the Sandbox internal request context.
#[derive(Clone, Default)]
pub struct SandboxInternalContextInjector;

impl DomainContextInjector for SandboxInternalContextInjector {
    fn inject(&self, request: &mut axum::extract::Request, context: &WebRequestContext) {
        if let Some(app_context) = sandbox_internal_context_from_web_request(context) {
            request.extensions_mut().insert(app_context);
        }
    }
}

fn sandbox_internal_context_from_web_request(
    context: &WebRequestContext,
) -> Option<SandboxInternalRequestContext> {
    let principal = context.principal.as_ref()?;
    Some(SandboxInternalRequestContext {
        tenant_id: principal.tenant_id().to_owned(),
        actor_id: Some(principal.user_id().to_owned()),
        organization_id: principal.organization_id().map(str::to_owned),
        session_id: principal.session_id().map(str::to_owned),
    })
}

/// Applies the web framework layer to the Sandbox internal-api router.
///
/// This is the only router constructor this crate exports with its business
/// routes: an un-wrapped mount would have no ingress-token gate, so no bare
/// `Router` is exposed (`INTERNAL_API_SPEC.md` section 4).
///
/// # Panics
///
/// Panics when a declared public prefix covers a protected manifest route. That
/// is an authored source defect, not a runtime condition: the manifest and the
/// prefix list live in this crate and are checked at startup rather than
/// silently allowing an unauthenticated path through.
#[must_use]
pub fn wrap_router_with_web_framework(
    resolver: IamWebRequestContextResolver,
    router: Router,
) -> Router {
    let route_manifest = internal_route_manifest();
    route_manifest
        .validate_public_path_prefixes(&sandbox_public_path_prefixes())
        .expect("sandbox internal-api public prefixes must not cover protected manifest routes");

    let layer = WebFrameworkLayer::new(resolver)
        .with_profile(WebRequestContextProfile {
            public_path_prefixes: sandbox_public_path_prefixes(),
            ..WebRequestContextProfile::default()
        })
        .with_route_manifest(route_manifest)
        .with_domain_injector(Arc::new(SandboxInternalContextInjector))
        .with_rate_limit_resolver(Arc::new(DefaultRateLimitPolicyResolver));
    with_web_request_context(router, layer)
}

/// Resolves the IAM context resolver from the environment, then wraps.
pub async fn wrap_router_with_web_framework_from_env(router: Router) -> Router {
    let resolver = sdkwork_iam_web_adapter::iam_web_request_context_resolver_from_env().await;
    wrap_router_with_web_framework(resolver, router)
}
