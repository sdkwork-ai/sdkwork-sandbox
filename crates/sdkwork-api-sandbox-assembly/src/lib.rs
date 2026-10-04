//! API assembly for sdkwork-sandbox.
//! Application bootstrap lives in `bootstrap.rs`; route inventory is in `assembly-manifest.json`.
//! SDKWORK-ASSEMBLY-LIB-CUSTOM: exports beyond the canonical materializer template.

mod bootstrap;
mod context;
mod generated;
mod readiness;
mod route_manifest;

pub use bootstrap::{
    assemble_api_router, assemble_api_router_with_drain, assemble_api_router_with_pool,
    assemble_api_router_with_pool_and_drain, bootstrap_database_from_env, web_module,
    web_module_with_pool, ApiAssembly, SandboxAssemblyError,
};
pub use context::SandboxDomainContextInjector;
pub use readiness::SandboxDrainGate;
pub use route_manifest::sandbox_api_route_manifest;

pub fn assembly_route_count() -> usize {
    generated::ROUTE_CRATE_COUNT
}
