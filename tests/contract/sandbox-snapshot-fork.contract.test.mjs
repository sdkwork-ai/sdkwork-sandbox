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

const contract = readJson("specs/sandbox-snapshot-fork.contract.json");

test("Snapshot authority stays a draft carrier with implementation unauthorized", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.snapshot-fork-contract");
  // 2026-10-05: REQ-2026-0031 registered as the Snapshot/Fork capability
  // carrier; the naming/review packet is still pending, so nothing may
  // implement behind it and no WarmMicroVmSlot gate moves.
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, false);
  assert.equal(contract["x-sdkwork-status"], "draft");
  assert.equal(contract["x-sdkwork-require-human-review"], true);
  assert.equal(
    readStatus("docs/product/requirements/REQ-2026-0031-sandbox-snapshot-and-fork.md"),
    "draft",
  );
});

test("Snapshot record fields are sandbox-prefixed with a closed immutable state set", () => {
  const snapshot = contract.snapshot;
  assert.equal(snapshot.type, "SandboxSnapshot");
  assert.equal(snapshot.immutableAfterCreation, true);
  assert.equal(snapshot.unknownStateRejected, true);
  assert.equal(snapshot.allSandboxOwnedFieldsRequirePrefix, "sandbox_");
  assert.equal(snapshot.sourceSessionRefIsOpaque, true);
  assert.equal(snapshot.providerPrivateMetadataAllowed, false);
  for (const field of snapshot.requiredFields) {
    assert.match(field, /^sandbox_/u);
  }
  assert.deepEqual(snapshot.states, [
    "creating",
    "available",
    "restoring",
    "deleted",
    "quarantined",
  ]);
});

test("Snapshot lifecycle is a fixed three-operation set with fail-closed deletion", () => {
  assert.deepEqual(contract.lifecycleOperations, [
    "sandbox_create_snapshot",
    "sandbox_list_snapshots",
    "sandbox_delete_snapshot",
  ]);
  assert.equal(contract.fixedOperationsOnly, true);
  assert.equal(contract.deletion.deterministicFailureRequired, true);
  assert.equal(contract.deletion.uncertainDeletionQuarantines, true);
  assert.equal(contract.deletion.silentDisappearanceAllowed, false);
});

test("Fork semantics keep the source immutable and derived sandboxes tenant-fresh", () => {
  const fork = contract.fork;
  assert.equal(fork.type, "SandboxForkDerivation");
  assert.equal(fork.sourceSnapshotImmutable, true);
  assert.equal(fork.derivationsPerSnapshotUnboundedAuthorized, false);
  assert.equal(fork.derivedSandboxGetsFreshGuestIdentity, true);
  assert.equal(fork.derivedSandboxRequiresIndependentTenantGrants, true);
  assert.equal(fork.derivedSandboxRunsInParallelWithSource, true);
  assert.equal(fork.sourceTenantStateReuseAllowed, false);
  assert.equal(fork.callerSelectedNodeOrProviderAllowed, false);
});

test("Every snapshot carries the real KVM evidence gates and no engine authorization", () => {
  const gates = contract.evidenceGates;
  assert.equal(gates.realKvmRestoreEvidenceRequired, true);
  assert.equal(gates.crossTenantResidueEvidenceRequired, true);
  assert.equal(gates.derivedIdentityRotationEvidenceRequired, true);
  assert.equal(gates.warmSlotReuseRequiresPoolEvidenceGate, true);
  assert.equal(gates.snapshotEngineOrStorageBackendAuthorized, false);
});

test("The artifact boundary stays layered on REQ-2026-0012 with one supply-chain authority", () => {
  const boundary = contract.artifactBoundary;
  assert.equal(boundary.artifactAuthority, "REQ-2026-0012");
  assert.equal(boundary.snapshotReferencesArtifactTuple, true);
  assert.equal(boundary.snapshotOwnsEvidenceOrSignature, false);
  assert.equal(boundary.secondSupplyChainAuthorityAllowed, false);
});

test("The layering keeps Checkpoint, Pool and Auto-Pause lines distinct", () => {
  const layering = contract.layering;
  assert.equal(layering.workspaceCheckpointAuthority, "REQ-2026-0021");
  assert.equal(layering.snapshotIsRuntimeFullStateNotWorkspaceCheckpoint, true);
  assert.equal(layering.warmMicroVmSlotGate, "REQ-2026-0019");
  assert.equal(layering.registrationDoesNotEnableWarmSlot, true);
  assert.equal(layering.autoPauseAutoResumeOutOfScope, true);
  const artifact = readJson("specs/sandbox-firecracker-artifact-compatibility.contract.json");
  assert.ok(artifact, "REQ-2026-0012 machine contract must exist");
});

test("Engine, storage, pipeline, CLI, API/SDK and deployment stay forbidden", () => {
  assert.deepEqual(contract.forbidden, {
    snapshotEngineRuntime: true,
    snapshotStorageBackend: true,
    restorePipeline: true,
    cliSurface: true,
    publicApiOrSdkSurface: true,
    deploymentProfile: true,
  });
  assert.equal(contract["x-sdkwork-no-runtime-implementation"], true);
  assert.equal(contract["x-sdkwork-no-storage-backend"], true);
  assert.equal(contract["x-sdkwork-no-api-sdk-deployment"], true);
});
