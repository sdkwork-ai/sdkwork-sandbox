import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const contract = JSON.parse(
  readFileSync(path.join(repoRoot, "specs/sandbox-internal-control-plane.contract.json"), "utf8"),
);

test("internal control plane remains a draft non-runtime contract", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.internal-control-plane-contract");
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, false);
  assert.equal(contract.candidateNamesArePubliclyApproved, false);
  assert.ok(Object.values(contract.materialization).every((allowed) => allowed === false));
  assert.equal(existsSync(path.join(repoRoot, "apis/rpc")), false);
  assert.equal(existsSync(path.join(repoRoot, "sdks/sdkwork-sandbox-rpc-sdk")), false);
});

test("internal control plane preserves one port with in-process and generated RPC adapters", () => {
  assert.equal(contract.authority.portCandidate, "SandboxControlPlanePort");
  assert.equal(contract.authority.crossRepositoryConsumer, "sdkwork-kernel");
  assert.equal(contract.authority.birdcoderDirectAccessAllowed, false);
  assert.equal(contract.authority.agentsDirectAccessAllowed, false);
  assert.equal(contract.authority.providerPrivateOperationsExposed, false);
  assert.equal(contract.adapterParity.standaloneAdapter, "in-process-port");
  assert.equal(contract.adapterParity.cloudAdapterCandidate, "generated-internal-rpc-client");
  assert.equal(contract.adapterParity.sharedConformanceRequired, true);
  assert.equal(contract.adapterParity.rawHttpFallbackAllowed, false);
  assert.equal(contract.adapterParity.handwrittenGrpcStubAllowed, false);
});

test("internal RPC candidate is private L3 and generated from future authority", () => {
  assert.equal(contract.rpcCandidate.level, "L3");
  assert.equal(contract.rpcCandidate.surface, "internal");
  assert.equal(contract.rpcCandidate.applicationPublicIngressAllowed, false);
  assert.equal(contract.rpcCandidate.privateServiceNetworkRequired, true);
  assert.equal(contract.rpcCandidate.protoAndRpcManifestAreFutureAuthority, true);
  assert.equal(contract.rpcCandidate.breakingChangeCheckRequired, true);
});

test("control-plane operations are provider-neutral and bounded", () => {
  assert.deepEqual(
    contract.candidateOperations.map(({ method }) => method),
    [
      "GetSandboxCapabilities",
      "AcquireSandboxWorkspaceRuntime",
      "GetSandboxOperation",
      "ExecuteSandboxCommand",
      "CancelSandboxCommand",
      "CreateSandboxCheckpoint",
      "ReleaseSandboxWorkspaceRuntime",
      "WatchSandboxOperationEvents",
    ],
  );
  for (const operation of contract.candidateOperations.filter(
    ({ idempotency }) => idempotency !== "read-only" && idempotency !== "resume-cursor",
  )) {
    assert.equal(operation.idempotency, "required");
  }
  assert.ok(!JSON.stringify(contract.candidateOperations).match(/provider|node|slot|host|mount/iu));
});

test("request context is trusted and excludes physical or secret-bearing authority", () => {
  assert.equal(contract.requestBoundary.typedCallerContextRequired, true);
  assert.equal(contract.requestBoundary.serviceIdentityRequired, true);
  assert.equal(contract.requestBoundary.tenantContextServerResolved, true);
  assert.equal(contract.requestBoundary.callerMetadataOverrideAllowed, false);
  for (const forbiddenField of [
    "sandbox_provider_kind",
    "sandbox_node_id",
    "sandbox_host_path",
    "sandbox_lease_token",
    "sandbox_fencing_token",
    "sandbox_provider_allocation_ref",
    "sandbox_secret_value",
  ]) {
    assert.ok(contract.requestBoundary.forbiddenFields.includes(forbiddenField));
  }
});

test("Kernel execution placement and Sandbox capacity placement never share authority", () => {
  const separation = contract.placementSeparation;
  assert.equal(separation.kernelExecutionPlacementOwner, "sdkwork-kernel");
  assert.equal(separation.sandboxCapacityPlacementOwner, "sdkwork-sandbox");
  assert.equal(separation.recordIdentityShared, false);
  assert.equal(separation.leaseOwnerShared, false);
  assert.equal(separation.fencingGenerationShared, false);
  assert.equal(separation.idempotencyScopeShared, false);
  assert.equal(separation.reconcilerShared, false);
  assert.equal(separation.kernelGenerationIsSandboxAuthority, false);
});

test("mutation retry and cancellation fail closed after ambiguity", () => {
  assert.equal(contract.reliability.durableOperationBeforeExternalSideEffectRequired, true);
  assert.equal(contract.reliability.ambiguousResultRequiresOperationLookup, true);
  assert.equal(contract.reliability.blindMutationRetryAllowed, false);
  assert.equal(contract.reliability.databaseTransactionHeldAcrossDependencyCallAllowed, false);
  assert.equal(contract.reliability.unknownOutcomeRequiresReconciliation, true);
  assert.equal(contract.reliability.hedgedMutationAllowed, false);
});

test("operation streaming cannot become an implicit interactive terminal", () => {
  assert.equal(contract.streaming.operationEventsOnly, true);
  assert.equal(contract.streaming.interactiveTerminalAllowed, false);
  assert.equal(contract.streaming.ptyAllowed, false);
  assert.equal(contract.streaming.stdinAllowed, false);
  assert.equal(contract.streaming.resizeAllowed, false);
  assert.equal(contract.streaming.genericPayloadAllowed, false);
  assert.equal(contract.streaming.boundedReplayRequired, true);
  assert.equal(contract.streaming.backpressureRequired, true);
});

test("security compatibility and deployment require real cloud evidence", () => {
  assert.equal(contract.security.productionMutualTlsRequired, true);
  assert.equal(contract.security.leastPrivilegePerOperationRequired, true);
  assert.equal(contract.security.userTokenForwardingAsServiceIdentityAllowed, false);
  assert.equal(contract.compatibility.unsupportedVersionFailsClosed, true);
  assert.equal(contract.compatibility.silentProtocolDowngradeAllowed, false);
  assert.equal(contract.compatibility.weakerAssuranceFallbackAllowed, false);
  assert.equal(contract.deployment.cloudDiscoveryRequired, true);
  assert.equal(contract.deployment.staticCloudEndpointFallbackAllowed, false);
  assert.ok(contract.requiredRealEvidence.length >= 12);
  assert.equal(contract.humanReview.approvedOutcomeRequiredBeforeImplementation, true);
});

test("The declared list surface pins the PAGINATION_SPEC cursor contract", () => {
  const pagination = contract.pagination;
  assert.equal(pagination.mode, "cursor");
  assert.equal(pagination.pageSizeParameter, "page_size");
  assert.equal(pagination.cursorParameter, "cursor");
  assert.equal(pagination.defaultPageSize, 20);
  assert.equal(pagination.maxPageSize, 200);
  assert.equal(pagination.pageSizeOutOfRange, "reject-with-40003-never-clamp");
  assert.equal(pagination.sort, "created_at DESC, sandbox_instance_id DESC");
  assert.deepEqual(pagination.forbiddenAliases, [
    "pageSize",
    "limit",
    "page_no",
    "pageNo",
    "per_page",
    "size",
    "page",
    "offset",
  ]);
  assert.equal(pagination.unknownQueryParameters, "rejected");
});
