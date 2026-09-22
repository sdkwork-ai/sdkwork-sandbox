#!/usr/bin/env node
/**
 * Component contract alignment gate.
 *
 * `COMPONENT_SPEC.md` is the authority for `specs/component.spec.json`, but no tool in
 * `sdkwork-specs/tools/` enforces the parts of it checked here. A contract that names a spec
 * file which does not resolve, declares a language the component does not author, or omits the
 * language spec for a language it does author, is a silent lie: nothing fails, so nothing is
 * fixed. This gate turns those sentences into assertions for this repository.
 *
 * Rules, each mirroring a specific `COMPONENT_SPEC.md` sentence:
 *
 *   R1  "canonicalSpecs must link to actual root spec files" (§3). Every `canonicalSpecs[].path`
 *       must resolve from the component root, and `file` must be the basename of `path`.
 *   R2  "component.name, component.type, component.root, component.domain, component.capability,
 *       and component.languages are required" (§3). Also, `component.root` must name the
 *       component's real on-disk location. The spec spells roots two ways — the §2 example uses
 *       `<repository>/<path>` and the `rust-route-crate` rule uses `crates/<package>/` — so both
 *       spellings are accepted and only a root that matches neither is rejected.
 *   R3  "canonicalSpecs must include CODE_STYLE_SPEC.md and NAMING_SPEC.md when the component owns
 *       authored source code" (§3).
 *   R4  "canonicalSpecs must include language-specific specs only for languages declared in
 *       component.languages" (§3) plus the "Language-specific root specs are on-demand" table
 *       (§4). Both directions are enforced: a declared language that authors source must carry its
 *       language spec, and a declared language that authors no source is a false declaration.
 *   R5  "component.manifests" entries must exist, relative to the component root.
 *   R6  The `rust-api-assembly` MUST sentence (§4): component and Cargo package name
 *       `sdkwork-api-<application-code>-assembly`, located at
 *       `crates/sdkwork-api-<application-code>-assembly/`, `component.surface: "api-assembly"`,
 *       `contracts.layerRole: "runtime-composition"`, owning `assembly-manifest.json`, and
 *       carrying that sentence's seven canonical specs.
 *   R7  Crate <-> component-spec reconciliation (SOUL.md section 2: every authored module
 *       MUST maintain `<module-root>/specs/component.spec.json`). A `crates/<name>/`
 *       directory that owns a `Cargo.toml` but no component spec is an authored module
 *       outside the contract system, and a component spec under `crates/<name>/` whose
 *       directory owns no `Cargo.toml` describes a crate that does not exist. The human
 *       module inventory missed `sdkwork-api-sandbox-assembly` exactly this way (the F-07
 *       finding in REVIEW-20260922), so the two sets are reconciled by the gate instead of
 *       by whoever remembers to re-read the list.
 *
 * Deliberately not enforced, with the evidence for each decision:
 *
 *   - The §4 `rust-crate` "Required root specs" row (`MODULE_SPEC.md`, `CONFIG_SPEC.md`,
 *     `DEPLOYMENT_SPEC.md`, `TEST_SPEC.md`). Measured 2026-09-22: 59 of 327 workspace
 *     `rust-crate` components satisfy the full row (18%), no generator emits it, no gate checks
 *     it, and `CONFIG_SPEC.md`/`DEPLOYMENT_SPEC.md` do not govern an in-process library adapter.
 *     Requiring it here would demand spec references that misdescribe the components.
 *   - `APP_COMPOSITION_SPEC.md` in the §4 `rust-api-assembly` row. Its scope is "client
 *     application composition", and 0 components in the workspace reference it; R6 therefore
 *     enforces the §4 prose MUST sentence instead, which is what the assembly generator and every
 *     real assembly already follow.
 *   - The `component.type` vocabulary. `multi-surface-workspace`, used by this repository's root
 *     contract, appears nowhere in `sdkwork-specs`, and workspace roots carry 21 distinct types
 *     including 12 with none at all. There is no authoritative list to check against.
 *
 * Usage:
 *   node tools/check-sandbox-component-contract-alignment.mjs [--root <dir>] [--json]
 */

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const SKIPPED_DIRECTORIES = new Set([
  "node_modules",
  ".git",
  "target",
  "dist",
  "build",
  ".venv",
  ".workbuddy",
]);

/** COMPONENT_SPEC.md §4 "Language-specific root specs are on-demand". */
const LANGUAGE_SPECS = {
  rust: { spec: "RUST_CODE_SPEC.md", extensions: [".rs"] },
  java: { spec: "JAVA_CODE_SPEC.md", extensions: [".java"] },
  typescript: { spec: "TYPESCRIPT_CODE_SPEC.md", extensions: [".ts", ".tsx"] },
  javascript: { spec: "TYPESCRIPT_CODE_SPEC.md", extensions: [".js", ".jsx", ".mjs", ".cjs"] },
  node: { spec: "TYPESCRIPT_CODE_SPEC.md", extensions: [".js", ".mjs", ".cjs"] },
  react: { spec: "FRONTEND_CODE_SPEC.md", extensions: [".tsx", ".jsx"] },
  tsx: { spec: "FRONTEND_CODE_SPEC.md", extensions: [".tsx"] },
  flutter: { spec: "FRONTEND_CODE_SPEC.md", extensions: [".dart"] },
  dart: { spec: "FRONTEND_CODE_SPEC.md", extensions: [".dart"] },
  ui: { spec: "FRONTEND_CODE_SPEC.md", extensions: [".tsx", ".jsx", ".vue"] },
  "android-ui": { spec: "FRONTEND_CODE_SPEC.md", extensions: [".kt", ".java"] },
  "ios-ui": { spec: "FRONTEND_CODE_SPEC.md", extensions: [".swift"] },
  "harmony-ui": { spec: "FRONTEND_CODE_SPEC.md", extensions: [".ets", ".ts"] },
};

/** Extensions that mean "this component owns authored source" for the §3 base-spec rule. */
const AUTHORED_SOURCE_EXTENSIONS = [
  ".rs",
  ".java",
  ".ts",
  ".tsx",
  ".js",
  ".jsx",
  ".mjs",
  ".cjs",
  ".py",
  ".go",
  ".cs",
  ".swift",
  ".kt",
  ".dart",
  ".ets",
  ".vue",
  ".rb",
  ".php",
];

const REQUIRED_BASE_SPECS = ["CODE_STYLE_SPEC.md", "NAMING_SPEC.md"];

const REQUIRED_COMPONENT_FIELDS = ["name", "type", "root", "domain", "capability"];

/** COMPONENT_SPEC.md §4, the `rust-api-assembly` MUST sentence. */
const ASSEMBLY_REQUIRED_SPECS = [
  "API_ASSEMBLY_SPEC.md",
  "APPLICATION_GATEWAY_SPEC.md",
  "WEB_FRAMEWORK_SPEC.md",
  "WEB_BACKEND_SPEC.md",
  "RUST_CODE_SPEC.md",
  "APP_RUNTIME_TOPOLOGY_SPEC.md",
  "TEST_SPEC.md",
];

const ASSEMBLY_NAME = /^sdkwork-api-(?<applicationCode>[a-z0-9]+(?:-[a-z0-9]+)*)-assembly$/u;

export function discoverComponentSpecs(root) {
  const found = [];
  const stack = [root];
  while (stack.length > 0) {
    const directory = stack.pop();
    let entries;
    try {
      entries = readdirSync(directory, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        if (SKIPPED_DIRECTORIES.has(entry.name)) {
          continue;
        }
        stack.push(join(directory, entry.name));
        continue;
      }
      if (entry.name === "component.spec.json" && basename(directory) === "specs") {
        found.push(join(directory, entry.name));
      }
    }
  }
  return found.sort();
}

/**
 * True when `directory` contains at least one file whose extension is in `extensions`.
 * Generated and vendored trees are skipped so a checked-in `node_modules` cannot make a
 * component look like it authors a language it does not.
 */
export function hasAuthoredSource(directory, extensions) {
  const wanted = extensions.map((extension) => extension.toLowerCase());
  const stack = [directory];
  while (stack.length > 0) {
    const current = stack.pop();
    let entries;
    try {
      entries = readdirSync(current, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        if (SKIPPED_DIRECTORIES.has(entry.name)) {
          continue;
        }
        stack.push(join(current, entry.name));
        continue;
      }
      const lower = entry.name.toLowerCase();
      if (wanted.some((extension) => lower.endsWith(extension))) {
        return true;
      }
    }
  }
  return false;
}

function readCargoPackageName(componentRoot) {
  const manifestPath = join(componentRoot, "Cargo.toml");
  if (!existsSync(manifestPath)) {
    return null;
  }
  const manifest = readFileSync(manifestPath, "utf8");
  const section = manifest.split(/^\[/mu).find((chunk) => chunk.startsWith("package]"));
  if (!section) {
    return null;
  }
  const name = section.match(/^\s*name\s*=\s*"([^"]+)"/mu);
  return name ? name[1] : null;
}

export function assessComponentContractAlignment({ repoRoot = repositoryRoot } = {}) {
  const repoName = basename(repoRoot);
  const specPaths = discoverComponentSpecs(repoRoot);
  const failures = [];
  const components = [];
  const componentRelPathsUnderCrates = [];

  for (const specPath of specPaths) {
    const componentRoot = dirname(dirname(specPath));
    const rel = relative(repoRoot, componentRoot).split(sep).join("/");
    const label = rel === "" ? "." : rel;

    let spec;
    try {
      spec = JSON.parse(readFileSync(specPath, "utf8"));
    } catch (error) {
      failures.push({
        component: label,
        reason: "unparsable-component-spec",
        message: `specs/component.spec.json is not valid JSON: ${error.message}`,
      });
      continue;
    }

    const component = spec.component ?? {};
    const contracts = spec.contracts ?? {};
    const specFiles = (spec.canonicalSpecs ?? []).map((entry) => entry.file);
    const languages = Array.isArray(component.languages) ? component.languages : [];
    if (rel.startsWith("crates/")) {
      componentRelPathsUnderCrates.push(rel);
    }

    for (const field of REQUIRED_COMPONENT_FIELDS) {
      if (typeof component[field] !== "string" || component[field].trim() === "") {
        failures.push({
          component: label,
          reason: "missing-required-field",
          message: `component.${field} is required by COMPONENT_SPEC.md section 3 and must be a non-empty string`,
        });
      }
    }
    if (languages.length === 0) {
      failures.push({
        component: label,
        reason: "missing-required-field",
        message: "component.languages is required by COMPONENT_SPEC.md section 3 and must not be empty",
      });
    }

    if (typeof component.root === "string" && component.root.trim() !== "") {
      const candidates =
        rel === "" ? new Set([repoName]) : new Set([`${repoName}/${rel}`, rel]);
      if (!candidates.has(component.root)) {
        failures.push({
          component: label,
          reason: "stale-component-root",
          message:
            `component.root is '${component.root}' but this component lives at ` +
            `${[...candidates].map((candidate) => `'${candidate}'`).join(" or ")}`,
        });
      }
    }

    for (const [index, entry] of (spec.canonicalSpecs ?? []).entries()) {
      if (typeof entry?.path !== "string" || entry.path.trim() === "") {
        failures.push({
          component: label,
          reason: "unresolved-canonical-spec",
          message: `canonicalSpecs[${index}] has no path`,
        });
        continue;
      }
      if (!existsSync(resolve(componentRoot, entry.path))) {
        failures.push({
          component: label,
          reason: "unresolved-canonical-spec",
          message: `canonicalSpecs[${index}] ${entry.file ?? "?"} -> '${entry.path}' does not exist`,
        });
        continue;
      }
      if (typeof entry.file === "string" && entry.file !== basename(entry.path)) {
        failures.push({
          component: label,
          reason: "canonical-spec-name-mismatch",
          message: `canonicalSpecs[${index}] declares file '${entry.file}' but path '${entry.path}'`,
        });
      }
    }

    for (const [index, manifest] of (component.manifests ?? []).entries()) {
      if (typeof manifest !== "string" || !existsSync(resolve(componentRoot, manifest))) {
        failures.push({
          component: label,
          reason: "missing-manifest",
          message: `component.manifests[${index}] '${String(manifest)}' does not exist under the component root`,
        });
      }
    }

    const ownsAuthoredSource = hasAuthoredSource(componentRoot, AUTHORED_SOURCE_EXTENSIONS);
    if (ownsAuthoredSource) {
      for (const required of REQUIRED_BASE_SPECS) {
        if (!specFiles.includes(required)) {
          failures.push({
            component: label,
            reason: "missing-base-spec",
            message:
              `the component owns authored source, so COMPONENT_SPEC.md section 3 requires ` +
              `${required} in canonicalSpecs`,
          });
        }
      }
    }

    for (const language of languages) {
      const key = String(language).toLowerCase();
      const rule = LANGUAGE_SPECS[key];
      if (!rule) {
        continue;
      }
      const authors = hasAuthoredSource(componentRoot, rule.extensions);
      if (authors && !specFiles.includes(rule.spec)) {
        failures.push({
          component: label,
          reason: "missing-language-spec",
          message:
            `component.languages declares '${language}' and the component authors ` +
            `${rule.extensions.join("/")} source, so COMPONENT_SPEC.md section 4 requires ` +
            `${rule.spec} in canonicalSpecs`,
        });
      }
      if (!authors) {
        failures.push({
          component: label,
          reason: "false-language-declaration",
          message:
            `component.languages declares '${language}' but the component authors no ` +
            `${rule.extensions.join("/")} source; the declaration is not backed by authored source`,
        });
      }
    }

    if (component.type === "rust-api-assembly") {
      const name = typeof component.name === "string" ? component.name : "";
      const match = name.match(ASSEMBLY_NAME);
      if (!match) {
        failures.push({
          component: label,
          reason: "assembly-name",
          message: `rust-api-assembly component name '${name}' must match sdkwork-api-<application-code>-assembly`,
        });
      } else {
        const expectedRoot = `crates/${name}`;
        if (rel !== expectedRoot) {
          failures.push({
            component: label,
            reason: "assembly-root",
            message: `rust-api-assembly must live under ${expectedRoot}/, not ${label}`,
          });
        }
        const packageName = readCargoPackageName(componentRoot);
        if (packageName !== name) {
          failures.push({
            component: label,
            reason: "assembly-package-name",
            message: `Cargo package name '${packageName ?? "(none)"}' must equal the component name '${name}'`,
          });
        }
      }
      if (component.surface !== "api-assembly") {
        failures.push({
          component: label,
          reason: "assembly-surface",
          message: `rust-api-assembly must declare component.surface: "api-assembly", saw '${String(component.surface)}'`,
        });
      }
      if (contracts.layerRole !== "runtime-composition") {
        failures.push({
          component: label,
          reason: "assembly-layer-role",
          message: `rust-api-assembly must declare contracts.layerRole: "runtime-composition", saw '${String(contracts.layerRole)}'`,
        });
      }
      if (!(component.manifests ?? []).includes("assembly-manifest.json")) {
        failures.push({
          component: label,
          reason: "assembly-manifest",
          message: "rust-api-assembly must own assembly-manifest.json in component.manifests",
        });
      }
      for (const required of ASSEMBLY_REQUIRED_SPECS) {
        if (!specFiles.includes(required)) {
          failures.push({
            component: label,
            reason: "assembly-required-spec",
            message: `the rust-api-assembly MUST sentence requires ${required} in canonicalSpecs`,
          });
        }
      }
    }

    components.push({
      component: label,
      type: component.type ?? null,
      languages,
      canonicalSpecs: specFiles.length,
      ownsAuthoredSource,
    });
  }

  // R7  Crate <-> component-spec reconciliation, both directions. The human
  // module inventory missed a whole crate (F-07), so the gate reconciles the
  // crate set against the spec set instead of trusting either list.
  const cratesDirectory = join(repoRoot, "crates");
  let crateDirectories = [];
  try {
    crateDirectories = readdirSync(cratesDirectory, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
      .sort();
  } catch {
    // A repository without crates/ has no crate side to reconcile.
  }
  let cratesChecked = 0;
  for (const crateName of crateDirectories) {
    const crateRoot = join(cratesDirectory, crateName);
    if (!existsSync(join(crateRoot, "Cargo.toml"))) {
      continue;
    }
    cratesChecked += 1;
    if (!existsSync(join(crateRoot, "specs", "component.spec.json"))) {
      failures.push({
        component: `crates/${crateName}`,
        reason: "missing-crate-component-spec",
        message:
          `crates/${crateName} owns a Cargo.toml but no specs/component.spec.json; ` +
          "every authored module must stay inside the component-contract system",
      });
    }
  }
  for (const rel of componentRelPathsUnderCrates) {
    if (!existsSync(join(repoRoot, rel, "Cargo.toml"))) {
      failures.push({
        component: rel,
        reason: "crate-spec-without-crate",
        message: `the component spec at ${rel}/specs describes a crate that does not exist: the directory owns no Cargo.toml`,
      });
    }
  }

  return {
    ok: failures.length === 0,
    repoRoot,
    componentsChecked: components.length,
    cratesChecked,
    components,
    failures,
  };
}

export function formatComponentContractAlignmentReport(assessment) {
  if (assessment.ok) {
    return (
      `Component contract alignment: ${assessment.componentsChecked} component spec(s) declare ` +
      "resolvable canonical specs, truthful languages and existing manifests\n"
    );
  }
  const lines = [
    `Component contract alignment: ${assessment.failures.length} problem(s) across ` +
      `${assessment.componentsChecked} component spec(s)`,
  ];
  for (const failure of assessment.failures) {
    lines.push(`- [${failure.reason}] ${failure.component}: ${failure.message}`);
  }
  return `${lines.join("\n")}\n`;
}

export function parseComponentContractAlignmentArgs(argv) {
  const options = { json: false, root: repositoryRoot };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--json") {
      options.json = true;
    } else if (argument === "--root") {
      const value = argv[index + 1];
      if (!value) {
        throw new Error("--root requires a directory");
      }
      options.root = resolve(value);
      index += 1;
    } else {
      throw new Error(`unsupported argument: ${argument}`);
    }
  }
  return options;
}

function isMainModule() {
  return process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
}

if (isMainModule()) {
  try {
    const options = parseComponentContractAlignmentArgs(process.argv.slice(2));
    const assessment = assessComponentContractAlignment({ repoRoot: options.root });
    process.stdout.write(
      options.json
        ? `${JSON.stringify(assessment, null, 2)}\n`
        : formatComponentContractAlignmentReport(assessment),
    );
    if (!assessment.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(`sandbox component contract alignment check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
