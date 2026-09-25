//! Standalone gateway host for the sdkwork-sandbox API assembly.
//!
//! `standalone` is the only deployment profile this repository supports
//! (`SDKWORK_WEBSERVER_SPEC.md` section 17.4): there is no cloud build, no
//! cloud package, and no cloud runtime-env surface here. What
//! `sdkwork-webserver` composes in-process under the public edge, this binary
//! serves directly, so a Sandbox API can be exercised without an edge in front
//! of it.
//!
//! The composition is deliberately identical to the edge's: the same
//! `ApiAssemblyContribution`, the same IAM request-context resolver, and the
//! same public-path rules. A second composition path would be a second place
//! for the auth shape and the route manifest to drift.

use axum::Router;
use tracing_subscriber::EnvFilter;

/// Serves `router` on `listen_addr` until SIGINT/SIGTERM.
///
/// # Panics
///
/// Panics when the address cannot be bound or the server fails, because a
/// gateway that cannot listen has no useful degraded mode to fall back to.
pub async fn serve_router(listen_addr: &str, service_name: &str, router: Router) {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let listener = tokio::net::TcpListener::bind(listen_addr)
        .await
        .expect("bind sandbox gateway listener");
    tracing::info!(%listen_addr, service = service_name, "listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve sandbox gateway");
}

/// Resolves on Ctrl+C on every platform and additionally on `SIGTERM` on Unix,
/// so the container stop signal drains in-flight requests instead of cutting
/// them off.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
}
