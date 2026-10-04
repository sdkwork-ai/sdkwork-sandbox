import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function readJson(relativePath) {
  return JSON.parse(readFileSync(path.join(repoRoot, relativePath), "utf8"));
}

const contract = readJson("specs/sandbox-workspace-runtime-transaction.contract.json");
const poolContract = readJson("specs/sandbox-runtime-pool.contract.json");
const attachmentContract = readJson(
  "specs/sandbox-workspace-block-device-attachment.contract.json",
);
const commandContract = readJson("apis/commands/sandbox-command-contract.json");

test("Workspace runtime transaction authorizes the landed control-plane slice only", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.workspace-runtime-transaction-contract");
  // 2026-10-05: REVIEW-20260730 (transaction architecture/security, ten roles)
  // accepted via single-owner structured approval; the control-plane slice
  // landed as crates/sdkwork-intelligence-sandbox-transaction-control, so the
  // contract's implementation gate is open for that slice only. Runtime
  // against real providers, the PostgreSQL transaction authority, storage/KMS
  // adapters, worker and API/SDK/transport stay gated, and the contract itself
  // remains draft.
  assert.equal(contract.status, "draft");
  assert.equal(contract.requirementId, "REQ-2026-0021");
  assert.equal(contract.implementationAuthorized, true);
  assert.equal(contract["x-sdkwork-require-human-review"], true);
  assert.equal(contract["x-sdkwork-no-runtime-implementation"], true);
  assert.equal(contract["x-sdkwork-no-database-implementation"], true);
  assert.equal(contract["x-sdkwork-no-api-sdk-transport"], true);
  assert.equal(contract["x-sdkwork-no-provider-storage-kms-or-deployment"], true);
  const readStatus = (relative) => {
    const source = readFileSync(path.join(repoRoot, relative), "utf8");
    const match = source.match(/^(?:status|Status):\s*(\S+)\s*$/mu);
    assert.ok(match, `${relative} must declare status`);
    return match[1];
  };
  assert.equal(
    readStatus("docs/product/requirements/REQ-2026-0021-sandbox-workspace-runtime-transaction-and-checkpoint.md"),
    "ready",
  );
  assert.equal(
    readStatus("docs/architecture/decisions/ADR-20260730-sandbox-workspace-runtime-transaction-and-checkpoint.md"),
    "accepted",
  );
  assert.equal(
    readStatus(
      "docs/engineering/reviews/REVIEW-20260730-sandbox-workspace-runtime-transaction-architecture-security.md",
    ),
    "accepted",
  );
});

test("The landed transaction-control crate implements the pinned states, stages, bounds and error codes", () => {
  const crateRoot = path.join(repoRoot, "crates/sdkwork-intelligence-sandbox-transaction-control");
  const source = (relative) => readFileSync(path.join(crateRoot, "src", relative), "utf8");
  const stateSource = source("state.rs");
  for (const stateName of contract.transaction.states) {
    assert.ok(
      stateSource.includes(`"${stateName}"`),
      `transaction state ${stateName} must exist in the crate state machine`,
    );
  }
  const stageSource = source("stage.rs");
  for (const stageName of contract.orchestrationOrder) {
    assert.ok(
      stageSource.includes(`"${stageName}"`),
      `orchestration stage ${stageName} must exist in the crate stage order`,
    );
  }
  const boundsSource = source("bounds.rs");
  const boundConstants = {
    sandbox_request_max_bytes: "SANDBOX_REQUEST_MAX_BYTES",
    sandbox_orchestration_stage_count_max: "SANDBOX_ORCHESTRATION_STAGE_COUNT_MAX",
    sandbox_concurrent_commands_per_transaction_max: "SANDBOX_CONCURRENT_COMMANDS_PER_TRANSACTION_MAX",
    sandbox_queue_wait_ms_max: "SANDBOX_QUEUE_WAIT_MS_MAX",
    sandbox_retry_after_ms_max: "SANDBOX_RETRY_AFTER_MS_MAX",
    sandbox_reconciliation_batch_size_max: "SANDBOX_RECONCILIATION_BATCH_SIZE_MAX",
    sandbox_compensation_attempt_count_max: "SANDBOX_COMPENSATION_ATTEMPT_COUNT_MAX",
    sandbox_reference_max_length: "SANDBOX_REFERENCE_MAX_LENGTH",
  };
  for (const [boundKey, constantName] of Object.entries(boundConstants)) {
    const boundValue = contract.bounds[boundKey];
    assert.ok(
      boundsSource.includes(`${constantName}: `) && boundsSource.includes(`= ${boundValue};`),
      `bound ${boundKey}=${boundValue} must be restated by ${constantName}`,
    );
  }
  const errorSource = source("error.rs");
  for (const error of contract.errors) {
    assert.ok(
      errorSource.includes(`"${error.code}"`),
      `contract error code ${error.code} must exist in the typed error enum`,
    );
  }
  const checkpointSource = source("checkpoint.rs");
  for (const field of contract.checkpoint.sandbox_candidateRequiredFields) {
    assert.ok(
      checkpointSource.includes(field),
      `checkpoint candidate field ${field} must exist in the crate`,
    );
  }
});

test("Local and Firecracker lanes share semantics without sharing isolation claims", () => {
  const local = contract.executionLanes.sandbox_standalone_local;
  const standaloneFirecracker = contract.executionLanes.sandbox_standalone_firecracker;
  const cloud = contract.executionLanes.sandbox_cloud_firecracker;

  assert.equal(local.sandbox_deployment_profile, "standalone");
  assert.equal(local.sandbox_isolation_assurance, "HostUser");
  assert.equal(local.sandbox_multi_tenant_isolation_claim_allowed, false);
  assert.equal(local.sandbox_workspace_data_residency, "device-local");
  assert.equal(local.sandbox_remote_workspace_transfer_enabled_by_default, false);
  assert.equal(local.sandbox_cloud_scheduler_or_runtime_pool_allowed, false);
  assert.equal(local.sandbox_runtime_and_workspace_capabilities_must_be_distinct, true);

  assert.equal(standaloneFirecracker.sandbox_deployment_profile, "standalone");
  assert.equal(standaloneFirecracker.sandbox_isolation_assurance, "MicroVm");
  assert.equal(standaloneFirecracker.sandbox_cloud_scheduler_or_runtime_pool_allowed, false);
  assert.equal(standaloneFirecracker.sandbox_workspace_block_device_contract_required, true);

  assert.equal(cloud.sandbox_deployment_profile, "cloud");
  assert.equal(cloud.sandbox_isolation_assurance, "MicroVm");
  assert.equal(cloud.sandbox_multi_tenant_isolation_required, true);
  assert.equal(cloud.sandbox_runtime_pool_is_optional_acceleration, true);
  assert.equal(cloud.sandbox_cold_firecracker_is_correctness_fallback, true);
  assert.equal(cloud.sandbox_local_or_weaker_assurance_fallback_allowed, false);
});

test("Request is provider-neutral, revision-bound and contains no physical authority", () => {
  for (const field of contract.request.requiredFields) {
    assert.match(field, /^sandbox_/u);
  }
  for (const field of [
    "sandbox_workspace_revision_ref",
    "sandbox_workspace_authorization_grant_ref",
    "sandbox_kernel_execution_placement_ref",
    "sandbox_kernel_execution_placement_generation",
    "sandbox_workload_class_ref",
    "sandbox_checkpoint_policy",
    "sandbox_fencing_token",
  ]) {
    assert.ok(contract.request.requiredFields.includes(field));
  }
  assert.equal(contract.request.unknownFieldsRejected, true);
  assert.equal(contract.request.sandbox_caller_selected_provider_node_pool_or_slot_allowed, false);
  assert.equal(
    contract.request.sandbox_caller_supplied_cpu_memory_disk_or_priority_escalation_allowed,
    false,
  );
  assert.equal(contract.request.sandbox_raw_host_path_device_path_storage_key_or_credential_allowed, false);
});

test("End-to-end ordering closes capacity, attachment, command, checkpoint and release gaps", () => {
  const order = contract.orchestrationOrder;
  assert.equal(order.length, 21);
  const index = (stage) => order.indexOf(stage);

  assert.ok(index("sandbox_workspace_authorization_and_revision_verified") < index("sandbox_kernel_execution_placement_reference_and_generation_verified"));
  assert.ok(index("sandbox_kernel_execution_placement_reference_and_generation_verified") < index("sandbox_admission_reservation_confirmed_or_local_policy_admitted"));
  assert.ok(index("sandbox_admission_reservation_confirmed_or_local_policy_admitted") < index("sandbox_verified_node_and_capacity_reservation_confirmed_or_local_capacity_marked_not_applicable"));
  assert.ok(index("sandbox_verified_node_and_capacity_reservation_confirmed_or_local_capacity_marked_not_applicable") < index("sandbox_pool_claimed_or_cold_or_local_path_selected"));
  assert.ok(index("sandbox_pool_claimed_or_cold_or_local_path_selected") < index("sandbox_provider_allocate_and_start"));
  assert.ok(index("sandbox_environment_ready") < index("sandbox_command_admission_opened"));
  assert.ok(index("sandbox_command_admission_frozen_and_active_commands_drained") < index("sandbox_workspace_write_access_revoked_and_writes_flushed"));
  assert.ok(index("sandbox_workspace_write_access_revoked_and_writes_flushed") < index("sandbox_workspace_checkpoint_candidate_made_durable_or_read_only_noop_recorded"));
  assert.ok(index("sandbox_checkpoint_handoff_made_durable_or_read_only_noop_recorded") < index("sandbox_provider_and_descendants_stopped"));
  assert.ok(index("sandbox_cross_tenant_residue_scan_passed_or_local_zero_residue_verified") < index("sandbox_pool_capacity_and_admission_released_or_local_noop_recorded"));

  assert.equal(contract.orderRules.sandbox_not_applicable_stage_requires_durable_typed_noop_evidence, true);
  assert.equal(contract.orderRules.sandbox_slot_or_capacity_may_release_before_cleanup_evidence, false);
  assert.equal(contract.orderRules.sandbox_ttl_alone_may_skip_checkpoint_detach_or_sanitization, false);
});

test("Kernel execution placement and Sandbox capacity allocation use independent authorities", () => {
  const separation = contract.placementAuthoritySeparation;
  assert.equal(separation.sandbox_kernel_execution_placement_owner, "sdkwork-kernel");
  assert.equal(separation.sandbox_capacity_placement_owner, "SandboxSchedulerPort");
  assert.equal(separation.sandbox_runtime_allocation_binding_owner, "Sandbox lifecycle service");
  assert.equal(separation.sandbox_kernel_and_sandbox_records_have_distinct_ids, true);
  assert.equal(
    separation.sandbox_kernel_and_sandbox_records_have_distinct_lease_and_fencing_domains,
    true,
  );
  assert.equal(
    separation.sandbox_kernel_and_sandbox_records_have_distinct_idempotency_scopes,
    true,
  );
  assert.equal(
    separation.sandbox_kernel_execution_placement_may_select_provider_node_pool_slot_or_storage,
    false,
  );
  assert.equal(
    separation.sandbox_capacity_placement_may_replace_or_advance_kernel_execution_placement,
    false,
  );
  assert.equal(
    separation.sandbox_runtime_transaction_may_reuse_kernel_placement_operation_or_fencing_token,
    false,
  );
  assert.equal(
    contract.fencingAndIdempotency
      .sandbox_kernel_placement_and_runtime_transaction_fencing_tokens_are_distinct,
    true,
  );
  assert.equal(
    contract.fencingAndIdempotency
      .sandbox_kernel_placement_and_runtime_transaction_operation_ids_are_distinct,
    true,
  );
});

test("Composite order preserves the existing Pool reservation and effective-readiness order", () => {
  const poolOrder = poolContract.allocationOrdering;
  assert.ok(poolOrder.indexOf("sandbox_capacity_reservation_confirmed") < poolOrder.indexOf("sandbox_pool_slot_claimed"));
  assert.ok(poolOrder.indexOf("sandbox_pool_slot_claimed") < poolOrder.indexOf("sandbox_provider_allocate_and_start"));
  assert.ok(poolOrder.indexOf("sandbox_provider_allocate_and_start") < poolOrder.indexOf("sandbox_effective_readiness_verified"));
  assert.equal(contract.executionLanes.sandbox_cloud_firecracker.sandbox_runtime_pool_is_optional_acceleration, true);
});

test("Workspace writes use a single writer lease and non-destructive revision promotion", () => {
  const concurrency = contract.workspaceConcurrency;
  assert.equal(concurrency.sandbox_workspace_revision_immutable_during_attachment, true);
  assert.equal(concurrency.sandbox_single_writer_lease_required_per_workspace_revision_target, true);
  assert.equal(concurrency.sandbox_multiple_read_write_transactions_require_distinct_agents_revision_targets, true);
  assert.equal(concurrency.sandbox_stale_writer_fencing_rejected_before_write_or_checkpoint, true);
  assert.equal(concurrency.sandbox_agents_compare_and_swap_required_to_advance_revision, true);
  assert.equal(concurrency.sandbox_checkpoint_conflict_may_overwrite_newer_revision, false);
  assert.equal(concurrency.sandbox_shared_writable_device_across_active_bindings_allowed, false);
  assert.equal(attachmentContract.lifecycle.sandbox_workspace_revision_immutable_during_one_attachment, true);
});

test("Read-write release cannot lose changes or let Sandbox author Agents revisions", () => {
  const checkpoint = contract.checkpoint;
  assert.equal(checkpoint.readOnlyPolicy, "ReadOnlyNoCheckpoint");
  assert.equal(checkpoint.readWritePolicy, "DurableCheckpointRequired");
  assert.equal(checkpoint.sandbox_read_write_release_without_durable_candidate_allowed, false);
  assert.equal(checkpoint.sandbox_silent_discard_of_workspace_writes_allowed, false);
  assert.equal(checkpoint.sandbox_candidate_sealed_before_handoff, true);
  assert.equal(checkpoint.sandbox_handoff_persisted_before_runtime_release, true);
  assert.equal(checkpoint.sandbox_agents_alone_promotes_candidate_to_workspace_revision, true);
  assert.equal(checkpoint.sandbox_promotion_conflict_is_explicit_and_non_destructive, true);
  assert.equal(checkpoint.sandbox_checkpoint_or_handoff_uncertainty_quarantines_binding_and_capacity, true);
  assert.equal(contract.ownership.sandbox_may_advance_agents_workspace_revision, false);
  for (const field of checkpoint.sandbox_candidateRequiredFields) {
    assert.match(field, /^sandbox_/u);
  }
});

test("Command sessions reuse the shared safe contract and checkpoint on disconnect expiry", () => {
  assert.equal(contract.commandSession.sandbox_executor, "SandboxCommandExecutor");
  assert.equal(commandContract.executionModes.includes("executable-argv"), true);
  assert.equal(commandContract.forbiddenExecutionModes.includes("shell-string"), true);
  assert.equal(contract.commandSession.sandbox_shell_string_or_caller_executable_path_allowed, false);
  assert.equal(contract.commandSession.sandbox_concurrent_command_count_bounded_by_policy, true);
  assert.equal(contract.commandSession.sandbox_new_command_rejected_after_freeze, true);
  assert.equal(contract.commandSession.sandbox_client_disconnect_immediately_releases_runtime, false);
  assert.equal(contract.commandSession.sandbox_expired_reconnect_grace_triggers_fenced_checkpoint_and_cleanup, true);
});

test("Every failure window has deterministic bounded compensation or quarantine", () => {
  assert.deepEqual(
    contract.failureCompensation.map((entry) => entry.sandbox_failure_window),
    [
      "before-capacity-reservation",
      "after-capacity-before-workspace-attachment",
      "after-attachment-before-command",
      "during-or-after-write-execution",
      "after-durable-checkpoint-handoff",
      "uncertain-host-storage-or-cleanup-side-effect",
    ],
  );
  for (const entry of contract.failureCompensation) {
    assert.ok(entry.sandbox_required_actions.length > 0);
    for (const action of entry.sandbox_required_actions) {
      assert.match(action, /^sandbox_/u);
    }
  }
  const uncertain = contract.failureCompensation.at(-1).sandbox_required_actions;
  assert.ok(uncertain.includes("sandbox_keep_uncertain_capacity_consumed"));
});

test("High-concurrency operation is bounded, fair and safe for public metadata", () => {
  const backpressure = contract.backpressureAndFairness;
  assert.equal(backpressure.sandbox_cloud_queue_is_bounded, true);
  assert.equal(backpressure.sandbox_tenant_aware_fairness_required, true);
  assert.equal(backpressure.sandbox_per_tenant_active_and_queued_limits_required, true);
  assert.equal(backpressure.sandbox_caller_priority_escalation_allowed, false);
  assert.equal(backpressure.sandbox_unbounded_retry_spin_or_reconciliation_scan_allowed, false);
  assert.ok(contract.bounds.sandbox_reconciliation_batch_size_max <= 100);
  assert.ok(contract.bounds.sandbox_concurrent_commands_per_transaction_max <= 16);
  assert.equal(contract.telemetryAndAudit.sandbox_low_cardinality_labels_required, true);
  assert.equal(contract.telemetryAndAudit.sandbox_tenant_workspace_session_binding_checkpoint_node_slot_labels_allowed, false);
  assert.equal(contract.safeOutcome.sandbox_physical_or_provider_private_metadata_allowed, false);
});

test("Kernel and BirdCoder boundaries remain open-closed and require cross-repository review", () => {
  const kernel = contract.kernelBoundary;
  assert.ok(
    kernel.sandbox_requiredProviderNeutralInputs.includes(
      "sandbox_opaque_kernel_execution_placement_reference_and_generation",
    ),
  );
  assert.equal(kernel.sandbox_kernel_may_select_provider_node_pool_slot_or_physical_storage, false);
  assert.equal(kernel.sandbox_kernel_may_receive_provider_private_attachment_or_allocation_reference, false);
  assert.equal(kernel.sandbox_kernel_legacy_one_shot_provider_may_execute_production_command, false);
  assert.equal(kernel.sandbox_birdcoder_direct_kernel_or_sandbox_dependency_allowed, false);
  assert.equal(kernel.sandbox_cross_repository_human_review_required, true);
  assert.equal(contract.ownership.sandbox_reverse_dependency_to_kernel_agents_or_birdcoder_allowed, false);
});
