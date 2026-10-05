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

const contract = readJson("specs/sandbox-worker.contract.json");

test("Worker launch execution is ready and the contract authorizes the authority-model slice", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.worker-contract");
  // 2026-10-06: REVIEW-20261006 (WRK-01..07) accepted via single-owner
  // structured approval; the authority-model slice landed as
  // crates/sdkwork-intelligence-sandbox-worker-authority, so the contract's
  // implementation gate is open for that slice only. The local-lane
  // execution adapter, Firecracker VMM runtime, warm-slot consumption, CLI,
  // public API/SDK and deployment stay forbidden, and the contract remains
  // draft.
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, true);
  assert.equal(contract["x-sdkwork-status"], "draft");
  assert.equal(contract["x-sdkwork-require-human-review"], true);
  assert.equal(
    readStatus("docs/product/requirements/REQ-2026-0034-sandbox-worker-launch-execution.md"),
    "ready",
  );
  assert.equal(
    readStatus("docs/architecture/decisions/ADR-20261006-sandbox-worker-authority.md"),
    "accepted",
  );
  assert.equal(
    readStatus("docs/engineering/reviews/REVIEW-20261006-sandbox-worker-authority.md"),
    "accepted",
  );
  assert.equal(contract.requirementId, "REQ-2026-0034");
});

test("The landed worker-authority crate implements the pinned states, fields, gates and layering", () => {
  const crateRoot = path.join(repoRoot, "crates/sdkwork-intelligence-sandbox-worker-authority");
  const source = (relative) => readFileSync(path.join(crateRoot, "src", relative), "utf8");
  const stateSource = source("state.rs");
  for (const stateName of contract.execution.states) {
    assert.ok(
      stateSource.includes(`"${stateName}"`),
      `execution state ${stateName} must exist in the crate state machine`,
    );
  }
  const executionSource = source("execution.rs");
  for (const field of contract.execution.requiredFields) {
    assert.ok(
      executionSource.includes(field),
      `execution field ${field} must exist in the crate record`,
    );
  }
  assert.equal(
    executionSource.includes("sandbox_can_transition_to"),
    contract.execution.illegalTransitionRejected === true,
  );
  const gatesSource = source("gates.rs");
  for (const gate of [
    "realWorkerExecutionEvidenceRequired",
    "kvmLaneEvidenceRequired",
    "firstCommandZeroWaitEvidenceRequired",
  ]) {
    assert.equal(contract.evidenceGates[gate], true, `${gate} must stay blocking`);
  }
  for (const layering of [
    ["launchPlanAuthority", contract.layering.launchPlanAuthority],
    ["commandExecutionAuthority", contract.layering.commandExecutionAuthority],
    ["providerSpiBoundary", contract.layering.providerSpiBoundary],
    ["firecrackerProviderAuthority", contract.layering.firecrackerProviderAuthority],
    ["warmMicroVmSlotGate", contract.layering.warmMicroVmSlotGate],
  ]) {
    assert.equal(
      gatesSource.includes(`"${layering[1]}"`),
      true,
      `the ${layering[0]} authority ${layering[1]} must be named by the crate`,
    );
  }
  assert.equal(
    contract.evidenceGates.workerAuthorityModelSliceAuthorized,
    true,
    "the landed authority-model slice is the authorized one",
  );
  assert.equal(contract.evidenceGates.localLaneExecutionSliceAuthorized, false);
  assert.equal(contract.evidenceGates.firecrackerLaneExecutionAuthorized, false);
  const forbiddenFlags = Object.values(contract.forbidden);
  assert.ok(
    forbiddenFlags.length === 5 && forbiddenFlags.every((flag) => flag === true),
    "the contract forbidden block must stay closed",
  );
});

test("The execution record shape is sandbox-prefixed with a closed six-state lifecycle", () => {
  const execution = contract.execution;
  assert.equal(execution.type, "SandboxWorkerExecution");
  assert.equal(execution.immutableAfterTerminalState, true);
  assert.equal(execution.unknownStateRejected, true);
  assert.equal(execution.illegalTransitionRejected, true);
  assert.equal(execution.allSandboxOwnedFieldsRequirePrefix, "sandbox_");
  assert.equal(execution.referencesAreOpaque, true);
  assert.equal(execution.providerPrivateMetadataAllowed, false);
  for (const field of execution.requiredFields) {
    assert.match(field, /^sandbox_/u);
  }
  assert.deepEqual(execution.states, [
    "accepted",
    "provisioning",
    "starting",
    "started",
    "failed",
    "quarantined",
  ]);
  assert.equal(execution.initialState, "accepted");
  assert.deepEqual(execution.terminalStates, ["started", "failed", "quarantined"]);
});

test("The lifecycle is a fixed six-operation set", () => {
  assert.deepEqual(contract.lifecycleOperations, [
    "sandbox_accept_launch",
    "sandbox_record_provisioning",
    "sandbox_record_starting",
    "sandbox_record_started",
    "sandbox_record_execution_failed",
    "sandbox_quarantine_execution",
  ]);
  assert.equal(contract.fixedOperationsOnly, true);
});

test("Binding semantics keep plan handoff single and execution honest", () => {
  const binding = contract.bindingSemantics;
  assert.equal(binding.oneExecutionPerLaunchPlan, true);
  assert.equal(binding.consumedPlanMayNeverBeReaccepted, true);
  assert.equal(binding.acceptanceMovesThePlanViaTheLaunchAuthority, true);
  assert.equal(binding.workerNeverWritesPlanStateDirectly, true);
  assert.equal(binding.startCommandViaCommandExecutorPort, true);
  assert.equal(binding.workerReimplementsNoExecutor, true);
  assert.equal(binding.providerAllocateStartViaProviderSpi, true);
  assert.equal(binding.localLaneOnlyUntilKvmEvidence, true);
  assert.equal(binding.outcomeUncertaintyQuarantines, true);
  assert.equal(binding.silentSuccessAllowed, false);
  assert.equal(binding.silentFailureAllowed, false);
});

test("Real worker, KVM-lane and first-command-zero-wait evidence stay blocking", () => {
  const gates = contract.evidenceGates;
  assert.equal(gates.realWorkerExecutionEvidenceRequired, true);
  assert.equal(gates.kvmLaneEvidenceRequired, true);
  assert.equal(gates.firstCommandZeroWaitEvidenceRequired, true);
  // The authority-model slice is the authorized one; every runtime surface
  // behind it stays closed.
  assert.equal(gates.workerAuthorityModelSliceAuthorized, true);
  assert.equal(gates.localLaneExecutionSliceAuthorized, false);
  assert.equal(gates.firecrackerLaneExecutionAuthorized, false);
});

test("The layering keeps launch, command, provider and pool lines distinct", () => {
  const layering = contract.layering;
  assert.equal(layering.launchPlanAuthority, "REQ-2026-0033");
  assert.equal(layering.acceptanceIsTheOnlyPlanHandoff, true);
  assert.equal(layering.commandExecutionAuthority, "REQ-2026-0007");
  assert.equal(layering.providerSpiBoundary, "REQ-2026-0002");
  assert.equal(layering.firecrackerProviderAuthority, "REQ-2026-0008");
  assert.equal(layering.warmMicroVmSlotGate, "REQ-2026-0019");
  assert.equal(layering.registrationDoesNotEnableWarmSlot, true);
  assert.equal(layering.registrationDoesNotTouchTheVmm, true);
  const pool = readJson("specs/sandbox-runtime-pool.contract.json");
  assert.equal(pool.requirementId, "REQ-2026-0019");
});

test("VMM runtime, warm slot, CLI, API/SDK and deployment stay forbidden", () => {
  assert.deepEqual(contract.forbidden, {
    firecrackerVmmRuntime: true,
    warmSlotConsumption: true,
    cliSurface: true,
    publicApiOrSdkSurface: true,
    deploymentProfile: true,
  });
  assert.equal(contract["x-sdkwork-no-runtime-implementation"], true);
  assert.equal(contract["x-sdkwork-no-storage-backend"], true);
  assert.equal(contract["x-sdkwork-no-api-sdk-deployment"], true);
});
