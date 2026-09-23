import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const contractPath = path.join(
  repoRoot,
  "crates/sdkwork-sandbox-service-host/specs/sandbox-service-host-composition.contract.json",
);
const componentPath = path.join(
  repoRoot,
  "crates/sdkwork-sandbox-service-host/specs/component.spec.json",
);
const contract = JSON.parse(readFileSync(contractPath, "utf8"));
const component = JSON.parse(readFileSync(componentPath, "utf8"));

test("Sandbox Service Host contract remains a non-implementable Gate 0 authority", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.service-host-composition-contract");
  assert.equal(contract.status, "draft");
  assert.equal(contract.requirementId, "REQ-2026-0009");
  assert.deepEqual(contract.relatedRequirementIds, [
    "REQ-2026-0003",
    "REQ-2026-0005",
    "REQ-2026-0007",
    "REQ-2026-0008",
    "REQ-2026-0010",
    "REQ-2026-0011",
    "REQ-2026-0012",
    "REQ-2026-0013",
    "REQ-2026-0014",
    "REQ-2026-0015",
    "REQ-2026-0016",
    "REQ-2026-0017",
    "REQ-2026-0018",
    "REQ-2026-0019",
    "REQ-2026-0020",
    "REQ-2026-0021",
    "REQ-2026-0022",
  ]);
  assert.equal(contract.component, "sdkwork-sandbox-service-host");
  assert.equal(contract.layerRole, "runtime-service-host");
  assert.equal(contract.implementationAuthorized, false);
  assert.equal(contract["x-sdkwork-require-human-review"], true);
  assert.equal(contract["x-sdkwork-no-runtime-wiring"], true);
  assert.equal(contract["x-sdkwork-no-config-materialization"], true);
  assert.equal(contract["x-sdkwork-no-deployment"], true);
  assert.equal(contract["x-sdkwork-no-provider-or-scheduler-runtime"], true);
});

test("Sandbox Service Host gate dependencies resolve and remain closed at Gate 0", () => {
  const resolution = contract.gateDependencies.resolution;
  assert.equal(resolution.sandbox_failure_mode, "fail-closed");
  assert.equal(resolution.sandbox_missing_contract_is_ready, false);
  assert.equal(resolution.sandbox_unknown_contract_status_is_ready, false);
  assert.equal(resolution.sandbox_unknown_profile_is_ready, false);
  assert.equal(resolution.sandbox_unrequested_profile_fallback_allowed, false);
  assert.equal(resolution.sandbox_pending_human_review_is_ready, false);
  assert.equal(resolution.sandbox_missing_implementation_authorization_is_ready, false);
  assert.equal(resolution.sandbox_delegated_authorization_source_must_resolve, true);
  assert.equal(resolution.sandbox_dependency_state_may_be_inferred_from_service_host, false);
  assert.equal(resolution.sandbox_all_required_dependencies_must_be_ready, true);

  const sandbox_dependency_ids = new Set();
  // 2026-09-24: the local host boundary contract flipped to authorized with its packet signed, so
  // it is the one gate dependency no longer closed; every other dependency must remain closed.
  // The command contract joined the authorized set when REQ-2026-0007 reached ready (2026-09-24).
  const authorized_dependencies = new Set(["sandbox_local_host_boundary", "sandbox_command_contract"]);
  for (const sandbox_dependency of contract.gateDependencies.contracts) {
    assert.match(sandbox_dependency.sandbox_dependency_id, /^sandbox_/u);
    assert.equal(sandbox_dependency_ids.has(sandbox_dependency.sandbox_dependency_id), false);
    sandbox_dependency_ids.add(sandbox_dependency.sandbox_dependency_id);

    const sandbox_dependency_path = path.resolve(
      path.dirname(contractPath),
      sandbox_dependency.sandbox_contract,
    );
    const sandbox_dependency_contract = JSON.parse(readFileSync(sandbox_dependency_path, "utf8"));
    assert.equal(sandbox_dependency_contract.status, "draft");
    assert.equal(sandbox_dependency_contract["x-sdkwork-require-human-review"], true);

    const sandbox_authorization_field =
      sandbox_dependency.sandbox_implementation_authorization_field;
    if (sandbox_authorization_field !== null) {
      if (authorized_dependencies.has(sandbox_dependency.sandbox_dependency_id)) {
        assert.equal(sandbox_dependency_contract[sandbox_authorization_field], true);
      } else {
        assert.equal(sandbox_dependency_contract[sandbox_authorization_field], false);
      }
    }
  }

  assert.equal(sandbox_dependency_ids.size, contract.gateDependencies.contracts.length);
  assert.equal(contract.gateDependencies.contracts.length, 18);
  assert.ok(sandbox_dependency_ids.has("sandbox_local_host_boundary"));
  assert.ok(sandbox_dependency_ids.has("sandbox_lifecycle_history_and_idempotency"));
  assert.ok(sandbox_dependency_ids.has("sandbox_runtime_pool"));
  assert.ok(sandbox_dependency_ids.has("sandbox_workspace_runtime_transaction"));
  assert.ok(sandbox_dependency_ids.has("sandbox_standalone_data_residency"));
  assert.ok(sandbox_dependency_ids.has("sandbox_service_host_bootstrap"));
  assert.ok(sandbox_dependency_ids.has("sandbox_outbox_contract"));

  const sandbox_provider_delivery = contract.gateDependencies.contracts.find(
    (sandbox_dependency) =>
      sandbox_dependency.sandbox_dependency_id === "sandbox_provider_delivery_gates",
  );
  const sandbox_provider_delivery_contract = JSON.parse(
    readFileSync(
      path.resolve(path.dirname(contractPath), sandbox_provider_delivery.sandbox_contract),
      "utf8",
    ),
  );
  assert.equal(sandbox_provider_delivery.sandbox_authorization_source, "selected-provider-entry");
  for (const sandbox_provider of sandbox_provider_delivery_contract.providers) {
    assert.equal(sandbox_provider.status, "draft");
    assert.equal(sandbox_provider.implementationAuthorized, false);
  }
});

test("Sandbox Service Host profile gates bind Local and cold Firecracker prerequisites", () => {
  const sandbox_dependency_ids = new Set(
    contract.gateDependencies.contracts.map(
      (sandbox_dependency) => sandbox_dependency.sandbox_dependency_id,
    ),
  );
  const sandbox_common = contract.profileReadinessGates.common;
  const sandbox_selection = contract.profileReadinessGates.executionProfileSelection;
  assert.equal(sandbox_selection.sandbox_execution_profile_id_is_caller_supplied, false);
  assert.equal(sandbox_selection.sandbox_execution_profile_id_is_derived, true);
  assert.deepEqual(sandbox_selection.sandbox_derivation_inputs, [
    "sandbox_deployment_profile",
    "sandbox_selected_provider_kind",
  ]);
  assert.deepEqual(
    sandbox_selection.sandbox_exact_mappings.map((sandbox_mapping) => [
      sandbox_mapping.sandbox_deployment_profile,
      sandbox_mapping.sandbox_selected_provider_kind,
      sandbox_mapping.sandbox_execution_profile_id,
    ]),
    [
      ["standalone", "local", "sandbox_standalone_local"],
      ["standalone", "firecracker", "sandbox_standalone_firecracker"],
      ["cloud", "firecracker", "sandbox_cloud_firecracker"],
    ],
  );
  assert.equal(sandbox_selection.sandbox_unknown_mapping_action, "fail-closed");
  assert.equal(sandbox_selection.sandbox_cloud_local_mapping_allowed, false);
  assert.equal(sandbox_selection.sandbox_provider_policy_owned_by_service_host, false);
  assert.equal(sandbox_selection.sandbox_profile_gate_required_before_provider_side_effect, true);
  for (const sandbox_dependency_id of sandbox_common.sandbox_required_dependency_ids) {
    assert.ok(sandbox_dependency_ids.has(sandbox_dependency_id));
  }
  assert.ok(
    sandbox_common.sandbox_required_dependency_ids.includes(
      "sandbox_lifecycle_history_and_idempotency",
    ),
  );
  assert.ok(
    sandbox_common.sandbox_required_dependency_ids.includes("sandbox_service_host_bootstrap"),
  );
  assert.ok(sandbox_common.sandbox_required_dependency_ids.includes("sandbox_outbox_contract"));
  assert.ok(
    sandbox_common.sandbox_required_dependency_ids.includes(
      "sandbox_workspace_runtime_transaction",
    ),
  );
  assert.equal(
    sandbox_common.sandbox_required_dependency_ids.includes("sandbox_workspace_attachment"),
    false,
  );
  assert.deepEqual(
    sandbox_common.sandbox_required_readiness_dimensions,
    contract.readiness.dimensions,
  );

  const sandbox_profiles = new Map(
    contract.profileReadinessGates.profiles.map((sandbox_profile) => [
      sandbox_profile.sandbox_execution_profile_id,
      sandbox_profile,
    ]),
  );
  assert.equal(sandbox_profiles.size, contract.profileReadinessGates.profiles.length);
  assert.deepEqual(
    new Set(
      sandbox_selection.sandbox_exact_mappings.map(
        (sandbox_mapping) => sandbox_mapping.sandbox_execution_profile_id,
      ),
    ),
    new Set(sandbox_profiles.keys()),
  );
  for (const sandbox_profile of contract.profileReadinessGates.profiles) {
    assert.equal(Object.hasOwn(sandbox_profile, "sandbox_profile_id"), false);
    for (const sandbox_dependency_id of sandbox_profile.sandbox_required_dependency_ids) {
      assert.ok(sandbox_dependency_ids.has(sandbox_dependency_id));
    }
  }
  const sandbox_local = sandbox_profiles.get("sandbox_standalone_local");
  assert.equal(sandbox_local.sandbox_deployment_profile, "standalone");
  assert.equal(sandbox_local.sandbox_provider_kind, "local");
  assert.equal(sandbox_local.sandbox_required_isolation_assurance, "HostUser");
  assert.deepEqual(sandbox_local.sandbox_required_dependency_ids, [
    "sandbox_provider_delivery_gates",
    "sandbox_standalone_data_residency",
    "sandbox_local_host_boundary",
  ]);
  const sandbox_local_residency_dependency = contract.gateDependencies.contracts.find(
    (sandbox_dependency) =>
      sandbox_dependency.sandbox_dependency_id === "sandbox_standalone_data_residency",
  );
  const sandbox_local_residency_contract = JSON.parse(
    readFileSync(
      path.resolve(path.dirname(contractPath), sandbox_local_residency_dependency.sandbox_contract),
      "utf8",
    ),
  );
  assert.deepEqual(sandbox_local_residency_contract.readiness.sandbox_required_for_execution_profile_ids, [
    "sandbox_standalone_local",
  ]);
  assert.deepEqual(
    sandbox_local_residency_contract.readiness.sandbox_forbidden_for_execution_profile_ids,
    ["sandbox_standalone_firecracker", "sandbox_cloud_firecracker"],
  );
  assert.equal(sandbox_local.sandbox_provider_gate_selector, "sandbox_local_provider");
  assert.equal(sandbox_local.sandbox_cloud_dependencies_allowed, false);

  const sandbox_standalone_firecracker = sandbox_profiles.get(
    "sandbox_standalone_firecracker",
  );
  assert.equal(sandbox_standalone_firecracker.sandbox_deployment_profile, "standalone");
  assert.equal(sandbox_standalone_firecracker.sandbox_provider_kind, "firecracker");
  assert.equal(sandbox_standalone_firecracker.sandbox_required_isolation_assurance, "MicroVm");
  for (const sandbox_dependency_id of [
    "sandbox_host_isolation_broker",
    "sandbox_firecracker_artifact_compatibility",
    "sandbox_workspace_attachment",
    "sandbox_firecracker_network_isolation",
    "sandbox_firecracker_resource_isolation",
  ]) {
    assert.ok(
      sandbox_standalone_firecracker.sandbox_required_dependency_ids.includes(
        sandbox_dependency_id,
      ),
    );
  }
  assert.equal(
    sandbox_standalone_firecracker.sandbox_required_dependency_ids.includes(
      "sandbox_runtime_pool",
    ),
    false,
  );
  assert.equal(
    sandbox_standalone_firecracker.sandbox_provider_gate_selector,
    "sandbox_firecracker_provider",
  );
});

test("Sandbox Service Host cloud Firecracker gate requires trust, admission, and capacity", () => {
  const sandbox_cloud_firecracker = contract.profileReadinessGates.profiles.find(
    (sandbox_profile) =>
      sandbox_profile.sandbox_execution_profile_id === "sandbox_cloud_firecracker",
  );
  assert.equal(sandbox_cloud_firecracker.sandbox_deployment_profile, "cloud");
  assert.equal(sandbox_cloud_firecracker.sandbox_provider_kind, "firecracker");
  assert.equal(sandbox_cloud_firecracker.sandbox_required_isolation_assurance, "MicroVm");
  for (const sandbox_dependency_id of [
    "sandbox_host_isolation_broker",
    "sandbox_firecracker_artifact_compatibility",
    "sandbox_workspace_attachment",
    "sandbox_firecracker_network_isolation",
    "sandbox_firecracker_resource_isolation",
    "sandbox_multi_tenant_scheduling",
    "sandbox_node_trust_and_inventory",
    "sandbox_quota_and_capacity_persistence",
  ]) {
    assert.ok(
      sandbox_cloud_firecracker.sandbox_required_dependency_ids.includes(
        sandbox_dependency_id,
      ),
    );
  }
  assert.equal(sandbox_cloud_firecracker.sandbox_local_fallback_allowed, false);
  assert.equal(
    sandbox_cloud_firecracker.sandbox_provider_gate_selector,
    "sandbox_firecracker_provider",
  );
});

test("Sandbox Runtime Pool remains an explicit optional cloud overlay", () => {
  const sandbox_pool = contract.profileReadinessGates.runtimePoolOverlay;
  assert.deepEqual(sandbox_pool.sandbox_applicable_execution_profile_ids, [
    "sandbox_cloud_firecracker",
  ]);
  assert.equal(Object.hasOwn(sandbox_pool, "sandbox_applicable_profile_ids"), false);
  assert.equal(sandbox_pool.sandbox_activation, "explicit-optional-acceleration");
  assert.deepEqual(sandbox_pool.sandbox_required_dependency_ids_when_active, [
    "sandbox_runtime_pool",
  ]);
  const sandbox_dependency_ids = new Set(
    contract.gateDependencies.contracts.map(
      (sandbox_dependency) => sandbox_dependency.sandbox_dependency_id,
    ),
  );
  for (const sandbox_dependency_id of sandbox_pool.sandbox_required_dependency_ids_when_active) {
    assert.ok(sandbox_dependency_ids.has(sandbox_dependency_id));
  }
  assert.equal(sandbox_pool.sandbox_required_for_cold_firecracker, false);
  assert.equal(sandbox_pool.sandbox_pool_required_request_may_fallback_to_cold, false);
  assert.equal(sandbox_pool.sandbox_optional_pool_failure_may_use_cold_firecracker, true);
  assert.equal(sandbox_pool.sandbox_cold_fallback_requires_cloud_firecracker_profile_ready, true);
  assert.equal(sandbox_pool.sandbox_assurance_downgrade_allowed, false);

  for (const sandbox_profile of contract.profileReadinessGates.profiles) {
    assert.equal(
      sandbox_profile.sandbox_required_dependency_ids.includes("sandbox_runtime_pool"),
      false,
    );
  }
});

test("Sandbox Command and Terminal require executable and conformance evidence", () => {
  const sandbox_command = contract.capabilityReadinessGates.sandbox_command;
  const sandbox_dependency_ids = new Set(
    contract.gateDependencies.contracts.map(
      (sandbox_dependency) => sandbox_dependency.sandbox_dependency_id,
    ),
  );
  for (const sandbox_dependency_id of sandbox_command.sandbox_required_dependency_ids) {
    assert.ok(sandbox_dependency_ids.has(sandbox_dependency_id));
  }
  assert.deepEqual(sandbox_command.sandbox_required_bindings, ["SandboxCommandExecutor"]);
  assert.ok(
    sandbox_command.sandbox_required_dependency_ids.includes("sandbox_command_contract"),
  );
  assert.ok(
    sandbox_command.sandbox_required_evidence.includes(
      "sandbox_common_command_conformance_passed",
    ),
  );
  assert.equal(sandbox_command.sandbox_provider_descriptor_alone_is_ready, false);
  assert.equal(sandbox_command.sandbox_selected_profile_gate_required, true);

  const sandbox_terminal = contract.capabilityReadinessGates.sandbox_terminal;
  assert.deepEqual(sandbox_terminal.sandbox_depends_on_capability_gates, ["sandbox_command"]);
  assert.ok(
    sandbox_terminal.sandbox_required_evidence.includes(
      "sandbox_descendant_supervision_and_cleanup_conformance_passed",
    ),
  );
  assert.equal(sandbox_terminal.sandbox_local_windows_and_linux_real_runner_evidence_required, true);
  assert.equal(sandbox_terminal.sandbox_local_macos_terminal_allowed, false);
  assert.equal(sandbox_terminal.sandbox_unsupported_terminal_request_action, "fail-closed");
  assert.equal(sandbox_terminal.sandbox_provider_descriptor_alone_is_ready, false);
});

test("Sandbox Service Host contract preserves standalone and cloud parity", () => {
  assert.deepEqual(
    contract.profileParity.profiles.map((sandbox_profile) => sandbox_profile.sandbox_deployment_profile),
    ["standalone", "cloud"],
  );
  assert.ok(contract.profileParity.sharedContracts.includes("SandboxSessionLifecyclePort"));
  assert.ok(contract.profileParity.sharedContracts.includes("SandboxProvider"));
  assert.deepEqual(contract.profileParity.forbiddenServiceBranches, [
    "sandbox_deployment_profile",
    "sandbox_environment",
    "sandbox_runtime_target",
  ]);
});

test("Sandbox Service Host contract requires typed prefixed config and injected ports", () => {
  assert.equal(contract.typedConfig.type, "SandboxServiceHostConfig");
  assert.equal(contract.typedConfig.sourceAuthority, "runtime-bootstrap-normalized-safe-config");
  for (const sandbox_field of contract.typedConfig.fields) {
    assert.match(sandbox_field.name, /^sandbox_/u);
    assert.equal(sandbox_field.required, true);
  }
  assert.ok(contract.typedConfig.forbiddenSources.includes("process-environment"));
  assert.ok(contract.typedConfig.forbiddenSources.includes("embedded-secret-material"));

  const sandbox_dependency_names = contract.injectedDependencies.map(
    (sandbox_dependency) => sandbox_dependency.name,
  );
  for (const sandbox_dependency_name of sandbox_dependency_names) {
    assert.match(sandbox_dependency_name, /^sandbox_/u);
  }
  assert.equal(sandbox_dependency_names.includes("sandbox_lifecycle_service"), false);
  assert.ok(sandbox_dependency_names.includes("sandbox_session_repository"));
  assert.ok(sandbox_dependency_names.includes("sandbox_provider_registry"));
  assert.ok(sandbox_dependency_names.includes("sandbox_workspace_attachment"));
  assert.ok(sandbox_dependency_names.includes("sandbox_runtime_directory_capabilities"));
  assert.ok(sandbox_dependency_names.includes("sandbox_secret_key_source"));
  assert.ok(sandbox_dependency_names.includes("sandbox_telemetry"));
  const sandbox_workspace_dependency = contract.injectedDependencies.find(
    (sandbox_dependency) => sandbox_dependency.name === "sandbox_workspace_attachment",
  );
  assert.equal(sandbox_workspace_dependency.port, "SandboxWorkspaceAttachmentPort");
  assert.equal(
    sandbox_workspace_dependency.boundaryContract,
    "../../../specs/sandbox-workspace-block-device-attachment.contract.json",
  );
  assert.equal(sandbox_workspace_dependency.providerSpecificMechanismInjectedBehindPort, true);

  const sandbox_construction = contract.serviceConstruction;
  assert.equal(sandbox_construction.sandbox_composition_owner, "sdkwork-sandbox-service-host");
  assert.equal(sandbox_construction.sandbox_constructed_port, "SandboxSessionLifecyclePort");
  assert.equal(
    sandbox_construction.sandbox_constructs_lifecycle_service_from_injected_dependencies,
    true,
  );
  assert.equal(sandbox_construction.sandbox_lifecycle_service_is_injected_input, false);
  assert.equal(sandbox_construction.sandbox_single_composition_root_required, true);
});

test("Sandbox Service Host readiness is complete, bounded, and redacted", () => {
  assert.equal(contract.readiness.type, "SandboxServiceHostReadiness");
  assert.equal(contract.readiness.aggregation, "all-required-dimensions-ready");
  assert.equal(contract.readiness.failureMode, "fail-closed");
  assert.deepEqual(contract.readiness.dimensions, [
    "sandbox_config",
    "sandbox_runtime_directory_capabilities",
    "sandbox_store",
    "sandbox_provider_registry",
    "sandbox_workspace_attachment",
    "sandbox_secret_key_source",
    "sandbox_telemetry",
    "sandbox_fencing",
  ]);
  assert.equal(contract.readiness.bounds.sandbox_dimension_count, 8);
  assert.ok(contract.readiness.bounds.sandbox_check_timeout_ms > 0);
  for (const sandbox_field of contract.readiness.safeFields) {
    assert.match(sandbox_field, /^sandbox_/u);
  }
  for (const sandbox_field of [
    "sandbox_secret_material",
    "sandbox_database_url",
    "sandbox_physical_host_path",
    "sandbox_provider_allocation_reference",
    "sandbox_raw_command",
  ]) {
    assert.ok(contract.readiness.forbiddenFields.includes(sandbox_field));
  }
});

test("Sandbox Service Host shutdown and failure behavior are explicit", () => {
  assert.equal(contract.shutdown.type, "SandboxServiceHostShutdown");
  assert.equal(contract.shutdown.bounded, true);
  assert.equal(contract.shutdown.idempotent, true);
  assert.equal(contract.shutdown.stopNewLifecycleSideEffectsFirst, true);
  assert.equal(contract.shutdown.timeoutOutcome, "sandbox_internal_failure");
  assert.ok(contract.failClosedConditions.includes("sandbox_provider_assurance_insufficient"));
  assert.ok(contract.failClosedConditions.includes("sandbox_fencing_unprovable"));
});

test("Sandbox Service Host component remains free of executable surface", () => {
  assert.deepEqual(component.contracts.publicExports, []);
  assert.deepEqual(component.contracts.providedPorts, []);
  assert.deepEqual(component.contracts.requiredPorts, []);
  assert.deepEqual(component.contracts.runtimeEntrypoints, []);
  assert.deepEqual(component.contracts.configKeys, []);

  assert.equal(contract.authorization.publicExports, false);
  assert.equal(contract.authorization.runtimeEntrypoints, false);
  assert.equal(contract.authorization.configKeys, false);
  assert.equal(contract.authorization.httpOrRpc, false);
  assert.equal(contract.authorization.providerImplementation, false);
  assert.equal(contract.authorization.secretKmsImplementation, false);
  assert.equal(contract.authorization.deployment, false);
});
