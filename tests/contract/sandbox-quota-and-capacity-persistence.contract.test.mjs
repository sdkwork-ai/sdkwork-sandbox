import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const sandboxRepoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const loadSandboxJson = (sandboxRelativePath) =>
  JSON.parse(readFileSync(path.join(sandboxRepoRoot, sandboxRelativePath), "utf8"));

const sandboxContract = loadSandboxJson(
  "specs/sandbox-quota-and-capacity-persistence.contract.json",
);
const sandboxSchedulingContract = loadSandboxJson(
  "specs/sandbox-multi-tenant-scheduling.contract.json",
);
const sandboxNodeTrustContract = loadSandboxJson(
  "specs/sandbox-node-trust-and-inventory.contract.json",
);
const sandboxResourceContract = loadSandboxJson(
  "specs/sandbox-firecracker-resource-isolation.contract.json",
);
const sandboxEventCatalog = loadSandboxJson("apis/async/sandbox-event-catalog.json");
const sandboxObservabilityCatalog = loadSandboxJson(
  "apis/async/sandbox-observability-catalog.json",
);
const sandboxTableRegistry = loadSandboxJson("database/contract/table-registry.json");

test("quota and capacity persistence remains a draft non-implementation contract", () => {
  assert.equal(
    sandboxContract.kind,
    "sdkwork.sandbox.quota-and-capacity-persistence-contract",
  );
  assert.equal(sandboxContract.status, "draft");
  assert.equal(sandboxContract.requirementId, "REQ-2026-0018");
  assert.equal(sandboxContract.implementationAuthorized, false);
  for (const sandboxGate of [
    "x-sdkwork-require-human-review",
    "x-sdkwork-no-runtime-implementation",
    "x-sdkwork-no-database-implementation",
    "x-sdkwork-no-migration-implementation",
    "x-sdkwork-no-repository-implementation",
    "x-sdkwork-no-api-sdk-implementation",
    "x-sdkwork-no-deployment-implementation",
  ]) {
    assert.equal(sandboxContract[sandboxGate], true, `missing gate: ${sandboxGate}`);
  }
  assert.equal(
    sandboxContract.persistenceBoundary.sandbox_migration_or_repository_implementation_authorized,
    false,
  );
});

test("PostgreSQL is the only proposed authority and current tables stay unchanged", () => {
  assert.equal(sandboxContract.persistenceBoundary.sandbox_database_role, "authoritative-server");
  assert.equal(sandboxContract.persistenceBoundary.sandbox_engine, "postgres");
  assert.equal(sandboxContract.persistenceBoundary.sandbox_process_memory_authority_allowed, false);
  assert.equal(sandboxContract.persistenceBoundary.sandbox_sqlite_authority_or_fallback_allowed, false);
  assert.equal(sandboxContract.persistenceBoundary.sandbox_redis_authority_allowed, false);
  assert.equal(sandboxContract.persistenceBoundary.sandbox_auto_migrate_allowed_in_production, false);
  assert.equal(sandboxContract.persistenceBoundary.sandbox_proposed_tables_registered_as_active, false);

  const sandboxRegisteredTables = sandboxTableRegistry.tables.map(
    (sandboxTable) => sandboxTable.table_name,
  );
  assert.deepEqual(sandboxRegisteredTables, [
    "sandbox_session",
    "sandbox_session_operation",
    "sandbox_runtime_binding",
    "sandbox_session_lease",
    "sandbox_instance",
  ]);
  for (const sandboxProposedTable of sandboxContract.proposedTables) {
    assert.equal(sandboxRegisteredTables.includes(sandboxProposedTable.sandbox_table_name), false);
  }
});

test("four responsibility-specific Sandbox aggregates do not copy external authority", () => {
  assert.deepEqual(
    sandboxContract.proposedTables.map((sandboxTable) => ({
      sandboxDomainType: sandboxTable.sandbox_domain_type,
      sandboxTableName: sandboxTable.sandbox_table_name,
    })),
    [
      {
        sandboxDomainType: "SandboxTenantQuotaState",
        sandboxTableName: "sandbox_tenant_quota_state",
      },
      {
        sandboxDomainType: "SandboxAdmissionReservation",
        sandboxTableName: "sandbox_admission_reservation",
      },
      {
        sandboxDomainType: "SandboxNodeCapacityState",
        sandboxTableName: "sandbox_node_capacity_state",
      },
      {
        sandboxDomainType: "SandboxCapacityReservation",
        sandboxTableName: "sandbox_capacity_reservation",
      },
    ],
  );
  assert.equal(sandboxContract.ownership.sandbox_persistence_may_author_identity_entitlement_or_price, false);
  assert.equal(sandboxContract.ownership.sandbox_node_capacity_state_may_author_node_trust, false);
  assert.equal(sandboxContract.ownership.sandbox_metrics_logs_or_cache_may_author_quota_or_capacity, false);
  assert.equal(sandboxContract.ownership.sandbox_price_invoice_payment_owner, "SDKWork Commerce");
});

test("SQL subject alignment blocks new table implementation without hiding current debt", () => {
  assert.equal(sandboxContract.subjectScope.sandbox_sql_tenant_column, "tenant_id");
  assert.equal(sandboxContract.subjectScope.sandbox_sql_tenant_logical_type, "int64");
  assert.equal(sandboxContract.subjectScope.sandbox_sql_tenant_physical_type, "BIGINT");
  assert.equal(sandboxContract.subjectScope.sandbox_sql_tenant_must_be_positive, true);
  assert.equal(sandboxContract.subjectScope.sandbox_domain_variable_name, "sandbox_tenant_id");
  assert.equal(sandboxContract.subjectScope.sandbox_shared_domain_type_name, "TenantId");
  assert.equal(sandboxContract.subjectScope.sandbox_current_lifecycle_tenant_column_type, "TEXT");
  assert.equal(
    sandboxContract.subjectScope.sandbox_current_lifecycle_tenant_type_is_standard_aligned,
    false,
  );
  assert.equal(
    sandboxContract.subjectScope
      .sandbox_subject_id_alignment_migration_required_before_new_table_implementation,
    true,
  );
  assert.equal(sandboxContract.subjectScope.sandbox_client_supplied_tenant_selector_allowed, false);
});

test("table fields, counters, identities, and states are explicit and bounded", () => {
  for (const sandboxTable of sandboxContract.proposedTables) {
    assert.equal(sandboxTable.sandbox_profile, "operational_state");
    assert.ok(sandboxTable.sandbox_required_fields.includes("id"));
    assert.ok(sandboxTable.sandbox_required_fields.includes("version"));
    assert.ok(sandboxTable.sandbox_required_fields.includes("created_at"));
    assert.ok(sandboxTable.sandbox_required_fields.includes("updated_at"));
    for (const sandboxField of sandboxTable.sandbox_required_fields) {
      if (!["id", "tenant_id", "version", "created_at", "updated_at"].includes(sandboxField)) {
        assert.match(sandboxField, /^sandbox_/u);
      }
    }
  }
  const sandboxQuotaState = sandboxContract.proposedTables[0];
  assert.equal(sandboxQuotaState.sandbox_counter_invariants.sandbox_reserved_values_may_exceed_limits, false);
  assert.equal(sandboxQuotaState.sandbox_counter_invariants.sandbox_available_is_derived_as_limit_minus_reserved, true);
  const sandboxNodeState = sandboxContract.proposedTables[2];
  assert.equal(sandboxNodeState.sandbox_counter_invariants.sandbox_reserved_values_may_exceed_totals, false);
  assert.equal(
    sandboxNodeState.sandbox_counter_invariants
      .sandbox_inventory_refresh_may_reduce_total_below_reserved,
    false,
  );
  assert.equal(
    sandboxNodeState.sandbox_counter_invariants
      .sandbox_stale_revoked_or_unverified_inventory_accepts_new_reservation,
    false,
  );
});

test("resource vector uses first-class finite columns and preserves capability ownership", () => {
  assert.equal(sandboxContract.resourceVector.sandbox_type, "SandboxResourceVector");
  assert.deepEqual(sandboxContract.resourceVector.sandbox_first_version_dimensions, [
    "sandbox_runtime_units",
    "sandbox_guest_vcpu_count",
    "sandbox_guest_memory_bytes",
    "sandbox_vmm_overhead_memory_bytes",
  ]);
  assert.equal(sandboxContract.resourceVector.sandbox_dimensions_are_first_class_columns, true);
  assert.equal(sandboxContract.resourceVector.sandbox_core_quantities_in_untyped_json_allowed, false);
  assert.equal(sandboxContract.resourceVector.sandbox_negative_nan_or_infinite_quantity_allowed, false);
  assert.equal(sandboxContract.resourceVector.sandbox_vcpu_overcommit_allowed, false);
  assert.equal(sandboxContract.resourceVector.sandbox_memory_overcommit_allowed, false);
  assert.equal(sandboxContract.resourceVector.sandbox_workspace_storage_capacity_owned_here, false);
  assert.equal(sandboxContract.resourceVector.sandbox_pid_and_io_enforcement_policy_owned_here, false);
});

test("transactions lock globally, commit before side effects, and retry whole units only", () => {
  assert.deepEqual(sandboxContract.globalLockOrder.sandbox_order, [
    "sandbox_session_lease:(tenant_id,sandbox_session_id)",
    "sandbox_session:(tenant_id,sandbox_session_id)",
    "sandbox_runtime_binding:(tenant_id,sandbox_runtime_binding_id)",
    "sandbox_tenant_quota_state:(tenant_id,sandbox_quota_scope)",
    "sandbox_admission_reservation:(tenant_id,sandbox_admission_reservation_id)",
    "sandbox_node_capacity_state:(sandbox_node_reference)",
    "sandbox_capacity_reservation:(tenant_id,sandbox_capacity_reservation_id)",
  ]);
  assert.equal(sandboxContract.globalLockOrder.sandbox_multi_row_keys_sorted_ascending, true);
  assert.equal(
    sandboxContract.transactionWorkflows
      .sandbox_remote_http_rpc_kms_provider_or_user_interaction_while_locked_allowed,
    false,
  );
  assert.ok(
    sandboxContract.transactionWorkflows.sandbox_capacity_reservation_workflow.includes(
      "sandbox_commit_before_provider_allocate",
    ),
  );
  assert.equal(
    sandboxContract.transactionWorkflows
      .sandbox_partial_statement_retry_inside_aborted_transaction_allowed,
    false,
  );
  assert.deepEqual(sandboxContract.idempotencyCasAndFencing.sandbox_serialization_and_deadlock_sqlstates, [
    "40001",
    "40P01",
  ]);
  assert.equal(sandboxContract.idempotencyCasAndFencing.sandbox_retry_scope, "complete-idempotent-transaction");
  assert.ok(sandboxContract.idempotencyCasAndFencing.sandbox_retry_attempts_max <= 4);
});

test("CAS, fencing, idempotency, and database time protect every mutation", () => {
  assert.equal(sandboxContract.idempotencyCasAndFencing.sandbox_same_operation_same_fingerprint_replays_persisted_result, true);
  assert.equal(sandboxContract.idempotencyCasAndFencing.sandbox_same_operation_different_fingerprint_conflicts, true);
  assert.equal(sandboxContract.idempotencyCasAndFencing.sandbox_all_mutable_rows_require_version_cas, true);
  assert.equal(
    sandboxContract.idempotencyCasAndFencing
      .sandbox_stale_fencing_token_rejected_before_counter_or_state_mutation,
    true,
  );
  assert.equal(sandboxContract.idempotencyCasAndFencing.sandbox_counter_update_and_reservation_insert_are_one_transaction, true);
  assert.equal(sandboxContract.idempotencyCasAndFencing.sandbox_database_clock_is_expiry_authority, true);
  assert.equal(sandboxContract.idempotencyCasAndFencing.sandbox_application_clock_may_author_ttl, false);
});

test("expiry and recovery quarantine uncertainty instead of reselling capacity", () => {
  const sandboxAdmissionReservation = sandboxContract.proposedTables[1];
  const sandboxCapacityReservation = sandboxContract.proposedTables[3];
  assert.deepEqual(sandboxAdmissionReservation.sandbox_state_machine.sandbox_states, [
    "reserved",
    "bound",
    "released",
    "expired",
    "quarantined",
  ]);
  assert.deepEqual(sandboxCapacityReservation.sandbox_state_machine.sandbox_states, [
    "prepared",
    "confirmed",
    "released",
    "expired",
    "quarantined",
  ]);
  assert.equal(
    sandboxAdmissionReservation.sandbox_state_machine
      .sandbox_bound_expiry_may_release_quota_without_lifecycle_proof,
    false,
  );
  assert.equal(
    sandboxCapacityReservation.sandbox_state_machine
      .sandbox_confirmed_expiry_may_release_capacity_without_lifecycle_and_provider_cleanup_proof,
    false,
  );
  assert.equal(
    sandboxContract.expiryReleaseAndReconciliation
      .sandbox_uncertain_provider_or_node_state_quarantines_without_freeing_capacity,
    true,
  );
  assert.equal(
    sandboxContract.expiryReleaseAndReconciliation.sandbox_unbounded_scan_or_full_collect_allowed,
    false,
  );
  assert.ok(
    sandboxContract.expiryReleaseAndReconciliation.sandbox_reconciliation_batch_size_max <= 100,
  );
});

test("query, tenant isolation, role, and safe error contracts fail closed", () => {
  assert.equal(sandboxContract.queryAndIndexContract.sandbox_tenant_indexes_lead_with_tenant_id, true);
  assert.equal(sandboxContract.queryAndIndexContract.sandbox_stable_sort_has_unique_id_tiebreaker, true);
  assert.equal(sandboxContract.queryAndIndexContract.sandbox_list_queries_use_sql_keyset_or_skip_locked, true);
  assert.equal(sandboxContract.queryAndIndexContract.sandbox_select_star_allowed, false);
  assert.ok(sandboxContract.queryAndIndexContract.sandbox_query_page_size_max <= 100);
  assert.equal(sandboxContract.securityPrivacyAndRoles.sandbox_tenant_tables_require_rls_defense_in_depth, true);
  assert.equal(sandboxContract.securityPrivacyAndRoles.sandbox_runtime_role_owns_tables_or_bypasses_rls, false);
  assert.equal(sandboxContract.securityPrivacyAndRoles.sandbox_owner_migrator_runtime_readonly_backup_roles_separated, true);
  assert.equal(sandboxContract.errorContract.sandbox_sqlstate_classification_required, true);
  assert.equal(sandboxContract.errorContract.sandbox_localized_database_message_matching_allowed, false);
  assert.equal(sandboxContract.errorContract.sandbox_raw_constraint_sql_capacity_tenant_or_node_detail_allowed, false);
});

test("events and metrics use existing catalogs without becoming business authority", () => {
  const sandboxEventTypes = new Set(
    sandboxEventCatalog.eventTypes.map((sandboxEvent) => sandboxEvent.type),
  );
  for (const sandboxEventType of sandboxContract.eventAuditAndTelemetry.sandbox_event_types) {
    assert.ok(sandboxEventTypes.has(sandboxEventType), `missing event: ${sandboxEventType}`);
  }
  const sandboxMetricNames = new Set(
    sandboxObservabilityCatalog.metrics.catalog.map((sandboxMetric) => sandboxMetric.name),
  );
  for (const sandboxMetricName of sandboxContract.eventAuditAndTelemetry.sandbox_required_metric_names) {
    assert.ok(sandboxMetricNames.has(sandboxMetricName), `missing metric: ${sandboxMetricName}`);
  }
  assert.equal(
    sandboxContract.eventAuditAndTelemetry.sandbox_metrics_are_quota_capacity_or_billing_authority,
    false,
  );
  assert.equal(sandboxContract.eventAuditAndTelemetry.sandbox_logs_are_audit_or_recovery_authority, false);
  assert.equal(
    sandboxContract.securityPrivacyAndRoles
      .sandbox_raw_tenant_node_capacity_entitlement_or_sql_in_log_error_metric_allowed,
    false,
  );
  assert.equal(
    sandboxContract.securityPrivacyAndRoles
      .sandbox_tenant_session_node_reservation_metric_labels_allowed,
    false,
  );
});

test("retention, PITR, migration, and real evidence remain explicit release gates", () => {
  assert.equal(sandboxContract.retentionBackupAndRecovery.sandbox_backup_encrypted, true);
  assert.equal(sandboxContract.retentionBackupAndRecovery.sandbox_point_in_time_recovery_required, true);
  assert.ok(sandboxContract.retentionBackupAndRecovery.sandbox_pitr_window_days_min_candidate >= 7);
  assert.ok(sandboxContract.retentionBackupAndRecovery.sandbox_rpo_seconds_max_candidate <= 300);
  assert.ok(sandboxContract.retentionBackupAndRecovery.sandbox_rto_seconds_max_candidate <= 1800);
  assert.ok(
    sandboxContract.retentionBackupAndRecovery
      .sandbox_restore_exercise_interval_days_max_candidate <= 90,
  );
  assert.equal(sandboxContract.retentionBackupAndRecovery.sandbox_candidate_targets_require_human_approval, true);
  assert.equal(
    sandboxContract.retentionBackupAndRecovery
      .sandbox_backup_job_without_verified_restore_is_recovery_evidence,
    false,
  );
  assert.equal(
    sandboxContract.migrationAndReleaseGate
      .sandbox_existing_tenant_text_to_bigint_requires_dedicated_migration_plan,
    true,
  );
  assert.equal(sandboxContract.migrationAndReleaseGate.sandbox_real_postgresql_16_and_17_evidence_required, true);
  assert.equal(
    sandboxContract.migrationAndReleaseGate
      .sandbox_multi_replica_quota_and_capacity_race_evidence_required,
    true,
  );
  assert.equal(
    sandboxContract.migrationAndReleaseGate
      .sandbox_static_contract_test_is_database_or_commercial_readiness_evidence,
    false,
  );
});

test("scheduling, node trust, and resource contracts trace the persistence dependency", () => {
  assert.ok(sandboxSchedulingContract.relatedRequirementIds.includes("REQ-2026-0018"));
  assert.ok(
    sandboxSchedulingContract.relatedContracts.includes(
      "sandbox-quota-and-capacity-persistence.contract.json",
    ),
  );
  assert.ok(sandboxNodeTrustContract.relatedRequirementIds.includes("REQ-2026-0018"));
  assert.ok(
    sandboxNodeTrustContract.relatedContracts.includes(
      "sandbox-quota-and-capacity-persistence.contract.json",
    ),
  );
  assert.ok(sandboxResourceContract.relatedRequirementIds.includes("REQ-2026-0018"));
  assert.ok(
    sandboxResourceContract.relatedContracts.includes(
      "sandbox-quota-and-capacity-persistence.contract.json",
    ),
  );
});
