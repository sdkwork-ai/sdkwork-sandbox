#!/usr/bin/env node
/// Derives the two-way E2B parity ledger for the sandbox internal-api authority
/// from the pinned capability baseline (REQ-2026-0028 AC2/AC8). Every E2B
/// operation appears exactly once: `mapped` operations carry the authority
/// route this repository owns today; `pending-gate` operations carry their
/// baseline category and stay silent about nothing. Deterministic: same
/// baseline in, byte-identical ledger out.
///
/// Usage: node tools/generate-sandbox-e2b-parity-ledger.mjs [--check]

import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const baselinePath = path.join(repoRoot, "specs/sandbox-e2b-capability-baseline.json");
const ledgerPath = path.join(repoRoot, "apis/internal-api/intelligence/sandbox-e2b-parity-ledger.json");

/// The authority surface this repository owns today (REQ-2026-0028 ready
/// slice v0): the internal-api instance registry, E2B-referenced.
const IMPLEMENTED_MAPPINGS = {
  postSandboxes: {
    e2bPath: "POST /sandboxes",
    authority: {
      method: "POST",
      path: "/sandbox_instances",
      operationId: "createSandboxInstance",
    },
  },
  getSandbox: {
    e2bPath: "GET /sandboxes/{sandboxID}",
    authority: {
      method: "GET",
      path: "/sandbox_instances/{sandboxInstanceId}",
      operationId: "retrieveSandboxInstance",
    },
  },
  postSandboxTimeout: {
    e2bPath: "POST /sandboxes/{sandboxID}/timeout",
    authority: {
      method: "PATCH",
      path: "/sandbox_instances/{sandboxInstanceId}",
      operationId: "updateSandboxInstance",
    },
  },
  deleteSandbox: {
    e2bPath: "DELETE /sandboxes/{sandboxID}",
    authority: {
      method: "DELETE",
      path: "/sandbox_instances/{sandboxInstanceId}",
      operationId: "deleteSandboxInstance",
    },
  },
};

const baseline = JSON.parse(readFileSync(baselinePath, "utf8"));
const operations = baseline.operationCoverage.operations;

const operationCategory = new Map();
for (const row of baseline.rows) {
  for (const field of row.e2bFields) {
    for (const operation of operations) {
      if (field.includes(`[${operation}]`)) {
        if (!operationCategory.has(operation)) {
          operationCategory.set(operation, row.category);
        }
      }
    }
  }
}

const entries = operations.map((operation) => {
  const implemented = IMPLEMENTED_MAPPINGS[operation];
  if (implemented) {
    return {
      operationId: operation,
      status: "mapped",
      e2bReference: { path: implemented.e2bPath },
      authority: implemented.authority,
      category: operationCategory.get(operation) ?? null,
    };
  }
  return {
    operationId: operation,
    status: "pending-gate",
    category: operationCategory.get(operation) ?? null,
  };
});

entries.sort((left, right) => left.operationId.localeCompare(right.operationId));

const ledger = {
  schemaVersion: "1.0",
  kind: "sdkwork.sandbox.e2b-parity-ledger",
  requirementId: "REQ-2026-0028",
  baselineSource: {
    id: baseline.operationCoverage.sourceId,
    capturedAt: baseline.capturedAt,
    documentedOperations: baseline.operationCoverage.documentedOperations,
  },
  authoritySurface: "apis/internal-api/intelligence/sandbox-internal-api-authority.openapi.json",
  summary: {
    e2bOperations: operations.length,
    mapped: entries.filter((entry) => entry.status === "mapped").length,
    pendingGate: entries.filter((entry) => entry.status === "pending-gate").length,
  },
  entries,
};

if (process.argv.includes("--check")) {
  const current = readFileSync(ledgerPath, "utf8");
  if (current !== `${JSON.stringify(ledger, null, 2)}\n`) {
    console.error("sandbox E2B parity ledger is stale: re-run without --check");
    process.exit(1);
  }
  console.log(
    `sandbox E2B parity ledger is reproducible: ${operations.length} operation(s), ${ledger.summary.mapped} mapped, ${ledger.summary.pendingGate} pending-gate`,
  );
} else {
  writeFileSync(ledgerPath, `${JSON.stringify(ledger, null, 2)}\n`);
  console.log(
    `sandbox E2B parity ledger written: ${operations.length} operation(s), ${ledger.summary.mapped} mapped, ${ledger.summary.pendingGate} pending-gate`,
  );
}
