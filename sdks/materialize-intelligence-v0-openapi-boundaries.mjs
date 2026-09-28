#!/usr/bin/env node
/// Deterministic authority materialization for the
/// sdkwork-intelligence-internal-sdk family (REQ-2026-0028 slice v0).
///
/// Source of truth: apis/internal-api/intelligence/sandbox-internal-api-authority.openapi.json
/// Outputs (re-running without contract changes is byte-identical):
///   sdks/sdkwork-intelligence-internal-sdk/openapi/sdkwork-intelligence-internal-api.openapi.yaml
///   sdks/sdkwork-intelligence-internal-sdk/openapi/sdkwork-intelligence-internal-api.sdkgen.yaml
///
/// The materializer fails closed instead of guessing when the authority
/// violates the SDK_WORKSPACE_GENERATION_SPEC invariants it can check
/// statically: every operation must carry x-sdkwork-request-context and
/// x-sdkwork-api-surface, no current-tenant selector may appear, and the
/// forbidden pagination aliases must stay absent.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
// js-yaml is resolved from the canonical SDK generator's own dependency tree,
// so the materializer never adds a second YAML dependency to this repository.
const { createRequire } = await import("node:module");
const require = createRequire(
  path.join(repoRoot, "../sdkwork-sdk-generator/node_modules/js-yaml/package.json"),
);
const { dump: stringifyYaml } = require("js-yaml");

const authorityPath = path.join(
  repoRoot,
  "apis/internal-api/intelligence/sandbox-internal-api-authority.openapi.json",
);
const familyRoot = path.join(repoRoot, "sdks/sdkwork-intelligence-internal-sdk");

const authority = JSON.parse(readFileSync(authorityPath, "utf8"));

const FORBIDDEN_ALIASES = new Set([
  "pageSize", "limit", "page_no", "pageNo", "per_page", "size", "page", "offset",
]);
const TENANT_SELECTORS = /^(tenant_id|tenantId|tenant|tenant-id)$/iu;

const problems = [];
for (const [routePath, pathItem] of Object.entries(authority.paths ?? {})) {
  for (const [method, operation] of Object.entries(pathItem)) {
    if (method === "parameters" || typeof operation !== "object") continue;
    if (operation["x-sdkwork-request-context"] !== "WebRequestContext") {
      problems.push(`${method.toUpperCase()} ${routePath}: missing x-sdkwork-request-context: WebRequestContext`);
    }
    if (!operation["x-sdkwork-api-surface"]) {
      problems.push(`${method.toUpperCase()} ${routePath}: missing x-sdkwork-api-surface`);
    }
    if (TENANT_SELECTORS.test(routePath)) {
      problems.push(`${method.toUpperCase()} ${routePath}: current-tenant selector in path`);
    }
    for (const parameter of operation.parameters ?? []) {
      const name = typeof parameter === "string" ? parameter : parameter.name;
      if (name && TENANT_SELECTORS.test(name)) {
        problems.push(`${method.toUpperCase()} ${routePath}: current-tenant selector parameter ${name}`);
      }
      if (FORBIDDEN_ALIASES.has(name)) {
        problems.push(`${method.toUpperCase()} ${routePath}: forbidden pagination alias ${name}`);
      }
    }
  }
}
if (problems.length > 0) {
  console.error("authority materialization refused:");
  for (const problem of problems) console.error(`  - ${problem}`);
  process.exit(1);
}

// The derived generator input equals the materialized authority for slice v0:
// generator-specific normalization belongs here when a language quirk needs
// it, and the derived file must never introduce operations, schemas,
// security, or paths absent from the authority.
const openapiYaml = stringifyYaml(authority);
const sdkgenYaml = stringifyYaml({
  ...authority,
  "x-sdkwork-generation": {
    derivedFrom: "apis/internal-api/intelligence/sandbox-internal-api-authority.openapi.json",
    derivation: "identity; no generator-specific normalization required for slice v0",
  },
});

const openapiDir = path.join(familyRoot, "openapi");
mkdirSync(openapiDir, { recursive: true });
const openapiOut = path.join(openapiDir, "sdkwork-intelligence-internal-api.openapi.yaml");
const sdkgenOut = path.join(openapiDir, "sdkwork-intelligence-internal-api.sdkgen.yaml");
writeFileSync(openapiOut, openapiYaml);
writeFileSync(sdkgenOut, sdkgenYaml);
console.log(`materialized ${path.relative(repoRoot, openapiOut)}`);
console.log(`materialized ${path.relative(repoRoot, sdkgenOut)}`);
