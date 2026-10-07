//! Full-chain integration proof: from pool slot preparation through the
//! fenced claim, the launch plan, the worker adapter's single plan handoff,
//! the registry-backed start-command resolution, to a real host process
//! reaching `started` — the on-demand-allocation, resource-pooling and
//! fast-start capabilities composed end to end on the local lane
//! (`REQ-2026-0019` + `REQ-2026-0029` + `REQ-2026-0033` + `REQ-2026-0034`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use sdkwork_intelligence_sandbox_launch_authority::SandboxInstanceLaunchPlan;
use sdkwork_intelligence_sandbox_pool_control::{
    BoundedSandboxPoolControl, SandboxIsolationAssurance, SandboxPoolClaimRequest,
    SandboxPoolClass, SandboxPoolFencingToken, SandboxPoolOpaqueRef, SandboxPoolPreparationRequest,
    SandboxPoolProviderKind, SandboxPoolReadyRequest, SandboxResourceProfileId,
    SandboxRuntimePoolPort,
};
use sdkwork_intelligence_sandbox_template_authority::BoundedSandboxTemplateRegistry;
use sdkwork_intelligence_sandbox_worker_local::{
    SandboxLaunchProvisionerPort, SandboxLocalLaunchAdapter,
};
use sdkwork_intelligence_sandbox_worker_local_exec::{
    SandboxLocalStartCommandExecutor, SandboxTemplateRegistryResolver,
};
use sdkwork_sandbox_provider_local::command_executor::SandboxLocalCommandExecutor;
use sdkwork_sandbox_provider_local::host_boundary::SandboxLocalHostBoundary;
use sdkwork_sandbox_provider_local::process_runner::{
    SandboxLocalProcessRunnerConfig, SandboxLocalTokioProcessRunner,
};
use sdkwork_sandbox_provider_spi::SandboxCommandLimits;

fn fingerprint(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
}

fn sandbox_executable_roots() -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![PathBuf::from("C:\\Windows\\System32")]
    } else {
        vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")]
    }
}

fn sandbox_limits() -> SandboxCommandLimits {
    SandboxCommandLimits {
        sandbox_timeout_ms: 10_000,
        sandbox_stdout_byte_limit: 64 * 1024,
        sandbox_stderr_byte_limit: 64 * 1024,
        sandbox_cleanup_timeout_ms: 2_000,
        sandbox_max_process_count: 8,
    }
}

/// The provisioner port backed by nothing but the flow: the local lane's
/// allocation reference IS the pool claim the plan is bound to.
struct ClaimBackedProvisioner;
impl SandboxLaunchProvisionerPort for ClaimBackedProvisioner {
    fn sandbox_provision(
        &self,
        sandbox_launch_plan_ref: &str,
    ) -> Result<String, sdkwork_intelligence_sandbox_worker_local::SandboxWorkerLocalAdapterError>
    {
        Ok(format!("allocation-for-{sandbox_launch_plan_ref}"))
    }
}

#[test]
fn the_full_fast_start_chain_runs_a_real_process_from_a_pooled_slot() {
    // 1. Publish the template definition and version through the bounded
    //    registry: the start command is the published one.
    let mut registry = BoundedSandboxTemplateRegistry::sandbox_new();
    let definition_id =
        sdkwork_intelligence_sandbox_template_authority::SandboxTemplateDefinitionId::new(
            "definition-1",
        )
        .expect("valid id");
    registry
        .sandbox_publish_definition(
            sdkwork_intelligence_sandbox_template_authority::SandboxTemplateDefinition::sandbox_publish(
                definition_id.clone(),
                "1",
                sdkwork_intelligence_sandbox_template_authority::SandboxTemplateOpaqueRef::new(
                    "base-env-1",
                )
                .expect("valid ref"),
                Vec::new(),
                BTreeMap::new(),
                if cfg!(windows) {
                    "cmd /C echo fast-start"
                } else {
                    "echo fast-start"
                },
                900,
            )
            .expect("valid definition"),
        )
        .expect("definition published");
    registry
        .sandbox_publish_version(
            sdkwork_intelligence_sandbox_template_authority::SandboxTemplateVersion::sandbox_publish(
                sdkwork_intelligence_sandbox_template_authority::SandboxTemplateVersionId::new(
                    "version-1",
                )
                .expect("valid id"),
                definition_id.clone(),
                BTreeSet::new(),
                BTreeSet::new(),
                sdkwork_intelligence_sandbox_template_authority::SandboxTemplateOpaqueRef::new(
                    "req-0012-rootfs-sha256-0000000000000000000000000000000000000000000000000000000000000000",
                )
                .expect("valid tuple ref"),
            )
            .expect("valid version"),
        )
        .expect("version published");

    // 2. The pool prepares a tenant-neutral slot, readies it on fresh
    //    evidence, and the fenced claim binds it to one tenant runtime.
    let control = BoundedSandboxPoolControl::sandbox_with_clock(Arc::new(|| 1_000));
    let slot_id = sdkwork_intelligence_sandbox_pool_control::SandboxPoolSlotId::new("slot-1")
        .expect("valid slot");
    let profile_id = SandboxResourceProfileId::new("profile-1").expect("valid profile");
    control
        .sandbox_prepare_pool_slot(SandboxPoolPreparationRequest {
            sandbox_pool_slot_id: slot_id.clone(),
            sandbox_pool_class: SandboxPoolClass::PreparedSlot,
            sandbox_resource_profile_id: profile_id.clone(),
            sandbox_node_reference: SandboxPoolOpaqueRef::new("node-1").expect("valid node"),
            sandbox_provider_id: SandboxPoolOpaqueRef::new("provider-1").expect("valid provider"),
            sandbox_provider_kind: SandboxPoolProviderKind::new("firecracker")
                .expect("valid kind"),
            sandbox_isolation_assurance: SandboxIsolationAssurance::MicroVm,
            sandbox_artifact_manifest_revision: SandboxPoolOpaqueRef::new(
                "req-0012-rootfs-sha256-0000000000000000000000000000000000000000000000000000000000000000",
            )
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
    let claim = control
        .sandbox_claim_pool_slot(SandboxPoolClaimRequest {
            sandbox_tenant_id: sdkwork_intelligence_sandbox_pool_control::SandboxPoolTenantId::new(
                "tenant-1",
            )
            .expect("valid tenant"),
            sandbox_pool_slot_id: slot_id.clone(),
            sandbox_session_id: SandboxPoolOpaqueRef::new("session-1").expect("valid session"),
            sandbox_runtime_binding_id: SandboxPoolOpaqueRef::new("binding-1")
                .expect("valid binding"),
            sandbox_operation_id:
                sdkwork_intelligence_sandbox_pool_control::SandboxPoolOperationId::new("op-1")
                    .expect("valid operation"),
            sandbox_request_fingerprint: fingerprint('b'),
            sandbox_admission_grant_id: Some(SandboxPoolOpaqueRef::new("grant-1").expect("valid")),
            sandbox_capacity_reservation_id: Some(
                SandboxPoolOpaqueRef::new("capacity-1").expect("valid capacity"),
            ),
            sandbox_capacity_revision: 1,
            sandbox_expected_fencing_token: SandboxPoolFencingToken::sandbox_initial(),
            sandbox_ttl_seconds: 30,
            sandbox_resource_profile_id: profile_id.clone(),
        })
        .expect("claim bound");

    // 3. The launch plan binds the claimed slot's claim with fresh identity
    //    evidence.
    let mut plan = SandboxInstanceLaunchPlan::sandbox_new(
        "plan-1",
        "version-1",
        claim.sandbox_pool_claim_id.as_str(),
        "identity-evidence-1",
        1_050,
    )
    .expect("valid plan");

    // 4. The worker adapter runs the single plan handoff, earns the
    //    allocation reference through the provisioner port, and dispatches
    //    the start command — which resolves from the published registry
    //    entry and executes as a real host process.
    let registry = Arc::new(Mutex::new(registry));
    let start_commands = Arc::new(SandboxTemplateRegistryResolver::sandbox_new(registry));
    let executor = Arc::new(SandboxLocalCommandExecutor::new(
        Arc::new(SandboxLocalHostBoundary::new(
            ["cmd", "echo"].into_iter().map(str::to_owned).collect(),
            [].into_iter().map(str::to_owned).collect(),
        )),
        Arc::new(SandboxLocalTokioProcessRunner::new(
            SandboxLocalProcessRunnerConfig {
                sandbox_executable_roots: sandbox_executable_roots(),
                sandbox_workspace_root: std::env::temp_dir(),
            },
        )),
    ));
    let wiring = SandboxLocalStartCommandExecutor::sandbox_new(
        executor,
        start_commands,
        sdkwork_intelligence_sandbox_worker_local_exec::SandboxStartCommandScope {
            sandbox_tenant_id: "tenant-1".to_owned(),
            sandbox_provider_id: "provider-local".to_owned(),
            sandbox_workspace_id: "workspace-1".to_owned(),
            sandbox_session_id: "session-1".to_owned(),
            sandbox_id: "sandbox-1".to_owned(),
            sandbox_runtime_binding_id: "binding-1".to_owned(),
            sandbox_fencing_token: claim.sandbox_fencing_token.sandbox_as_i64() as u64,
            sandbox_working_directory: ".".to_owned(),
            sandbox_environment: BTreeMap::new(),
            sandbox_command_limits: sandbox_limits(),
        },
    )
    .expect("bridge runtime builds");
    let adapter =
        SandboxLocalLaunchAdapter::sandbox_new(Arc::new(ClaimBackedProvisioner), Arc::new(wiring));
    let execution = adapter
        .sandbox_execute(&mut plan, "execution-1", 1_100)
        .expect("real execution started");

    // 5. The chain closes: the plan is consumed, the execution record earned
    //    every reference and reached `started`, and the slot stays claimed
    //    under the advanced fencing token.
    assert_eq!(
        plan.sandbox_instance_launch_plan_state(),
        sdkwork_intelligence_sandbox_launch_authority::SandboxLaunchPlanState::Consumed
    );
    assert_eq!(
        execution.sandbox_worker_execution_state(),
        sdkwork_intelligence_sandbox_worker_authority::SandboxWorkerExecutionState::Started
    );
    assert_eq!(execution.sandbox_launch_plan_ref(), "plan-1");
    assert!(execution.sandbox_provider_allocation_ref().is_some());
    let slot_after = control.sandbox_slot(&slot_id).expect("slot present");
    assert_eq!(slot_after.sandbox_fencing_token.sandbox_as_i64(), 1);
}
