#!/usr/bin/env node
// Keep the Sandbox platform-support claim honest and machine-checked.
//
// Authority: `TECH-performance-and-capacity.md` section 5 makes the reference environment part of
// every benchmark record, and `DEPENDENCY_MANAGEMENT_SPEC.md` section 1 requires sources to be
// portable across Windows, macOS and Linux. Neither clause had an executor for the part that
// actually breaks portability: platform-conditional code.
//
// `../../sdkwork-specs/tools/check-workspace-path-portability.mjs` covers machine-specific absolute
// paths and `check-shell-portability.mjs` covers shell syntax. Neither looks at `#[cfg(windows)]`,
// `std::process::Command`, `libc::` or `std::path::MAIN_SEPARATOR`, so a runtime backend could go
// Windows-only (or Linux-only) with every existing gate still green. That is the invariant here.
//
// Rules (all are errors; the process exits 1 when any is reported)
//
//   PLATFORM-DOC-*     The platform support document exists, declares the platform vocabulary, the
//                      support matrix, the platform-conditional code declarations and the
//                      portability gates, and is linked from the architecture entry, the tech
//                      README and `docs/INDEX.yaml`.
//   PLATFORM-MATRIX-*  The matrix covers exactly the declared vocabulary, every status comes from
//                      the status vocabulary, and every row cites at least one repository-relative
//                      path or host-capability id. Repository paths must resolve and capability ids
//                      must exist in the probe vocabulary; host observations stay free-form.
//   PLATFORM-CODE-*    Every platform marker in `crates/*/src` is declared, every declaration
//                      names a marker that is still present, and the declared census count matches.
//                      This is what stops the control plane from silently becoming single-platform.
//   PLATFORM-GATE-*    Every gate the document names as backing the claim exists and is wired into
//                      this repository's `package.json`. A portability gate that never runs is the
//                      exact failure mode the tooling README warns about.
//
// Usage:
//   node tools/check-sandbox-platform-code.mjs
//   node tools/check-sandbox-platform-code.mjs --json
//   node tools/check-sandbox-platform-code.mjs --root <dir>
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { HOST_CAPABILITY_IDS } from "./testing/sandbox-host-capability-evidence.mjs";

export const PLATFORM_DOCUMENT = "docs/architecture/tech/TECH-platform-support.md";
export const ARCHITECTURE_ENTRY = "docs/architecture/tech/TECH_ARCHITECTURE.md";
export const TECH_README = "docs/architecture/tech/README.md";
export const DOCS_INDEX = "docs/INDEX.yaml";

/// Fixed vocabulary. A platform that is not listed here cannot appear in the matrix, so adding one
/// is a deliberate edit to this file rather than an incidental row in a table.
export const PLATFORM_IDS = Object.freeze([
  "windows-x64",
  "linux-x64-wsl2",
  "linux-x64-native",
  "linux-aarch64",
  "macos-arm64",
]);

export const STATUS_IDS = Object.freeze(["verified", "partial", "unsupported", "unmeasured"]);

export const SECTIONS = Object.freeze({
  vocabulary: "### 1.1 平台词汇",
  matrix: "### 1.2 平台支持矩阵",
  markers: "### 3.1 平台条件代码声明",
  gates: "### 4.1 可移植性门禁",
});

/// Platform markers. A hit means the source is not platform-neutral, so it must be declared with
/// the platform it targets and the reason it is there.
export const PLATFORM_MARKERS = Object.freeze([
  { id: "cfg-windows", pattern: /#\[cfg\([^)]*\bwindows\b/u },
  { id: "cfg-unix", pattern: /#\[cfg\([^)]*\bunix\b/u },
  { id: "cfg-target-os", pattern: /#\[cfg\([^)]*target_os/u },
  { id: "cfg-target-family", pattern: /#\[cfg\([^)]*target_family/u },
  { id: "cfg-target-arch", pattern: /#\[cfg\([^)]*target_arch/u },
  { id: "cfg-macro", pattern: /\bcfg!\(/u },
  { id: "std-os", pattern: /std::os::(?:windows|unix)/u },
  { id: "std-process-command", pattern: /std::process::Command/u },
  { id: "tokio-process", pattern: /tokio::process/u },
  { id: "libc", pattern: /\blibc::/u },
  { id: "nix", pattern: /\bnix::/u },
  { id: "windows-crate", pattern: /\b(?:winapi|windows_sys|windows)::/u },
  { id: "env-consts-os", pattern: /env::consts::(?:OS|FAMILY)/u },
  { id: "path-separator", pattern: /path::MAIN_SEPARATOR/u },
]);

export const PLATFORM_MARKER_IDS = Object.freeze(PLATFORM_MARKERS.map((marker) => marker.id));

function toPosix(value) {
  return value.split(sep).join("/");
}

function stripBackticks(value) {
  return value.replace(/`/gu, "").trim();
}

/// Splits a markdown table row into cells. A `|` inside an inline-code span is content, not a
/// delimiter: capability shorthand such as `namespace.mount|pid|uts` is one citation. Splitting
/// naively on every `|` silently truncated the evidence cell — the text after the first such span
/// became extra cells, so citations past it were never checked (and never reported as missing).
export function splitTableRow(line) {
  const trimmed = line.trim();
  if (!trimmed.startsWith("|") || !trimmed.endsWith("|")) {
    return null;
  }
  const cells = [];
  let current = "";
  let insideCode = false;
  for (const character of trimmed.slice(1, -1)) {
    if (character === "`") {
      insideCode = !insideCode;
      current += character;
    } else if (character === "|" && !insideCode) {
      cells.push(current.trim());
      current = "";
    } else {
      current += character;
    }
  }
  cells.push(current.trim());
  return cells;
}

/// Placeholder cells used to say "this table has no rows yet". They must not be read as
/// declarations, or a census of zero reports three more problems than it has.
const PLACEHOLDER_CELL = /^(?:|（无）|\(无\)|无|none|-{2,}|—+|n\/a)$/u;

export function isPlaceholderRow(cells) {
  return cells.every((cell) => PLACEHOLDER_CELL.test(cell.replace(/`/gu, "").trim()));
}

const PATH_EXTENSION = /\.(?:md|mjs|cjs|js|json|ya?ml|rs|sh|bash|toml|sql|lock|txt)$/u;
const CAPABILITY_ID = /^[a-z][a-z0-9-]*(?:\.[a-z0-9-]+)+$/u;

/// Classifies a backticked citation in the support matrix. The matrix legitimately cites three
/// different kinds of thing — repository paths, host-capability ids from the probe vocabulary, and
/// free-form observations such as a kernel string or a cgroup controller list. Only the first two
/// are checkable, and conflating them made every capability id look like a broken path.
export function classifyCitation(target) {
  if (/^https?:/u.test(target) || target.startsWith("#")) {
    return "external";
  }
  // An absolute path is an observation about the host (for example `/etc/os-release`), not a
  // repository-relative citation, so it is not required to resolve from the repository root.
  if (target.startsWith("/")) {
    return "host";
  }
  if (target.includes("/") || PATH_EXTENSION.test(target)) {
    return "path";
  }
  if (CAPABILITY_ID.test(target)) {
    return "capability";
  }
  return "freeform";
}

function isSeparatorRow(cells) {
  return cells.length > 0 && cells.every((cell) => /^:?-{2,}:?$/u.test(cell.replace(/\s/gu, "")));
}

/// Returns the body of a level-3 section, or null when the heading is absent.
export function sectionBody(content, heading) {
  const lines = content.split(/\r?\n/u);
  const start = lines.findIndex(
    (line) => line.trim() === heading || line.trim().startsWith(`${heading} `),
  );
  if (start < 0) {
    return null;
  }
  const body = [];
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^#{1,3}\s/u.test(lines[index])) {
      break;
    }
    body.push(lines[index]);
  }
  return body.join("\n");
}

function tableRows(body) {
  if (body === null) {
    return [];
  }
  const rows = [];
  for (const line of body.split(/\r?\n/u)) {
    const cells = splitTableRow(line);
    if (!cells || isSeparatorRow(cells)) {
      continue;
    }
    rows.push(cells);
  }
  // Drop the header row, which is the only row whose first cell names the column.
  return rows.slice(1);
}

export function parsePlatformDocument(content) {
  const vocabularyBody = sectionBody(content, SECTIONS.vocabulary);
  const matrixBody = sectionBody(content, SECTIONS.matrix);
  const markersBody = sectionBody(content, SECTIONS.markers);
  const gatesBody = sectionBody(content, SECTIONS.gates);

  const vocabulary = tableRows(vocabularyBody).map((cells) => stripBackticks(cells[0]));
  const matrix = tableRows(matrixBody).map((cells) => ({
    platform: stripBackticks(cells[0]),
    status: stripBackticks(cells[1]),
    evidence: cells[2] ?? "",
  }));
  const declaredMarkers = tableRows(markersBody)
    .filter((cells) => !isPlaceholderRow(cells))
    .map((cells) => ({
      path: stripBackticks(cells[0]),
      marker: stripBackticks(cells[1]),
      platform: stripBackticks(cells[2]),
      reason: cells[3] ?? "",
    }));
  const declaredCensusMatch = /当前声明：(\d+)/u.exec(markersBody ?? "");
  const declaredCensus = declaredCensusMatch ? Number.parseInt(declaredCensusMatch[1], 10) : null;
  const gates = tableRows(gatesBody).map((cells) => ({
    gate: stripBackticks(cells[0]),
    script: stripBackticks(cells[1]),
    scriptName: stripBackticks(cells[2]),
  }));

  return {
    hasVocabulary: vocabularyBody !== null,
    hasMatrix: matrixBody !== null,
    hasMarkers: markersBody !== null,
    hasGates: gatesBody !== null,
    vocabulary,
    matrix,
    declaredMarkers,
    declaredCensus,
    gates,
  };
}

/// Collects every platform marker under `crates/<crate>/src`. Test code is included on purpose:
/// a platform-conditional test is still a platform claim, and excluding it would let the runtime
/// become Windows-only behind a green suite.
export function collectPlatformMarkers(root) {
  const cratesDirectory = join(root, "crates");
  if (!existsSync(cratesDirectory)) {
    return [];
  }
  const findings = [];
  for (const entry of readdirSync(cratesDirectory, { withFileTypes: true })) {
    if (!entry.isDirectory()) {
      continue;
    }
    const sourceDirectory = join(cratesDirectory, entry.name, "src");
    if (!existsSync(sourceDirectory)) {
      continue;
    }
    for (const file of listRustFiles(sourceDirectory)) {
      const relative = toPosix(file.slice(root.length + 1));
      const lines = readFileSync(file, "utf8").split(/\r?\n/u);
      for (const [index, line] of lines.entries()) {
        for (const marker of PLATFORM_MARKERS) {
          if (marker.pattern.test(line)) {
            findings.push({ path: relative, line: index + 1, marker: marker.id });
          }
        }
      }
    }
  }
  return findings;
}

function listRustFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const full = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...listRustFiles(full));
    } else if (entry.name.endsWith(".rs")) {
      files.push(full);
    }
  }
  return files;
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

export function assessPlatformRegime({
  root,
  documentContent,
  markers,
  packageJson,
  indexContent,
  architectureEntry,
  techReadme,
  hostCapabilityIds = [],
}) {
  const problems = [];
  const document = parsePlatformDocument(documentContent);

  for (const [key, heading] of Object.entries(SECTIONS)) {
    const flag = `has${key.charAt(0).toUpperCase()}${key.slice(1)}`;
    if (!document[flag]) {
      problems.push(`${PLATFORM_DOCUMENT} is missing the "${heading}" section`);
    }
  }

  // Rule family: vocabulary and matrix.
  for (const platform of PLATFORM_IDS) {
    if (!document.vocabulary.includes(platform)) {
      problems.push(`${SECTIONS.vocabulary} does not declare platform ${platform}`);
    }
  }
  for (const declared of document.vocabulary) {
    if (!PLATFORM_IDS.includes(declared)) {
      problems.push(
        `${SECTIONS.vocabulary} declares platform ${declared}, which is not in the tool's fixed vocabulary`,
      );
    }
  }
  const matrixPlatforms = document.matrix.map((row) => row.platform);
  for (const platform of PLATFORM_IDS) {
    if (!matrixPlatforms.includes(platform)) {
      problems.push(`${SECTIONS.matrix} has no row for platform ${platform}`);
    }
  }
  for (const row of document.matrix) {
    if (!PLATFORM_IDS.includes(row.platform)) {
      problems.push(`${SECTIONS.matrix} has an unknown platform row ${row.platform}`);
    }
    if (!STATUS_IDS.includes(row.status)) {
      problems.push(
        `${SECTIONS.matrix} row ${row.platform} uses status "${row.status}", which is outside the status vocabulary`,
      );
    }
    const cited = [...row.evidence.matchAll(/`([^`]+)`/gu)].map((match) => match[1]);
    const anchored = cited.some((target) => {
      const kind = classifyCitation(target);
      return kind === "path" || kind === "capability";
    });
    if (!anchored) {
      problems.push(
        `${SECTIONS.matrix} row ${row.platform} cites no repository path and no host-capability id, so the claim is unanchored`,
      );
    }
    for (const target of cited) {
      const kind = classifyCitation(target);
      if (kind === "path" && !existsSync(resolve(root, target))) {
        problems.push(
          `${SECTIONS.matrix} row ${row.platform} cites ${target}, which does not resolve from the repository root`,
        );
      }
      if (kind === "capability" && !hostCapabilityIds.includes(target)) {
        problems.push(
          `${SECTIONS.matrix} row ${row.platform} cites capability ${target}, which is not in the probe vocabulary`,
        );
      }
    }
    if (row.status === "verified" && !/\bREQ-\d{4}-\d{4}\b|\bADR-\d{8}\b|`docs\//u.test(row.evidence)) {
      problems.push(
        `${SECTIONS.matrix} row ${row.platform} claims "verified" without citing a requirement, decision or document path`,
      );
    }
  }

  // Rule family: platform-conditional code declarations.
  const detectedKeys = new Set(markers.map((marker) => `${marker.path}\u0000${marker.marker}`));
  const declaredKeys = new Set(
    document.declaredMarkers.map((marker) => `${marker.path}\u0000${marker.marker}`),
  );
  for (const marker of markers) {
    if (!declaredKeys.has(`${marker.path}\u0000${marker.marker}`)) {
      problems.push(
        `PLATFORM-CODE-UNDECLARED ${marker.path}:${marker.line} uses marker "${marker.marker}" but ${SECTIONS.markers} does not declare it`,
      );
    }
  }
  for (const declared of document.declaredMarkers) {
    if (!detectedKeys.has(`${declared.path}\u0000${declared.marker}`)) {
      problems.push(
        `${SECTIONS.markers} declares ${declared.path} / ${declared.marker}, which no longer appears in the source`,
      );
    }
    if (!PLATFORM_MARKER_IDS.includes(declared.marker)) {
      problems.push(
        `${SECTIONS.markers} declares marker "${declared.marker}", which is not one of the known markers (${PLATFORM_MARKER_IDS.join(", ")})`,
      );
    }
    if (!PLATFORM_IDS.includes(declared.platform)) {
      problems.push(
        `${SECTIONS.markers} declares ${declared.path} for platform "${declared.platform}", which is outside the vocabulary`,
      );
    }
    if (declared.reason.trim().length < 8) {
      problems.push(`${SECTIONS.markers} declares ${declared.path} without a usable reason`);
    }
  }
  if (document.declaredCensus === null) {
    problems.push(`${SECTIONS.markers} must state 当前声明：N so the census is explicit`);
  } else if (document.declaredCensus !== markers.length) {
    problems.push(
      `${SECTIONS.markers} states 当前声明：${document.declaredCensus} but the source carries ${markers.length} platform marker(s)`,
    );
  }

  // Rule family: the gates that back the claim must exist and be wired.
  if (document.gates.length === 0) {
    problems.push(`${SECTIONS.gates} must name at least one portability gate`);
  }
  for (const gate of document.gates) {
    if (!existsSync(resolve(root, gate.script))) {
      problems.push(`${SECTIONS.gates} names gate script ${gate.script}, which does not exist`);
    }
    const command = packageJson.scripts ? packageJson.scripts[gate.scriptName] : undefined;
    if (command === undefined) {
      problems.push(
        `${SECTIONS.gates} names package.json script "${gate.scriptName}", which is not declared`,
      );
    } else if (!command.includes(gate.script.split("/").pop())) {
      problems.push(
        `package.json script "${gate.scriptName}" does not invoke ${gate.script}`,
      );
    }
  }

  // Registration.
  if (!architectureEntry.includes("TECH-platform-support.md")) {
    problems.push(`${ARCHITECTURE_ENTRY} does not link ${PLATFORM_DOCUMENT}`);
  }
  if (!techReadme.includes("TECH-platform-support.md")) {
    problems.push(`${TECH_README} does not link ${PLATFORM_DOCUMENT}`);
  }
  if (!indexContent.includes("TECH-platform-support.md")) {
    problems.push(`${DOCS_INDEX} does not register ${PLATFORM_DOCUMENT}`);
  }

  return {
    document,
    markerCount: markers.length,
    markers,
    problems,
    platformCount: PLATFORM_IDS.length,
  };
}

export function formatPlatformRegimeReport(result) {
  const lines = [];
  if (result.problems.length === 0) {
    lines.push(
      `sandbox platform regime passed: ${result.platformCount} platform(s), ${result.markerCount} platform-conditional marker(s) declared`,
    );
    for (const row of result.document.matrix) {
      lines.push(`  ${row.platform.padEnd(18)} ${row.status}`);
    }
    return `${lines.join("\n")}\n`;
  }
  lines.push(`sandbox platform regime failed with ${result.problems.length} problem(s):`);
  for (const problem of result.problems) {
    lines.push(`  - ${problem}`);
  }
  return `${lines.join("\n")}\n`;
}

export function parsePlatformRegimeArgs(argv) {
  let root = null;
  let json = false;
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--root") {
      const value = argv[index + 1];
      if (value === undefined) {
        throw new Error("--root requires a directory");
      }
      root = value;
      index += 1;
    } else if (argument === "--json") {
      json = true;
    } else {
      throw new Error(`unsupported argument ${argument}`);
    }
  }
  return { root, json };
}

export function assessPlatformRegimeAtRoot(root) {
  const documentPath = join(root, PLATFORM_DOCUMENT);
  if (!existsSync(documentPath)) {
    return {
      document: null,
      markerCount: 0,
      markers: [],
      platformCount: PLATFORM_IDS.length,
      problems: [`${PLATFORM_DOCUMENT} does not exist`],
    };
  }
  const readIfPresent = (relative) => {
    const full = join(root, relative);
    return existsSync(full) ? readFileSync(full, "utf8") : "";
  };
  return assessPlatformRegime({
    root,
    documentContent: readFileSync(documentPath, "utf8"),
    markers: collectPlatformMarkers(root),
    packageJson: existsSync(join(root, "package.json"))
      ? readJson(join(root, "package.json"))
      : {},
    indexContent: readIfPresent(DOCS_INDEX),
    architectureEntry: readIfPresent(ARCHITECTURE_ENTRY),
    techReadme: readIfPresent(TECH_README),
    // The probe vocabulary is imported rather than restated, so a capability id renamed in the
    // probe tool cannot keep passing here.
    hostCapabilityIds: HOST_CAPABILITY_IDS,
  });
}

export function main(argv) {
  const options = parsePlatformRegimeArgs(argv);
  const root = options.root ? resolve(process.cwd(), options.root) : resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const result = assessPlatformRegimeAtRoot(root);
  if (options.json) {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  } else {
    process.stdout.write(formatPlatformRegimeReport(result));
  }
  if (result.problems.length > 0) {
    process.exitCode = 1;
  }
}

const invokedDirectly =
  process.argv[1] &&
  resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url));
if (invokedDirectly) {
  try {
    main(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`sandbox platform regime check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
