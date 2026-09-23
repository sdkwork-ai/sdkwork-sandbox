import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function readJson(relativePath) {
  return JSON.parse(readFileSync(path.join(repoRoot, relativePath), "utf8"));
}

const contract = readJson("specs/sandbox-local-provider-host-boundary.contract.json");

test("Local Host Boundary remains a draft standalone HostUser contract", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.local-provider-host-boundary");
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, true);
  assert.equal(contract.identity.sandbox_kind, "local");
  assert.equal(contract.identity.sandbox_assurance, "HostUser");
  assert.deepEqual(contract.identity.sandbox_deployment_profiles, ["standalone"]);
  assert.equal(contract.identity.sandbox_multi_tenant_isolation_claim_allowed, false);
  assert.equal(contract.identity.sandbox_assurance_upgrade_by_config_allowed, false);
  assert.equal(contract.identity.sandbox_weaker_provider_fallback_allowed, false);
});

test("Local Host Boundary consumes opened capabilities and never derives host paths", () => {
  const authority = contract.authorityBoundary;
  assert.equal(authority.sandbox_workspace_authority_owner, "sdkwork-agents");
  assert.equal(authority.sandbox_workspace_input, "already-opened-capability-handle");
  assert.equal(authority.sandbox_runtime_root_input, "composition-opened-capability-handle");
  assert.equal(authority.sandbox_host_root_string_input_allowed, false);
  assert.equal(authority.sandbox_id_to_host_path_derivation_allowed, false);
  assert.equal(authority.sandbox_request_time_ambient_authority_allowed, false);
  assert.equal(authority.sandbox_source_checkout_runtime_root_allowed, false);
  assert.equal(authority.sandbox_provider_private_reference_publication_allowed, false);
  assert.deepEqual(authority.sandbox_request_binding_fields, [
    "tenantId",
    "sandboxProviderId",
    "sandboxWorkspaceId",
    "sandboxSessionId",
    "sandboxId",
    "sandboxRuntimeBindingId",
    "sandboxFencingToken",
  ]);
  assert.equal(authority.sandbox_request_identity_must_match_opened_capabilities_and_binding, true);
  assert.equal(
    authority.sandbox_identity_or_capability_mismatch_action,
    "fail-closed-before-side-effect",
  );
});

test("Filesystem containment is handle-relative and rejects string TOCTOU boundaries", () => {
  const filesystem = contract.filesystemBoundary;
  assert.equal(filesystem.sandbox_request_path_type, "portable-logical-relative-path");
  assert.equal(filesystem.sandbox_workspace_root_token, ".");
  assert.equal(filesystem.sandbox_path_separator, "/");
  assert.equal(
    filesystem.sandbox_access_model,
    "handle-relative-no-follow-and-file-identity-verification",
  );
  assert.equal(filesystem.sandbox_string_canonicalization_is_security_boundary, false);
  assert.equal(filesystem.sandbox_check_then_open_allowed, false);
  assert.equal(filesystem.sandbox_open_then_unverified_use_allowed, false);
  assert.equal(filesystem.sandbox_unsupported_file_type_action, "deny");
  assert.equal(filesystem.sandbox_capability_claim_requires_real_runner_evidence, true);
});

test("Windows and Unix filesystem races have explicit fail-closed controls", () => {
  const windows = contract.filesystemBoundary.sandbox_windows;
  for (const field of [
    "sandbox_drive_unc_device_prefix_denied",
    "sandbox_reserved_device_name_denied",
    "sandbox_alternate_data_stream_denied",
    "sandbox_reparse_traversal_denied",
    "sandbox_hardlink_escape_denied",
    "sandbox_final_path_and_file_identity_verification_required",
    "sandbox_reparse_file_identity_swap_tests_required",
  ]) {
    assert.equal(windows[field], true, `${field} must remain required`);
  }

  const unix = contract.filesystemBoundary.sandbox_unix;
  assert.equal(unix.sandbox_symlink_traversal_denied, true);
  assert.equal(unix.sandbox_mount_traversal_denied, true);
  assert.equal(unix.sandbox_rename_race_denied, true);
  assert.match(unix.sandbox_linux_preferred_primitive, /^openat2-/u);
  assert.match(unix.sandbox_fallback_primitive, /^per-segment-openat-/u);
  assert.equal(unix.sandbox_equivalent_race_resistance_required, true);
});

test("Command execution stays provider-neutral, no-shell, bounded and fenced", () => {
  const command = contract.commandBoundary;
  assert.equal(command.sandbox_executor_port_type, "SandboxCommandExecutor");
  assert.equal(command.sandbox_shared_contract, "../apis/commands/sandbox-command-contract.json");
  assert.equal(command.sandbox_provider_private_command_dto_allowed, false);
  assert.equal(command.sandbox_executable_and_argv_only, true);
  assert.equal(command.sandbox_shell_string_allowed, false);
  assert.equal(command.sandbox_implicit_shell_allowed, false);
  assert.equal(command.sandbox_pty_allowed, false);
  assert.equal(command.sandbox_timeout_hard_bound_required, true);
  assert.equal(command.sandbox_output_hard_bound_required, true);
  assert.equal(command.sandbox_process_count_hard_bound_required, true);
  assert.equal(command.sandbox_executor_recomputes_request_fingerprint, true);
  assert.equal(command.sandbox_durable_idempotency_and_first_terminal_arbitration_required, true);
  assert.equal(command.sandbox_executable_resolution_uses_execution_policy_snapshot, true);
  assert.equal(command.sandbox_os_path_or_working_directory_resolution_allowed, false);
  assert.equal(command.sandbox_fencing_before_spawn_required, true);
  assert.equal(command.sandbox_cleanup_result_required, true);
  assert.match(command.sandbox_cleanup_uncertainty_action, /quarantine/u);

  const policy = contract.executionPolicyBoundary;
  assert.equal(policy.sandbox_policy_owner, "approved-standalone-local-composition");
  assert.equal(policy.sandbox_policy_bound_to_runtime_binding, true);
  assert.equal(policy.sandbox_policy_immutable_for_binding_lifetime, true);
  assert.equal(policy.sandbox_caller_can_supply_or_extend_policy, false);
  assert.equal(policy.sandbox_executable_registry_provider_owned, true);
  assert.equal(policy.sandbox_executable_registry_uses_logical_identifiers, true);
  assert.equal(policy.sandbox_os_path_search_allowed, false);
  assert.equal(policy.sandbox_working_directory_executable_lookup_allowed, false);
  assert.equal(policy.sandbox_resolved_executable_identity_is_private, true);
  assert.equal(policy.sandbox_resolved_executable_identity_must_be_stable_for_replay, true);
  assert.equal(policy.sandbox_policy_snapshot_identity_must_match_provider_and_capabilities, true);
});

test("Host environment starts empty and cannot inherit credential channels", () => {
  const environment = contract.environmentBoundary;
  assert.equal(environment.sandbox_construction_mode, "empty-then-explicit-allowlist");
  assert.equal(environment.sandbox_ambient_environment_inheritance_allowed, false);
  assert.equal(environment.sandbox_allowed_name_and_value_bounds_required, true);
  assert.deepEqual(environment.sandbox_forbidden_inherited_categories, [
    "ssh-agent",
    "cloud-credentials",
    "proxy-credentials",
    "docker-socket",
    "secret-bearing-environment",
    "host-runtime-control",
  ]);
  assert.deepEqual(environment.sandbox_protected_request_names, [
    "PATH",
    "PATHEXT",
    "COMSPEC",
    "SYSTEMROOT",
    "WINDIR",
    "HOME",
    "USERPROFILE",
    "TMP",
    "TEMP",
  ]);
  assert.equal(environment.sandbox_request_may_override_protected_names, false);
  assert.equal(environment.sandbox_request_values_require_per_name_policy_validation, true);
  assert.equal(environment.sandbox_secret_value_request_input_allowed, false);
  assert.equal(environment.sandbox_secret_reference_resolution_authorized, false);
  assert.equal(environment.sandbox_environment_dump_allowed, false);
});

test("Windows supervision prevents execution before Job Object containment", () => {
  const windows = contract.platformSupervision.sandbox_windows;
  assert.equal(windows.sandbox_terminal_claim, "blocked-until-real-runner-conformance");
  assert.equal(windows.sandbox_spawn_state, "suspended");
  assert.equal(windows.sandbox_job_object_required, true);
  assert.equal(windows.sandbox_job_kill_on_close_required, true);
  assert.equal(windows.sandbox_job_completion_port_required, true);
  assert.equal(windows.sandbox_assign_before_resume_required, true);
  assert.equal(windows.sandbox_breakaway_allowed, false);
  assert.equal(windows.sandbox_nested_job_preflight_required, true);
  assert.equal(windows.sandbox_preflight_or_bind_failure_action, "terminate-and-quarantine");
  assert.equal(windows.sandbox_process_group_is_sufficient, false);
});

test("Linux supervision requires race-free delegated cgroup v2 containment", () => {
  const linux = contract.platformSupervision.sandbox_linux;
  assert.equal(linux.sandbox_terminal_claim, "blocked-until-real-runner-conformance");
  assert.equal(linux.sandbox_unified_cgroup_v2_required, true);
  assert.equal(linux.sandbox_delegated_subtree_required, true);
  assert.equal(linux.sandbox_per_runtime_binding_scope_required, true);
  assert.equal(linux.sandbox_membership_before_user_code_required, true);
  assert.equal(linux.sandbox_race_free_launcher_or_broker_required, true);
  assert.equal(linux.sandbox_spawn_then_write_cgroup_procs_is_sufficient, false);
  assert.equal(linux.sandbox_process_group_is_sufficient, false);
  assert.equal(linux.sandbox_cgroup_kill_required, true);
  assert.equal(linux.sandbox_cgroup_events_required, true);
  assert.equal(linux.sandbox_pid_controller_required, true);
  assert.equal(linux.sandbox_cleanup_success_fact, "cgroup-populated-zero");
});

test("macOS Terminal is denied until detached descendants can be contained", () => {
  const macos = contract.platformSupervision.sandbox_macos;
  assert.equal(macos.sandbox_terminal_claim, "denied");
  assert.equal(macos.sandbox_process_group_is_sufficient, false);
  assert.equal(macos.sandbox_session_is_sufficient, false);
  assert.equal(macos.sandbox_detached_descendant_containment_proven, false);
  assert.equal(macos.sandbox_unsupported_terminal_request_action, "fail-closed");
  assert.equal(macos.sandbox_platform_fallback_allowed, false);
});

test("Cleanup is idempotent, preserves Agents workspace and quarantines uncertainty", () => {
  const cleanup = contract.cleanupBoundary;
  assert.equal(cleanup.sandbox_stop_idempotent_required, true);
  assert.equal(cleanup.sandbox_destroy_idempotent_required, true);
  assert.equal(cleanup.sandbox_descendant_tree_cleanup_required, true);
  assert.equal(cleanup.sandbox_temporary_handle_cleanup_required, true);
  assert.equal(cleanup.sandbox_temporary_allocation_cleanup_required, true);
  assert.equal(cleanup.sandbox_persistent_workspace_deletion_allowed, false);
  assert.equal(cleanup.sandbox_success_active_process_count, 0);
  assert.equal(cleanup.sandbox_success_temporary_handle_count, 0);
  assert.equal(cleanup.sandbox_success_temporary_residue_count, 0);
  assert.equal(cleanup.sandbox_cleanup_failure_visible, true);
  assert.equal(cleanup.sandbox_cleanup_failure_quarantines_binding, true);
  assert.equal(cleanup.sandbox_uncertain_capacity_reuse_allowed, false);
});

test("Supply-chain candidates remain non-authoritative and runtime dependencies stay unchanged", () => {
  const supplyChain = contract.supplyChain;
  assert.equal(supplyChain.sandbox_runtime_dependency_changes_authorized, false);
  assert.deepEqual(
    supplyChain.sandbox_candidates.map((candidate) => [
      candidate.sandbox_package,
      candidate.sandbox_disposition,
    ]),
    [
      ["process-wrap", "conditional-candidate"],
      ["cap-std", "conditional-candidate"],
      ["tokio", "existing-dependency-candidate"],
      ["cgroups-rs", "not-selected"],
    ],
  );
  assert.equal(
    supplyChain.sandbox_candidates.find((candidate) => candidate.sandbox_package === "cap-std")
      .sandbox_msrv_closed,
    false,
  );
  assert.ok(supplyChain.sandbox_required_before_selection.includes("fresh-online-rustsec"));
  assert.ok(supplyChain.sandbox_required_before_selection.includes("windows-and-linux-build"));
  assert.ok(
    supplyChain.sandbox_required_before_selection.includes(
      "macos-build-for-any-claimed-capability",
    ),
  );
  assert.ok(
    supplyChain.sandbox_required_before_selection.includes(
      "human-dependency-security-approval",
    ),
  );
});

test("Readiness and real evidence fail closed for every platform slice", () => {
  for (const value of Object.values(contract.readiness)) {
    assert.equal(typeof value, "boolean");
  }
  assert.equal(contract.readiness.sandbox_missing_required_dependency_is_ready, false);
  assert.equal(contract.readiness.sandbox_missing_workspace_capability_is_ready, false);
  assert.equal(contract.readiness.sandbox_missing_or_mismatched_execution_policy_is_ready, false);
  assert.equal(
    contract.readiness.sandbox_unstable_or_path_searched_executable_resolution_is_ready,
    false,
  );
  assert.equal(contract.readiness.sandbox_request_identity_or_capability_mismatch_is_ready, false);
  assert.equal(contract.readiness.sandbox_missing_platform_supervisor_is_ready, false);
  assert.equal(contract.readiness.sandbox_missing_real_runner_evidence_is_ready, false);
  assert.equal(contract.readiness.sandbox_missing_cleanup_reconciliation_is_ready, false);
  assert.equal(contract.readiness.sandbox_capability_descriptor_must_match_verified_platform, true);
  assert.equal(contract.readiness.sandbox_unsupported_capability_fallback_allowed, false);
  assert.ok(contract.requiredRealEvidence.sandbox_windows.includes("nested-job-fail-closed"));
  assert.ok(
    contract.requiredRealEvidence.sandbox_linux.includes(
      "race-free-cgroup-membership-before-user-code",
    ),
  );
  assert.deepEqual(contract.requiredRealEvidence.sandbox_macos, [
    "terminal-capability-denial",
    "unsupported-terminal-request-fail-closed",
    "no-platform-or-provider-fallback",
  ]);
  for (const evidence of [
    "runtime-binding-policy-snapshot-immutability",
    "provider-owned-executable-resolution-without-path-search",
    "protected-environment-override-denial",
  ]) {
    assert.ok(contract.requiredRealEvidence.sandbox_common.includes(evidence), evidence);
  }
});

test("Sensitive Host details stay out of observability and implementation remains prohibited", () => {
  const observability = contract.observabilityBoundary;
  assert.equal(observability.sandbox_server_owned_trace_required, true);
  assert.equal(observability.sandbox_low_cardinality_metrics_required, true);
  for (const field of [
    "sandbox_raw_command_logging_allowed",
    "sandbox_argument_logging_allowed",
    "sandbox_environment_logging_allowed",
    "sandbox_output_logging_allowed",
    "sandbox_physical_path_logging_allowed",
    "sandbox_host_pid_logging_allowed",
    "sandbox_provider_private_reference_logging_allowed",
  ]) {
    assert.equal(observability[field], false, `${field} must remain denied`);
  }
  assert.equal(observability.sandbox_cleanup_and_quarantine_facts_required, true);
  assert.equal(contract.humanReview.required, true);
  assert.equal(contract.humanReview.approvedOutcomeRequiredBeforeImplementation, true);
  assert.equal(contract["x-sdkwork-no-provider-runtime"], true);
  assert.equal(contract["x-sdkwork-no-host-io"], true);
  assert.equal(contract["x-sdkwork-no-process-spawn"], true);
  assert.equal(contract["x-sdkwork-no-secret-injection"], true);
  assert.equal(contract["x-sdkwork-no-runtime-dependency-change"], true);
});
