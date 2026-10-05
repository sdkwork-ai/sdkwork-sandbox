#![forbid(unsafe_code)]
//! Provider-neutral authority model for SDKWork Sandbox Templates
//! (`REQ-2026-0029`, `ADR-20261004`).
//!
//! The crate carries the authority-model slice REVIEW-20261004 authorized:
//! the [`definition::SandboxTemplateDefinition`], [`version::
//! SandboxTemplateVersion`] and [`build_input::SandboxTemplateBuildInput`]
//! record shapes with fail-closed validation, and the [`cache`] semantics
//! authority (Hot/Warm/Cold, explicit eviction, exact-digest reuse). Records
//! are immutable after publication by construction — fields are private with
//! read-only accessors, so every change is a new version.
//!
//! What this crate deliberately does not own: any Builder runtime, Registry
//! service, build pipeline, build-artifact or cache storage backend, CLI,
//! public API/SDK or deployment profile — the contract's `forbidden` block
//! keeps every one of those surfaces closed until its own requirement slice
//! lands, and no E2B Template capability-parity claim may be made before the
//! Builder slice exists. Docker and build scripts are build-input formats
//! only; Docker is never the runtime dependency or isolation boundary.

mod authority;
mod bounds;
mod build_input;
mod cache;
mod definition;
mod error;
mod identity;
mod registry;
mod version;

pub use authority::{
    sandbox_artifact_boundary_holds, sandbox_build_pipeline_or_artifact_storage_authorized,
    sandbox_second_supply_chain_authority_allowed, sandbox_template_builder_runtime_authorized,
    sandbox_template_cli_surface_authorized, sandbox_template_deployment_profile_authorized,
    sandbox_template_owns_evidence_or_signature, sandbox_template_public_api_sdk_authorized,
    sandbox_template_references_artifact_tuple, sandbox_template_registry_service_authorized,
    SANDBOX_TEMPLATE_ARTIFACT_AUTHORITY,
};
pub use bounds::{
    MAX_SANDBOX_TEMPLATE_ENV_COUNT, MAX_SANDBOX_TEMPLATE_ENV_KEY_LENGTH,
    MAX_SANDBOX_TEMPLATE_ENV_VALUE_LENGTH, MAX_SANDBOX_TEMPLATE_EVICTION_POLICY_LENGTH,
    MAX_SANDBOX_TEMPLATE_FILE_LAYERS, MAX_SANDBOX_TEMPLATE_ID_LENGTH,
    MAX_SANDBOX_TEMPLATE_NAME_COUNT, MAX_SANDBOX_TEMPLATE_NAME_LENGTH,
    MAX_SANDBOX_TEMPLATE_START_COMMAND_LENGTH, MAX_SANDBOX_TEMPLATE_VERSION_LENGTH,
};
pub use build_input::{
    SandboxTemplateBuildInput, SandboxTemplateBuildInputFormat,
    SANDBOX_TEMPLATE_DOCKER_RUNTIME_BOUNDARY_ALLOWED,
};
pub use cache::{SandboxTemplateCacheLayer, SandboxTemplateCachePolicy};
pub use definition::{SandboxTemplateDefinition, SandboxTemplateFileLayer};
pub use error::{SandboxTemplateAuthorityError, SandboxTemplateAuthorityResult};
pub use identity::{
    SandboxTemplateDefinitionId, SandboxTemplateName, SandboxTemplateOpaqueRef,
    SandboxTemplateVersionId,
};
pub use registry::{BoundedSandboxTemplateRegistry, SANDBOX_TEMPLATE_REGISTRY_CAPACITY_MAX};
pub use version::SandboxTemplateVersion;

#[cfg(test)]
mod tests;
