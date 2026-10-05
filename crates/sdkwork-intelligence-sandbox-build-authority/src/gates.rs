//! Evidence gates, the artifact boundary and the line layering
//! (`contract`: `evidenceGates`, `artifactBoundary`, `layering`).
//!
//! The gates are fail-closed documentation: they state, as constants, what
//! must be true before any builder runtime, pipeline, registry or storage
//! surface is authorized. The artifact boundary pins `REQ-2026-0012` as the
//! single supply-chain authority.

/// The single supply-chain authority every successful build references.
pub const SANDBOX_TEMPLATE_BUILD_ARTIFACT_AUTHORITY: &str = "REQ-2026-0012";

/// The Template authority this line consumes definitions, versions, build
/// inputs and cache semantics from (`layering.templateAuthority`).
pub const SANDBOX_TEMPLATE_BUILD_TEMPLATE_AUTHORITY: &str = "REQ-2026-0029";

/// The Pool evidence gate any warm-slot consumption of a build artifact
/// additionally requires (`layering.warmMicroVmSlotGate`).
pub const SANDBOX_TEMPLATE_BUILD_WARM_SLOT_GATE: &str = "REQ-2026-0019";

/// Real builder execution evidence is release-blocking.
pub const SANDBOX_TEMPLATE_BUILD_REAL_BUILDER_EXECUTION_EVIDENCE_REQUIRED: bool = true;

/// Build artifact tuple evidence is release-blocking.
pub const SANDBOX_TEMPLATE_BUILD_ARTIFACT_TUPLE_EVIDENCE_REQUIRED: bool = true;

/// Whether registering this authority enables the Pool warm slot. It does
/// not.
pub const SANDBOX_TEMPLATE_BUILD_REGISTRATION_ENABLES_WARM_SLOT: bool = false;

/// Whether this registration binds any real artifact today. It does not:
/// no released `REQ-2026-0012` tuple exists yet.
pub const SANDBOX_TEMPLATE_BUILD_REGISTRATION_BINDS_NO_REAL_ARTIFACT: bool = true;

/// Whether the authority-model slice is authorized. It is (REVIEW-20261005,
/// single-owner structured approval); the runtime surfaces below stay shut.
#[must_use]
pub const fn sandbox_template_build_authority_model_slice_authorized() -> bool {
    true
}

/// Whether a builder runtime or pipeline execution is authorized. It is not.
#[must_use]
pub const fn sandbox_template_builder_runtime_or_pipeline_authorized() -> bool {
    false
}

/// Whether a build registry service is authorized. It is not.
#[must_use]
pub const fn sandbox_template_build_registry_service_authorized() -> bool {
    false
}

/// Whether builds reference (rather than embed) artifact tuples. They do.
#[must_use]
pub const fn sandbox_template_build_references_artifact_tuple() -> bool {
    true
}

/// Whether the build authority owns artifact evidence. It does not.
#[must_use]
pub const fn sandbox_template_build_owns_evidence_or_signature() -> bool {
    false
}

/// Whether a second supply-chain authority may exist next to
/// `REQ-2026-0012`. It may not.
#[must_use]
pub const fn sandbox_template_build_second_supply_chain_authority_allowed() -> bool {
    false
}
