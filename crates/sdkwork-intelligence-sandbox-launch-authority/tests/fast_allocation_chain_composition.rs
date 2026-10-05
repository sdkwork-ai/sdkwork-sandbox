//! Cross-crate composition proof of the fast-allocation chain: the six
//! authority/control-plane types fit together end to end without adapters —
//! a published template version is built successfully against the exact
//! `REQ-2026-0012` artifact tuple the version runs on, a prepared pool slot
//! is readied and claimed under fencing, the launch plan binds that claim,
//! and the worker-side consumption closes the chain. This is the mechanical
//! "the capability can be implemented" proof behind the governance chain
//! (`REQ-2026-0029` -> `REQ-2026-0032` -> `REQ-2026-0019` -> `REQ-2026-0033`).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use sdkwork_intelligence_sandbox_build_authority::SandboxTemplateBuild;
use sdkwork_intelligence_sandbox_launch_authority::SandboxInstanceLaunchPlan;
use sdkwork_intelligence_sandbox_pool_control::{
    BoundedSandboxPoolControl, SandboxIsolationAssurance, SandboxPoolClaimRequest,
    SandboxPoolClass, SandboxPoolFencingToken, SandboxPoolPreparationRequest,
    SandboxPoolProviderKind, SandboxPoolReadyRequest, SandboxResourceProfileId,
    SandboxRuntimePoolError, SandboxRuntimePoolPort,
};
use sdkwork_intelligence_sandbox_template_authority::{
    SandboxTemplateDefinition, SandboxTemplateDefinitionId, SandboxTemplateFileLayer,
    SandboxTemplateName, SandboxTemplateOpaqueRef, SandboxTemplateVersion,
    SandboxTemplateVersionId,
};

const TUPLE: &str =
    "req-0012-rootfs-sha256-0000000000000000000000000000000000000000000000000000000000000000";

fn fingerprint(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
}

/// Publishes the template side of the chain: one immutable definition with a
/// start command and one immutable version bound to the artifact tuple.
fn publish_template_side() -> (SandboxTemplateVersion, SandboxTemplateVersionId) {
    let definition_id = SandboxTemplateDefinitionId::new("definition-1").expect("valid id");
    let definition = SandboxTemplateDefinition::sandbox_publish(
        definition_id.clone(),
        "1",
        SandboxTemplateOpaqueRef::new("base-env-1").expect("valid ref"),
        vec![SandboxTemplateFileLayer::sandbox_new(
            "layer-1",
            SandboxTemplateOpaqueRef::new("content-1").expect("valid ref"),
        )
        .expect("valid layer")],
        BTreeMap::from([("APP_ENV".to_string(), "on".to_string())]),
        "echo fast-start",
        900,
    )
    .expect("valid definition");
    assert_eq!(definition.sandbox_set_start_cmd(), "echo fast-start");

    let version_id = SandboxTemplateVersionId::new("version-1").expect("valid id");
    let version = SandboxTemplateVersion::sandbox_publish(
        version_id.clone(),
        definition.sandbox_template_definition_id().clone(),
        BTreeSet::from([SandboxTemplateName::new("latest").expect("valid name")]),
        BTreeSet::new(),
        SandboxTemplateOpaqueRef::new(TUPLE).expect("valid tuple ref"),
    )
    .expect("valid version");
    (version, version_id)
}

fn pool_request_ids() -> (
    sdkwork_intelligence_sandbox_pool_control::SandboxPoolSlotId,
    SandboxResourceProfileId,
    sdkwork_intelligence_sandbox_pool_control::SandboxPoolTenantId,
) {
    use sdkwork_intelligence_sandbox_pool_control::{SandboxPoolSlotId, SandboxPoolTenantId};
    (
        SandboxPoolSlotId::new("slot-1").expect("valid slot"),
        SandboxResourceProfileId::new("profile-1").expect("valid profile"),
        SandboxPoolTenantId::new("tenant-1").expect("valid tenant"),
    )
}

#[test]
fn the_fast_allocation_chain_composes_end_to_end() {
    // 1. The template side publishes; the version carries the exact tuple.
    let (version, version_id) = publish_template_side();
    assert_eq!(version.sandbox_artifact_tuple_ref().as_str(), TUPLE);

    // 2. The build runs against that version and succeeds binding exactly
    //    the tuple the version runs on.
    let mut build = SandboxTemplateBuild::sandbox_new(
        "build-1",
        version.sandbox_template_definition_id().as_str(),
        version_id.as_str(),
        "build-input-1",
        950,
    )
    .expect("valid build");
    build.sandbox_record_building(960).expect("building");
    build
        .sandbox_record_outcome(
            true,
            Some(version.sandbox_artifact_tuple_ref().as_str()),
            970,
        )
        .expect("success binds the version tuple");
    assert_eq!(build.sandbox_artifact_tuple_ref(), Some(TUPLE));

    // 3. The pool prepares a tenant-neutral slot, readies it on fresh
    //    evidence, and the fenced claim binds it to one tenant runtime.
    let (slot_id, profile_id, tenant_id) = pool_request_ids();
    let control = BoundedSandboxPoolControl::sandbox_with_clock(Arc::new(|| 1_000));
    let slot = control
        .sandbox_prepare_pool_slot(SandboxPoolPreparationRequest {
            sandbox_pool_slot_id: slot_id.clone(),
            sandbox_pool_class: SandboxPoolClass::PreparedSlot,
            sandbox_resource_profile_id: profile_id.clone(),
            sandbox_node_reference:
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new("node-1")
                    .expect("valid node"),
            sandbox_provider_id:
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new("provider-1")
                    .expect("valid provider"),
            sandbox_provider_kind: SandboxPoolProviderKind::new("firecracker").expect("valid kind"),
            sandbox_isolation_assurance: SandboxIsolationAssurance::MicroVm,
            sandbox_artifact_manifest_revision:
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new(TUPLE)
                    .expect("valid revision"),
            sandbox_capacity_revision: 1,
            sandbox_warm_kvm_evidence_ref: None,
        })
        .expect("slot prepared");
    assert_eq!(
        slot.sandbox_fencing_token,
        SandboxPoolFencingToken::sandbox_initial()
    );
    control
        .sandbox_mark_pool_slot_ready(SandboxPoolReadyRequest {
            sandbox_pool_slot_id: slot_id.clone(),
            sandbox_expected_fencing_token: SandboxPoolFencingToken::sandbox_initial(),
            sandbox_preparation_evidence_fingerprint: fingerprint('a'),
        })
        .expect("slot ready");

    let claim = control
        .sandbox_claim_pool_slot(SandboxPoolClaimRequest {
            sandbox_tenant_id: tenant_id,
            sandbox_pool_slot_id: slot_id.clone(),
            sandbox_session_id:
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new("session-1")
                    .expect("valid session"),
            sandbox_runtime_binding_id:
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new("binding-1")
                    .expect("valid binding"),
            sandbox_operation_id:
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOperationId::new("op-1")
                    .expect("valid operation"),
            sandbox_request_fingerprint: fingerprint('b'),
            sandbox_admission_grant_id: Some(
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new("grant-1")
                    .expect("valid grant"),
            ),
            sandbox_capacity_reservation_id: Some(
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new("capacity-1")
                    .expect("valid capacity"),
            ),
            sandbox_capacity_revision: 1,
            sandbox_expected_fencing_token: SandboxPoolFencingToken::sandbox_initial(),
            sandbox_ttl_seconds: 30,
            sandbox_resource_profile_id: profile_id.clone(),
        })
        .expect("claim bound");

    // 4. The launch plan binds the claimed slot's claim and the fresh
    //    identity evidence, then the worker consumes it.
    let mut plan = SandboxInstanceLaunchPlan::sandbox_new(
        "plan-1",
        version_id.as_str(),
        claim.sandbox_pool_claim_id.as_str(),
        "identity-evidence-1",
        1_050,
    )
    .expect("valid plan");
    assert_eq!(
        plan.sandbox_pool_claim_ref(),
        claim.sandbox_pool_claim_id.as_str()
    );
    plan.sandbox_mark_consumed(1_100).expect("consumed");

    // The chain closes: template tuple == build tuple, the claim owns the
    // prepared slot under the advanced fencing token, and the plan that
    // referenced the claim reached its terminal.
    let slot_after = control.sandbox_slot(&slot_id).expect("slot still present");
    assert_eq!(
        slot_after.sandbox_fencing_token.sandbox_as_i64(),
        1,
        "the claim advanced the slot fencing token"
    );
    assert_eq!(claim.sandbox_fencing_token.sandbox_as_i64(), 1);
}

#[test]
fn a_claimed_slot_refuses_a_second_claim_across_the_chain() {
    let (_version, version_id) = publish_template_side();
    let (slot_id, profile_id, _tenant_id) = pool_request_ids();
    let control = BoundedSandboxPoolControl::sandbox_with_clock(Arc::new(|| 1_000));
    control
        .sandbox_prepare_pool_slot(SandboxPoolPreparationRequest {
            sandbox_pool_slot_id: slot_id.clone(),
            sandbox_pool_class: SandboxPoolClass::PreparedSlot,
            sandbox_resource_profile_id: profile_id.clone(),
            sandbox_node_reference:
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new("node-1")
                    .expect("valid node"),
            sandbox_provider_id:
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new("provider-1")
                    .expect("valid provider"),
            sandbox_provider_kind: SandboxPoolProviderKind::new("firecracker").expect("valid kind"),
            sandbox_isolation_assurance: SandboxIsolationAssurance::MicroVm,
            sandbox_artifact_manifest_revision:
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new(TUPLE)
                    .expect("valid revision"),
            sandbox_capacity_revision: 1,
            sandbox_warm_kvm_evidence_ref: None,
        })
        .expect("slot prepared");
    control
        .sandbox_mark_pool_slot_ready(SandboxPoolReadyRequest {
            sandbox_pool_slot_id: slot_id.clone(),
            sandbox_expected_fencing_token: SandboxPoolFencingToken::sandbox_initial(),
            sandbox_preparation_evidence_fingerprint: fingerprint('a'),
        })
        .expect("slot ready");
    let request = |operation: &str, session: &str, binding: &str| SandboxPoolClaimRequest {
        sandbox_tenant_id: sdkwork_intelligence_sandbox_pool_control::SandboxPoolTenantId::new(
            "tenant-1",
        )
        .expect("valid tenant"),
        sandbox_pool_slot_id: slot_id.clone(),
        sandbox_session_id: sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new(
            session,
        )
        .expect("valid session"),
        sandbox_runtime_binding_id:
            sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new(binding)
                .expect("valid binding"),
        sandbox_operation_id:
            sdkwork_intelligence_sandbox_pool_control::SandboxPoolOperationId::new(operation)
                .expect("valid operation"),
        sandbox_request_fingerprint: fingerprint('b'),
        sandbox_admission_grant_id: Some(
            sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new("grant-1")
                .expect("valid grant"),
        ),
        sandbox_capacity_reservation_id: Some(
            sdkwork_intelligence_sandbox_pool_control::SandboxPoolOpaqueRef::new("capacity-1")
                .expect("valid capacity"),
        ),
        sandbox_capacity_revision: 1,
        sandbox_expected_fencing_token: SandboxPoolFencingToken::sandbox_initial(),
        sandbox_ttl_seconds: 30,
        sandbox_resource_profile_id: profile_id.clone(),
    };
    let first = control
        .sandbox_claim_pool_slot(request("op-1", "session-1", "binding-1"))
        .expect("first claim");
    let _plan = SandboxInstanceLaunchPlan::sandbox_new(
        "plan-1",
        version_id.as_str(),
        first.sandbox_pool_claim_id.as_str(),
        "identity-evidence-1",
        1_000,
    )
    .expect("plan bound to the live claim");
    // The same operation replays to the same claim: idempotency holds across
    // the composed chain.
    let replay = control
        .sandbox_claim_pool_slot(request("op-1", "session-1", "binding-1"))
        .expect("idempotent replay");
    assert_eq!(replay.sandbox_pool_claim_id, first.sandbox_pool_claim_id);
    // Single ownership: a different operation cannot claim the same slot
    // while the first claim is active, whatever plan it carries.
    assert!(matches!(
        control.sandbox_claim_pool_slot(request("op-2", "session-2", "binding-2")),
        Err(SandboxRuntimePoolError::SandboxPoolNotReady { .. }),
    ));
}
