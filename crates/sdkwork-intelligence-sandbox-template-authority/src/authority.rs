//! The artifact boundary and forbidden-surface constants.
//!
//! The Template authority references one exact `REQ-2026-0012` artifact tuple
//! per version and owns none of its evidence (`artifactBoundary`); no second
//! supply-chain authority may appear. The forbidden block
//! (`forbidden`) stays false-valued until each surface's own requirement
//! slice lands.

use crate::version::SandboxTemplateVersion;

/// The single supply-chain authority every version references.
pub const SANDBOX_TEMPLATE_ARTIFACT_AUTHORITY: &str = "REQ-2026-0012";

/// Versions reference artifact tuples; they do not embed them.
#[must_use]
pub const fn sandbox_template_references_artifact_tuple() -> bool {
    true
}

/// The authority owns no artifact evidence or signature material.
#[must_use]
pub const fn sandbox_template_owns_evidence_or_signature() -> bool {
    false
}

/// A second supply-chain authority is not allowed next to `REQ-2026-0012`.
#[must_use]
pub const fn sandbox_second_supply_chain_authority_allowed() -> bool {
    false
}

/// Builder runtime stays unauthorized in this requirement slice.
#[must_use]
pub const fn sandbox_template_builder_runtime_authorized() -> bool {
    false
}

/// A registry service stays unauthorized in this requirement slice.
#[must_use]
pub const fn sandbox_template_registry_service_authorized() -> bool {
    false
}

/// Build pipelines and build-artifact storage stay unauthorized in this
/// requirement slice.
#[must_use]
pub const fn sandbox_build_pipeline_or_artifact_storage_authorized() -> bool {
    false
}

/// A CLI surface (`template init/build/deploy`) stays unauthorized in this
/// requirement slice.
#[must_use]
pub const fn sandbox_template_cli_surface_authorized() -> bool {
    false
}

/// Public API/SDK surfaces stay unauthorized in this requirement slice.
#[must_use]
pub const fn sandbox_template_public_api_sdk_authorized() -> bool {
    false
}

/// Deployment profiles stay unauthorized in this requirement slice.
#[must_use]
pub const fn sandbox_template_deployment_profile_authorized() -> bool {
    false
}

/// Checks that a version's artifact-tuple reference is present and that the
/// boundary constants hold. The record shape makes "no evidence material" true
/// by construction; this check exists so a caller can assert the whole
/// boundary at the seam.
#[must_use]
pub fn sandbox_artifact_boundary_holds(version: &SandboxTemplateVersion) -> bool {
    !version.sandbox_artifact_tuple_ref().as_str().is_empty()
        && sandbox_template_references_artifact_tuple()
        && !sandbox_template_owns_evidence_or_signature()
        && !sandbox_second_supply_chain_authority_allowed()
        && SANDBOX_TEMPLATE_ARTIFACT_AUTHORITY == "REQ-2026-0012"
}
