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

const contract = readJson("specs/sandbox-template-build.contract.json");

test("Template build is ready and the contract authorizes the authority-model slice", () => {
  assert.equal(contract.kind, "sdkwork.sandbox.template-build-contract");
  // 2026-10-05: REVIEW-20261005 (BLD-01..07) accepted via single-owner
  // structured approval; the authority-model slice landed as
  // crates/sdkwork-intelligence-sandbox-build-authority, so the contract's
  // implementation gate is open for that slice only. Builder runtime,
  // pipeline execution, artifact/cache storage, registry service, CLI,
  // public API/SDK and deployment stay forbidden, no WarmMicroVmSlot gate
  // moves, and the contract remains draft.
  assert.equal(contract.status, "draft");
  assert.equal(contract.implementationAuthorized, true);
  assert.equal(contract["x-sdkwork-status"], "draft");
  assert.equal(contract["x-sdkwork-require-human-review"], true);
  assert.equal(
    readStatus("docs/product/requirements/REQ-2026-0032-sandbox-template-build.md"),
    "ready",
  );
  assert.equal(
    readStatus("docs/architecture/decisions/ADR-20261005-sandbox-template-build-authority.md"),
    "accepted",
  );
  assert.equal(
    readStatus("docs/engineering/reviews/REVIEW-20261005-sandbox-template-build-authority.md"),
    "accepted",
  );
  assert.equal(contract.requirementId, "REQ-2026-0032");
});

test("The landed build-authority crate implements the pinned states, fields, gates and layering", () => {
  const crateRoot = path.join(repoRoot, "crates/sdkwork-intelligence-sandbox-build-authority");
  const source = (relative) => readFileSync(path.join(crateRoot, "src", relative), "utf8");
  const stateSource = source("state.rs");
  for (const stateName of contract.build.states) {
    assert.ok(
      stateSource.includes(`"${stateName}"`),
      `build state ${stateName} must exist in the crate state machine`,
    );
  }
  const buildSource = source("build.rs");
  for (const field of contract.build.requiredFields) {
    assert.ok(
      buildSource.includes(field),
      `build field ${field} must exist in the crate record`,
    );
  }
  assert.equal(
    buildSource.includes("sandbox_can_transition_to"),
    contract.build.illegalTransitionRejected === true,
  );
  const gatesSource = source("gates.rs");
  for (const gate of [
    "realBuilderExecutionEvidenceRequired",
    "buildArtifactTupleEvidenceRequired",
  ]) {
    assert.equal(contract.evidenceGates[gate], true, `${gate} must stay blocking`);
  }
  assert.equal(
    gatesSource.includes(`"${contract.artifactBoundary.artifactAuthority}"`),
    true,
    `the artifact authority ${contract.artifactBoundary.artifactAuthority} must be named by the crate`,
  );
  assert.equal(
    gatesSource.includes(`"${contract.layering.templateAuthority}"`),
    true,
    `the template authority ${contract.layering.templateAuthority} must be named by the crate`,
  );
  assert.equal(
    gatesSource.includes(`"${contract.layering.warmMicroVmSlotGate}"`),
    true,
    `the warm-slot gate ${contract.layering.warmMicroVmSlotGate} must be named by the crate`,
  );
  assert.equal(
    contract.evidenceGates.buildAuthorityModelSliceAuthorized,
    true,
    "the landed authority-model slice is the authorized one",
  );
  assert.equal(contract.evidenceGates.builderRuntimeOrPipelineAuthorized, false);
  assert.equal(contract.evidenceGates.registryServiceAuthorized, false);
  const forbiddenFlags = Object.values(contract.forbidden);
  assert.ok(
    forbiddenFlags.length === 7 && forbiddenFlags.every((flag) => flag === true),
    "the contract forbidden block must stay closed",
  );
});

test("The build record shape is sandbox-prefixed with a closed five-state lifecycle", () => {
  const build = contract.build;
  assert.equal(build.type, "SandboxTemplateBuild");
  assert.equal(build.immutableAfterTerminalState, true);
  assert.equal(build.unknownStateRejected, true);
  assert.equal(build.illegalTransitionRejected, true);
  assert.equal(build.allSandboxOwnedFieldsRequirePrefix, "sandbox_");
  assert.equal(build.referencesAreOpaque, true);
  assert.equal(build.providerPrivateMetadataAllowed, false);
  for (const field of build.requiredFields) {
    assert.match(field, /^sandbox_/u);
  }
  assert.deepEqual(build.states, [
    "requested",
    "building",
    "succeeded",
    "failed",
    "quarantined",
  ]);
  assert.equal(build.initialState, "requested");
  assert.deepEqual(build.terminalStates, ["succeeded", "failed", "quarantined"]);
});

test("The lifecycle is a fixed four-operation set and failure never lies", () => {
  assert.deepEqual(contract.lifecycleOperations, [
    "sandbox_request_build",
    "sandbox_record_building",
    "sandbox_record_build_outcome",
    "sandbox_quarantine_build",
  ]);
  assert.equal(contract.fixedOperationsOnly, true);
  assert.equal(contract.failureSemantics.uncertainOutcomeQuarantines, true);
  assert.equal(contract.failureSemantics.silentSuccessAllowed, false);
  assert.equal(contract.failureSemantics.silentFailureAllowed, false);
  assert.equal(contract.failureSemantics.timeoutAloneMaySucceed, false);
});

test("Real builder execution and artifact evidence stay blocking", () => {
  const gates = contract.evidenceGates;
  assert.equal(gates.realBuilderExecutionEvidenceRequired, true);
  assert.equal(gates.buildArtifactTupleEvidenceRequired, true);
  // The authority-model slice is the authorized one; every runtime surface
  // behind it stays closed.
  assert.equal(gates.buildAuthorityModelSliceAuthorized, true);
  assert.equal(gates.builderRuntimeOrPipelineAuthorized, false);
  assert.equal(gates.registryServiceAuthorized, false);
});

test("The artifact boundary stays layered on REQ-2026-0012 with one supply-chain authority", () => {
  const boundary = contract.artifactBoundary;
  assert.equal(boundary.artifactAuthority, "REQ-2026-0012");
  assert.equal(boundary.successRequiresArtifactTupleRef, true);
  assert.equal(boundary.buildReferencesArtifactTuple, true);
  assert.equal(boundary.buildOwnsEvidenceOrSignature, false);
  assert.equal(boundary.secondSupplyChainAuthorityAllowed, false);
  const artifact = readJson("specs/sandbox-firecracker-artifact-compatibility.contract.json");
  assert.ok(artifact, "REQ-2026-0012 machine contract must exist");
});

test("The layering keeps the Template authority, cache semantics and Pool gates distinct", () => {
  const layering = contract.layering;
  assert.equal(layering.templateAuthority, "REQ-2026-0029");
  assert.equal(layering.buildDoesNotRedefineTemplateDefinitionOrBuildInput, true);
  assert.equal(layering.cachePolicyAuthority, "REQ-2026-0029");
  assert.equal(layering.buildDoesNotImplementCacheBackendOrEviction, true);
  assert.equal(layering.warmMicroVmSlotGate, "REQ-2026-0019");
  assert.equal(layering.registrationDoesNotEnableWarmSlot, true);
  assert.equal(layering.registrationDoesNotBindableAnyRealArtifact, true);
  const template = readJson("specs/sandbox-template-authority.contract.json");
  assert.equal(template.requirementId, "REQ-2026-0029");
});

test("Builder runtime, pipeline, storage, registry, CLI, API/SDK and deployment stay forbidden", () => {
  assert.deepEqual(contract.forbidden, {
    builderRuntime: true,
    buildPipelineExecution: true,
    buildArtifactOrCacheStorageBackend: true,
    registryService: true,
    cliSurface: true,
    publicApiOrSdkSurface: true,
    deploymentProfile: true,
  });
  assert.equal(contract["x-sdkwork-no-runtime-implementation"], true);
  assert.equal(contract["x-sdkwork-no-storage-backend"], true);
  assert.equal(contract["x-sdkwork-no-api-sdk-deployment"], true);
});
