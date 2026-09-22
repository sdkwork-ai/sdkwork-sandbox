#!/usr/bin/env node
/**
 * Static gate: the E2B parity matrix must stay internally consistent and keep resolving.
 *
 * Why this exists. `docs/architecture/tech/TECH-e2b-capability-parity.md` is the repository's
 * authoritative answer to "is our capability set aligned with the mature microVM agent runtime
 * baseline". Every other audit artifact in this repository has a machine gate; this one had none,
 * and a capability matrix is exactly the artifact that rots silently: a row is added, the census
 * table above it is not, a `REQ-*` is renamed, a status value drifts to a synonym. None of that
 * fails `check-sandbox-doc-integrity.mjs` (which only checks that links resolve and prescribed
 * commands are runnable), nor `check-sandbox-requirement-traceability.mjs` (which only reads the
 * 34-row `PRD-capabilities.md` census). A census that does not add up is worse than no census,
 * because the number gets quoted in review.
 *
 * Six deterministic rule families:
 *
 *   1. VOCABULARY. The document must declare exactly the four status markers in its status
 *      vocabulary section, and every matrix row must use one of them. A fifth marker invented
 *      mid-document, or a synonym such as `部分`, is rejected.
 *   2. NUMBERING. Matrix rows are numbered 1..N, ascending, each exactly once. Gaps and
 *      duplicates break the cross-reference from the pull-request comment to the row.
 *   3. STATUS. Every row's status cell is one of the four declared markers. An empty or
 *      free-text status is rejected, because an unparseable status reads exactly like a
 *      deliberate one.
 *   4. CATEGORY ALIGNMENT. The census section's category rows must correspond, in order, to the
 *      matrix's subsection headings. This is what stops a new subsection from being added
 *      without a census row.
 *   5. CENSUS ARITHMETIC. Per-category counts are recomputed from the matrix and must equal the
 *      declared counts; the category rows must sum to the declared totals; the declared totals
 *      must equal the recomputed whole-document totals.
 *   6. CITATION RESOLUTION and REGISTRATION. Every `REQ-####-####` and `ADR-########-<slug>`
 *      token must resolve to a record in this repository (the same invariant
 *      `check-sandbox-requirement-traceability.mjs` enforces, restated here so this document
 *      cannot reference a record that does not exist), and the document must be linked from
 *      `TECH_ARCHITECTURE.md` and the tech README and registered in `docs/INDEX.yaml`.
 *
 * Usage:
 *   node tools/check-sandbox-e2b-parity-matrix.mjs
 *   node tools/check-sandbox-e2b-parity-matrix.mjs --json
 *   node tools/check-sandbox-e2b-parity-matrix.mjs --root <dir>
 */

import { readFileSync, existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const PARITY_DOCUMENT = "docs/architecture/tech/TECH-e2b-capability-parity.md";
export const STATUS_MARKERS = Object.freeze(["✅", "🟡", "❌", "⛔"]);
export const ARCHITECTURE_ENTRY = "docs/architecture/tech/TECH_ARCHITECTURE.md";
export const TECH_README = "docs/architecture/tech/README.md";
export const DOCS_INDEX = "docs/INDEX.yaml";

function normalizeHeading(value) {
  return value.replace(/[\s\u3000]+/gu, "").trim();
}

/**
 * Census cells are written with markdown emphasis (`| **合计** | **78** | ... |`). Emphasis must be
 * stripped before any numeric comparison: without it the total row parses as `null`, every total-row
 * assertion is skipped, and the gate reports the *recomputed* sum while claiming to have checked the
 * declared one. That silent hole is the exact class of defect this gate exists to prevent.
 */
function stripEmphasis(value) {
  return value.replace(/\*+/gu, "").replace(/^_+|_+$/gu, "").trim();
}

function splitTableRow(line) {
  const trimmed = line.trim();
  if (!trimmed.startsWith("|") || !trimmed.endsWith("|")) {
    return null;
  }
  return trimmed
    .slice(1, -1)
    .split("|")
    .map((cell) => cell.trim());
}

function isSeparatorRow(cells) {
  return cells.every((cell) => /^:?-{2,}:?$/u.test(cell.replace(/\s+/gu, "")));
}

export function parseStatusVocabulary(text) {
  const markers = [];
  for (const line of text.split(/\r?\n/u)) {
    const cells = splitTableRow(line);
    if (!cells || cells.length < 2 || isSeparatorRow(cells)) {
      continue;
    }
    const head = cells[0];
    const marker = STATUS_MARKERS.find((candidate) => head.startsWith(candidate));
    if (marker && !markers.includes(marker)) {
      markers.push(marker);
    }
  }
  return markers;
}

function sliceSection(text, startPattern, endPattern) {
  const start = text.search(startPattern);
  if (start < 0) {
    return null;
  }
  const rest = text.slice(start);
  const end = endPattern ? rest.slice(1).search(endPattern) : -1;
  return end < 0 ? rest : rest.slice(0, end + 1);
}

export function parseMatrixCategories(text) {
  const body = sliceSection(text, /^## 2\./mu, /^## 3\./mu);
  if (!body) {
    return null;
  }
  const lines = body.split(/\r?\n/u);
  const categories = [];
  let current = null;
  for (const line of lines) {
    const heading = /^###\s+2\.\d+\s+(.*)$/u.exec(line.trim());
    if (heading) {
      current = { title: heading[1].trim(), rows: [] };
      categories.push(current);
      continue;
    }
    if (!current) {
      continue;
    }
    const cells = splitTableRow(line);
    if (!cells || isSeparatorRow(cells)) {
      continue;
    }
    if (!/^\d+$/u.test(cells[0])) {
      continue;
    }
    current.rows.push({
      number: Number.parseInt(cells[0], 10),
      status: cells.length >= 4 ? cells[3] : "",
      evidence: cells.length >= 5 ? cells[4] : "",
      cellCount: cells.length,
      line,
    });
  }
  return categories;
}

export function parseCensus(text) {
  const body = sliceSection(text, /^### 1\.3/mu, /^## 2\./mu);
  if (!body) {
    return null;
  }
  const categories = [];
  const malformed = [];
  let total = null;
  for (const line of body.split(/\r?\n/u)) {
    const cells = splitTableRow(line);
    if (!cells || isSeparatorRow(cells) || cells.length < 6) {
      continue;
    }
    // A data row is identified by a numeric row count. The header row (`行数`) and separator rows
    // are skipped; anything else with a numeric row count but non-numeric state counts is reported
    // rather than silently dropped, because a census that quietly loses a category reads as a
    // smaller product than it is.
    if (!/^\d+$/u.test(stripEmphasis(cells[1]))) {
      continue;
    }
    const counts = {
      rows: Number.parseInt(stripEmphasis(cells[1]), 10),
      ok: Number.parseInt(stripEmphasis(cells[2]), 10),
      partial: Number.parseInt(stripEmphasis(cells[3]), 10),
      missing: Number.parseInt(stripEmphasis(cells[4]), 10),
      deliberate: Number.parseInt(stripEmphasis(cells[5]), 10),
    };
    if (["ok", "partial", "missing", "deliberate"].some((key) => !Number.isInteger(counts[key]))) {
      malformed.push(cells.join(" | "));
      continue;
    }
    const label = stripEmphasis(cells[0]);
    if (label.includes("合计") || /^total$/iu.test(label)) {
      total = counts;
    } else {
      categories.push({ label: cells[0], counts });
    }
  }
  return { categories, total, malformed };
}

function collectRequirementIds(repoRoot) {
  const directory = join(repoRoot, "docs", "product", "requirements");
  if (!existsSync(directory)) {
    return new Set();
  }
  const ids = new Set();
  for (const entry of readdirSync(directory)) {
    const match = /^(REQ-\d{4}-\d{4})-/u.exec(entry);
    if (match) {
      ids.add(match[1]);
    }
  }
  return ids;
}

function collectDecisionIds(repoRoot) {
  const directory = join(repoRoot, "docs", "architecture", "decisions");
  if (!existsSync(directory)) {
    return new Set();
  }
  const ids = new Set();
  for (const entry of readdirSync(directory)) {
    const match = /^(ADR-\d{8}-.+?)\.md$/u.exec(entry);
    if (match) {
      ids.add(match[1]);
    }
  }
  return ids;
}

function isInsideSiblingRepositoryPath(content, index) {
  const before = content.slice(0, index);
  const boundary = Math.max(
    before.lastIndexOf(" "),
    before.lastIndexOf("`"),
    before.lastIndexOf("("),
    before.lastIndexOf("["),
    before.lastIndexOf('"'),
    before.lastIndexOf("'"),
  );
  return /sdkwork-[a-z0-9-]+\//u.test(before.slice(boundary + 1));
}

function checkCitations(text, repoRoot) {
  const problems = [];
  const requirementIds = collectRequirementIds(repoRoot);
  const decisionIds = collectDecisionIds(repoRoot);
  const lines = text.split(/\r?\n/u);

  lines.forEach((line, index) => {
    for (const match of line.matchAll(/REQ-\d{4}-\d{4}/gu)) {
      const id = match[0];
      if (requirementIds.has(id) || isInsideSiblingRepositoryPath(line, match.index)) {
        continue;
      }
      problems.push(`${PARITY_DOCUMENT}:${index + 1} cites ${id}, which resolves to no requirement record`);
    }
    for (const match of line.matchAll(/ADR-\d{8}-[a-z0-9-]+/gu)) {
      const id = match[0];
      if (decisionIds.has(id) || isInsideSiblingRepositoryPath(line, match.index)) {
        continue;
      }
      if (line.includes(`${id}.md`)) {
        continue;
      }
      problems.push(`${PARITY_DOCUMENT}:${index + 1} cites ${id}, which resolves to no decision record`);
    }
  });

  return problems;
}

function checkRegistration(repoRoot) {
  const problems = [];
  const base = "TECH-e2b-capability-parity.md";
  for (const anchor of [ARCHITECTURE_ENTRY, TECH_README]) {
    const filePath = join(repoRoot, anchor);
    if (!existsSync(filePath)) {
      problems.push(`${anchor} does not exist, so it cannot link the parity document`);
      continue;
    }
    if (!readFileSync(filePath, "utf8").includes(base)) {
      problems.push(`${anchor} does not link ${base}; every architecture shard must be reachable from the entry document`);
    }
  }
  const indexPath = join(repoRoot, DOCS_INDEX);
  if (!existsSync(indexPath)) {
    problems.push(`${DOCS_INDEX} does not exist, so the parity document cannot be registered`);
  } else if (!readFileSync(indexPath, "utf8").includes(PARITY_DOCUMENT)) {
    problems.push(`${DOCS_INDEX} does not register ${PARITY_DOCUMENT}`);
  }
  return problems;
}

export function assessE2bParityMatrix({ repoRoot = process.cwd() } = {}) {
  const documentPath = join(repoRoot, PARITY_DOCUMENT);
  const problems = [];
  const empty = {
    ok: true,
    document: PARITY_DOCUMENT,
    declaredMarkers: [],
    categoryCount: 0,
    rowCount: 0,
    totals: { ok: 0, partial: 0, missing: 0, deliberate: 0 },
    problems,
  };

  if (!existsSync(documentPath)) {
    problems.push(`${PARITY_DOCUMENT} does not exist`);
    return { ...empty, ok: false };
  }

  const text = readFileSync(documentPath, "utf8");
  const declaredMarkers = parseStatusVocabulary(text);
  const categories = parseMatrixCategories(text);
  const census = parseCensus(text);

  if (!categories) {
    problems.push(`${PARITY_DOCUMENT} has no "## 2." matrix section`);
  }
  if (!census) {
    problems.push(`${PARITY_DOCUMENT} has no "### 1.3" census section`);
  }
  for (const row of census?.malformed ?? []) {
    problems.push(`${PARITY_DOCUMENT} census row "${row}" has a non-numeric state count`);
  }

  for (const marker of STATUS_MARKERS) {
    if (!declaredMarkers.includes(marker)) {
      problems.push(`${PARITY_DOCUMENT} does not declare the status marker ${marker} in its vocabulary`);
    }
  }
  if (declaredMarkers.length !== STATUS_MARKERS.length) {
    problems.push(
      `${PARITY_DOCUMENT} declares ${declaredMarkers.length} status marker(s) (${declaredMarkers.join(" ") || "none"}); exactly ${STATUS_MARKERS.length} are allowed`,
    );
  }

  if (!categories || !census) {
    return { ...empty, ok: false, declaredMarkers, problems };
  }

  const rows = categories.flatMap((category) => category.rows);
  const recomputed = { ok: 0, partial: 0, missing: 0, deliberate: 0 };
  let expectedNumber = 1;
  for (const category of categories) {
    for (const row of category.rows) {
      if (row.number !== expectedNumber) {
        problems.push(
          `${PARITY_DOCUMENT} row numbering breaks at ${row.number}; expected ${expectedNumber}`,
        );
        expectedNumber = row.number;
      }
      expectedNumber += 1;
      if (row.cellCount !== 5) {
        problems.push(
          `${PARITY_DOCUMENT} row ${row.number} has ${row.cellCount} cell(s); matrix rows must have 5 (# | capability | mapping | status | evidence)`,
        );
      }
      const marker = STATUS_MARKERS.find((candidate) => row.status.startsWith(candidate));
      if (!marker) {
        problems.push(
          `${PARITY_DOCUMENT} row ${row.number} has status "${row.status}", which is not one of ${STATUS_MARKERS.join(" ")}`,
        );
        continue;
      }
      if (/^✅/u.test(row.status)) recomputed.ok += 1;
      else if (/^🟡/u.test(row.status)) recomputed.partial += 1;
      else if (/^❌/u.test(row.status)) recomputed.missing += 1;
      else recomputed.deliberate += 1;
    }
  }

  if (categories.length !== census.categories.length) {
    problems.push(
      `${PARITY_DOCUMENT} has ${categories.length} matrix subsection(s) but ${census.categories.length} census category row(s); they must correspond one to one`,
    );
  } else {
    categories.forEach((category, index) => {
      const declared = census.categories[index];
      if (normalizeHeading(category.title) !== normalizeHeading(declared.label)) {
        problems.push(
          `${PARITY_DOCUMENT} census category ${index + 1} is "${declared.label}" but matrix subsection ${index + 1} is "${category.title}"`,
        );
      }
      const actual = {
        rows: category.rows.length,
        ok: category.rows.filter((row) => /^✅/u.test(row.status)).length,
        partial: category.rows.filter((row) => /^🟡/u.test(row.status)).length,
        missing: category.rows.filter((row) => /^❌/u.test(row.status)).length,
        deliberate: category.rows.filter((row) => /^⛔/u.test(row.status)).length,
      };
      for (const key of ["rows", "ok", "partial", "missing", "deliberate"]) {
        if (actual[key] !== declared.counts[key]) {
          problems.push(
            `${PARITY_DOCUMENT} census for "${declared.label}" declares ${key}=${declared.counts[key]}, but the matrix contains ${actual[key]}`,
          );
        }
      }
    });
  }

  const summed = census.categories.reduce(
    (accumulator, category) => ({
      rows: accumulator.rows + category.counts.rows,
      ok: accumulator.ok + category.counts.ok,
      partial: accumulator.partial + category.counts.partial,
      missing: accumulator.missing + category.counts.missing,
      deliberate: accumulator.deliberate + category.counts.deliberate,
    }),
    { rows: 0, ok: 0, partial: 0, missing: 0, deliberate: 0 },
  );

  if (!census.total) {
    problems.push(
      `${PARITY_DOCUMENT} census declares no total row; without it the declared totals are never checked and only the recomputed sum would be reported as if it had been verified`,
    );
  } else {
    for (const key of ["rows", "ok", "partial", "missing", "deliberate"]) {
      if (summed[key] !== census.total[key]) {
        problems.push(
          `${PARITY_DOCUMENT} census total row declares ${key}=${census.total[key]}, but the category rows sum to ${summed[key]}`,
        );
      }
    }
    if (census.total.rows !== rows.length) {
      problems.push(
        `${PARITY_DOCUMENT} census total declares ${census.total.rows} row(s), but the matrix contains ${rows.length}`,
      );
    }
    for (const key of ["ok", "partial", "missing", "deliberate"]) {
      if (census.total[key] !== recomputed[key]) {
        problems.push(
          `${PARITY_DOCUMENT} census total declares ${key}=${census.total[key]}, but the matrix contains ${recomputed[key]}`,
        );
      }
    }
    if (census.total.ok + census.total.partial + census.total.missing + census.total.deliberate !== census.total.rows) {
      problems.push(
        `${PARITY_DOCUMENT} census total row is not a partition: ${census.total.ok}+${census.total.partial}+${census.total.missing}+${census.total.deliberate} != ${census.total.rows}`,
      );
    }
  }

  problems.push(...checkCitations(text, repoRoot));
  problems.push(...checkRegistration(repoRoot));

  return {
    ok: problems.length === 0,
    document: PARITY_DOCUMENT,
    declaredMarkers,
    categoryCount: categories.length,
    rowCount: rows.length,
    totals: census.total ?? summed,
    problems,
  };
}

export function formatE2bParityMatrixReport(assessment) {
  const lines = [
    "SDKWork Sandbox E2B capability parity matrix",
    `document: ${assessment.document}`,
    `status markers declared: ${assessment.declaredMarkers.join(" ") || "none"}`,
    `categories: ${assessment.categoryCount}`,
    `rows: ${assessment.rowCount}`,
  ];
  if (assessment.totals) {
    lines.push(
      `census: ✅ ${assessment.totals.ok} | 🟡 ${assessment.totals.partial} | ❌ ${assessment.totals.missing} | ⛔ ${assessment.totals.deliberate}`,
    );
  }
  lines.push(assessment.ok ? "parity matrix: consistent" : "parity matrix: inconsistent");
  for (const problem of assessment.problems) {
    lines.push(`- ${problem}`);
  }
  return `${lines.join("\n")}\n`;
}

export function parseE2bParityMatrixArgs(argv) {
  const options = { root: process.cwd(), json: false };
  let index = 0;
  while (index < argv.length) {
    const argument = argv[index];
    if (argument === "--json") {
      options.json = true;
      index += 1;
    } else if (argument === "--root") {
      const value = argv[index + 1];
      if (!value) {
        throw new Error("--root requires a directory");
      }
      options.root = resolve(value);
      index += 2;
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
    const options = parseE2bParityMatrixArgs(process.argv.slice(2));
    const assessment = assessE2bParityMatrix({ repoRoot: options.root });
    process.stdout.write(
      options.json ? `${JSON.stringify(assessment, null, 2)}\n` : formatE2bParityMatrixReport(assessment),
    );
    if (!assessment.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(`sandbox E2B parity matrix check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
