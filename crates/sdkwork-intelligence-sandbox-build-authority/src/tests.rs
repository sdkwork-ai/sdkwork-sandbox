//! Authority-model slice tests: the record validation, the closed lifecycle
//! with terminal immutability, the artifact-tuple binding rules and the
//! evidence/layering gates of `specs/sandbox-template-build.contract.json`.

use crate::build::SandboxTemplateBuild;
use crate::error::SandboxTemplateBuildAuthorityError;
use crate::gates::{
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
use crate::state::SandboxTemplateBuildState;

const TUPLE: &str =
    "req-0012-rootfs-sha256-0000000000000000000000000000000000000000000000000000000000000000";

fn sandbox_build() -> SandboxTemplateBuild {
    SandboxTemplateBuild::sandbox_new(
        "build-1",
        "definition-1",
        "version-1",
        "build-input-1",
        1_000,
    )
    .expect("valid build")
}

#[test]
fn records_reject_malformed_references_and_path_or_url_shapes() {
    for malformed in [
        ("", "definition-1", "version-1", "build-input-1"),
        ("build-1", "/absolute", "version-1", "build-input-1"),
        ("build-1", "definition-1", "https://x", "build-input-1"),
        ("build-1", "definition-1", "version-1", "has space"),
    ] {
        assert!(matches!(
            SandboxTemplateBuild::sandbox_new(
                malformed.0,
                malformed.1,
                malformed.2,
                malformed.3,
                1_000
            ),
            Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildInvalidBuild),
        ));
    }
    assert_eq!(
        sandbox_build().sandbox_template_build_state(),
        SandboxTemplateBuildState::Requested
    );
}

#[test]
fn the_lifecycle_walks_requested_building_succeeded_with_one_bound_tuple() {
    let mut build = sandbox_build();
    build.sandbox_record_building(1_100).expect("building");
    assert_eq!(
        build.sandbox_template_build_state(),
        SandboxTemplateBuildState::Building
    );
    build
        .sandbox_record_outcome(true, Some(TUPLE), 1_200)
        .expect("success binds its tuple");
    assert_eq!(
        build.sandbox_template_build_state(),
        SandboxTemplateBuildState::Succeeded
    );
    assert_eq!(build.sandbox_artifact_tuple_ref(), Some(TUPLE));
    assert_eq!(build.sandbox_requested_at(), 1_000);
    assert_eq!(build.sandbox_updated_at(), 1_200);
    // Terminal: no further transition may rewrite the record.
    assert!(build.sandbox_record_building(1_300).is_err());
    assert!(build.sandbox_quarantine(1_300).is_err());
}

#[test]
fn success_without_a_tuple_and_failure_with_one_are_both_refused() {
    let mut unbound = sandbox_build();
    unbound.sandbox_record_building(1_100).expect("building");
    assert!(matches!(
        unbound.sandbox_record_outcome(true, None, 1_200),
        Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildInvalidBuild),
    ));
    // Nothing was recorded; the deterministic failure path still works.
    unbound
        .sandbox_record_outcome(false, None, 1_200)
        .expect("failure binds no tuple");
    assert_eq!(
        unbound.sandbox_template_build_state(),
        SandboxTemplateBuildState::Failed
    );
    assert_eq!(unbound.sandbox_artifact_tuple_ref(), None);

    let mut lying = sandbox_build();
    lying.sandbox_record_building(1_100).expect("building");
    assert!(matches!(
        lying.sandbox_record_outcome(false, Some(TUPLE), 1_200),
        Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildInvalidBuild),
    ));
}

#[test]
fn uncertain_outcomes_quarantine_and_bind_no_artifact() {
    let mut requested = sandbox_build();
    requested
        .sandbox_quarantine(1_100)
        .expect("quarantine from requested");
    assert_eq!(
        requested.sandbox_template_build_state(),
        SandboxTemplateBuildState::Quarantined
    );
    assert_eq!(requested.sandbox_artifact_tuple_ref(), None);
    assert!(requested.sandbox_record_building(1_200).is_err());

    let mut building = sandbox_build();
    building.sandbox_record_building(1_100).expect("building");
    building
        .sandbox_quarantine(1_200)
        .expect("quarantine from building");
    assert_eq!(
        building.sandbox_template_build_state(),
        SandboxTemplateBuildState::Quarantined
    );
}

#[test]
fn timestamps_may_never_move_backwards() {
    let mut build = sandbox_build();
    build.sandbox_record_building(1_100).expect("building");
    assert!(matches!(
        build.sandbox_record_outcome(true, Some(TUPLE), 1_050),
        Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildInvalidBuild),
    ));
    assert_eq!(
        build.sandbox_template_build_state(),
        SandboxTemplateBuildState::Building
    );
}

#[test]
fn the_evidence_and_layering_gates_match_the_contract() {
    assert_eq!(SANDBOX_TEMPLATE_BUILD_ARTIFACT_AUTHORITY, "REQ-2026-0012");
    assert_eq!(SANDBOX_TEMPLATE_BUILD_TEMPLATE_AUTHORITY, "REQ-2026-0029");
    assert_eq!(SANDBOX_TEMPLATE_BUILD_WARM_SLOT_GATE, "REQ-2026-0019");
    // The gate constants hold at compile time, like the snapshot line's.
    const {
        assert!(SANDBOX_TEMPLATE_BUILD_REAL_BUILDER_EXECUTION_EVIDENCE_REQUIRED);
        assert!(SANDBOX_TEMPLATE_BUILD_ARTIFACT_TUPLE_EVIDENCE_REQUIRED);
        assert!(!SANDBOX_TEMPLATE_BUILD_REGISTRATION_ENABLES_WARM_SLOT);
        assert!(SANDBOX_TEMPLATE_BUILD_REGISTRATION_BINDS_NO_REAL_ARTIFACT);
    }
    assert!(sandbox_template_build_authority_model_slice_authorized());
    assert!(!sandbox_template_builder_runtime_or_pipeline_authorized());
    assert!(!sandbox_template_build_registry_service_authorized());
    assert!(sandbox_template_build_references_artifact_tuple());
    assert!(!sandbox_template_build_owns_evidence_or_signature());
    assert!(!sandbox_template_build_second_supply_chain_authority_allowed());
}
