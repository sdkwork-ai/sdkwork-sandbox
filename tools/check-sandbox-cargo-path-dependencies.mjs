#!/usr/bin/env node
/**
 * Static gate: every Cargo path dependency in this repository must resolve.
 *
 * Why this exists. A `path = "../sibling/crates/x"` dependency is resolved relative to the
 * manifest that declares it, so one extra `../` silently retargets the whole workspace. When
 * that happens `cargo metadata` fails before any cargo command runs, which means the failure
 * is invisible to every static checker: the documentation, naming, layout, port, layering,
 * path-portability, manifest-standard, dependency-completeness and backend-composition
 * validators all still pass, because none of them resolve Cargo path dependencies. The
 * 2026-09-22 audit confirmed exactly that: `../../../sdkwork-web-framework/...` in the root
 * `Cargo.toml` broke `cargo metadata`, `cargo fmt`, `cargo check`, `cargo clippy` and
 * `cargo test` for the whole workspace while all thirteen gates stayed green.
 *
 * Two rules, both deterministic:
 *   1. `resolved` must exist and must contain a `Cargo.toml` (the target is a real package).
 *   2. `resolved` must stay inside the workspace root, so a sibling-repository dependency
 *      cannot silently escape the multi-repository workspace with surplus `..` segments.
 *
 * Usage:
 *   node tools/check-sandbox-cargo-path-dependencies.mjs [--json]
 */
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

/** Directories that never contain owned manifests. */
const IGNORED_DIRECTORY_NAMES = new Set([
  ".git",
  ".workbuddy",
  "target",
  "node_modules",
  "dist",
  "build",
  "coverage",
]);

/**
 * A Cargo section is dependency-bearing when its header mentions `dependencies`, which covers
 * `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]`, `[workspace.dependencies]`,
 * `[target.'cfg(unix)'.dependencies]` and `[dependencies.<crate>]`. `[patch.*]` sections also
 * declare packages by path. `[lib]`, `[[bin]]`, `[[test]]` and `[package]` are excluded on
 * purpose: their `path` key names a source file, not a dependency.
 */
function isDependencySection(sectionHeader) {
  return /dependencies/u.test(sectionHeader) || /^patch([.:]|$)/u.test(sectionHeader);
}

/** Collect every `Cargo.toml` this repository owns. */
export function discoverManifests(root = repositoryRoot) {
  const manifests = [];
  const walk = (directory) => {
    const entries = readdirSync(directory, { withFileTypes: true });
    for (const entry of entries) {
      if (entry.isDirectory()) {
        if (IGNORED_DIRECTORY_NAMES.has(entry.name)) {
          continue;
        }
        walk(join(directory, entry.name));
      } else if (entry.isFile() && entry.name === "Cargo.toml") {
        manifests.push(join(directory, entry.name));
      }
    }
  };
  walk(root);
  return manifests.sort();
}

/**
 * Extract declared path dependencies with the line number that declares them, so a failure
 * points at the exact line a maintainer has to edit.
 */
export function parsePathDependencies(manifestPath) {
  const lines = readFileSync(manifestPath, "utf8").split(/\r?\n/u);
  const dependencies = [];
  let sectionHeader = "";
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const header = line.match(/^\s*\[+([^\]]+)\]+\s*$/u);
    if (header) {
      sectionHeader = header[1].trim();
      continue;
    }
    if (!isDependencySection(sectionHeader)) {
      continue;
    }
    // Inline table form: `crate = { version = "1", path = "../crate" }`.
    const inline = line.match(/^\s*([A-Za-z0-9_.-]+)\s*=\s*\{(.*)\}\s*$/u);
    if (inline) {
      const declared = inline[2].match(/\bpath\s*=\s*"([^"]+)"/u);
      if (declared) {
        dependencies.push({
          crate: inline[1],
          path: declared[1],
          line: index + 1,
          form: "inline-table",
        });
      }
      continue;
    }
    // Section form: `[dependencies.crate]` followed by a bare `path = "..."`.
    const bare = line.match(/^\s*path\s*=\s*"([^"]+)"/u);
    if (bare) {
      dependencies.push({
        crate: sectionHeader.split(".").pop(),
        path: bare[1],
        line: index + 1,
        form: "section",
      });
    }
  }
  return dependencies;
}

function isInside(parent, child) {
  const rel = relative(resolve(parent), resolve(child));
  if (rel === "") {
    return true;
  }
  // `relative` returns an absolute path when the two paths live on different Windows drives.
  if (isAbsolute(rel)) {
    return false;
  }
  return rel !== ".." && !rel.startsWith(`..${sep}`);
}

/**
 * @param repositoryRootPath the repository whose manifests are audited
 * @param workspaceRootPath  the multi-repository workspace that path dependencies may not escape
 */
export function assessCargoPathDependencies({
  repoRoot = repositoryRoot,
  workspaceRoot = resolve(repositoryRoot, ".."),
} = {}) {
  const manifests = discoverManifests(repoRoot);
  if (manifests.length === 0) {
    throw new Error("no Cargo.toml found; the path-dependency gate would be vacuous");
  }

  const failures = [];
  const resolvedWorkspaceRoot = resolve(workspaceRoot);
  let checkedDependencies = 0;

  for (const manifestPath of manifests) {
    for (const dependency of parsePathDependencies(manifestPath)) {
      checkedDependencies += 1;
      const target = resolve(dirname(manifestPath), dependency.path);
      const location = `${relative(repoRoot, manifestPath) || "Cargo.toml"}:${dependency.line}`;
      const describe = `${location} ${dependency.crate} -> ${dependency.path}`;

      if (!existsSync(target) || !statSync(target).isDirectory()) {
        failures.push({
          manifest: relative(repoRoot, manifestPath) || "Cargo.toml",
          line: dependency.line,
          crate: dependency.crate,
          declaredPath: dependency.path,
          resolvedPath: target,
          reason: "unresolved",
          message: `${describe} does not exist (resolved to ${target})`,
        });
        continue;
      }

      if (!existsSync(join(target, "Cargo.toml"))) {
        failures.push({
          manifest: relative(repoRoot, manifestPath) || "Cargo.toml",
          line: dependency.line,
          crate: dependency.crate,
          declaredPath: dependency.path,
          resolvedPath: target,
          reason: "not-a-package",
          message: `${describe} is not a Cargo package: no Cargo.toml under ${target}`,
        });
        continue;
      }

      if (!isInside(resolvedWorkspaceRoot, target)) {
        failures.push({
          manifest: relative(repoRoot, manifestPath) || "Cargo.toml",
          line: dependency.line,
          crate: dependency.crate,
          declaredPath: dependency.path,
          resolvedPath: target,
          reason: "escapes-workspace",
          message:
            `${describe} resolves to ${target}, outside the workspace root ${resolvedWorkspaceRoot}; ` +
            "a sibling repository dependency must use exactly one leading '..' segment",
        });
      }
    }
  }

  return {
    ok: failures.length === 0,
    repoRoot,
    workspaceRoot: resolvedWorkspaceRoot,
    manifestsChecked: manifests.length,
    dependenciesChecked: checkedDependencies,
    failures,
  };
}

export function formatCargoPathDependencyReport(assessment) {
  const lines = [];
  if (assessment.ok) {
    lines.push(
      `Cargo path dependencies resolve: ${assessment.dependenciesChecked} path dependency(ies) ` +
        `across ${assessment.manifestsChecked} manifest(s) inside ${assessment.workspaceRoot}`,
    );
    return `${lines.join("\n")}\n`;
  }
  lines.push(
    `Cargo path dependency check failed: ${assessment.failures.length} problem(s) across ` +
      `${assessment.manifestsChecked} manifest(s)`,
  );
  for (const failure of assessment.failures) {
    lines.push(`- [${failure.reason}] ${failure.message}`);
  }
  return `${lines.join("\n")}\n`;
}

export function parseCargoPathDependencyArgs(argv) {
  const options = { json: false };
  for (const argument of argv) {
    if (argument === "--json") {
      options.json = true;
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
    const options = parseCargoPathDependencyArgs(process.argv.slice(2));
    const assessment = assessCargoPathDependencies();
    process.stdout.write(
      options.json
        ? `${JSON.stringify(assessment, null, 2)}\n`
        : formatCargoPathDependencyReport(assessment),
    );
    if (!assessment.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(`sandbox cargo path dependency check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
