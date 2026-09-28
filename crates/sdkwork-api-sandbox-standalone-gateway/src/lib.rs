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
use sdkwork_api_sandbox_assembly::SandboxDrainGate;
use tracing_subscriber::EnvFilter;

/// Serves `router` on `listen_addr` until SIGINT/SIGTERM.
///
/// The first signal starts the graceful drain and flips `sandbox_drain_gate`,
/// so `/readyz` fails immediately and load balancers stop routing new
/// connections while in-flight requests finish. A second signal force-exits a
/// stuck drain instead of hanging forever.
///
/// # Panics
///
/// Panics when the address cannot be bound or the server fails, because a
/// gateway that cannot listen has no useful degraded mode to fall back to.
pub async fn serve_router(
    listen_addr: &str,
    service_name: &str,
    router: Router,
    sandbox_drain_gate: &SandboxDrainGate,
) {
    let sandbox_drain_gate = sandbox_drain_gate.clone();
    // Structured observability (OBSERVABILITY_SPEC section 2): `RUST_LOG`
    // keeps precedence, an unset filter falls back to `info` so a default
    // deployment is not silent, and `SDKWORK_SANDBOX_LOG_FORMAT=json` emits
    // structured JSON events for a log pipeline.
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let format_json = std::env::var("SDKWORK_SANDBOX_LOG_FORMAT")
        .map(|value| value.eq_ignore_ascii_case("json"))
        .unwrap_or(false);
    if format_json {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }

    let listener = tokio::net::TcpListener::bind(listen_addr)
        .await
        .expect("bind sandbox gateway listener");
    tracing::info!(%listen_addr, service = service_name, "listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(sandbox_drain_gate))
        .await
        .expect("serve sandbox gateway");
}

/// Resolves on Ctrl+C on every platform and additionally on `SIGTERM` on Unix,
/// so the container stop signal drains in-flight requests instead of cutting
/// them off.
///
/// The first signal begins the drain (readiness fails; see
/// [`SandboxDrainGate`]) and arms a second-signal watcher: a second Ctrl+C or
/// `SIGTERM` force-exits a stuck drain with the conventional signal exit
/// status instead of hanging until an external timeout kills the process.
async fn shutdown_signal(sandbox_drain_gate: SandboxDrainGate) {
    wait_for_shutdown_signal().await;
    sandbox_drain_gate.begin_drain();
    tracing::info!(
        "graceful drain started; readiness now fails so load balancers stop routing \
         (a second signal force-exits a stuck drain)"
    );
    // The watcher captures nothing, so it outlives this future and stays
    // armed for the whole drain.
    tokio::spawn(async {
        wait_for_shutdown_signal().await;
        tracing::warn!("second shutdown signal received; force exit");
        // 130/143 are the conventional `128 + signal` statuses for SIGINT and
        // SIGTERM, so supervisors see an intentional termination, not a crash.
        std::process::exit(if cfg!(unix) { 143 } else { 130 });
    });
}

async fn wait_for_shutdown_signal() {
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
