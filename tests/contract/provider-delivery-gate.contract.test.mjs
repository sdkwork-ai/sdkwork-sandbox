import assert from "node:assert/strict";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function readJson(relativePath) {
  return JSON.parse(readRepoFile(relativePath));
}

function readRepoFile(relativePath) {
  return readFileSync(path.join(repoRoot, relativePath), "utf8");
}

function readYamlStatus(relativePath) {
  const source = readRepoFile(relativePath);
  const statusMatch = source.match(/^status:\s*([^\s]+)\s*$/mu);
  assert.ok(statusMatch, `${relativePath} must declare a YAML status`);
  return statusMatch[1];
}

function readMarkdownStatus(relativePath) {
  const source = readRepoFile(relativePath);
  const statusMatch = source.match(/^Status:\s*([^\s]+)\s*$/mu);
  assert.ok(statusMatch, `${relativePath} must declare a Markdown Status`);
  return statusMatch[1];
}

function collectRustSources(relativeDirectory) {
  const absoluteDirectory = path.join(repoRoot, relativeDirectory);
  const rustSources = [];

  for (const entry of readdirSync(absoluteDirectory, { withFileTypes: true })) {
    const entryPath = path.join(absoluteDirectory, entry.name);
    if (entry.isDirectory()) {
      rustSources.push(...collectRustSources(path.relative(repoRoot, entryPath)));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      rustSources.push(readFileSync(entryPath, "utf8"));
    }
  }

  return rustSources;
}

test("Gate 0 records the 2026-09-24 approvals and keeps the remaining gates closed", () => {
  // Approved 2026-09-24 by the repository owner (all listed reviewer roles): the three
  // provider-side requirements move to ready, their decision records to accepted.
  assert.equal(readYamlStatus("docs/product/requirements/REQ-2026-0003-secure-local-provider.md"), "ready");
  assert.equal(readYamlStatus("docs/product/requirements/REQ-2026-0007-sandbox-command-execution-contract.md"), "ready");
  assert.equal(readYamlStatus("docs/product/requirements/REQ-2026-0008-firecracker-sandbox-provider.md"), "ready");
  assert.equal(
    readYamlStatus(
      "docs/product/requirements/REQ-2026-0012-sandbox-firecracker-artifact-compatibility-and-supply-chain.md",
    ),
    "draft",
  );
  assert.equal(
    readYamlStatus(
      "docs/product/requirements/REQ-2026-0013-sandbox-workspace-block-device-attachment-and-sanitization.md",
    ),
    "draft",
  );
  assert.equal(
    readYamlStatus("docs/product/requirements/REQ-2026-0009-sandbox-service-host-composition-and-readiness.md"),
    "draft",
  );
  assert.equal(
    readYamlStatus(
      "docs/product/requirements/REQ-2026-0016-sandbox-multi-tenant-admission-scheduling-and-capacity.md",
    ),
    "draft",
  );
  assert.equal(
    readYamlStatus(
      "docs/product/requirements/REQ-2026-0017-sandbox-node-trust-enrollment-attestation-and-inventory.md",
    ),
    "draft",
  );

  assert.equal(
    readMarkdownStatus("docs/architecture/decisions/ADR-20260728-local-provider-assurance-and-host-boundaries.md"),
    "accepted",
  );
  assert.equal(
    readMarkdownStatus("docs/architecture/decisions/ADR-20260729-sandbox-command-execution-and-terminal-boundary.md"),
    "accepted",
  );
  assert.equal(
    readMarkdownStatus("docs/architecture/decisions/ADR-20260729-firecracker-provider-isolation-and-node-boundaries.md"),
    "accepted",
  );
  assert.equal(
    readMarkdownStatus(
      "docs/architecture/decisions/ADR-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain.md",
    ),
    "proposed",
  );
  assert.equal(
    readMarkdownStatus(
      "docs/architecture/decisions/ADR-20260729-sandbox-workspace-block-device-attachment-and-sanitization.md",
    ),
    "proposed",
  );
  assert.equal(
    readMarkdownStatus("docs/architecture/decisions/ADR-20260729-sandbox-service-host-composition-and-readiness.md"),
    "proposed",
  );
  assert.equal(
    readMarkdownStatus(
      "docs/architecture/decisions/ADR-20260729-sandbox-multi-tenant-admission-scheduling-and-capacity-reservation.md",
    ),
    "proposed",
  );
  assert.equal(
    readMarkdownStatus(
      "docs/architecture/decisions/ADR-20260729-sandbox-node-trust-enrollment-attestation-and-inventory.md",
    ),
    "proposed",
  );

  const deliveryPlan = readRepoFile(
    "docs/engineering/plans/PLAN-2026-0001-local-and-firecracker-provider-delivery.md",
  );
  assert.match(deliveryPlan, /Gate 0 未完成时只允许 Contract、Test Harness、Fake Host Boundary 与文档工作/u);
  assert.match(deliveryPlan, /禁止真实 Host Command、KVM、Jailer、Network Namespace、Secret Injection 或发布配置/u);
});

test("Gate 0 keeps the Local component free of public ports and entrypoints", () => {
  const componentSpec = JSON.parse(
    readRepoFile("crates/sdkwork-sandbox-provider-local/specs/component.spec.json"),
  );

  assert.deepEqual(componentSpec.contracts.publicExports, []);
  assert.deepEqual(componentSpec.contracts.providedPorts, []);
  assert.deepEqual(componentSpec.contracts.requiredPorts, []);
  assert.deepEqual(componentSpec.contracts.runtimeEntrypoints, []);
  assert.deepEqual(componentSpec.contracts.configKeys, []);

  const localSource = collectRustSources("crates/sdkwork-sandbox-provider-local/src").join("\n");
  assert.match(localSource, /#\[cfg\(test\)\]/u);
  assert.doesNotMatch(localSource, /\b(?:std::process|tokio::process|Command::new)\b/u);
});

test("Gate 0 does not materialize deferred Provider crates or public command ports", () => {
  assert.equal(
    existsSync(path.join(repoRoot, "crates/sdkwork-sandbox-provider-firecracker")),
    false,
  );
  assert.equal(
    existsSync(path.join(repoRoot, "crates/sdkwork-sandbox-provider-docker")),
    false,
  );

  const rustSources = collectRustSources("crates").join("\n");
  assert.doesNotMatch(rustSources, /\bSandboxCommandExecutor\b/u);
  assert.doesNotMatch(rustSources, /\bSandboxCommandExecution(?:Request|Result|Error|Limits)\b/u);
});

test("Gate 0 review packet ownership decisions are recorded or still pending, never silent", () => {
  const expected = {
    "docs/engineering/reviews/REVIEW-20260729-sandbox-command-execution-architecture-security.md": "accepted",
    "docs/engineering/reviews/REVIEW-20260729-local-provider-architecture-security.md": "accepted",
    "docs/engineering/reviews/REVIEW-20260729-firecracker-provider-architecture-security.md": "accepted",
    "docs/engineering/reviews/REVIEW-20260729-sandbox-host-isolation-broker.md": "pending-human-review",
    "docs/engineering/reviews/REVIEW-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain.md": "pending-human-review",
    "docs/engineering/reviews/REVIEW-20260729-sandbox-workspace-block-device-attachment-and-sanitization.md": "pending-human-review",
    "docs/engineering/reviews/REVIEW-20260729-sandbox-service-host-composition-and-readiness.md": "pending-human-review",
    "docs/engineering/reviews/REVIEW-20260729-sandbox-multi-tenant-admission-scheduling-and-capacity.md": "pending-human-review",
    "docs/engineering/reviews/REVIEW-20260729-sandbox-node-trust-enrollment-attestation-and-inventory.md": "pending-human-review",
  };
  for (const relativePath of Object.keys(expected)) {
    assert.equal(readMarkdownStatus(relativePath), expected[relativePath]);
  }
  // The packets the transition approved are exactly the ones the map marks accepted; every other
  // packet above stays pending until its own human decision lands.
  assert.equal(Object.values(expected).filter((status) => status === "accepted").length, 3);
});

test("Provider delivery gate contract keeps Local and Firecracker provider-neutral and unimplemented", () => {
  const contract = readJson("specs/sandbox-provider-delivery-gates.contract.json");
  const commandContract = readJson("apis/commands/sandbox-command-contract.json");
  assert.equal(contract.kind, "sdkwork.sandbox.provider-delivery-gates");
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, false);
  assert.deepEqual(contract.sharedConformance.requiredScenarios, commandContract.conformanceScenarios);
  assert.equal(contract.sharedConformance.kernelBranchingAllowed, false);
  assert.equal(contract.sharedConformance.providerPrivateCommandDtosAllowed, false);
  assert.equal(contract.sharedConformance.shellFallbackAllowed, false);
  assert.equal(contract.humanReview.required, true);
  assert.equal(contract.humanReview.approvedOutcomeRequiredBeforeImplementation, true);
  assert.ok(contract.requirementIds.includes("REQ-2026-0011"));
  assert.ok(
    contract.decisionIds.includes(
      "ADR-20260729-sandbox-host-isolation-broker-boundary",
    ),
  );
  assert.ok(
    contract.humanReview.reviewPackets.includes(
      "REVIEW-20260729-sandbox-host-isolation-broker",
    ),
  );
  assert.deepEqual(
    contract.providers.map((sandbox_provider) => sandbox_provider.sandbox_provider_name),
    ["sandbox_local_provider", "sandbox_firecracker_provider"],
  );
  for (const sandbox_provider of contract.providers) {
    assert.equal(sandbox_provider.status, "draft");
    assert.equal(sandbox_provider.implementationAuthorized, false);
    assert.match(sandbox_provider.componentName, /^sdkwork-sandbox-provider-/u);
  }
});

test("Local provider gate is honest HostUser assurance with fail-closed capability limits", () => {
  const contract = readJson("specs/sandbox-provider-delivery-gates.contract.json");
  const localProvider = contract.providers.find(
    (sandbox_provider) => sandbox_provider.sandbox_provider_name === "sandbox_local_provider",
  );
  assert.equal(localProvider.identity.sandbox_kind, "local");
  assert.equal(localProvider.identity.sandbox_assurance, "HostUser");
  assert.deepEqual(localProvider.identity.sandbox_deployment_profiles, ["standalone"]);
  assert.equal(localProvider.capabilityPolicy.sandbox_browser, "denied");
  assert.equal(localProvider.capabilityPolicy.sandbox_port_forward, "denied");
  assert.equal(localProvider.capabilityPolicy.sandbox_network, "denied-until-egress-enforcement-evidence");
  assert.equal(
    localProvider.hostBoundary.sandbox_contract,
    "sandbox-local-provider-host-boundary.contract.json",
  );
  assert.equal(localProvider.hostBoundary.sandbox_status, "draft");
  assert.equal(localProvider.hostBoundary.sandbox_implementation_authorized, false);
  assert.equal(localProvider.hostBoundary.sandbox_opened_capability_handle_required, true);
  assert.equal(localProvider.hostBoundary.sandbox_platform_specific_supervision_required, true);
  assert.equal(localProvider.hostBoundary.sandbox_real_runner_evidence_required, true);
  assert.equal(localProvider.hostBoundary.sandbox_preflight_dependency_required, true);
  assert.equal(localProvider.gate0Evidence.sandbox_host_io, false);
  assert.equal(localProvider.gate0Evidence.sandbox_process_spawn, false);
  assert.ok(localProvider.requiredEvidence.includes("sandbox_local_provider_host_boundary_contract"));
  assert.ok(localProvider.requiredEvidence.includes("sandbox_local_execution_policy_snapshot"));
  assert.ok(
    localProvider.requiredEvidence.includes(
      "sandbox_provider_owned_executable_registry_without_path_search",
    ),
  );
  assert.ok(
    localProvider.requiredEvidence.includes("sandbox_protected_environment_override_denial"),
  );
  assert.ok(localProvider.requiredEvidence.includes("sandbox_linux_cgroup_v2_descendant_cleanup"));
  assert.ok(
    !localProvider.requiredEvidence.includes(
      "sandbox_linux_process_group_or_cgroup_descendant_cleanup",
    ),
  );
  assert.ok(localProvider.requiredEvidence.includes("sandbox_real_host_runner_matrix"));
  assert.ok(localProvider.forbiddenAssuranceClaims.includes("MicroVm"));
});

test("Firecracker provider gate requires real KVM preflight and forbids weak fallback", () => {
  const contract = readJson("specs/sandbox-provider-delivery-gates.contract.json");
  const firecrackerProvider = contract.providers.find(
    (sandbox_provider) => sandbox_provider.sandbox_provider_name === "sandbox_firecracker_provider",
  );
  assert.equal(firecrackerProvider.identity.sandbox_kind, "firecracker");
  assert.equal(firecrackerProvider.identity.sandbox_assurance, "MicroVm");
  assert.deepEqual(firecrackerProvider.identity.sandbox_supported_host_platforms, [
    "linux-kvm-x86_64",
    "linux-kvm-aarch64",
  ]);
  for (const sandbox_preflight_field of [
    "sandbox_linux_required",
    "sandbox_kvm_required",
    "sandbox_cgroup_v2_required",
    "sandbox_jailer_required",
    "sandbox_artifact_digest_and_signature_required",
    "sandbox_workspace_attachment_required",
    "sandbox_fencing_required",
    "sandbox_policy_enforcement_required",
  ]) {
    assert.equal(firecrackerProvider.preflight[sandbox_preflight_field], true);
  }
  assert.equal(firecrackerProvider.preflight.sandbox_missing_preflight_is_ready, false);
  assert.equal(
    firecrackerProvider.artifactCompatibility.sandbox_contract,
    "sandbox-firecracker-artifact-compatibility.contract.json",
  );
  assert.equal(
    firecrackerProvider.artifactCompatibility.sandbox_manifest_type,
    "SandboxFirecrackerArtifactManifest",
  );
  assert.equal(firecrackerProvider.artifactCompatibility.sandbox_status, "draft");
  assert.equal(firecrackerProvider.artifactCompatibility.sandbox_implementation_authorized, false);
  assert.equal(firecrackerProvider.artifactCompatibility.sandbox_exact_tuple_required, true);
  assert.equal(firecrackerProvider.artifactCompatibility.sandbox_runtime_download_allowed, false);
  assert.equal(firecrackerProvider.artifactCompatibility.sandbox_mutable_alias_allowed, false);
  assert.equal(firecrackerProvider.artifactCompatibility.sandbox_preflight_dependency_required, true);
  assert.equal(
    firecrackerProvider.workspaceAttachment.sandbox_contract,
    "sandbox-workspace-block-device-attachment.contract.json",
  );
  assert.equal(
    firecrackerProvider.workspaceAttachment.sandbox_port_type,
    "SandboxWorkspaceBlockDevicePort",
  );
  assert.equal(firecrackerProvider.workspaceAttachment.sandbox_status, "draft");
  assert.equal(firecrackerProvider.workspaceAttachment.sandbox_implementation_authorized, false);
  assert.equal(firecrackerProvider.workspaceAttachment.sandbox_guest_block_device_required, true);
  assert.equal(firecrackerProvider.workspaceAttachment.sandbox_direct_host_directory_mount_allowed, false);
  assert.equal(firecrackerProvider.workspaceAttachment.sandbox_sanitization_and_residue_gate_required, true);
  assert.equal(firecrackerProvider.workspaceAttachment.sandbox_preflight_dependency_required, true);
  assert.equal(
    firecrackerProvider.networkIsolation.sandbox_contract,
    "sandbox-firecracker-network-isolation.contract.json",
  );
  assert.equal(
    firecrackerProvider.networkIsolation.sandbox_policy_port_type,
    "SandboxNetworkPolicyPort",
  );
  assert.equal(
    firecrackerProvider.networkIsolation.sandbox_mechanism_port_type,
    "SandboxNetworkIsolationPort",
  );
  assert.equal(firecrackerProvider.networkIsolation.sandbox_status, "draft");
  assert.equal(firecrackerProvider.networkIsolation.sandbox_implementation_authorized, false);
  assert.equal(firecrackerProvider.networkIsolation.sandbox_default_action, "DenyAll");
  assert.equal(
    firecrackerProvider.networkIsolation.sandbox_per_binding_namespace_and_tap_required,
    true,
  );
  assert.equal(firecrackerProvider.networkIsolation.sandbox_permanent_denials_required, true);
  assert.equal(firecrackerProvider.networkIsolation.sandbox_atomic_apply_and_verify_required, true);
  assert.equal(firecrackerProvider.networkIsolation.sandbox_preflight_dependency_required, true);
  assert.equal(
    firecrackerProvider.resourceIsolation.sandbox_contract,
    "sandbox-firecracker-resource-isolation.contract.json",
  );
  assert.equal(
    firecrackerProvider.resourceIsolation.sandbox_policy_port_type,
    "SandboxResourcePolicyPort",
  );
  assert.equal(
    firecrackerProvider.resourceIsolation.sandbox_mechanism_port_type,
    "SandboxResourceIsolationPort",
  );
  assert.equal(
    firecrackerProvider.resourceIsolation.sandbox_usage_fact_type,
    "SandboxResourceUsageFact",
  );
  assert.equal(firecrackerProvider.resourceIsolation.sandbox_status, "draft");
  assert.equal(firecrackerProvider.resourceIsolation.sandbox_implementation_authorized, false);
  assert.equal(
    firecrackerProvider.resourceIsolation.sandbox_machine_config_and_cgroup_v2_required,
    true,
  );
  assert.equal(
    firecrackerProvider.resourceIsolation.sandbox_cpu_memory_pid_io_verification_required,
    true,
  );
  assert.equal(
    firecrackerProvider.resourceIsolation.sandbox_final_usage_and_residue_gate_required,
    true,
  );
  assert.equal(firecrackerProvider.resourceIsolation.sandbox_metrics_are_billing_truth, false);
  assert.equal(firecrackerProvider.resourceIsolation.sandbox_preflight_dependency_required, true);
  assert.equal(
    firecrackerProvider.schedulingAndCapacity.sandbox_contract,
    "sandbox-multi-tenant-scheduling.contract.json",
  );
  assert.equal(
    firecrackerProvider.schedulingAndCapacity.sandbox_admission_port_type,
    "SandboxAdmissionPolicyPort",
  );
  assert.equal(
    firecrackerProvider.schedulingAndCapacity.sandbox_scheduler_port_type,
    "SandboxSchedulerPort",
  );
  assert.equal(
    firecrackerProvider.schedulingAndCapacity.sandbox_capacity_reservation_port_type,
    "SandboxCapacityReservationPort",
  );
  assert.equal(firecrackerProvider.schedulingAndCapacity.sandbox_status, "draft");
  assert.equal(firecrackerProvider.schedulingAndCapacity.sandbox_implementation_authorized, false);
  assert.equal(
    firecrackerProvider.schedulingAndCapacity.sandbox_admission_before_provider_selection_required,
    true,
  );
  assert.equal(
    firecrackerProvider.schedulingAndCapacity.sandbox_capacity_reservation_before_provider_allocate_required,
    true,
  );
  assert.equal(firecrackerProvider.schedulingAndCapacity.sandbox_assurance_downgrade_allowed, false);
  assert.equal(
    firecrackerProvider.nodeTrustAndInventory.sandbox_contract,
    "sandbox-node-trust-and-inventory.contract.json",
  );
  assert.equal(
    firecrackerProvider.nodeTrustAndInventory.sandbox_enrollment_port_type,
    "SandboxNodeEnrollmentPort",
  );
  assert.equal(
    firecrackerProvider.nodeTrustAndInventory.sandbox_attestation_verification_port_type,
    "SandboxNodeAttestationVerificationPort",
  );
  assert.equal(
    firecrackerProvider.nodeTrustAndInventory.sandbox_inventory_publication_port_type,
    "SandboxNodeInventoryPublicationPort",
  );
  assert.equal(
    firecrackerProvider.nodeTrustAndInventory.sandbox_short_lived_mutual_identity_required,
    true,
  );
  assert.equal(firecrackerProvider.nodeTrustAndInventory.sandbox_authentication_is_attestation, false);
  assert.equal(
    firecrackerProvider.nodeTrustAndInventory.sandbox_verified_inventory_before_scheduler_candidate_required,
    true,
  );
  assert.equal(
    firecrackerProvider.nodeTrustAndInventory.sandbox_cloud_preflight_dependency_required,
    true,
  );
  assert.equal(firecrackerProvider.capabilityPolicy.sandbox_network, "denied-until-policy-verification");
  assert.equal(firecrackerProvider.capabilityPolicy.sandbox_snapshot, "deferred");
  assert.ok(firecrackerProvider.forbiddenFallbacks.includes("sandbox_local_provider"));
  assert.ok(firecrackerProvider.requiredEvidence.includes("sandbox_real_linux_kvm_node_matrix"));
  assert.ok(
    firecrackerProvider.requiredEvidence.includes("sandbox_firecracker_artifact_compatibility_contract"),
  );
  assert.ok(
    firecrackerProvider.requiredEvidence.includes("sandbox_workspace_block_device_attachment_contract"),
  );
  assert.ok(
    firecrackerProvider.requiredEvidence.includes("sandbox_firecracker_network_isolation_contract"),
  );
  assert.ok(
    firecrackerProvider.requiredEvidence.includes("sandbox_firecracker_resource_isolation_contract"),
  );
  assert.ok(
    firecrackerProvider.requiredEvidence.includes(
      "sandbox_multi_tenant_admission_scheduling_capacity_contract",
    ),
  );
  assert.ok(
    firecrackerProvider.requiredEvidence.includes(
      "sandbox_node_trust_enrollment_attestation_inventory_contract",
    ),
  );
  assert.ok(firecrackerProvider.requiredEvidence.includes("sandbox_cross_tenant_residue_scan"));
});
