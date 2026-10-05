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

const contract = readJson("specs/sandbox-instance-fast-start.contract.json");

test("Fast-start launch is a draft carrier with no implementation authorization", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.instance-fast-start-contract");
  // 2026-10-06: the carrier registers the instance fast-start launch
  // capability class as draft. No slice is authorized, no crate exists for
  // it, and every runtime surface stays behind the forbidden block until its
  // own review lands.
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, false);
  assert.equal(contract["x-sdkwork-status"], "draft");
  assert.equal(contract["x-sdkwork-require-human-review"], true);
  assert.equal(
    readStatus("docs/product/requirements/REQ-2026-0033-sandbox-instance-fast-start-launch.md"),
    "draft",
  );
  assert.equal(contract.requirementId, "REQ-2026-0033");
});

test("The launch plan shape is sandbox-prefixed with a closed four-state lifecycle", () => {
  const plan = contract.launchPlan;
  assert.equal(plan.type, "SandboxInstanceLaunchPlan");
  assert.equal(plan.immutableAfterCreation, true);
  assert.equal(plan.unknownStateRejected, true);
  assert.equal(plan.illegalTransitionRejected, true);
  assert.equal(plan.allSandboxOwnedFieldsRequirePrefix, "sandbox_");
  assert.equal(plan.referencesAreOpaque, true);
  assert.equal(plan.providerPrivateMetadataAllowed, false);
  for (const field of plan.requiredFields) {
    assert.match(field, /^sandbox_/u);
  }
  assert.deepEqual(plan.states, ["planned", "consumed", "expired", "quarantined"]);
  assert.equal(plan.initialState, "planned");
  assert.deepEqual(plan.terminalStates, ["consumed", "expired", "quarantined"]);
});

test("The lifecycle is a fixed four-operation set", () => {
  assert.deepEqual(contract.lifecycleOperations, [
    "sandbox_plan_launch",
    "sandbox_mark_launch_consumed",
    "sandbox_expire_launch",
    "sandbox_quarantine_launch",
  ]);
  assert.equal(contract.fixedOperationsOnly, true);
});

test("Binding semantics keep execution fail-closed", () => {
  const binding = contract.bindingSemantics;
  assert.equal(binding.startCommandAuthorityStaysWithTemplateVersion, true);
  assert.equal(binding.startCommandCopiedIntoThePlan, false);
  assert.equal(binding.planRequiresActiveFencedPoolClaim, true);
  assert.equal(binding.expiredOrReleasedClaimMayNeverExecute, true);
  assert.equal(binding.freshGuestIdentityRequiredPerLaunch, true);
  assert.equal(binding.plannedAloneAuthorizesExecution, false);
  assert.equal(binding.bindingUncertaintyQuarantines, true);
  assert.equal(binding.silentExecutionAllowed, false);
});

test("Real fast-start runtime and first-command-zero-wait evidence stay blocking", () => {
  const gates = contract.evidenceGates;
  assert.equal(gates.realFastStartRuntimeEvidenceRequired, true);
  assert.equal(gates.firstCommandZeroWaitEvidenceRequired, true);
  assert.equal(gates.launchAuthorityModelSliceAuthorized, false);
  assert.equal(gates.launchExecutionOrWorkerAuthorized, false);
});

test("The layering keeps template, build, pool, command and snapshot lines distinct", () => {
  const layering = contract.layering;
  assert.equal(layering.templateAuthority, "REQ-2026-0029");
  assert.equal(layering.buildAuthority, "REQ-2026-0032");
  assert.equal(layering.poolClaimAuthority, "REQ-2026-0019");
  assert.equal(layering.commandExecutionAuthority, "REQ-2026-0007");
  assert.equal(layering.snapshotForkAuthority, "REQ-2026-0031");
  assert.equal(layering.snapshotDerivedInstancesPlanThroughThisAuthorityToo, true);
  assert.equal(layering.warmMicroVmSlotGate, "REQ-2026-0019");
  assert.equal(layering.registrationDoesNotEnableWarmSlot, true);
  assert.equal(layering.registrationDoesNotExecuteCommands, true);
  const pool = readJson("specs/sandbox-runtime-pool.contract.json");
  assert.equal(pool.requirementId, "REQ-2026-0019");
});

test("Launch execution runtime, worker, CLI, API/SDK and deployment stay forbidden", () => {
  assert.deepEqual(contract.forbidden, {
    launchExecutionRuntime: true,
    workerRuntime: true,
    cliSurface: true,
    publicApiOrSdkSurface: true,
    deploymentProfile: true,
  });
  assert.equal(contract["x-sdkwork-no-runtime-implementation"], true);
  assert.equal(contract["x-sdkwork-no-storage-backend"], true);
  assert.equal(contract["x-sdkwork-no-api-sdk-deployment"], true);
});
