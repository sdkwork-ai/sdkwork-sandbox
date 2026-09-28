import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const ledgerPath = path.join(repoRoot, "apis/internal-api/intelligence/sandbox-e2b-parity-ledger.json");
const authorityPath = path.join(
  repoRoot,
  "apis/internal-api/intelligence/sandbox-internal-api-authority.openapi.json",
);
const baseline = JSON.parse(
  readFileSync(path.join(repoRoot, "specs/sandbox-e2b-capability-baseline.json"), "utf8"),
);

test("the parity ledger maps every pinned E2B operation exactly once", () => {
  const ledger = JSON.parse(readFileSync(ledgerPath, "utf8"));
  const pinned = baseline.operationCoverage.operations;
  assert.equal(ledger.entries.length, pinned.length);
  assert.deepEqual(
    ledger.entries.map((entry) => entry.operationId).sort(),
    [...pinned].sort(),
  );
  assert.equal(new Set(ledger.entries.map((entry) => entry.operationId)).size, pinned.length);
  assert.equal(ledger.summary.e2bOperations, pinned.length);
  assert.equal(
    ledger.summary.mapped + ledger.summary.pendingGate,
    pinned.length,
    "no operation may be silent: mapped plus pending-gate covers everything",
  );
});

test("every mapped ledger entry points at a real authority operation", () => {
  const ledger = JSON.parse(readFileSync(ledgerPath, "utf8"));
  const authority = JSON.parse(readFileSync(authorityPath, "utf8"));
  for (const entry of ledger.entries.filter((item) => item.status === "mapped")) {
    assert.ok(entry.e2bReference?.path, `${entry.operationId} needs its E2B reference`);
    assert.ok(entry.authority, `${entry.operationId} needs an authority mapping`);
    const pathItem = authority.paths[entry.authority.path];
    assert.ok(pathItem, `authority path ${entry.authority.path} must exist`);
    assert.ok(
      pathItem[entry.authority.method.toLowerCase()],
      `authority ${entry.authority.method} ${entry.authority.path} must exist`,
    );
    assert.equal(
      pathItem[entry.authority.method.toLowerCase()].operationId,
      entry.authority.operationId,
    );
  }
  // The implemented surface is exactly the four mapped operations of slice v0.
  assert.deepEqual(
    ledger.entries.filter((entry) => entry.status === "mapped").map((entry) => entry.operationId).sort(),
    ["deleteSandbox", "getSandbox", "postSandboxTimeout", "postSandboxes"],
  );
});

test("the authority declares int64-as-string and the internal-only security scheme", () => {
  const authority = JSON.parse(readFileSync(authorityPath, "utf8"));
  assert.equal(authority.components.securitySchemes.ingressToken.name, "X-SDKWork-Ingress-Token");
  const version = authority.components.schemas.SandboxVersion;
  assert.equal(version.type, "string");
  assert.equal(version.format, "int64");
  assert.equal(version["x-sdkwork-int64-string"], true);
  for (const [, pathItem] of Object.entries(authority.paths)) {
    for (const [method, operation] of Object.entries(pathItem)) {
      if (method === "parameters" || !operation["x-e2b-reference"]) continue;
      assert.ok(operation["x-e2b-reference"].operationId, `${method} needs an E2B reference`);
    }
  }
});

test("the ledger is reproducible from the pinned baseline (AC8 regression)", () => {
  execFileSync(process.execPath, ["tools/generate-sandbox-e2b-parity-ledger.mjs", "--check"], {
    cwd: repoRoot,
    stdio: "pipe",
  });
});
