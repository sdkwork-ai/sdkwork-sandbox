//! Standalone entry point: assemble the Sandbox API contribution and serve it.
//!
//! `SDKWORK_SANDBOX_APPLICATION_PUBLIC_INGRESS_BIND` carries the bind address
//! that `specs/topology.spec.json` declares for the
//! `application.public-ingress` surface, so the same profile file drives both
//! the standalone gateway and the browser proxy in front of it.
//!
//! The framework posture resolves from `SDKWORK_ENVIRONMENT` (the resolver
//! helper reads that name), while this repository's canonical variable is
//! `SDKWORK_SANDBOX_ENVIRONMENT` (`etc/topology/*.env`), so `main` bridges the
//! module variable into the framework one when the operator has not set the
//! platform name explicitly. Production posture is deliberately refused here
//! with an operator-directed message: this binary is the development exercise
//! harness — it wires no audit emitter, and the standalone-only deployment
//! profile serves production through the `sdkwork-webserver` edge, which owns
//! the audit pipeline.

use sdkwork_api_sandbox_assembly::SandboxDrainGate;
use sdkwork_api_sandbox_standalone_gateway::serve_router;
use sdkwork_iam_web_adapter::{
    build_web_framework_builder, iam_web_request_context_resolver_from_env,
};
use sdkwork_web_bootstrap::{infra_public_path_prefixes, ApiModuleRegistry};

fn main() {
    // Bridge the module-canonical environment into the framework-resolved
    // variable before any framework construction reads it. Setting the
    // process environment here is single-threaded and precedes every reader.
    if std::env::var_os("SDKWORK_ENVIRONMENT").is_none() {
        if let Some(module_environment) = std::env::var_os("SDKWORK_SANDBOX_ENVIRONMENT") {
            std::env::set_var("SDKWORK_ENVIRONMENT", module_environment);
        }
    }
    let environment = std::env::var("SDKWORK_ENVIRONMENT")
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|_| "prod".to_owned());
    if matches!(environment.as_str(), "prod" | "production") {
        eprintln!(
            "refusing to start: this binary is the development exercise gateway and wires no \
             audit emitter; serve production through the sdkwork-webserver edge (set \
             SDKWORK_SANDBOX_ENVIRONMENT=development to exercise this binary)"
        );
        std::process::exit(78);
    }
    let exit_code = real_main();
    if let Some(code) = exit_code {
        std::process::exit(code);
    }
}

/// The tokio runtime body, split from `main` so the posture gate above runs
/// before any async work.
fn real_main() -> Option<i32> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    runtime.block_on(async_main())
}

async fn async_main() -> Option<i32> {
    let listen_addr = std::env::var("SDKWORK_SANDBOX_APPLICATION_PUBLIC_INGRESS_BIND")
        .unwrap_or_else(|_| "127.0.0.1:18093".to_string());

    let sandbox_drain_gate = SandboxDrainGate::new();
    let assembly =
        sdkwork_api_sandbox_assembly::assemble_api_router_with_drain(&sandbox_drain_gate)
            .await
            .expect("assemble sdkwork-sandbox gateway router");
    let framework = build_web_framework_builder(
        iam_web_request_context_resolver_from_env().await,
        assembly.route_manifest.clone(),
        infra_public_path_prefixes(),
    )
    .request_timeout(std::time::Duration::from_secs(30));
    let mut module_registry = ApiModuleRegistry::new();
    module_registry.add_modules(vec![assembly]);
    let router = module_registry
        .try_compose("SDKWork Sandbox API")
        .expect("compose sdkwork-sandbox API contribution")
        .into_hosted(framework)
        .router;
    serve_router(
        &listen_addr,
        "sdkwork-api-sandbox-standalone-gateway",
        router,
        &sandbox_drain_gate,
    )
    .await;
    None
}
