//! Standalone entry point: assemble the Sandbox API contribution and serve it.
//!
//! `SDKWORK_SANDBOX_APPLICATION_PUBLIC_INGRESS_BIND` carries the bind address
//! that `specs/topology.spec.json` declares for the
//! `application.public-ingress` surface, so the same profile file drives both
//! the standalone gateway and the browser proxy in front of it.

use sdkwork_api_sandbox_standalone_gateway::serve_router;
use sdkwork_iam_web_adapter::{
    build_web_framework_builder, iam_web_request_context_resolver_from_env,
};
use sdkwork_web_bootstrap::{infra_public_path_prefixes, ApiModuleRegistry};

#[tokio::main]
async fn main() {
    let listen_addr = std::env::var("SDKWORK_SANDBOX_APPLICATION_PUBLIC_INGRESS_BIND")
        .unwrap_or_else(|_| "127.0.0.1:18093".to_string());

    let assembly = sdkwork_api_sandbox_assembly::assemble_api_router()
        .await
        .expect("assemble sdkwork-sandbox gateway router");
    let framework = build_web_framework_builder(
        iam_web_request_context_resolver_from_env().await,
        assembly.route_manifest.clone(),
        infra_public_path_prefixes(),
    );
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
    )
    .await;
}
