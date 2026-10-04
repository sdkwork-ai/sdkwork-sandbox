//! Authority-model slice tests: the record shapes, the build-input boundary,
//! the cache-semantics authority and the artifact layering of
//! `specs/sandbox-template-authority.contract.json`.

use std::collections::{BTreeMap, BTreeSet};

use crate::build_input::{
    SandboxTemplateBuildInput, SandboxTemplateBuildInputFormat,
    SANDBOX_TEMPLATE_DOCKER_RUNTIME_BOUNDARY_ALLOWED,
};
use crate::cache::{SandboxTemplateCacheLayer, SandboxTemplateCachePolicy};
use crate::definition::{SandboxTemplateDefinition, SandboxTemplateFileLayer};
use crate::error::SandboxTemplateAuthorityError;
use crate::identity::{
    SandboxTemplateDefinitionId, SandboxTemplateName, SandboxTemplateOpaqueRef,
    SandboxTemplateVersionId,
};
use crate::version::SandboxTemplateVersion;

fn sandbox_layer(name: &str) -> SandboxTemplateFileLayer {
    SandboxTemplateFileLayer::sandbox_new(
        name,
        SandboxTemplateOpaqueRef::new(format!("content-{name}")).expect("valid content ref"),
    )
    .expect("valid layer")
}

fn sandbox_definition() -> SandboxTemplateDefinition {
    let mut sandbox_set_envs = BTreeMap::new();
    sandbox_set_envs.insert("SANDBOX_EXAMPLE".to_owned(), "1".to_owned());
    SandboxTemplateDefinition::sandbox_publish(
        SandboxTemplateDefinitionId::new("template-def-1").expect("valid id"),
        "v1",
        SandboxTemplateOpaqueRef::new("base-env-ubuntu-2404").expect("valid base ref"),
        vec![sandbox_layer("app")],
        sandbox_set_envs,
        "python -m http.server 3000",
        1_000,
    )
    .expect("valid definition")
}

#[test]
fn a_published_definition_is_complete_and_read_only() {
    let definition = sandbox_definition();
    assert_eq!(
        definition.sandbox_template_definition_id().as_str(),
        "template-def-1",
    );
    assert_eq!(definition.sandbox_template_definition_version(), "v1");
    assert_eq!(
        definition.sandbox_base_environment_ref().as_str(),
        "base-env-ubuntu-2404"
    );
    assert_eq!(definition.sandbox_file_layers().len(), 1);
    assert_eq!(
        definition.sandbox_file_layers()[0].sandbox_layer_name(),
        "app"
    );
    assert_eq!(
        definition
            .sandbox_set_envs()
            .get("SANDBOX_EXAMPLE")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        definition.sandbox_set_start_cmd(),
        "python -m http.server 3000"
    );
    assert_eq!(definition.sandbox_published_at(), 1_000);
}

#[test]
fn definition_validation_is_fail_closed_field_by_field() {
    let mut sandbox_set_envs = BTreeMap::new();
    sandbox_set_envs.insert("SANDBOX_EXAMPLE".to_owned(), "1".to_owned());

    let oversized_version = SandboxTemplateDefinition::sandbox_publish(
        SandboxTemplateDefinitionId::new("d").expect("valid id"),
        "x".repeat(crate::bounds::MAX_SANDBOX_TEMPLATE_VERSION_LENGTH + 1),
        SandboxTemplateOpaqueRef::new("base").expect("valid base"),
        vec![],
        sandbox_set_envs.clone(),
        "start",
        1,
    );
    assert!(matches!(
        oversized_version,
        Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition),
    ));

    let empty_start = SandboxTemplateDefinition::sandbox_publish(
        SandboxTemplateDefinitionId::new("d").expect("valid id"),
        "v1",
        SandboxTemplateOpaqueRef::new("base").expect("valid base"),
        vec![],
        sandbox_set_envs.clone(),
        "",
        1,
    );
    assert!(matches!(
        empty_start,
        Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition)
    ));

    let oversized_env_value = SandboxTemplateDefinition::sandbox_publish(
        SandboxTemplateDefinitionId::new("d").expect("valid id"),
        "v1",
        SandboxTemplateOpaqueRef::new("base").expect("valid base"),
        vec![],
        BTreeMap::from([(
            "SANDBOX_BIG".to_owned(),
            "x".repeat(crate::bounds::MAX_SANDBOX_TEMPLATE_ENV_VALUE_LENGTH + 1),
        )]),
        "start",
        1,
    );
    assert!(matches!(
        oversized_env_value,
        Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition),
    ));

    let too_many_layers = (0..=crate::bounds::MAX_SANDBOX_TEMPLATE_FILE_LAYERS)
        .map(|index| sandbox_layer(&format!("layer-{index}")))
        .collect();
    let layered = SandboxTemplateDefinition::sandbox_publish(
        SandboxTemplateDefinitionId::new("d").expect("valid id"),
        "v1",
        SandboxTemplateOpaqueRef::new("base").expect("valid base"),
        too_many_layers,
        sandbox_set_envs,
        "start",
        1,
    );
    assert!(matches!(
        layered,
        Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition)
    ));

    let path_like_layer = SandboxTemplateFileLayer::sandbox_new(
        "../escape",
        SandboxTemplateOpaqueRef::new("content").expect("valid content"),
    );
    assert!(matches!(
        path_like_layer,
        Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition),
    ));
}

#[test]
fn a_version_binds_tags_aliases_and_one_artifact_tuple() {
    let version = SandboxTemplateVersion::sandbox_publish(
        SandboxTemplateVersionId::new("template-ver-1").expect("valid id"),
        SandboxTemplateDefinitionId::new("template-def-1").expect("valid id"),
        BTreeSet::from([
            SandboxTemplateName::new("stable").expect("valid tag"),
            SandboxTemplateName::new("2026.10").expect("valid tag"),
        ]),
        BTreeSet::from([SandboxTemplateName::new("latest-stable").expect("valid alias")]),
        SandboxTemplateOpaqueRef::new("artifact-tuple-r12").expect("valid tuple ref"),
    )
    .expect("valid version");
    assert_eq!(version.sandbox_tags().len(), 2);
    assert_eq!(version.sandbox_aliases().len(), 1);
    assert_eq!(
        version.sandbox_artifact_tuple_ref().as_str(),
        "artifact-tuple-r12"
    );
    assert!(crate::sandbox_artifact_boundary_holds(&version));

    let oversized = SandboxTemplateVersion::sandbox_publish(
        SandboxTemplateVersionId::new("v").expect("valid id"),
        SandboxTemplateDefinitionId::new("d").expect("valid id"),
        (0..=crate::bounds::MAX_SANDBOX_TEMPLATE_NAME_COUNT)
            .map(|index| SandboxTemplateName::new(format!("tag-{index}")).expect("valid tag"))
            .collect(),
        BTreeSet::new(),
        SandboxTemplateOpaqueRef::new("tuple").expect("valid tuple"),
    );
    assert!(matches!(
        oversized,
        Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidVersion),
    ));
}

#[test]
fn build_inputs_stay_build_inputs_and_reject_locators_and_key_material() {
    for format in [
        SandboxTemplateBuildInputFormat::DockerfileRef,
        SandboxTemplateBuildInputFormat::BuildScriptRef,
    ] {
        assert!(
            SandboxTemplateBuildInput::sandbox_new(
                format,
                SandboxTemplateOpaqueRef::new("build/ref-1").expect("valid ref"),
            )
            .is_ok(),
            "{} accepts a plain opaque reference",
            format.as_str(),
        );
    }
    assert_eq!(
        SandboxTemplateBuildInputFormat::parse("sandbox_dockerfile_ref"),
        Some(SandboxTemplateBuildInputFormat::DockerfileRef),
    );
    assert!(SandboxTemplateBuildInputFormat::parse("docker").is_none());

    for forbidden in [
        "/etc/passwd",
        "https://example.com/base.dockerfile",
        "-----BEGIN PRIVATE KEY-----",
        "../escape",
    ] {
        let reference = SandboxTemplateOpaqueRef::new(forbidden);
        let rejected = match reference {
            Ok(reference) => SandboxTemplateBuildInput::sandbox_new(
                SandboxTemplateBuildInputFormat::DockerfileRef,
                reference,
            )
            .is_err(),
            Err(_) => true,
        };
        assert!(
            rejected,
            "a locator or key-material build input must be rejected: {forbidden}"
        );
    }
    const {
        assert!(!SANDBOX_TEMPLATE_DOCKER_RUNTIME_BOUNDARY_ALLOWED);
    }
}

#[test]
fn cache_semantics_are_the_three_layer_authority() {
    assert_eq!(
        SandboxTemplateCachePolicy::sandbox_layers()
            .map(|layer| layer.as_str())
            .to_vec(),
        vec!["sandbox_hot", "sandbox_warm", "sandbox_cold"],
    );
    assert!(SandboxTemplateCacheLayer::parse("sandbox_warm").is_some());
    assert!(SandboxTemplateCacheLayer::parse("warm").is_none());

    let policy = SandboxTemplateCachePolicy::sandbox_new("ttl 7d, capacity 10GiB per node")
        .expect("explicit eviction policy");
    assert_eq!(
        policy.sandbox_eviction_policy(),
        "ttl 7d, capacity 10GiB per node"
    );
    assert!(policy.sandbox_cross_template_reuse_requires_exact_digest());
    assert!(!SandboxTemplateCachePolicy::sandbox_cache_implementation_authorized());
    assert!(!SandboxTemplateCachePolicy::sandbox_cache_storage_backend_in_scope());
    assert!(matches!(
        SandboxTemplateCachePolicy::sandbox_new(""),
        Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidCachePolicy),
    ));
}

#[test]
fn the_artifact_boundary_and_forbidden_surfaces_hold() {
    assert_eq!(crate::SANDBOX_TEMPLATE_ARTIFACT_AUTHORITY, "REQ-2026-0012");
    assert!(crate::sandbox_template_references_artifact_tuple());
    assert!(!crate::sandbox_template_owns_evidence_or_signature());
    assert!(!crate::sandbox_second_supply_chain_authority_allowed());
    assert!(!crate::sandbox_template_builder_runtime_authorized());
    assert!(!crate::sandbox_template_registry_service_authorized());
    assert!(!crate::sandbox_build_pipeline_or_artifact_storage_authorized());
    assert!(!crate::sandbox_template_cli_surface_authorized());
    assert!(!crate::sandbox_template_public_api_sdk_authorized());
    assert!(!crate::sandbox_template_deployment_profile_authorized());
}
