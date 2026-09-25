//! Health and readiness probes for a standalone Sandbox mount.
//!
//! When the Sandbox internal-api is composed same-origin by the Web Server edge
//! the edge owns the probes and this module is unused; a standalone mount still
//! has to answer them (`WEB_FRAMEWORK_SPEC.md` health contract). PostgreSQL
//! readiness uses `sdkwork_web_bootstrap::PgPoolReadinessCheck` at the
//! composition root, so this crate carries no database dependency.

use std::sync::Arc;

use axum::response::{IntoResponse, Response};
use sdkwork_web_bootstrap::ReadinessCheck;

/// Liveness: the process is up. Never consults a dependency.
pub async fn livez() -> impl IntoResponse {
    sdkwork_web_bootstrap::livez_handler().await
}

/// Readiness: every configured dependency answers.
pub async fn readyz(readiness: Option<Arc<dyn ReadinessCheck>>) -> Response {
    sdkwork_web_bootstrap::readyz_handler(readiness).await
}

/// Health alias for readiness; an unhealthy dependency is not healthy.
pub async fn healthz(readiness: Option<Arc<dyn ReadinessCheck>>) -> Response {
    sdkwork_web_bootstrap::readyz_handler(readiness).await
}
