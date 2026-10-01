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

test("Runtime Pool requirement is ready while the contract stays implementation-gated", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.runtime-pool-contract");
  // 2026-09-29: REVIEW-20260730 accepted (single-owner, REVIEW-20260929 ALIGN-01 slice 2);
  // the contract itself stays draft/unauthorized until the control-plane slice lands.
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, false);
  assert.equal(contract["x-sdkwork-no-runtime-implementation"], true);
  assert.equal(contract["x-sdkwork-no-database-implementation"], true);
  assert.equal(contract["x-sdkwork-no-snapshot-implementation"], true);
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
