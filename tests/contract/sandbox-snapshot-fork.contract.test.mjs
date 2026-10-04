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

test("Snapshot authority is ready and the contract authorizes the authority-model slice", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.snapshot-fork-contract");
  // 2026-10-05: REVIEW-20261005 (SNAP-01..07) accepted via single-owner
  // structured approval; the authority-model slice landed as
  // crates/sdkwork-intelligence-sandbox-snapshot-authority, so the contract's
  // implementation gate is open for that slice only. Engine runtime, storage
  // backend, restore pipeline, CLI, public API/SDK and deployment stay
  // forbidden, no WarmMicroVmSlot gate moves, and the contract remains draft.
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, true);
  assert.equal(contract["x-sdkwork-status"], "draft");
  assert.equal(contract["x-sdkwork-require-human-review"], true);
  assert.equal(
    readStatus("docs/product/requirements/REQ-2026-0031-sandbox-snapshot-and-fork.md"),
    "ready",
  );
  assert.equal(
    readStatus("docs/architecture/decisions/ADR-20261005-sandbox-snapshot-fork-authority.md"),
    "accepted",
  );
  assert.equal(
    readStatus("docs/engineering/reviews/REVIEW-20261005-sandbox-snapshot-fork-authority.md"),
    "accepted",
  );
});

test("The landed snapshot-authority crate implements the pinned states, fields, fork flags and gates", () => {
  const crateRoot = path.join(repoRoot, "crates/sdkwork-intelligence-sandbox-snapshot-authority");
  const source = (relative) => readFileSync(path.join(crateRoot, "src", relative), "utf8");
  const stateSource = source("state.rs");
  for (const stateName of contract.snapshot.states) {
    assert.ok(
      stateSource.includes(`"${stateName}"`),
      `snapshot state ${stateName} must exist in the crate state machine`,
    );
  }
  const snapshotSource = source("snapshot.rs");
  for (const field of contract.snapshot.requiredFields) {
    assert.ok(
      snapshotSource.includes(field),
      `snapshot field ${field} must exist in the crate record`,
    );
  }
  const forkSource = source("fork.rs");
  assert.equal(
    forkSource.includes("SANDBOX_SNAPSHOT_FORK_SOURCE_SNAPSHOT_IMMUTABLE: bool = true"),
    contract.fork.sourceSnapshotImmutable === true,
  );
  assert.equal(
    forkSource.includes("SANDBOX_SNAPSHOT_FORK_PARALLEL_RUNNING: bool = true"),
    contract.fork.derivedSandboxRunsInParallelWithSource === true,
  );
  assert.equal(
    forkSource.includes("SANDBOX_SNAPSHOT_FORK_FRESH_IDENTITY_REQUIRED: bool = true"),
    contract.fork.derivedSandboxGetsFreshGuestIdentity === true,
  );
  const gatesSource = source("gates.rs");
  for (const gate of [
    "realKvmRestoreEvidenceRequired",
    "crossTenantResidueEvidenceRequired",
    "derivedIdentityRotationEvidenceRequired",
    "warmSlotReuseRequiresPoolEvidenceGate",
  ]) {
    assert.equal(contract.evidenceGates[gate], true, `${gate} must stay blocking`);
  }
  assert.equal(
    gatesSource.includes(`"${contract.artifactBoundary.artifactAuthority}"`),
    true,
    `the artifact authority ${contract.artifactBoundary.artifactAuthority} must be named by the crate`,
  );
  const forbiddenFlags = Object.values(contract.forbidden);
  assert.ok(
    forbiddenFlags.length === 6 && forbiddenFlags.every((flag) => flag === true),
    "the contract forbidden block must stay closed",
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
