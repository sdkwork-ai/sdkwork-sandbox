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
 * Eight deterministic rule families:
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
 *   7. RESIDUAL GAPS. The section 3.2 coverage-gap table must be typed and checkable. A gap row is
 *      a claim about what the repository does *not* have, and untyped it cannot be falsified:
 *      "no benchmark suite exists" and "nothing gates the benchmark suite" read alike, while the
 *      first is refuted by a directory listing. Each row therefore declares a kind --
 *      `治理阻塞` (a human decision or a ready `REQ-*` is required), `缺门禁` (the artifact exists,
 *      nothing keeps it true), `缺产物` (the artifact does not exist) -- and names the artifacts it
 *      is about. The gate then checks the direction the claim asserts: a `缺产物` row's paths must
 *      all be absent, a `缺门禁` row's paths must all exist, and a `治理阻塞` row must name a `REQ-*`
 *      that both exists and is still short of `ready` -- a blocker blamed on a ready requirement is
 *      one that has already been lifted and is now misdirecting the reader, and one blamed on a
 *      non-existent id is one nobody can lift. The repository carried the failure this rule exists
 *      for: the table asserted no benchmark suite existed while `tools/bench-sandbox-lifecycle.mjs`
 *      and a published two-platform baseline were sitting in the tree.
 *   8. ZERO-REQUIREMENT CLAIMS. The document asserts, in a dozen places, that some capability has no
 *      requirement behind it. That is a claim about the requirement records, and the records can
 *      refute it: the section 3.4 registry declares which capabilities are asserted unowned, what
 *      keywords stand for each, and the gate re-derives ownership from every record's id, slug and
 *      title. A keyword that now matches a record turns the row red. The claim may only be asserted
 *      from a registry row, so every such phrasing elsewhere in the document must cite one
 *      (`〔§3.4/N〕`) and every row must be cited at least once -- the same two-way accounting rule 7
 *      of the field gate applies to operations. This is the rule that would have caught the false
 *      row rule 7 found: a row asserting no requirement owned the performance baseline, with
 *      `REQ-2026-0019-sandbox-runtime-pool-and-fast-allocation` in the tree.
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

/**
 * The kinds a residual gap may declare. The vocabulary is closed: a new kind means a new existence
 * rule, so inventing one must be a deliberate change to this gate rather than a new adjective.
 */
export const GAP_KINDS = Object.freeze(["治理阻塞", "缺门禁", "缺产物"]);

export const GAP_COLUMNS = Object.freeze(["优先级", "空档", "性质", "取证", "说明"]);

/**
 * The section 3.2 coverage-gap table. Rows carry a kind and the artifacts the claim is about, so
 * the gate can check the absence assertion instead of taking it on trust.
 */
export function parseCoverageGaps(text) {
  const lines = text.split(/\r?\n/u);
  const start = lines.findIndex((line) => /^###\s*3\.2\s/u.test(line));
  if (start === -1) return null;
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    // Any following subsection ends the table, so inserting 3.4 above the gates section cannot
    // silently extend the gap table's scope into it.
    if (/^###\s/u.test(lines[index]) || /^##\s*4\.\s/u.test(lines[index])) {
      end = index;
      break;
    }
  }
  let header = null;
  const rows = [];
  const malformed = [];
  for (let index = start + 1; index < end; index += 1) {
    const line = lines[index].trim();
    if (!line.startsWith("|")) {
      if (header && rows.length > 0) break;
      continue;
    }
    const cells = line
      .replace(/^\|/u, "")
      .replace(/\|$/u, "")
      .split("|")
      .map((cell) => cell.trim());
    if (cells.every((cell) => /^:?-{2,}:?$/u.test(cell))) continue;
    if (!header) {
      header = cells;
      continue;
    }
    if (cells.length !== (header.length || 0)) {
      malformed.push({ line: index + 1, cells });
      continue;
    }
    const record = {};
    GAP_COLUMNS.forEach((column, position) => {
      record[column] = cells[position] ?? "";
    });
    record.line = index + 1;
    rows.push(record);
  }
  return { header, rows, malformed };
}

/** Backticked tokens in a cell: requirement ids and repository-relative paths. */
export function parseGapEvidence(cell) {
  const evidence = { requirements: [], paths: [] };
  for (const match of String(cell).matchAll(/`([^`]+)`/gu)) {
    const token = match[1].trim();
    if (/^REQ-\d{4}-\d{4}$/u.test(token)) evidence.requirements.push(token);
    else if (token.includes("/") && !/\s/u.test(token)) evidence.paths.push(token);
  }
  return evidence;
}

export const REQUIREMENTS_DIRECTORY = "docs/product/requirements";

/**
 * The phrasings this document uses to assert that a capability has no requirement behind it. The
 * list is written out on purpose: it is a closed set, so inventing a twelfth way to say "unowned"
 * is a deliberate change to this gate rather than a sentence nobody checks. Both are anchored on
 * the literal `REQ-*` token, which keeps ordinary prose about capabilities readable.
 */
export const ZERO_REQUIREMENT_PATTERNS = Object.freeze([
  "(?:尚无任何|尚无|无|没有任何|零)\\s*`REQ-\\*`",
  "`REQ-\\*`\\s*为零",
]);

/** The citation a zero-requirement claim must carry: which registry row backs it. */
export const CLAIM_MARKER_PATTERN = "〔§3\\.4\\/(\\d+)〕";

/** The section the registry lives in. It is the one place the claim may be asserted, and it is
 *  exempt from the citation scan because it has to be able to quote the phrasings it defines. */
export const CLAIM_SECTION = "### 3.4";
export const CLAIM_COLUMNS = Object.freeze(["#", "主题", "关键词", "说明"]);

/**
 * The section 3.4 registry: which capabilities this document asserts are unowned, and the keywords
 * that stand for each. The keywords are the falsifiable part -- they are matched against every
 * requirement record, so the claim dies the moment a record claims the capability.
 */
export function parseRequirementClaimRegistry(text) {
  const lines = text.split(/\r?\n/u);
  const start = lines.findIndex((line) => line.trim().startsWith(CLAIM_SECTION));
  if (start === -1) return null;
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^###\s/u.test(lines[index]) || /^##\s*4\.\s/u.test(lines[index])) {
      end = index;
      break;
    }
  }
  let header = null;
  const rows = [];
  const malformed = [];
  for (let index = start + 1; index < end; index += 1) {
    const line = lines[index].trim();
    if (!line.startsWith("|")) continue;
    const cells = line
      .replace(/^\|/u, "")
      .replace(/\|$/u, "")
      .split("|")
      .map((cell) => cell.trim());
    if (cells.every((cell) => /^:?-{2,}:?$/u.test(cell))) continue;
    if (!header) {
      header = cells;
      continue;
    }
    if (cells.length !== (header.length || 0)) {
      malformed.push({ line: index + 1, cells });
      continue;
    }
    const record = {};
    CLAIM_COLUMNS.forEach((column, position) => {
      record[column] = cells[position] ?? "";
    });
    record.line = index + 1;
    rows.push(record);
  }
  return { header, rows, malformed, start: start + 1, end };
}

/**
 * Every requirement record, reduced to the parts that can assert ownership: the id, the file slug
 * and the title. The body is deliberately excluded -- a requirement that says "resume cursor" or
 * "Pause/Resume of a key rotation" is talking about something else, and treating those mentions as
 * ownership would make every claim in the registry permanently red for the wrong reason. The
 * narrower reading is the honest one, and it is stated in the document next to the registry.
 */
export function requirementOwnershipIndex(repoRoot) {
  const directory = join(repoRoot, REQUIREMENTS_DIRECTORY);
  if (!existsSync(directory)) return [];
  return readdirSync(directory)
    .filter((name) => /^REQ-\d{4}-\d{4}-.+\.md$/u.test(name))
    .sort()
    .map((name) => {
      const id = name.slice(0, "REQ-0000-0000".length);
      const slug = name.slice(id.length + 1, -".md".length);
      const text = readFileSync(join(directory, name), "utf8");
      const title = /^title:\s*(.+)$/mu.exec(text);
      const titleText = title ? title[1].trim() : "";
      return {
        id,
        file: `${REQUIREMENTS_DIRECTORY}/${name}`,
        needle: `${id} ${slug} ${titleText}`.toLowerCase(),
      };
    });
}

/** The registry row numbers a line cites. */
export function parseClaimMarkers(line) {
  const markers = [];
  for (const match of String(line).matchAll(new RegExp(CLAIM_MARKER_PATTERN, "gu"))) {
    markers.push(Number(match[1]));
  }
  return markers;
}

/** The keywords a registry row declares, lowercased. */
export function parseClaimKeywords(cell) {
  return [...String(cell).matchAll(/`([^`]+)`/gu)].map((match) => match[1].trim().toLowerCase());
}

/**
 * The status a named requirement currently declares, read from its record rather than from the
 * prose that cites it. A `治理阻塞` row is only a blocker while the requirement that owns it is
 * short of `ready`; reading the record is what makes the adjective falsifiable.
 */
export function readRequirementStatus(repoRoot, id) {
  const directory = join(repoRoot, REQUIREMENTS_DIRECTORY);
  if (!existsSync(directory)) return null;
  const name = readdirSync(directory).find(
    (entry) => entry.startsWith(`${id}-`) && entry.endsWith(".md"),
  );
  if (!name) return null;
  const text = readFileSync(join(directory, name), "utf8");
  const match = /^\s*(?:\*\*)?status(?:\*\*)?\s*[:：]\s*(?:\*\*)?([A-Za-z][A-Za-z-]*)/imu.exec(text);
  return { file: `${REQUIREMENTS_DIRECTORY}/${name}`, status: match ? match[1].toLowerCase() : null };
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
    gapCount: 0,
    claimCount: 0,
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
      // A static gate reports; it does not throw. The length check above normally makes this
      // unreachable, but a guard that only exists behind another guard is the kind of thing a
      // later edit removes, and an out-of-range read would surface as a stack trace instead of a
      // finding -- which reads like a broken tool rather than a broken document.
      if (!declared) return;
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

  // ---- 7. RESIDUAL GAPS
  const gaps = parseCoverageGaps(text);
  const gapColumnsMatch =
    gaps?.header !== null &&
    gaps?.header !== undefined &&
    gaps.header.length === GAP_COLUMNS.length &&
    gaps.header.join("|") === GAP_COLUMNS.join("|");
  if (!gaps) {
    problems.push(`${PARITY_DOCUMENT} has no "### 3.2" coverage-gap section`);
  } else if (!gaps.header) {
    problems.push(`${PARITY_DOCUMENT} section 3.2 has no coverage-gap table`);
  } else if (!gapColumnsMatch) {
    // A header mismatch means every cell below it is read at the wrong offset, so the per-row
    // findings would all be artefacts of the misparse. Report the root cause and stop, or the
    // reader chases five symptoms of one defect.
    problems.push(
      `${PARITY_DOCUMENT} section 3.2 gap table header is [${gaps.header.join(", ")}]; expected [${GAP_COLUMNS.join(", ")}]`,
    );
  } else {
    for (const row of gaps.malformed) {
      problems.push(
        `${PARITY_DOCUMENT}:${row.line} coverage-gap row has ${row.cells.length} cell(s), expected ${GAP_COLUMNS.length}`,
      );
    }
    if (gaps.rows.length === 0) {
      problems.push(
        `${PARITY_DOCUMENT} section 3.2 declares no coverage gap, which asserts the audit is complete; state the residual gaps or delete the section`,
      );
    }
    let expectedPriority = 1;
    for (const row of gaps.rows) {
      const priority = Number(row["优先级"]);
      const label = `section 3.2 gap row ${Number.isInteger(priority) ? priority : `at line ${row.line}`}`;
      if (!Number.isInteger(priority)) {
        problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} has a non-numeric priority`);
      } else {
        if (priority !== expectedPriority) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} breaks the priority sequence; expected ${expectedPriority}`,
          );
          expectedPriority = priority;
        }
        expectedPriority += 1;
      }
      const kind = row["性质"];
      if (!GAP_KINDS.includes(kind)) {
        problems.push(
          `${PARITY_DOCUMENT}:${row.line} ${label} declares kind "${kind || "none"}"; expected one of ${GAP_KINDS.join(" / ")}`,
        );
        continue;
      }
      const evidence = parseGapEvidence(row["取证"]);
      if (evidence.requirements.length === 0 && evidence.paths.length === 0) {
        problems.push(
          `${PARITY_DOCUMENT}:${row.line} ${label} names no backticked artifact, so the claim it makes cannot be checked`,
        );
        continue;
      }
      if (kind === "治理阻塞") {
        if (evidence.requirements.length === 0) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} is 治理阻塞 but names no \`REQ-*\`; a blocker with no requirement is one nobody can lift`,
          );
          continue;
        }
        for (const id of evidence.requirements) {
          const record = readRequirementStatus(repoRoot, id);
          if (!record) {
            problems.push(
              `${PARITY_DOCUMENT}:${row.line} ${label} blames \`${id}\`, which has no record in ${REQUIREMENTS_DIRECTORY}`,
            );
          } else if (record.status === "ready") {
            problems.push(
              `${PARITY_DOCUMENT}:${row.line} ${label} blames \`${id}\`, but \`${record.file}\` is already ready; re-triage the gap instead of repeating the blocker`,
            );
          }
        }
        continue;
      }
      if (evidence.paths.length === 0) {
        problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} is ${kind} but names no repository-relative path`);
        continue;
      }
      for (const path of evidence.paths) {
        const present = existsSync(join(repoRoot, path));
        if (kind === "缺产物" && present) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} declares \`${path}\` missing, but it exists`,
          );
        }
        if (kind === "缺门禁" && !present) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} declares \`${path}\` ungated, but ${path} does not exist`,
          );
        }
      }
    }
  }

  // ---- 8. ZERO-REQUIREMENT CLAIMS
  const claims = parseRequirementClaimRegistry(text);
  const ownership = requirementOwnershipIndex(repoRoot);
  const claimColumnsMatch =
    claims?.header?.length === CLAIM_COLUMNS.length &&
    claims.header.join("|") === CLAIM_COLUMNS.join("|");
  if (!claims) {
    problems.push(`${PARITY_DOCUMENT} has no "${CLAIM_SECTION}" zero-requirement claim registry`);
  } else if (claims.header && !claimColumnsMatch) {
    problems.push(
      `${PARITY_DOCUMENT} section 3.4 registry header is [${claims.header.join(", ")}]; expected [${CLAIM_COLUMNS.join(", ")}]`,
    );
  } else {
    for (const row of claims.malformed) {
      problems.push(
        `${PARITY_DOCUMENT}:${row.line} zero-requirement row has ${row.cells.length} cell(s), expected ${CLAIM_COLUMNS.length}`,
      );
    }
    if (claims.rows.length === 0) {
      problems.push(
        `${PARITY_DOCUMENT} section 3.4 registers no zero-requirement claim, which asserts that every capability has a requirement; state the claims or delete the section`,
      );
    }
    let expectedClaim = 1;
    for (const row of claims.rows) {
      const number = Number(row["#"]);
      const label = `section 3.4 claim ${Number.isInteger(number) ? number : `at line ${row.line}`}`;
      if (!Number.isInteger(number)) {
        problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} has a non-numeric number`);
      } else {
        if (number !== expectedClaim) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} breaks the claim numbering; expected ${expectedClaim}`,
          );
          expectedClaim = number;
        }
        expectedClaim += 1;
      }
      const keywords = parseClaimKeywords(row["关键词"]);
      if (keywords.length === 0) {
        problems.push(
          `${PARITY_DOCUMENT}:${row.line} ${label} names no backticked keyword, so nothing can refute it`,
        );
        continue;
      }
      for (const keyword of keywords) {
        if (!/^[a-z0-9-]+$/u.test(keyword)) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} declares keyword "${keyword}", which is not a lowercase token that can be matched against a record`,
          );
          continue;
        }
        const owner = ownership.find((record) => record.needle.includes(keyword));
        if (owner) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} claims no requirement owns \`${keyword}\`, but \`${owner.file}\` claims it in its id, slug or title`,
          );
        }
      }
    }

    // The claim may only be asserted from a registry row: every phrasing elsewhere has to say which
    // row backs it, and every row has to be used, or the registry is a place claims go to be
    // forgotten. The section itself is exempt because it has to quote the phrasings it defines.
    const rowNumbers = new Set(claims.rows.map((row) => Number(row["#"])));
    const cited = new Map([...rowNumbers].map((number) => [number, 0]));
    const claimPattern = new RegExp(ZERO_REQUIREMENT_PATTERNS.join("|"), "u");
    const lines = text.split(/\r?\n/u);
    lines.forEach((line, index) => {
      const lineNumber = index + 1;
      const inSection = lineNumber >= claims.start && lineNumber <= claims.end;
      if (!claimPattern.test(line)) return;
      if (inSection) return;
      const markers = parseClaimMarkers(line);
      if (markers.length === 0) {
        problems.push(
          `${PARITY_DOCUMENT}:${lineNumber} asserts a capability is unowned but cites no ${CLAIM_SECTION.replace("### ", "§")} row; register it and cite the row`,
        );
        return;
      }
      for (const marker of markers) {
        if (!rowNumbers.has(marker)) {
          problems.push(
            `${PARITY_DOCUMENT}:${lineNumber} cites ${CLAIM_SECTION.replace("### ", "§")} row ${marker}, which does not exist`,
          );
          continue;
        }
        cited.set(marker, (cited.get(marker) ?? 0) + 1);
      }
    });
    for (const [number, count] of cited) {
      if (count === 0) {
        problems.push(
          `${PARITY_DOCUMENT} section 3.4 row ${number} is cited nowhere, so nothing in the document rests on it; remove the row or cite it where the claim is made`,
        );
      }
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
    gapCount: gaps?.rows.length ?? 0,
    claimCount: claims?.rows.length ?? 0,
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
    `residual gaps: ${assessment.gapCount}`,
    `zero-requirement claims: ${assessment.claimCount}`,
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
