import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import path from "node:path";
import test from "node:test";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

function readJson(relativePath) {
  return JSON.parse(readFileSync(path.join(repoRoot, relativePath), "utf8"));
}

function readStatus(relativePath) {
  const source = readFileSync(path.join(repoRoot, relativePath), "utf8");
  const match = source.match(/^(?:status|Status):\s*(\S+)\s*$/mu);
  assert.ok(match, `${relativePath} must declare status`);
  return match[1];
}

const contract = readJson("specs/sandbox-runtime-pool.contract.json");

test("Runtime Pool contract authorizes the landed control-plane slice only", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.runtime-pool-contract");
  // 2026-10-04: the authorized control-plane slice landed
  // (crates/sdkwork-intelligence-sandbox-pool-control) with its state machine,
  // claim, fencing and bounded-registry code and tests, so the contract's
  // implementation gate is open for that slice only. Runtime against a real
  // VMM, the PostgreSQL claim authority, Snapshot reuse, API/SDK and
  // deployment surfaces stay gated, and the contract itself remains draft.
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, true);
  assert.equal(contract["x-sdkwork-no-runtime-implementation"], true);
  assert.equal(contract["x-sdkwork-no-database-implementation"], true);
  assert.equal(contract["x-sdkwork-no-snapshot-implementation"], true);
  assert.equal(contract["x-sdkwork-no-api-sdk-deployment"], true);
  assert.equal(
    readStatus(
      "docs/product/requirements/REQ-2026-0019-sandbox-runtime-pool-and-fast-allocation.md",
    ),
    "ready",
  );
  assert.equal(
    readStatus(
      "docs/architecture/decisions/ADR-20260730-sandbox-runtime-pool-claim-and-sanitization.md",
    ),
    "accepted",
  );
  assert.equal(
    readStatus(
      "docs/engineering/reviews/REVIEW-20260730-sandbox-runtime-pool-architecture-security.md",
    ),
    "accepted",
  );
});

test("The landed pool control-plane crate implements the pinned states, operations and bounds", () => {
  const crateRoot = path.join(repoRoot, "crates/sdkwork-intelligence-sandbox-pool-control");
  const source = (relative) => readFileSync(path.join(crateRoot, "src", relative), "utf8");
  const stateSource = source("state.rs");
  for (const stateName of contract.slot.states) {
    assert.ok(
      stateSource.includes(`"${stateName}"`),
      `slot state ${stateName} must exist in the crate state machine`,
    );
  }
  for (const stateName of contract.claim.states) {
    assert.ok(
      stateSource.includes(`"${stateName}"`),
      `claim state ${stateName} must exist in the crate state machine`,
    );
  }
  const portSource = source("port.rs");
  for (const operation of contract.operations) {
    assert.ok(
      portSource.includes(`fn ${operation}(`),
      `contract operation ${operation} must be a control-plane port method`,
    );
  }
  const boundsSource = source("bounds.rs");
  const boundConstants = {
    sandbox_claim_ttlSecondsMax: "SANDBOX_POOL_CLAIM_TTL_SECONDS_MAX",
    sandbox_reconciliationBatchSizeMax: "SANDBOX_POOL_RECONCILIATION_BATCH_SIZE_MAX",
    sandbox_candidateSlotCountMax: "SANDBOX_POOL_CANDIDATE_SLOT_COUNT_MAX",
    sandbox_claimAttemptCountMax: "SANDBOX_POOL_CLAIM_ATTEMPT_COUNT_MAX",
    sandbox_retryAfterSecondsMax: "SANDBOX_POOL_RETRY_AFTER_SECONDS_MAX",
    sandbox_cleanupDeadlineSecondsMax: "SANDBOX_POOL_CLEANUP_DEADLINE_SECONDS_MAX",
    sandbox_refillOperationsPerNodeMax: "SANDBOX_POOL_REFILL_OPERATIONS_PER_NODE_MAX",
    sandbox_perProfileTargetMax: "SANDBOX_POOL_PER_PROFILE_TARGET_MAX",
  };
  for (const [boundKey, constantName] of Object.entries(boundConstants)) {
    const boundValue = contract.bounds[boundKey];
    assert.ok(
      boundsSource.includes(`${constantName}: `) && boundsSource.includes(`= ${boundValue};`),
      `bound ${boundKey}=${boundValue} must be restated by ${constantName}`,
    );
  }
  const errorSource = source("error.rs");
  for (const errorCode of contract.errors.sandbox_errorCodes) {
    assert.ok(
      errorSource.includes(`"${errorCode}"`),
      `contract error code ${errorCode} must exist in the typed error enum`,
    );
  }
});

test("Pool classes keep Prepared first and Warm behind a separate KVM evidence gate", () => {
  const prepared = contract.poolClasses.sandbox_prepared_slot;
  const warm = contract.poolClasses.sandbox_warm_microvm_slot;
  assert.equal(prepared.firstCommercialImplementationTarget, true);
  assert.equal(prepared.tenantStateAllowed, false);
  assert.equal(prepared.tenantVmmRunning, false);
  assert.equal(prepared.providerAllocateBeforeClaimAllowed, false);
  assert.equal(warm.separateApprovalAndEvidenceRequired, true);
  assert.equal(warm.cleanImmutableSnapshotRequired, true);
  assert.equal(warm.tenantStateAllowed, false);
  assert.equal(warm.realKvmResidueEvidenceRequired, true);
});

test("Pool slot state and public fields are closed and Sandbox-prefixed", () => {
  assert.deepEqual(contract.slot.states, [
    "preparing",
    "ready",
    "claiming",
    "claimed",
    "sanitizing",
    "quarantined",
    "retired",
  ]);
  assert.equal(contract.slot.unknownStateRejected, true);
  assert.equal(contract.slot.illegalTransitionRejected, true);
  assert.equal(contract.slot.readyRequiresTenantNeutralEvidence, true);
  for (const field of [...contract.slot.requiredFields, ...contract.claim.requiredFields]) {
    assert.match(field, /^sandbox_/u);
  }
});

test("Capacity reservation, claim, grants, Provider and readiness have one fixed order", () => {
  assert.deepEqual(contract.allocationOrdering, [
    "sandbox_admission_reservation_confirmed",
    "sandbox_verified_node_hard_filter_passed",
    "sandbox_capacity_reservation_confirmed",
    "sandbox_pool_slot_claimed",
    "sandbox_fresh_guest_identity_issued",
    "sandbox_workspace_grant_applied",
    "sandbox_network_grant_applied",
    "sandbox_resource_grant_applied",
    "sandbox_provider_allocate_and_start",
    "sandbox_effective_readiness_verified",
    "sandbox_admission_bound",
  ]);
  assert.equal(contract.claim.confirmedCapacityReservationRequired, true);
  assert.equal(contract.claim.callerSelectedSlotNodeOrProviderAllowed, false);
});

test("Pool claim is single-owner, fenced, CAS-protected and idempotent", () => {
  assert.equal(contract.claim.singleActiveClaimPerSlot, true);
  assert.equal(contract.claim.singleActiveClaimPerRuntimeBinding, true);
  assert.equal(contract.claim.atomicCompareAndSwapRequired, true);
  assert.equal(contract.claim.highestFencingTokenPersisted, true);
  assert.equal(contract.claim.sameOperationSameFingerprintReplays, true);
  assert.equal(contract.claim.sameOperationDifferentFingerprintConflicts, true);
  assert.equal(contract.claim.staleFencingRejectedBeforeSideEffect, true);
});

test("Ready slots contain no tenant state and every claim receives fresh effective grants", () => {
  for (const [key, value] of Object.entries(contract.tenantNeutrality)) {
    if (key.endsWith("_allowed") && !key.includes("shared_read_only_base_artifact")) {
      assert.equal(value, false, `${key} must fail closed`);
    }
  }
  assert.equal(contract.claimReadiness.sandbox_fresh_guest_identity_required, true);
  assert.equal(
    contract.claimReadiness.sandbox_workspace_revision_and_fencing_verified,
    true,
  );
  assert.equal(
    contract.claimReadiness.sandbox_network_policy_revision_and_effective_state_verified,
    true,
  );
  assert.equal(
    contract.claimReadiness.sandbox_resource_grant_and_effective_cgroup_verified,
    true,
  );
  assert.equal(contract.claimReadiness.sandbox_partial_readinessMayEnterRunning, false);
});

test("Cleanup uncertainty quarantines both slot and capacity", () => {
  const release = contract.releaseAndSanitization;
  assert.equal(release.sandbox_release_idempotent, true);
  assert.equal(release.sandbox_cleanup_bounded, true);
  assert.equal(release.sandbox_cleanup_failure_visible, true);
  assert.equal(release.sandbox_uncertain_cleanup_quarantines_slot, true);
  assert.equal(release.sandbox_uncertain_cleanup_keeps_capacity_consumed, true);
  assert.equal(release.sandbox_ttl_aloneMayReturnSlotToReady, false);
  assert.equal(release.sandbox_quarantineMayBeBypassedForAvailability, false);
  assert.ok(release.orderedSteps.includes("sandbox_scan_cross_tenant_residue"));
});

test("Cloud Pool uses PostgreSQL, bounded recovery and no overcommit", () => {
  const persistence = contract.persistenceConcurrencyAndRecovery;
  assert.equal(persistence.sandbox_cloud_authoritativeStore, "postgresql");
  assert.equal(persistence.sandbox_processLocalMemoryAuthorityAllowed, false);
  assert.equal(persistence.sandbox_multiControllerClaimRequired, true);
  assert.equal(persistence.sandbox_reconciliationTenantAwareAndBounded, true);
  assert.equal(persistence.sandbox_unboundedScanAllowed, false);
  assert.equal(persistence.sandbox_capacityOvercommitAllowed, false);
  assert.ok(contract.bounds.sandbox_reconciliationBatchSizeMax <= 100);
  assert.equal(contract.scaling.sandbox_refillRateLimitRequired, true);
  assert.equal(contract.scaling.sandbox_quarantinedCapacityExcludedFromAvailableCapacity, true);
});

test("Kernel remains provider-neutral and cannot reuse its legacy one-shot provider", () => {
  const kernel = contract.kernelBoundary;
  assert.equal(kernel.sandbox_kernelAdapter, "SandboxSessionLifecycleAdapter");
  assert.equal(kernel.sandbox_kernelProviderBranchingAllowed, false);
  assert.equal(kernel.sandbox_kernelNodeOrPoolSelectionAllowed, false);
  assert.equal(kernel.sandbox_legacyOneShotProviderMayOwnLifecycleOrPool, false);
  assert.equal(kernel.sandbox_reverseDependencyToKernelOrAgentsAllowed, false);
  assert.equal(
    kernel.sandbox_kernelExecutionPlacementLeaseOrFenceMayBeReusedAsPoolClaimLeaseOrFence,
    false,
  );
  assert.equal(kernel.sandbox_poolClaimOperationIdMayBeReusedAsKernelPlacementOperationId, false);
  assert.equal(contract.ownership.sandbox_kernel_may_select_pool_node_or_provider, false);
});

test("Pool claims are independent from Kernel execution-placement records", () => {
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
  assert.equal(separation.sandbox_capacity_placement_may_replace_kernel_execution_placement, false);
  assert.equal(separation.sandbox_pool_claim_may_advance_kernel_execution_placement_state, false);
});

test("Pool telemetry separates paths and forbids unmeasured commercial claims", () => {
  const telemetry = contract.telemetryAndPerformance;
  assert.equal(telemetry.sandbox_coldPreparedWarmReportedSeparately, true);
  assert.equal(telemetry.sandbox_claimToReadyP50P95P99Required, true);
  assert.equal(telemetry.sandbox_usageOrMetricIsBillingTruth, false);
  assert.equal(telemetry.sandbox_tenantSessionNodeSlotClaimLabelsAllowed, false);
  assert.equal(telemetry.sandbox_fastAllocationClaimAllowedWithoutReferenceEvidence, false);
  assert.equal(telemetry.sandbox_candidatePreparedP95TargetMs, 500);
  assert.equal(telemetry.sandbox_targetBecomesReleaseGateOnlyForPublishedMeasuredProfile, true);
});
