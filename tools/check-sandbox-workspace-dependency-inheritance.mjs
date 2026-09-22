#!/usr/bin/env node
/**
 * Static gate: every dependency table entry in this repository must be declared once at the
 * workspace root and inherited, never re-specified inside a member crate.
 *
 * Why this exists. `RUST_CODE_SPEC.md` section 14 states it as a MUST:
 *
 *   "Dependencies MUST be declared at the workspace root (`[workspace.dependencies]`) and
 *    inherited; member crates MUST NOT invent divergent third-party versions."
 *
 * and `NAMING_SPEC.md` section 3.2 rule 6 states the converse:
 *
 *   "A dependency key that resolves through `workspace = true` MUST also be declared in the
 *    workspace root `[workspace.dependencies]` table."
 *
 * Neither direction was covered by any checker. On 2026-09-22 the sandbox repository carried
 * `axum = "0.8"` directly in `crates/sdkwork-api-sandbox-assembly/Cargo.toml` while
 * `check-rust-manifest-standard.mjs` reported PASS: that validator only inspects `[package]`
 * inheritance (`edition`, `rust-version`) and `[lints]` wiring, so a member-local third-party
 * version is invisible to it.
 *
 * Two rules, both deterministic:
 *   1. A member dependency entry must be inherited: `crate.workspace = true` or
 *      `crate = { workspace = true, ... }`. A bare version or a bare `path` is a failure.
 *   2. A key inherited with `workspace = true` must exist in the root table, so an inheritance
 *      never dangles against a key nobody declares.
 *
 * Scope: `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` and their
 * `[target.<triple>.*]` and `[dependencies.<crate>]` forms. `[patch.*]` is excluded because it
 * is a root-only override, and `[package]` is excluded because its `edition`/`version`/`license`
 * keys inherit from `[workspace.package]`, not from `[workspace.dependencies]`.
 *
 * Usage:
 *   node tools/check-sandbox-workspace-dependency-inheritance.mjs [--json]
 */
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { discoverManifests } from "./check-sandbox-cargo-path-dependencies.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const ROOT_MANIFEST_NAME = "Cargo.toml";

/**
 * Classify a Cargo section header.
 *
 * @returns `{ kind: "table" }` for `[dependencies]`, `[dev-dependencies]`,
 * `[build-dependencies]`, `[target.'cfg(unix)'.dependencies]` and their dev/build variants;
 * `{ kind: "table-entry", crate }` for the section form `[dependencies.<crate>]`;
 * `null` for anything else, including `[patch.*]`, `[package]` and `[workspace.dependencies]`.
 */
export function classifyDependencySection(sectionHeader) {
  const header = sectionHeader.trim();
  if (header.length === 0 || /^patch([.:]|$)/u.test(header)) {
    return null;
  }
  // The authority table is never a member declaration target.
  if (/^workspace\s*\.\s*dependencies$/u.test(header)) {
    return null;
  }
  const named = header.match(/^(?:target\..+\.)?(?:dev-|build-)?dependencies\.([A-Za-z0-9_.-]+)$/u);
  if (named) {
    return { kind: "table-entry", crate: named[1] };
  }
  if (/^(?:target\..+\.)?(?:dev-|build-)?dependencies$/u.test(header)) {
    return { kind: "table" };
  }
  return null;
}

/** Collect the keys declared by the root `[workspace.dependencies]` table. */
export function parseWorkspaceDependencyKeys(rootManifestPath) {
  const lines = readFileSync(rootManifestPath, "utf8").split(/\r?\n/u);
  const keys = new Map();
  let inAuthorityTable = false;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const header = line.match(/^\s*\[+([^\]]+)\]+\s*$/u);
    if (header) {
      inAuthorityTable = /^workspace\s*\.\s*dependencies$/u.test(header[1].trim());
      continue;
    }
    if (!inAuthorityTable) {
      continue;
    }
    const key = line.match(/^\s*([A-Za-z0-9_.-]+)\s*=/u);
    if (key) {
      keys.set(key[1], index + 1);
    }
  }
  return keys;
}

/**
 * Extract dependency entries from one member manifest.
 *
 * @returns an array of `{ crate, line, inherited }`.
 */
export function parseMemberDependencyEntries(manifestPath) {
  const lines = readFileSync(manifestPath, "utf8").split(/\r?\n/u);
  const entries = [];
  let section = null;
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const header = line.match(/^\s*\[+([^\]]+)\]+\s*$/u);
    if (header) {
      section = classifyDependencySection(header[1]);
      if (section && section.kind === "table-entry") {
        // The crate is named by the header, so the header *is* the declaration: a section such
        // as `[dependencies.axum]` followed by `version = "0.8"` is member-local even though it
        // never repeats the crate name. `resolved` starts true and `inherited` flips only when a
        // `workspace = true` line appears inside the section.
        entries.push({ crate: section.crate, line: index + 1, inherited: false, resolved: true });
      }
      continue;
    }
    if (!section) {
      continue;
    }
    if (section.kind === "table-entry") {
      const entry = entries[entries.length - 1];
      if (entry && /^\s*workspace\s*=\s*true\s*$/u.test(line)) {
        entry.inherited = true;
        entry.resolved = true;
      }
      continue;
    }
    // Table form. `crate.workspace = true` is the dotted spelling.
    const dotted = line.match(/^\s*([A-Za-z0-9_.-]+)\.workspace\s*=\s*true\s*$/u);
    if (dotted) {
      entries.push({ crate: dotted[1], line: index + 1, inherited: true, resolved: true });
      continue;
    }
    const plain = line.match(/^\s*([A-Za-z0-9_.-]+)\s*=(.*)$/u);
    if (!plain) {
      continue;
    }
    const crate = plain[1];
    const value = plain[2];
    // A dotted option line such as `features.workspace` is not a dependency entry.
    if (/^\s*\./u.test(value) || /\.workspace\s*=/u.test(line)) {
      continue;
    }
    entries.push({
      crate,
      line: index + 1,
      inherited: /\bworkspace\s*=\s*true\b/u.test(value),
      resolved: true,
    });
  }
  return entries;
}

/**
 * @param repositoryRootPath the repository whose member manifests are audited
 */
export function assessWorkspaceDependencyInheritance({ repoRoot = repositoryRoot } = {}) {
  const rootManifestPath = join(repoRoot, ROOT_MANIFEST_NAME);
  if (!existsSync(rootManifestPath)) {
    throw new Error(`no ${ROOT_MANIFEST_NAME} at ${repoRoot}; the inheritance gate would be vacuous`);
  }
  const authorityKeys = parseWorkspaceDependencyKeys(rootManifestPath);
  if (authorityKeys.size === 0) {
    throw new Error("the root [workspace.dependencies] table is empty or missing");
  }

  const manifests = discoverManifests(repoRoot).filter(
    (manifestPath) => resolve(manifestPath) !== resolve(rootManifestPath),
  );
  if (manifests.length === 0) {
    throw new Error("no member Cargo.toml found; the inheritance gate would be vacuous");
  }

  const failures = [];
  let entriesChecked = 0;
  let inheritedChecked = 0;

  for (const manifestPath of manifests) {
    const manifestLabel = relative(repoRoot, manifestPath) || ROOT_MANIFEST_NAME;
    for (const entry of parseMemberDependencyEntries(manifestPath)) {
      if (!entry.resolved) {
        continue;
      }
      entriesChecked += 1;
      if (!entry.inherited) {
        failures.push({
          manifest: manifestLabel,
          line: entry.line,
          crate: entry.crate,
          reason: "member-local-declaration",
          message:
            `${manifestLabel}:${entry.line} declares '${entry.crate}' inside a member crate; ` +
            "RUST_CODE_SPEC.md section 14 requires the version to live in the root " +
            "[workspace.dependencies] table and the member to inherit it with 'workspace = true'",
        });
        continue;
      }
      inheritedChecked += 1;
      if (!authorityKeys.has(entry.crate)) {
        failures.push({
          manifest: manifestLabel,
          line: entry.line,
          crate: entry.crate,
          reason: "missing-workspace-declaration",
          message:
            `${manifestLabel}:${entry.line} inherits '${entry.crate}' with 'workspace = true' but ` +
            "the root [workspace.dependencies] table does not declare that key " +
            "(NAMING_SPEC.md section 3.2 rule 6)",
        });
      }
    }
  }

  return {
    ok: failures.length === 0,
    repoRoot,
    authorityKeys: [...authorityKeys.keys()].sort(),
    memberManifestsChecked: manifests.length,
    entriesChecked,
    inheritedChecked,
    failures,
  };
}

export function formatWorkspaceDependencyInheritanceReport(assessment) {
  if (assessment.ok) {
    return (
      `Cargo workspace dependency inheritance: ${assessment.entriesChecked} member dependency ` +
      `entry(ies) across ${assessment.memberManifestsChecked} member manifest(s) all inherit from ` +
      `the root [workspace.dependencies] table (${assessment.authorityKeys.length} key(s))\n`
    );
  }
  const lines = [
    `Cargo workspace dependency inheritance failed: ${assessment.failures.length} problem(s) across ` +
      `${assessment.memberManifestsChecked} member manifest(s)`,
  ];
  for (const failure of assessment.failures) {
    lines.push(`- [${failure.reason}] ${failure.message}`);
  }
  return `${lines.join("\n")}\n`;
}

export function parseWorkspaceDependencyInheritanceArgs(argv) {
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
    const options = parseWorkspaceDependencyInheritanceArgs(process.argv.slice(2));
    const assessment = assessWorkspaceDependencyInheritance();
    process.stdout.write(
      options.json
        ? `${JSON.stringify(assessment, null, 2)}\n`
        : formatWorkspaceDependencyInheritanceReport(assessment),
    );
    if (!assessment.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(`sandbox workspace dependency inheritance check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
