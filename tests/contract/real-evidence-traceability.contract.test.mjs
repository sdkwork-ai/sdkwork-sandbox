import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  assessEvidenceTraceability,
  collectRequiredEvidence,
  probeEmitsParametrisedId,
  readEvidenceRegistry,
  readRepositoryContracts,
} from "../../tools/check-sandbox-evidence-traceability.mjs";
import { HOST_CAPABILITY_IDS } from "../../tools/testing/sandbox-host-capability-evidence.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const registryPath = path.join(repoRoot, "specs/sandbox-real-evidence-registry.json");

const contracts = () => readRepositoryContracts();
const registry = () => readEvidenceRegistry();
const assess = (overrides = {}) =>
  assessEvidenceTraceability({
    contracts: contracts(),
    registry: registry(),
    ...overrides,
  });

test("the live repository evidence traceability is consistent", () => {
  const assessment = assess();

  assert.equal(assessment.ok, true, assessment.failures.join("\n"));
  assert.equal(assessment.snapshot.sandbox_contracts, 8);
  assert.equal(assessment.snapshot.sandbox_distinct_evidence_ids, 127);
  assert.equal(assessment.snapshot.sandbox_distinct_evidence_ids, assessment.snapshot.sandbox_occurrences);
  assert.equal(assessment.hostPrecondition.sandbox_evidence_ids_with_partial_producer, 2);
  assert.equal(assessment.gated.sandbox_count, 125);
  assert.equal(
    assessment.gated.sandbox_count + assessment.hostPrecondition.sandbox_evidence_ids_with_partial_producer,
    assessment.snapshot.sandbox_distinct_evidence_ids,
  );
});

test("the gate must never be readable as evidence or as an implementation authority", () => {
  const value = registry();

  assert.equal(value.implementationAuthorized, false);
  assert.equal(value.readiness.sandbox_missing_producer_is_ready, false);
  assert.equal(value.readiness.sandbox_registry_completeness_is_implementation_authority, false);
  assert.equal(value.readiness.sandbox_host_probe_output_is_conformance_evidence, false);
  assert.equal(value.readiness.sandbox_snapshot_drift_requires_human_update, true);
  assert.equal(value.humanReview.required, true);
  assert.equal(value.humanReview.approvedOutcomeRequiredBeforeImplementation, true);
  assert.deepEqual(value.requirementIds, [
    "REQ-2026-0003",
    "REQ-2026-0007",
    "REQ-2026-0008",
    "REQ-2026-0011",
    "REQ-2026-0012",
    "REQ-2026-0013",
    "REQ-2026-0014",
    "REQ-2026-0015",
    "REQ-2026-0016",
    "REQ-2026-0017",
    "REQ-2026-0018",
  ]);
});

test("the evidence registry never declares evidence requirements of its own", () => {
  const source = JSON.parse(readFileSync(registryPath, "utf8"));

  assert.deepEqual(collectRequiredEvidence(source), []);
});

test("a contract evidence requirement missing from the registry snapshot fails", () => {
  const mutated = registry();
  mutated.evidenceRequirementSnapshot["sandbox-provider-delivery-gates.contract.json"] =
    mutated.evidenceRequirementSnapshot["sandbox-provider-delivery-gates.contract.json"].filter(
      (id) => id !== "sandbox_real_host_runner_matrix",
    );

  const assessment = assess({ registry: mutated });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("sandbox_real_host_runner_matrix")),
    assessment.failures.join("\n"),
  );
});

test("a stale registry snapshot id that no contract requires fails", () => {
  const mutated = registry();
  mutated.evidenceRequirementSnapshot["sandbox-internal-control-plane.contract.json"] = [
    ...mutated.evidenceRequirementSnapshot["sandbox-internal-control-plane.contract.json"],
    "sandbox_internal_control_plane_retired_evidence",
  ];

  const assessment = assess({ registry: mutated });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("no longer required")),
    assessment.failures.join("\n"),
  );
});

test("a contract that starts requiring evidence fails until the registry snapshots it", () => {
  const mutatedContracts = contracts().map((entry) =>
    entry.file === "sandbox-local-provider-host-boundary.contract.json"
      ? {
          ...entry,
          value: {
            ...entry.value,
            requiredRealEvidence: {
              ...entry.value.requiredRealEvidence,
              sandbox_common: [
                ...entry.value.requiredRealEvidence.sandbox_common,
                "brand-new-unproduced-requirement",
              ],
            },
          },
        }
      : entry,
  );

  const assessment = assess({ contracts: mutatedContracts });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("brand-new-unproduced-requirement")),
    assessment.failures.join("\n"),
  );
});

test("acknowledged counts must keep matching the live contracts", () => {
  for (const key of [
    "sandbox_total_distinct_evidence_ids",
    "sandbox_total_occurrences",
    "sandbox_contracts_with_requirements",
    "sandbox_host_precondition_partial",
    "sandbox_fully_gated",
  ]) {
    const mutated = registry();
    mutated.acknowledged[key] = mutated.acknowledged[key] + 1;
    const assessment = assess({ registry: mutated });
    assert.equal(assessment.ok, false, `${key} drift must fail the gate`);
  }
});

test("a host precondition witness outside the declared capability set fails", () => {
  const mutated = registry();
  mutated.hostPreconditionEvidence[0].witnesses = [
    ...mutated.hostPreconditionEvidence[0].witnesses,
    "fabricated.undeclared-capability",
  ];

  const assessment = assess({ registry: mutated });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("which HOST_CAPABILITY_IDS does not declare")),
    assessment.failures.join("\n"),
  );
});

test("a host precondition witness the probe never emits fails", () => {
  const mutated = registry();
  mutated.hostPreconditionEvidence[0].witnesses = ["fabricated.declared-but-not-emitted"];

  const assessment = assess({
    registry: mutated,
    capabilityIds: [...HOST_CAPABILITY_IDS, "fabricated.declared-but-not-emitted"],
  });

  assert.equal(assessment.ok, false);
  assert.ok(
    assessment.failures.some((line) => line.includes("the probe script never emits")),
    assessment.failures.join("\n"),
  );
});

test("a host precondition claim against a contract that does not require it fails", () => {
  for (const mutation of [
    (entry) => ({ ...entry, evidenceId: "sandbox_cloud_primary_region_and_storage_tuple" }),
    (entry) => ({ ...entry, requiredBy: ["sandbox-cloud-data-residency.contract.json"] }),
  ]) {
    const mutated = registry();
    mutated.hostPreconditionEvidence[0] = mutation(mutated.hostPreconditionEvidence[0]);
    const assessment = assess({ registry: mutated });
    assert.equal(assessment.ok, false);
  }
});

test("every declared host precondition witness is really emitted by the probe", () => {
  const assessment = assess();

  assert.equal(assessment.ok, true, assessment.failures.join("\n"));
  const witnesses = registry().hostPreconditionEvidence.flatMap((entry) => entry.witnesses);
  assert.ok(witnesses.length > 0);
  for (const id of witnesses) {
    assert.ok(HOST_CAPABILITY_IDS.includes(id), `${id} must be declared`);
  }
});

test("parameterised capability loops are recognised only when the loop really emits the family", () => {
  const scripts = 'for tool in unshare capsh; do emit "toolchain.$tool" ok x; done\n';

  assert.equal(probeEmitsParametrisedId("toolchain.unshare", scripts), true);
  assert.equal(probeEmitsParametrisedId("toolchain.capsh", scripts), true);
  assert.equal(probeEmitsParametrisedId("toolchain.firecracker", scripts), false);
  assert.equal(probeEmitsParametrisedId("namespace.mount", scripts), false);
  assert.equal(probeEmitsParametrisedId("toolchain", scripts), false);
});

test("producer classes and readiness must stay fail-closed", () => {
  const openClass = registry();
  openClass.producerClasses["host-precondition-facts"].satisfiesEvidence = true;
  assert.equal(assess({ registry: openClass }).ok, false);

  for (const key of [
    "sandbox_missing_producer_is_ready",
    "sandbox_registry_completeness_is_implementation_authority",
    "sandbox_host_probe_output_is_conformance_evidence",
  ]) {
    const openReadiness = registry();
    openReadiness.readiness[key] = true;
    assert.equal(assess({ registry: openReadiness }).ok, false, `${key} must stay false`);
  }

  const noReview = registry();
  noReview.humanReview.required = false;
  assert.equal(assess({ registry: noReview }).ok, false);
});

test("the registry rejects a wrong kind and a non-false authorization flag", () => {
  const directory = mkdtempSync(path.join(tmpdir(), "sdkwork-evidence-registry-"));
  const wrongKind = path.join(directory, "wrong-kind.json");
  const authorized = path.join(directory, "authorized.json");
  const valid = JSON.parse(readFileSync(registryPath, "utf8"));

  writeFileSync(wrongKind, `${JSON.stringify({ ...valid, kind: "sdkwork.sandbox.something-else" })}\n`, "utf8");
  writeFileSync(authorized, `${JSON.stringify({ ...valid, implementationAuthorized: true })}\n`, "utf8");

  assert.throws(() => readEvidenceRegistry({ file: wrongKind }), /kind must be/u);
  assert.throws(() => readEvidenceRegistry({ file: authorized }), /must not authorize implementation/u);
  assert.equal(readEvidenceRegistry({ file: registryPath }).kind, "sdkwork.sandbox.real-evidence-producer-registry");

  const noHostEvidence = path.join(directory, "no-host-evidence.json");
  const { hostPreconditionEvidence, ...rest } = valid;
  writeFileSync(noHostEvidence, `${JSON.stringify(rest)}\n`, "utf8");
  assert.throws(() => readEvidenceRegistry({ file: noHostEvidence }), /must declare hostPreconditionEvidence/u);

  rmSync(directory, { recursive: true, force: true });
});
