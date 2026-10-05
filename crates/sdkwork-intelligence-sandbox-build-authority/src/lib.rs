#![forbid(unsafe_code)]
//! Provider-neutral authority model for SDKWork Sandbox Template Builds
//! (`REQ-2026-0032`, `ADR-20261005`).
//!
//! The crate carries the authority-model slice REVIEW-20261005 authorized:
//! the [`build::SandboxTemplateBuild`] record with the closed lifecycle
//! [`state`] machine (terminal immutability, uncertainty quarantines, a
//! successful build bound to exactly one `REQ-2026-0012` artifact tuple and
//! a failed build bound to none), and the [`gates`] evidence, artifact and
//! layering constants. Records are private-fielded with read-only accessors,
//! so every lifecycle move is a state-machine transition.
//!
//! What this crate deliberately does not own: any builder runtime, pipeline
//! execution, artifact or cache storage backend, registry service, CLI,
//! public API/SDK or deployment profile — the contract's `forbidden` block
//! keeps every one of those surfaces closed until its own requirement slice
//! lands, no E2B Template Build capability-parity claim may be made before a
//! runtime slice exists, and Pool warm-slot consumption additionally
//! requires the `REQ-2026-0019` pool evidence gate. Template definitions,
//! versions, build inputs and cache semantics stay owned by
//! `REQ-2026-0029`; this line consumes them by opaque reference.

mod bounds;
mod build;
mod error;
mod gates;
mod state;

pub use bounds::MAX_SANDBOX_TEMPLATE_BUILD_ID_LENGTH;
pub use build::SandboxTemplateBuild;
pub use error::{SandboxTemplateBuildAuthorityError, SandboxTemplateBuildAuthorityResult};
pub use gates::{
    sandbox_template_build_authority_model_slice_authorized,
    sandbox_template_build_owns_evidence_or_signature,
    sandbox_template_build_references_artifact_tuple,
    sandbox_template_build_registry_service_authorized,
    sandbox_template_build_second_supply_chain_authority_allowed,
    sandbox_template_builder_runtime_or_pipeline_authorized,
    SANDBOX_TEMPLATE_BUILD_ARTIFACT_AUTHORITY,
    SANDBOX_TEMPLATE_BUILD_ARTIFACT_TUPLE_EVIDENCE_REQUIRED,
    SANDBOX_TEMPLATE_BUILD_REAL_BUILDER_EXECUTION_EVIDENCE_REQUIRED,
    SANDBOX_TEMPLATE_BUILD_REGISTRATION_BINDS_NO_REAL_ARTIFACT,
    SANDBOX_TEMPLATE_BUILD_REGISTRATION_ENABLES_WARM_SLOT,
    SANDBOX_TEMPLATE_BUILD_TEMPLATE_AUTHORITY, SANDBOX_TEMPLATE_BUILD_WARM_SLOT_GATE,
};
pub use state::SandboxTemplateBuildState;

#[cfg(test)]
mod tests;
