#!/usr/bin/env node

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  HOST_CAPABILITY_IDS,
  buildHostCapabilityProbeScript,
} from "./testing/sandbox-host-capability-evidence.mjs";

/**
 * Real-evidence traceability gate.
 *
 * The Gate 0 contracts in `specs/` name the evidence they require, and the repository previously had
 * nothing that declared which of those requirements anything can actually produce. A required piece of
 * evidence with no producer does not fail any test; it simply never happens.
 *
 * This gate makes that failure mode impossible to reach silently. It recomputes the required evidence
 * ids from the live contracts and fails when:
 *
 *   - a contract requires an evidence id the registry does not snapshot, or the registry snapshots an
 *     id no contract requires any more;
 *   - a contract starts or stops declaring evidence requirements without a registry update;
 *   - the registry's acknowledged counts no longer match the live contracts;
 *   - the registry claims host-precondition coverage for an evidence id its contract does not require;
 *   - the registry claims a capability witness that `HOST_CAPABILITY_IDS` does not declare, or that the
 *     probe script does not actually emit.
 *
 * Passing this gate is a traceability statement, not evidence. It does not authorize a Provider, Port,
 * API route, SDK, scheduler, isolation policy, deployable profile or runtime dependency change, and it
 * does not convert any host fact into conformance or commercial readiness.
 */

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const specsDirectory = resolve(repositoryRoot, "specs");
const registryPath = join(specsDirectory, "sandbox-real-evidence-registry.json");
const REGISTRY_KIND = "sdkwork.sandbox.real-evidence-producer-registry";
const EVIDENCE_KEY = /requiredevidence|required_evidence|requiredrealevidence/iu;
const EVIDENCE_ID = /^[a-z0-9][a-z0-9._-]*$/u;

function fail(message) {
  throw new Error(message);
}

export function parseEvidenceTraceabilityArgs(argv) {
  const options = { json: false };
  for (const argument of argv) {
    if (argument === "--json") {
      options.json = true;
    } else {
      fail(`unsupported argument: ${argument}`);
    }
  }
  return options;
}

export function collectRequiredEvidence(contract) {
  const ids = new Set();
  const absorb = (value) => {
    if (Array.isArray(value)) {
      if (value.every((entry) => typeof entry === "string")) {
        for (const entry of value) {
          if (EVIDENCE_ID.test(entry)) {
            ids.add(entry);
          }
        }
      } else {
        for (const entry of value) {
          walk(entry);
        }
      }
      return;
    }
    if (value && typeof value === "object") {
      for (const entry of Object.values(value)) {
        absorb(entry);
      }
    }
  };
  const walk = (node) => {
    if (Array.isArray(node)) {
      for (const entry of node) {
        walk(entry);
      }
      return;
    }
    if (node && typeof node === "object") {
      for (const [key, value] of Object.entries(node)) {
        if (EVIDENCE_KEY.test(key)) {
          absorb(value);
        }
        walk(value);
      }
    }
  };
  walk(contract);
  return [...ids].sort();
}

export function readRepositoryContracts({ directory = specsDirectory } = {}) {
  const contracts = [];
  for (const file of readdirSync(directory).filter((name) => name.endsWith(".json")).sort()) {
    const value = JSON.parse(readFileSync(join(directory, file), "utf8"));
    if (value?.kind === REGISTRY_KIND) {
      continue;
    }
    contracts.push({ file, value });
  }
  return contracts;
}

export function readEvidenceRegistry({ file = registryPath } = {}) {
  const registry = JSON.parse(readFileSync(file, "utf8"));
  if (registry?.kind !== REGISTRY_KIND) {
    fail(`evidence registry kind must be ${REGISTRY_KIND}`);
  }
  if (registry.implementationAuthorized !== false) {
    fail("evidence registry must not authorize implementation");
  }
  if (!Array.isArray(registry.hostPreconditionEvidence)) {
    fail("evidence registry must declare hostPreconditionEvidence");
  }
  return registry;
}

/**
 * A parameterised capability family (`toolchain.unshare`, `namespace.mount-userns`) is emitted through a
 * `for <var> in <list>` loop rather than as a literal id, so a literal substring check is not enough.
 * Require the loop to name the member and to emit `<family>.$<var>`.
 */
export function probeEmitsParametrisedId(id, scripts) {
  const index = id.lastIndexOf(".");
  if (index <= 0) {
    return false;
  }
  const family = id.slice(0, index);
  const member = id.slice(index + 1);
  for (const match of scripts.matchAll(/for ([A-Za-z_][A-Za-z0-9_]*) in ([^;\n]+); do([^\n]*)/gu)) {
    const [, variable, list, body] = match;
    if (!body.includes(`"${family}.$${variable}"`)) {
      continue;
    }
    if (list.trim().split(/\s+/u).includes(member)) {
      return true;
    }
  }
  return false;
}

export function assessEvidenceTraceability({
  contracts,
  registry,
  capabilityIds = HOST_CAPABILITY_IDS,
  probeScripts = [
    buildHostCapabilityProbeScript(),
    buildHostCapabilityProbeScript({ probeWrite: true }),
  ],
} = {}) {
  if (!Array.isArray(contracts) || contracts.length === 0) {
    fail("evidence traceability requires at least one contract");
  }
  const failures = [];
  const live = {};
  for (const { file, value } of contracts) {
    const ids = collectRequiredEvidence(value);
    if (ids.length > 0) {
      live[file] = ids;
    }
  }
  const snapshot = registry.evidenceRequirementSnapshot ?? {};

  for (const file of Object.keys(snapshot).sort()) {
    const expected = new Set(snapshot[file]);
    const actual = live[file];
    if (!actual) {
      failures.push(
        `registry snapshots ${file} but that contract no longer declares any evidence requirement`,
      );
      continue;
    }
    const missing = actual.filter((id) => !expected.has(id));
    const stale = [...expected].filter((id) => !actual.includes(id)).sort();
    if (missing.length > 0) {
      failures.push(
        `${file}: ${missing.length} required evidence id(s) absent from the registry snapshot: ${missing.join(", ")}`,
      );
    }
    if (stale.length > 0) {
      failures.push(
        `${file}: ${stale.length} registry snapshot id(s) no longer required: ${stale.join(", ")}`,
      );
    }
  }
  for (const file of Object.keys(live).sort()) {
    if (!snapshot[file]) {
      failures.push(
        `${file}: ${live[file].length} evidence id(s) required by a contract the registry snapshot omits`,
      );
    }
  }

  const distinct = [...new Set(Object.values(live).flat())].sort();
  const occurrences = Object.values(live).reduce((count, ids) => count + ids.length, 0);
  const acknowledged = registry.acknowledged ?? {};
  const acknowledgedChecks = [
    ["sandbox_total_distinct_evidence_ids", distinct.length],
    ["sandbox_total_occurrences", occurrences],
    ["sandbox_contracts_with_requirements", Object.keys(live).length],
    ["sandbox_host_precondition_partial", registry.hostPreconditionEvidence.length],
  ];
  for (const [key, actual] of acknowledgedChecks) {
    if (acknowledged[key] !== actual) {
      failures.push(
        `registry ${key} is ${acknowledged[key]} but the live contracts yield ${actual}; update the registry deliberately`,
      );
    }
  }
  if (acknowledged.sandbox_fully_gated !== distinct.length - registry.hostPreconditionEvidence.length) {
    failures.push(
      `registry sandbox_fully_gated is ${acknowledged.sandbox_fully_gated} but ${distinct.length - registry.hostPreconditionEvidence.length} ids have no host-precondition producer`,
    );
  }

  const capabilitySet = new Set(capabilityIds);
  const scripts = probeScripts.join("\n");
  const hostProduced = new Set();
  for (const entry of registry.hostPreconditionEvidence) {
    const requiredBy = entry.requiredBy ?? [];
    if (requiredBy.length === 0) {
      failures.push(`host precondition ${entry.evidenceId} must name the contracts that require it`);
    }
    for (const file of requiredBy) {
      if (!live[file]?.includes(entry.evidenceId)) {
        failures.push(
          `host precondition ${entry.evidenceId} claims ${file} requires it, but that contract does not`,
        );
      }
    }
    if (entry.producerClass !== "host-precondition-facts") {
      failures.push(
        `host precondition ${entry.evidenceId} must use producerClass host-precondition-facts`,
      );
    }
    if (!Array.isArray(entry.witnesses) || entry.witnesses.length === 0) {
      failures.push(`host precondition ${entry.evidenceId} must declare the capability ids it witnesses`);
    }
    for (const id of entry.witnesses ?? []) {
      if (!capabilitySet.has(id)) {
        failures.push(
          `host precondition ${entry.evidenceId} claims witness ${id}, which HOST_CAPABILITY_IDS does not declare`,
        );
        continue;
      }
      if (!scripts.includes(id) && !probeEmitsParametrisedId(id, scripts)) {
        failures.push(
          `host precondition ${entry.evidenceId} claims witness ${id}, which the probe script never emits`,
        );
      }
    }
    hostProduced.add(entry.evidenceId);
  }
  for (const id of hostProduced) {
    if (!distinct.includes(id)) {
      failures.push(`host precondition ${id} is not required by any live contract`);
    }
  }

  for (const entry of Object.entries(registry.producerClasses ?? {})) {
    const [name, value] = entry;
    if (value?.satisfiesEvidence !== false) {
      failures.push(`producer class ${name} must declare satisfiesEvidence: false`);
    }
  }
  for (const key of [
    "sandbox_missing_producer_is_ready",
    "sandbox_registry_completeness_is_implementation_authority",
    "sandbox_host_probe_output_is_conformance_evidence",
  ]) {
    if (registry.readiness?.[key] !== false) {
      failures.push(`registry readiness.${key} must be false`);
    }
  }
  if (registry.humanReview?.required !== true) {
    failures.push("registry must require human review");
  }

  const gated = distinct.filter((id) => !hostProduced.has(id));
  return {
    ok: failures.length === 0,
    failures,
    snapshot: {
      sandbox_distinct_evidence_ids: distinct.length,
      sandbox_occurrences: occurrences,
      sandbox_contracts: Object.keys(live).length,
    },
    hostPrecondition: {
      sandbox_evidence_ids_with_partial_producer: hostProduced.size,
      sandbox_produced_by: registry.hostPreconditionEvidence.map((entry) => entry.producedBy)[0] ?? null,
    },
    gated: {
      sandbox_count: gated.length,
      sandbox_evidence_ids: gated,
    },
  };
}

export function formatEvidenceTraceabilityReport(assessment) {
  const lines = [
    "SDKWork Sandbox real-evidence traceability",
    `contracts declaring evidence requirements: ${assessment.snapshot.sandbox_contracts}`,
    `distinct required evidence ids: ${assessment.snapshot.sandbox_distinct_evidence_ids} (${assessment.snapshot.sandbox_occurrences} occurrences)`,
    `evidence ids with a host-precondition producer: ${assessment.snapshot.sandbox_distinct_evidence_ids === 0 ? 0 : assessment.hostPrecondition.sandbox_evidence_ids_with_partial_producer}`,
    `evidence ids still fully gated by a real runner or human review: ${assessment.gated.sandbox_count}`,
    "",
  ];
  if (assessment.ok) {
    lines.push(
      "traceability: consistent (this is a traceability statement, not evidence and not an implementation authority)",
    );
  } else {
    lines.push(`traceability: FAILED (${assessment.failures.length})`);
    for (const failure of assessment.failures) {
      lines.push(`- ${failure}`);
    }
  }
  return `${lines.join("\n")}\n`;
}

function isMainModule() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMainModule()) {
  try {
    const options = parseEvidenceTraceabilityArgs(process.argv.slice(2));
    const registry = readEvidenceRegistry();
    const assessment = assessEvidenceTraceability({
      contracts: readRepositoryContracts(),
      registry,
    });
    process.stdout.write(
      options.json
        ? `${JSON.stringify(assessment, null, 2)}\n`
        : formatEvidenceTraceabilityReport(assessment),
    );
    if (!assessment.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(`sandbox evidence traceability check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
