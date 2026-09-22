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
 * Twelve deterministic rule families:
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
 *   9. IMPLEMENTATION COVERAGE. The section 3.1 table is the document's claim that a given
 *      implementation surface is covered by a given test. It asserted it listed "every real
 *      implementation in this repository" while the workspace held 68 tests across ten files and
 *      the table accounted for 49 of them: two repository crates and 19 tests were simply absent,
 *      so the most reassuring table in the audit was the one nothing checked. Every cited
 *      implementation path must exist (and a `:line` anchor must still be inside the file), every
 *      cited test name must be declared by the cited test file, and the accounting runs both ways
 *      -- every `#[test]`/`#[tokio::test]` the workspace declares must be cited exactly once, or
 *      the table is a sample presented as the whole.
 *  10. SELF-DESCRIPTION. Every surface describing this gate -- its own header, `tools/README.md`,
 *      root `README.md` and the Gate 0 view -- must declare the same number of rule families the
 *      gate implements, and that number is derived from the registry rather than typed. Adding
 *      family 9 above left two of the three prose surfaces still saying "eight": a gate that
 *      understates its own coverage is making exactly the claim this gate rejects when the parity
 *      document makes it.
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
export const DECISIONS_DIRECTORY = "docs/architecture/decisions";

/**
 * The record-status line every requirement and decision record carries. Written once and shared,
 * because two readers of one convention drift: a reader that matched `status` case-sensitively
 * would count the records that write `Status:` as having none, while the requirement reader beside
 * it was already case-insensitive.
 */
const RECORD_STATUS = /^\s*(?:\*\*)?status(?:\*\*)?\s*[:：]\s*(?:\*\*)?([A-Za-z][A-Za-z-]*)/imu;

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
  const match = RECORD_STATUS.exec(text);
  return { file: `${REQUIREMENTS_DIRECTORY}/${name}`, status: match ? match[1].toLowerCase() : null };
}

/** Every `*.md` record in a directory, reduced to its file name and declared status. */
function readRecordStatuses(repoRoot, relativeDirectory, pattern) {
  const directory = join(repoRoot, relativeDirectory);
  if (!existsSync(directory)) return [];
  return readdirSync(directory)
    .filter((name) => pattern.test(name))
    .sort()
    .map((name) => {
      const text = readFileSync(join(directory, name), "utf8");
      const match = RECORD_STATUS.exec(text);
      return {
        file: `${relativeDirectory}/${name}`,
        status: match ? match[1].toLowerCase() : null,
      };
    });
}

/**
 * Every requirement record with its declared status. The section 1.1 headline claims "27 份 `REQ-*`
 * 中 0 份 `ready`（5 `accepted` / 22 `draft`）", and that sentence is the reason the whole repository
 * is understood to be blocked, so it is counted rather than believed.
 */
export function readRequirementStatuses(repoRoot) {
  return readRecordStatuses(repoRoot, REQUIREMENTS_DIRECTORY, /^REQ-\d{4}-\d{4}-.+\.md$/u);
}

/** Every architecture decision record with its declared status. */
export function readDecisionStatuses(repoRoot) {
  return readRecordStatuses(repoRoot, DECISIONS_DIRECTORY, /^ADR-\d{8}-.+\.md$/u);
}

/**
 * The rule families this gate implements, in the order the header documents them. This list -- not
 * the prose -- is the authority for the count every registration surface declares. It is written
 * out rather than derived from the code because a gate cannot infer how many things it checks; what
 * it can do is make the number a single edit instead of a paragraph nobody revisits.
 */
export const RULE_FAMILIES = Object.freeze([
  "vocabulary",
  "numbering",
  "status",
  "category-alignment",
  "census-arithmetic",
  "citation-resolution-and-registration",
  "residual-gaps",
  "zero-requirement-claims",
  "implementation-coverage",
  "self-description",
  "shape-evidence",
  "headline-numbers",
]);

/** The surfaces that describe this gate, and the language each states the count in. */
export const RULE_FAMILY_SURFACES = Object.freeze([
  // `own` marks the surface that *is* this gate: it cannot confuse its count with another gate's,
  // so the count may sit in any of its comment blocks.
  { id: "gate header", path: "tools/check-sandbox-e2b-parity-matrix.mjs", language: "english", own: true },
  { id: "tools README", path: "tools/README.md", language: "english" },
  { id: "root README", path: "README.md", language: "english" },
  { id: "Gate 0 view", path: "docs/architecture/views/gate-zero-current-state.md", language: "chinese" },
  // The parity document is a surface that block scoping cannot read. It describes this gate and the
  // field gate inside one table -- this gate's row carries `11 条规则族：…`, the field gate's own count
  // sits on the row below with no blank line between -- so a whole-block scope would read both counts
  // as one claim and could never tell which gate was wrong. `lineScoped` narrows the scope to a single
  // line: only a line that names *this* gate can carry its count. That also disposes of the document's
  // ordinal references ("this is why the 9th rule family exists"), which name a family, not a total.
  { id: "parity document", path: PARITY_DOCUMENT, language: "chinese", lineScoped: true },
]);

const ENGLISH_COUNT_WORDS = Object.freeze({
  one: 1, two: 2, three: 3, four: 4, five: 5, six: 6,
  seven: 7, eight: 8, nine: 9, ten: 10, eleven: 11, twelve: 12,
});

const CHINESE_COUNT_WORDS = Object.freeze({
  一: 1, 二: 2, 两: 2, 三: 3, 四: 4, 五: 5, 六: 6,
  七: 7, 八: 8, 九: 9, 十: 10, 十一: 11, 十二: 12,
});

export const GATE_FILE_NAME = "check-sandbox-e2b-parity-matrix.mjs";

/**
 * Any sibling gate file. A following block that names one belongs to that gate's section, so it
 * cannot be the continuation of this gate's count statement. Without this, a section that states no
 * count of its own silently borrowed the number from the section below it.
 */
const ANY_GATE_FILE = /check-sandbox-[a-z0-9-]+\.mjs/u;

function safeReadText(absolutePath) {
  try {
    return readFileSync(absolutePath, "utf8");
  } catch {
    return null;
  }
}

/**
 * The candidate spans of a document that describe *this* gate, narrowest first. A file may describe
 * several gates, and the first count in such a file belongs to whichever gate is described first:
 * `tools/README.md` carries this gate's section above the field gate's. The naming block is tried
 * alone first, because a document that states the count in the sentence naming the gate must not
 * have the *next* paragraph's count attributed to it — reading the pair unconditionally let
 * `README.md`'s two adjacent sections pass by borrowing each other's number, which is the same
 * misattribution this scoping exists to prevent. Only when the naming block declares no count is
 * the following block admitted, since a count introduced as a standalone line ("Ten rule
 * families:") lands there by ordinary prose habit. A usage fence is not prose: it names the gate
 * file *after* the sentence declaring the count, and treating it as the naming block started the
 * scope one block too late. A surface that *is* this gate carries no ambiguity — its comment blocks
 * are separated by ` *` lines — so it opts out of scoping.
 */
function declarationScopes(text, { own = false } = {}) {
  const source = String(text);
  if (own) return [source];
  const blocks = source.split(/\n\s*\n/u);
  const namesGate = (block) =>
    block.includes(GATE_FILE_NAME) &&
    !block.split(/\r?\n/u).some((line) => line.trimStart().startsWith("```"));
  const index = blocks.findLastIndex(namesGate);
  if (index === -1) return [source];
  const scopes = [blocks[index]];
  const next = blocks[index + 1];
  // A count introduced as a standalone line ("Ten rule families:") lands in the next block, so the
  // next block is admitted -- unless it names another gate file, which makes it that gate's section
  // rather than the continuation of this one.
  if (next !== undefined && !ANY_GATE_FILE.test(next.replaceAll(GATE_FILE_NAME, ""))) {
    scopes.push(`${blocks[index]}\n\n${next}`);
  }
  return scopes;
}

/**
 * The count a span declares, or null when it declares none. The alternation is anchored on the
 * count words themselves: a looser `(\w+)\s+rule famil` reads "the gate then holds seven rule
 * families" as "holds" and blames the document for the parser's greediness. At most one adjective
 * may sit between the count and the noun, so "Ten deterministic rule families" parses like
 * "nine rule families".
 */
function readRuleFamilyCount(source, language) {
  if (language === "chinese") {
    const match = /([一二三四五六七八九十]+)\s*条规则/u.exec(source);
    return match ? CHINESE_COUNT_WORDS[match[1]] ?? null : null;
  }
  const pattern = new RegExp(
    `(${Object.keys(ENGLISH_COUNT_WORDS).join("|")})(?:\\s+[A-Za-z]+)?\\s+rule famil(?:y|ies)`,
    "iu",
  );
  const match = pattern.exec(source);
  return match ? ENGLISH_COUNT_WORDS[match[1].toLowerCase()] ?? null : null;
}

export function parseDeclaredRuleFamilies(text, language, { own = false } = {}) {
  for (const scope of declarationScopes(text, { own })) {
    const declared = readRuleFamilyCount(scope, language);
    if (declared !== null) return declared;
  }
  return null;
}

/**
 * Every rule-family count stated on a line that names `gateFileName`, with the line it sits on.
 *
 * This exists because one surface cannot be scoped by blocks at all: the audit document describes
 * this gate and the field gate in adjacent rows of the *same* markdown table. Splitting that table
 * into blocks is impossible (its rows are separated by newlines, not blank lines) and splitting it
 * any other way is guesswork, so the scope is narrowed to a line instead -- a line asserts a count
 * for this gate only if it names this gate. The reader also has to leave the document's ordinal
 * references alone: "this is why the 9th rule family exists" and "the 8th rule family re-reads
 * section 3.2" both contain `N 条规则族` and would otherwise be read as the gate declaring nine or
 * eight families. A leading `第` marks an ordinal, so it is excluded.
 */
export function parseLineScopedRuleFamilies(text, gateFileName) {
  const declared = [];
  const lines = String(text).split(/\r?\n/u);
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (!line.includes(gateFileName)) continue;
    // Two lookbehinds, because one is not enough. `第九条规则族` is excluded by the character class
    // (`第` immediately before the numeral), but `第 11 条规则族` is not: the space between them lets
    // `\d+` start at the *second* `1`, reading a declaration of eleven as one. The class also stops a
    // match from starting mid-number, so the digit-boundary and the ordinal are both needed.
    const match = /(?<![0-9第])(?<!第\s{0,3})(\d+|[一二三四五六七八九十]+)\s*条规则族/u.exec(line);
    if (!match) continue;
    const value = /^\d+$/u.test(match[1])
      ? Number.parseInt(match[1], 10)
      : CHINESE_COUNT_WORDS[match[1]] ?? null;
    if (value !== null) declared.push({ value, line: index + 1 });
  }
  return declared;
}

export const COVERAGE_SECTION = "### 3.1";
export const COVERAGE_COLUMNS = Object.freeze(["实现面", "实现点", "测试文件", "用例"]);
export const CRATES_DIRECTORY = "crates";

/**
 * A Rust test attribute. The argument list is part of the pattern on purpose: a test declared as
 * `#[tokio::test(flavor = "multi_thread", worker_threads = 4)]` carries arguments, and a pattern
 * requiring `]` immediately after `test` drops it silently. That is not hypothetical -- the first
 * draft of this rule counted 67 tests where the workspace holds 68, because the ignored test in
 * `crates/sdkwork-intelligence-sandbox-repository-sqlx/tests/postgres_repository.rs` is declared
 * that way. A coverage table built on the narrow pattern would have reported every test accounted
 * for while one was invisible: the same class of defect as the character class that once reported
 * 36 uncovered operations instead of 19.
 */
const TEST_ATTRIBUTE = /^\s*#\[(?:tokio::)?test\b[^\]]*\]/u;
const ATTRIBUTE_LINE = /^\s*#\[/u;
const IGNORE_ATTRIBUTE = /^\s*#\[ignore\b/u;
const TEST_FN = /^\s*(?:pub\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)/u;

function listRustFiles(repoRoot, relativeDirectory) {
  const found = [];
  for (const entry of readdirSync(join(repoRoot, relativeDirectory), { withFileTypes: true })) {
    const relative = `${relativeDirectory}/${entry.name}`;
    if (entry.isDirectory()) found.push(...listRustFiles(repoRoot, relative));
    else if (entry.name.endsWith(".rs")) found.push(relative);
  }
  return found;
}

/**
 * Every `#[test]`/`#[tokio::test]` in the workspace, keyed by repository-relative file. The
 * workspace is the authority for what a coverage table has to account for: a test that exists and
 * is not in the table is an implementation nobody claims to have covered, which is exactly what
 * the table exists to make visible.
 */
export function discoverWorkspaceTests(repoRoot) {
  if (!existsSync(join(repoRoot, CRATES_DIRECTORY))) return new Map();
  const byFile = new Map();
  for (const relative of listRustFiles(repoRoot, CRATES_DIRECTORY).sort()) {
    const lines = readFileSync(join(repoRoot, relative), "utf8").split(/\r?\n/u);
    const tests = [];
    for (let index = 0; index < lines.length; index += 1) {
      if (!TEST_ATTRIBUTE.test(lines[index])) continue;
      let ignored = IGNORE_ATTRIBUTE.test(lines[index]);
      let cursor = index + 1;
      while (cursor < lines.length && ATTRIBUTE_LINE.test(lines[cursor])) {
        if (IGNORE_ATTRIBUTE.test(lines[cursor])) ignored = true;
        cursor += 1;
      }
      const name = cursor < lines.length ? TEST_FN.exec(lines[cursor]) : null;
      if (name) tests.push({ name: name[1], ignored, line: index + 1 });
    }
    if (tests.length) byFile.set(relative, tests);
  }
  return byFile;
}

/** The section 3.1 implementation-coverage table: which surface each workspace test covers. */
export function parseImplementationCoverage(text) {
  const lines = text.split(/\r?\n/u);
  const start = lines.findIndex((line) => line.trim().startsWith(COVERAGE_SECTION));
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
    if (cells.length !== header.length) {
      malformed.push({ line: index + 1, cells });
      continue;
    }
    const record = {};
    COVERAGE_COLUMNS.forEach((column, position) => {
      record[column] = cells[position] ?? "";
    });
    record.line = index + 1;
    rows.push(record);
  }
  return { header, rows, malformed };
}

export const SHAPE_SECTION = "### 1.2";
export const SHAPE_COLUMNS = Object.freeze(["组件", "路径", "规模", "真实状态"]);

/** The top-level directories a citation may open with. A token below one is a path, not prose. */
const REPO_PATH_PREFIXES = Object.freeze([
  "crates", "apis", "sdks", "specs", "docs", "tools", "tests",
]);

/** `provider.rs:136` -- a file and the line the row points the reader at. */
const LINE_ANCHOR = /^([A-Za-z0-9_./-]+\.rs):(\d+)$/u;

/** `5 模块` -- how many Rust source files the crate's `src/` holds. */
const MODULE_COUNT = /^(\d+)\s*模块$/u;

/** `8 行 \`lib.rs\`` / `5 行` -- the physical line count of a Rust source file. */
const LINE_COUNT = /^(\d+)\s*行(?:\s*`([^`]+)`)?$/u;

/** A Rust type name: the form an absence claim takes when it names an implementation. */
const TYPE_NAME = /^[A-Z][A-Za-z0-9_]*$/u;

/** A bare lowercase word: the form an absence claim takes when it names a crate family. */
const BARE_WORD = /^[a-z][a-z0-9_]*$/u;

/** How far past an anchor the row's own vocabulary may appear, in lines. */
const ANCHOR_WINDOW = 3;

/** The section 1.2 shape-evidence table: a component, where it lives, its size and its real state. */
export function parseShapeEvidence(text) {
  const lines = text.split(/\r?\n/u);
  const start = lines.findIndex((line) => line.trim().startsWith(SHAPE_SECTION));
  if (start === -1) return null;
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^#{2,4}\s/u.test(lines[index])) {
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
    if (cells.length !== header.length) {
      malformed.push({ line: index + 1, cells });
      continue;
    }
    const record = {};
    SHAPE_COLUMNS.forEach((column, position) => {
      record[column] = cells[position] ?? "";
    });
    record.line = index + 1;
    rows.push(record);
  }
  return { header, rows, malformed };
}

/** Recursively list the `.rs` files under an absolute directory, sorted. */
function listRustSources(absoluteDirectory) {
  if (!existsSync(absoluteDirectory)) return [];
  const found = [];
  const walk = (directory) => {
    for (const entry of readdirSync(directory, { withFileTypes: true }).sort((a, b) =>
      a.name < b.name ? -1 : 1,
    )) {
      const child = join(directory, entry.name);
      if (entry.isDirectory()) walk(child);
      else if (entry.name.endsWith(".rs")) found.push(child);
    }
  };
  walk(absoluteDirectory);
  return found;
}

/**
 * The physical line count of a file, which is what `wc -l` prints for a newline-terminated source
 * file -- the convention section 1.2 states its `N 行` figures in. `split("\n").length` would report
 * one more for the same file, because a trailing newline leaves an empty final element; a reader who
 * runs `wc -l` and gets 8 while the table says 9 has been handed a number that does not reproduce.
 */
function countSourceLines(absolutePath) {
  const content = readFileSync(absolutePath, "utf8");
  if (content === "") return 0;
  const newlines = (content.match(/\n/gu) ?? []).length;
  return content.endsWith("\n") ? newlines : newlines + 1;
}

/** Every file under an absolute directory, as paths relative to it. */
function walkRelativeFiles(directory, prefix = "") {
  const found = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
    if (entry.isDirectory()) found.push(...walkRelativeFiles(join(directory, entry.name), relative));
    else found.push(relative);
  }
  return found;
}

/**
 * Whether a cited repository-relative path exists. A `*` in a segment is expanded rather than
 * believed, so `apis/commands/*.json` is checked as the set of files it names; a `**` segment walks
 * the subtree, so a row may cite a shape such as `crates/**\/*.rs` and still be falsified by a
 * directory listing. Without this the citation would read like evidence while proving nothing.
 */
function citedPathExists(repoRoot, cited) {
  const star = cited.indexOf("*");
  if (star === -1) return existsSync(join(repoRoot, cited));
  const slash = cited.lastIndexOf("/", star);
  const directory = slash === -1 ? repoRoot : join(repoRoot, cited.slice(0, slash));
  if (!existsSync(directory)) return false;
  const pattern = cited.slice(slash + 1);
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/gu, "\\$&");
  const regex = new RegExp(
    `^${escaped.replace(/\*\*/gu, "\u0000").replace(/\*/gu, "[^/]*").replace(/\u0000/gu, ".*")}$`,
    "u",
  );
  const candidates = pattern.includes("**") ? walkRelativeFiles(directory) : readdirSync(directory);
  return candidates.some((entry) => regex.test(entry));
}

/** The first `.rs` file under `crates/` containing `identifier`, repository-relative, or null. */
function findRustOccurrence(repoRoot, identifier) {
  if (!existsSync(join(repoRoot, CRATES_DIRECTORY))) return null;
  for (const relative of listRustFiles(repoRoot, CRATES_DIRECTORY).sort()) {
    if (readFileSync(join(repoRoot, relative), "utf8").includes(identifier)) return relative;
  }
  return null;
}

/** The first crate directory whose name contains `word` (case-insensitive), or null. */
function findCrateNamed(repoRoot, word) {
  const directory = join(repoRoot, CRATES_DIRECTORY);
  if (!existsSync(directory)) return null;
  const lowered = word.toLowerCase();
  return readdirSync(directory).find((entry) => entry.toLowerCase().includes(lowered)) ?? null;
}

/** A repository-relative path, with separators normalized for a finding. */
function repositoryPath(repoRoot, absolutePath) {
  return absolutePath.slice(repoRoot.length + 1).replace(/\\/gu, "/");
}

/** Backticked tokens in a cell. */
function backtickedTokens(cell) {
  return [...String(cell).matchAll(/`([^`]+)`/gu)].map((match) => match[1].trim());
}

export const ANSWER_SECTION = "### 1.1";
export const ANSWER_COLUMNS = Object.freeze(["路径", "E2B 的形态", "本仓现状"]);
export const EVIDENCE_REGISTRY = "specs/sandbox-real-evidence-registry.json";
export const CONTRACT_SUFFIX = ".contract.json";
export const API_SURFACE = "apis";

/** The directories a machine contract may live under. */
const CONTRACT_ROOTS = Object.freeze(["specs", "apis", "crates"]);

/**
 * Only the doubled emphasis is stripped. The document writes `` `REQ-*` `` -- a single asterisk
 * inside backticks -- and the blanket `\*+` strip used on census cells turns that token into
 * `` `REQ-` ``. After that the sentence this family exists to read stops matching, no finding is
 * produced, and the gate reports a clean section it never actually parsed.
 */
function deEmphasize(value) {
  return String(value).replace(/\*\*/gu, "");
}

/**
 * The restated numbers, as a closed set of patterns. Each is anchored on the document's own literal
 * phrasing, so a figure is only checked where the document makes the claim. The set is written out
 * for the same reason the zero-requirement phrasings are: a new way to state one of these numbers
 * has to be a deliberate edit here, not a sentence nobody reads. Every pattern carries only the
 * numbers it states, so the expected values line up position by position.
 */
export const HEADLINE_PATTERNS = Object.freeze([
  {
    id: "census",
    pattern:
      /E2B 的能力集合共\s*(\d+)\s*项[^。]*本仓\s*✅\s*(\d+)\s*、\s*🟡\s*(\d+)\s*、\s*❌\s*(\d+)\s*、\s*⛔\s*(\d+)/gu,
  },
  {
    id: "requirements",
    pattern: /(\d+)\s*份\s*`REQ-\*`\s*中\s*(\d+)\s*份\s*`ready`\s*（\s*(\d+)\s*`accepted`\s*\/\s*(\d+)\s*`draft`\s*）/gu,
  },
  { id: "decisions", pattern: /(\d+)\s*份\s*`ADR`\s*全部\s*`proposed`/gu },
  {
    id: "contracts",
    pattern: /(\d+)\s*份\s*`\*\.contract\.json`\s*中\s*(\d+)\s*份显式声明\s*`implementationAuthorized: false`/gu,
  },
  {
    id: "evidence-partial",
    pattern: /(\d+)\s*份契约声明的\s*(\d+)\s*个证据 id\s*中，只有\s*(\d+)\s*个有 host-precondition 半产出，(\d+)\s*个仍被/gu,
  },
  {
    id: "evidence-gated",
    pattern: /(\d+)\s*份契约声明的\s*(\d+)\s*个证据 id\s*中\s*(\d+)\s*个无产出者/gu,
  },
  { id: "evidence-registry", pattern: /(\d+)\s*个证据 id 的机器可读注册表/gu },
]);

/** The figure patterns section 1.1 itself must state, because that is where the claims are made. */
export const ANSWER_REQUIRED_FIGURES = Object.freeze([
  "census",
  "requirements",
  "decisions",
  "contracts",
  "evidence-partial",
]);

/** `另有两份不以 `.contract.json` 命名的机器契约（…）` -- the count is a numeral or 两. */
export const UNNAMED_CONTRACT_CLAUSE =
  /另有\s*([一二两三四五六七八九十]+|\d+)\s*份不以\s*`\.contract\.json`\s*命名的机器契约/u;

/** `第 23 份 `specs/…contract.json`` -- the one contract that declares no authorization field. */
export const EXCEPTION_CONTRACT_CLAUSE = /第\s*(\d+)\s*份\s*`([^`]+\.json)`/u;

/** The section that describes the gates themselves, and so quotes their assertions as examples. */
export const GATE_DESCRIPTION_SECTION = "### 3.3";

/**
 * A `### N.N` section's 1-based inclusive line range, or null when the heading is absent.
 */
function sectionRange(text, heading) {
  const lines = String(text).split(/\r?\n/u);
  const start = lines.findIndex((line) => line.trim().startsWith(heading));
  if (start === -1) return null;
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^#{2,4}\s/u.test(lines[index])) {
      end = index;
      break;
    }
  }
  return { start: start + 1, end };
}

/** The section 1.1 answer table: E2B's two core paths and this repository's state on each. */
export function parseAnswerSection(text) {
  const lines = String(text).split(/\r?\n/u);
  const start = lines.findIndex((line) => line.trim().startsWith(ANSWER_SECTION));
  if (start === -1) return null;
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^#{2,4}\s/u.test(lines[index])) {
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
    if (cells.length !== header.length) {
      malformed.push({ line: index + 1, cells });
      continue;
    }
    const record = {};
    ANSWER_COLUMNS.forEach((column, position) => {
      record[column] = cells[position] ?? "";
    });
    record.line = index + 1;
    rows.push(record);
  }
  return { header, rows, malformed, start: start + 1, end };
}

/** A JSON file's parsed value, or null when it is absent or not readable as JSON. */
export function readJsonFile(absolutePath) {
  if (!existsSync(absolutePath)) return null;
  try {
    return JSON.parse(readFileSync(absolutePath, "utf8"));
  } catch {
    return null;
  }
}

/**
 * Whether a machine contract authorizes implementation, by either field the repository's gates
 * recognize: `check-sandbox-human-review-signoff.mjs` reads `implementationAuthorized === true` and
 * `check-sandbox-commercial-readiness.mjs` reads
 * `releaseDecision.runtimeImplementationAuthorizationGranted === true`. Both are checked here so a
 * contract cannot slip through on the field this gate happened to pick.
 */
export function authorizesImplementation(value) {
  if (!value || typeof value !== "object") return false;
  return (
    value.implementationAuthorized === true ||
    value.releaseDecision?.runtimeImplementationAuthorizationGranted === true
  );
}

/** Every `*.contract.json` under the contract roots, repository-relative and sorted. */
export function listNamedContracts(repoRoot) {
  const found = [];
  for (const root of CONTRACT_ROOTS) {
    const absolute = join(repoRoot, root);
    if (!existsSync(absolute)) continue;
    for (const relative of walkRelativeFiles(absolute)) {
      if (relative.endsWith(CONTRACT_SUFFIX)) found.push(`${root}/${relative}`);
    }
  }
  return found.sort();
}

/**
 * The machine contracts that are not named `*.contract.json`. The document calls these "`apis/`
 * 机器契约" and names two, so the surface is the API contract directory rather than "every JSON
 * file that happens to carry the authorization field" -- `specs/sandbox-real-evidence-registry.json`
 * carries one too and is a producer registry, not a contract. Using the document's own distinction
 * is what keeps the count reproducible instead of a judgement call.
 */
export function listApiContracts(repoRoot) {
  const absolute = join(repoRoot, API_SURFACE);
  if (!existsSync(absolute)) return [];
  return walkRelativeFiles(absolute)
    .filter((relative) => relative.endsWith(".json") && !relative.endsWith(CONTRACT_SUFFIX))
    .map((relative) => `${API_SURFACE}/${relative}`)
    .filter((relative) => {
      const value = readJsonFile(join(repoRoot, relative));
      return value !== null && Object.hasOwn(value, "implementationAuthorized");
    })
    .sort();
}

/**
 * The recorded evidence counts, read from the registry rather than re-derived. Re-deriving them here
 * would mean a second copy of the "what counts as a required evidence id" rule, which is exactly the
 * drift the evidence gate exists to prevent; the registry's `acknowledged` block is the number that
 * gate keeps equal to the live contracts, so this gate compares the document to that.
 */
export function readEvidenceCounts(repoRoot) {
  const acknowledged = readJsonFile(join(repoRoot, EVIDENCE_REGISTRY))?.acknowledged;
  if (!acknowledged || typeof acknowledged !== "object") return null;
  return {
    contracts: acknowledged.sandbox_contracts_with_requirements,
    ids: acknowledged.sandbox_total_distinct_evidence_ids,
    partial: acknowledged.sandbox_host_precondition_partial,
    gated: acknowledged.sandbox_fully_gated,
  };
}

/**
 * A cited token that does not resolve, or null when it does. Two forms are recognized: a path under
 * a top-level repository directory, and a bare gate file name, which the document writes without its
 * `tools/` prefix.
 */
function unresolvedCitation(repoRoot, token) {
  if (REPO_PATH_PREFIXES.some((prefix) => token.startsWith(`${prefix}/`))) {
    return citedPathExists(repoRoot, token) ? null : token;
  }
  const gate = /^check-sandbox-[a-z0-9-]+\.mjs$/u.exec(token);
  if (gate) {
    return existsSync(join(repoRoot, "tools", token)) ? null : token;
  }
  return null;
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
    coveredTests: 0,
    workspaceTests: 0,
    shapeRows: 0,
    shapeAnchors: 0,
    answerRows: 0,
    headlineChecks: 0,
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

  // ---- 9. IMPLEMENTATION COVERAGE
  // Section 3.1 is the one place the document claims "this implementation is covered by this test".
  // Nothing read it, so the claim could go stale in three directions at once and stay green: a test
  // renamed, an implementation line moved, or -- the direction that actually failed here -- a whole
  // tested crate simply left off the list. The table asserted it was "every real implementation in
  // this repository" while the workspace held two repository crates and 19 of its 68 tests that the
  // table never mentioned. The rule closes the loop in both directions: every cited reference must
  // resolve, and every test the workspace declares must be cited exactly once.
  const coverage = parseImplementationCoverage(text);
  const discovered = discoverWorkspaceTests(repoRoot);
  const testsByFile = new Map([...discovered].map(([file, tests]) => [file, new Set(tests.map((test) => test.name))]));
  let workspaceTests = 0;
  for (const names of testsByFile.values()) workspaceTests += names.size;
  let coveredTests = 0;
  const coverageColumnsMatch =
    coverage?.header?.length === COVERAGE_COLUMNS.length &&
    coverage.header.join("|") === COVERAGE_COLUMNS.join("|");
  if (!coverage) {
    problems.push(`${PARITY_DOCUMENT} has no "${COVERAGE_SECTION}" implementation-coverage section`);
  } else if (!coverage.header) {
    problems.push(`${PARITY_DOCUMENT} section 3.1 has no implementation-coverage table`);
  } else if (!coverageColumnsMatch) {
    // As in 3.2: a header mismatch reads every cell below at the wrong offset, so per-row findings
    // would be artefacts of the misparse rather than defects in the document.
    problems.push(
      `${PARITY_DOCUMENT} section 3.1 implementation-coverage header is [${coverage.header.join(", ")}]; expected [${COVERAGE_COLUMNS.join(", ")}]`,
    );
  } else {
    for (const row of coverage.malformed) {
      problems.push(
        `${PARITY_DOCUMENT}:${row.line} implementation-coverage row has ${row.cells.length} cell(s), expected ${COVERAGE_COLUMNS.length}`,
      );
    }
    if (coverage.rows.length === 0) {
      problems.push(
        `${PARITY_DOCUMENT} section 3.1 accounts for no covered implementation surface, which asserts the repository has no tested implementation; state the surfaces or delete the section`,
      );
    }
    const cited = new Set();
    for (const row of coverage.rows) {
      const label = `section 3.1 coverage row at line ${row.line}`;
      const anchors = backtickedTokens(row["实现点"]);
      if (anchors.length === 0) {
        problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} names no backticked implementation path`);
      }
      for (const anchor of anchors) {
        const parsed = /^(.+?)(?::(\d+))?$/u.exec(anchor);
        const path = parsed[1];
        const absolute = join(repoRoot, path);
        if (!existsSync(absolute)) {
          problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} cites \`${anchor}\`, which does not exist`);
          continue;
        }
        // A line number is a snapshot of a file that keeps moving. It is allowed to be absent, so
        // the table can point at a whole module; when it is present it must still be inside the
        // file, or the reader following it lands past the end and trusts whatever they find there.
        if (parsed[2] !== undefined) {
          const total = readFileSync(absolute, "utf8").split(/\r?\n/u).length;
          if (Number(parsed[2]) < 1 || Number(parsed[2]) > total) {
            problems.push(
              `${PARITY_DOCUMENT}:${row.line} ${label} cites \`${anchor}\`, but ${path} has ${total} line(s)`,
            );
          }
        }
      }
      const files = backtickedTokens(row["测试文件"]);
      if (files.length !== 1) {
        problems.push(
          `${PARITY_DOCUMENT}:${row.line} ${label} names ${files.length} test file(s); exactly one is required so each case resolves against a known set`,
        );
        continue;
      }
      const testFile = files[0];
      const known = testsByFile.get(testFile);
      if (!known) {
        problems.push(
          `${PARITY_DOCUMENT}:${row.line} ${label} names \`${testFile}\` as a test file, but a file under ${CRATES_DIRECTORY} declares no \`#[test]\` at that path`,
        );
        continue;
      }
      const cases = backtickedTokens(row["用例"]);
      if (cases.length === 0) {
        problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} names no backticked test case`);
        continue;
      }
      for (const name of cases) {
        if (!known.has(name)) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} cites \`${name}\`, which \`${testFile}\` does not declare`,
          );
          continue;
        }
        const key = `${testFile}::${name}`;
        if (cited.has(key)) {
          problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} cites \`${name}\` a second time`);
        }
        cited.add(key);
      }
    }
    coveredTests = cited.size;
    for (const [file, names] of testsByFile) {
      for (const name of names) {
        if (!cited.has(`${file}::${name}`)) {
          problems.push(
            `${PARITY_DOCUMENT} section 3.1 does not account for \`${name}\` in \`${file}\`; every test the workspace declares must be claimed exactly once, or the coverage table is a sample presented as the whole`,
          );
        }
      }
    }
  }

  // ---- 10. SELF-DESCRIPTION
  // Every surface that describes this gate states how many rule families it implements. That count
  // is derived from RULE_FAMILIES rather than typed, so a family added without its description --
  // or a description left behind by a family that moved -- is a finding. The rot is not
  // hypothetical: adding the implementation-coverage family above left tools/README.md and the Gate
  // 0 view both still saying "eight", which is precisely the claim this gate exists to reject when
  // it appears in the parity document. Unlike the field gate's counterpart, this gate reports its
  // findings as prose rather than tagged records, so this rule checks the count and not the
  // attribution of individual findings; the count is the part that rots, so it is the part enforced.
  const selfSource = safeReadText(fileURLToPath(import.meta.url));
  for (const surface of RULE_FAMILY_SURFACES) {
    const surfaceText = surface.own ? selfSource : safeReadText(join(repoRoot, surface.path));
    if (surfaceText === null) {
      problems.push(
        `the ${surface.id} (${surface.path}) is missing, so it describes no rule families`,
      );
      continue;
    }
    if (surface.lineScoped === true) {
      // A surface that names this gate many times, in a document that also names another gate: every
      // line that names this gate *and* states a count is a separate claim, and all of them must
      // agree. Reading only the first would let the second rot; reading only the last is the
      // misattribution that `declarationScopes` exists to avoid.
      const readings = parseLineScopedRuleFamilies(surfaceText, GATE_FILE_NAME);
      if (readings.length === 0) {
        problems.push(
          `the ${surface.id} (${surface.path}) names this gate but never states how many rule families it implements; it must say the ${RULE_FAMILIES.length} this gate implements`,
        );
        continue;
      }
      for (const reading of readings) {
        if (reading.value !== RULE_FAMILIES.length) {
          problems.push(
            `the ${surface.id} (${surface.path}) line ${reading.line} declares ${reading.value} rule families, this gate implements ${RULE_FAMILIES.length}`,
          );
        }
      }
      continue;
    }
    const declared = parseDeclaredRuleFamilies(surfaceText, surface.language, { own: surface.own === true });
    if (declared === null) {
      problems.push(
        `the ${surface.id} (${surface.path}) declares no rule-family count; it must say how many of the ${RULE_FAMILIES.length} families this gate implements`,
      );
      continue;
    }
    if (declared !== RULE_FAMILIES.length) {
      problems.push(
        `the ${surface.id} declares ${declared} rule families, this gate implements ${RULE_FAMILIES.length}`,
      );
    }
  }

  // ---- 11. SHAPE EVIDENCE
  // Section 1.2 is headed "本仓当前真实形状（可点证据）" -- clickable evidence -- and it is the part of
  // the audit a reader trusts fastest and checks least. Each row names a component, a path, a size and
  // a state, and the state cells carry line-numbered citations such as `provider.rs:136`. Until this
  // family existed nothing resolved one of them: a stale line number, a crate that gained a production
  // implementation, or a state enum that grew `Pausing` would leave the table asserting the old shape
  // indefinitely -- and this table is what a reader quotes when asked "so what does the repository
  // actually have?". The rules make each claim falsifiable. A cited path must resolve (a `*` segment is
  // expanded, so `apis/commands/*.json` is checked as a set rather than believed). A size written
  // `N 模块` must equal the Rust sources under the crate's `src/`; one written `N 行` must equal the
  // file's real line count, stated in the `wc -l` convention so the number reproduces at a shell. Every
  // `file:N` anchor must land inside its file *and* have something the row attributes to it visible at
  // that line, because a line number that still resolves while pointing at unrelated code is the quiet
  // half of this rot: the reader follows it, finds a plausible declaration and believes the row. Every
  // row must cite backticked evidence, and a row that declares a component absent must name what is
  // absent -- an identifier that must not occur in `crates/**/*.rs`, or a crate name that must match no
  // directory -- so the absence is re-derived from the tree instead of asserted.
  const shape = parseShapeEvidence(text);
  let shapeRows = 0;
  let shapeAnchors = 0;
  if (!shape) {
    problems.push(`${PARITY_DOCUMENT} has no "${SHAPE_SECTION}" shape-evidence section`);
  } else if (!shape.header) {
    problems.push(`${PARITY_DOCUMENT} section 1.2 has no shape-evidence table`);
  } else if (
    shape.header.length !== SHAPE_COLUMNS.length ||
    shape.header.join("|") !== SHAPE_COLUMNS.join("|")
  ) {
    // As in 3.1 and 3.2: a header mismatch reads every cell below at the wrong offset, so per-row
    // findings would be artefacts of the misparse rather than defects in the document.
    problems.push(
      `${PARITY_DOCUMENT} section 1.2 shape-evidence header is [${shape.header.join(", ")}]; expected [${SHAPE_COLUMNS.join(", ")}]`,
    );
  } else {
    for (const row of shape.malformed) {
      problems.push(
        `${PARITY_DOCUMENT}:${row.line} shape-evidence row has ${row.cells.length} cell(s), expected ${SHAPE_COLUMNS.length}`,
      );
    }
    if (shape.rows.length === 0) {
      problems.push(
        `${PARITY_DOCUMENT} section 1.2 describes no component, which asserts the repository has no shape to describe; state the components or delete the section`,
      );
    }
    shapeRows = shape.rows.length;
    for (const row of shape.rows) {
      const label = `section 1.2 row at line ${row.line}`;
      const sizes = stripEmphasis(row["规模"]);
      const declaresAbsent = sizes.includes("不存在");
      const evidence = backtickedTokens(row["真实状态"]);

      if (stripEmphasis(row["组件"]).trim() === "") {
        problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} names no component`);
      }

      // A row points at exactly one path, or declares the component absent. Both halves are claims:
      // the first that a path exists under the repository root, the second that none does.
      const paths = backtickedTokens(row["路径"]);
      if (paths.length === 0 && !declaresAbsent) {
        problems.push(
          `${PARITY_DOCUMENT}:${row.line} ${label} names no backticked path and does not declare the component absent`,
        );
      }
      if (paths.length > 1) {
        problems.push(
          `${PARITY_DOCUMENT}:${row.line} ${label} names ${paths.length} paths; exactly one is required, because every citation in the state cell resolves relative to it`,
        );
      }
      const cratePath = paths.length === 1 ? paths[0] : null;
      if (cratePath !== null && !citedPathExists(repoRoot, cratePath)) {
        problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} cites \`${cratePath}\`, which does not exist`);
      }
      if (declaresAbsent && cratePath !== null) {
        problems.push(
          `${PARITY_DOCUMENT}:${row.line} ${label} declares the component absent yet still names \`${cratePath}\` as its path`,
        );
      }

      // "可点证据" -- clickable evidence. A row that points at nothing is the failure this family is
      // for, because it reads exactly like a row that points at something.
      if (evidence.length === 0) {
        problems.push(
          `${PARITY_DOCUMENT}:${row.line} ${label} cites no backticked evidence; a row in a section headed 可点证据 must point at an artifact a reader can open`,
        );
      }

      // Any token under a top-level repository directory is a path citation, in whichever cell it
      // appears -- `apis/commands/*.json` in a state cell is as checkable as a crate in the path cell.
      for (const column of SHAPE_COLUMNS) {
        for (const token of backtickedTokens(row[column])) {
          if (token === cratePath) continue;
          if (!REPO_PATH_PREFIXES.some((prefix) => token.startsWith(`${prefix}/`))) continue;
          if (!citedPathExists(repoRoot, token)) {
            problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} cites \`${token}\`, which does not exist`);
          }
        }
      }

      const moduleMatch = MODULE_COUNT.exec(sizes);
      if (moduleMatch) {
        if (cratePath === null) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} declares ${moduleMatch[1]} 模块 but names no path to count`,
          );
        } else {
          const sources = listRustSources(join(repoRoot, cratePath, "src"));
          if (sources.length !== Number.parseInt(moduleMatch[1], 10)) {
            problems.push(
              `${PARITY_DOCUMENT}:${row.line} ${label} declares ${moduleMatch[1]} 模块, but \`${cratePath}/src\` holds ${sources.length} Rust source file(s)`,
            );
          }
        }
      }

      const lineMatch = LINE_COUNT.exec(sizes);
      if (lineMatch) {
        const declaredLines = Number.parseInt(lineMatch[1], 10);
        const named = lineMatch[2] ?? null;
        let target = null;
        if (named !== null && cratePath !== null) {
          target = join(repoRoot, cratePath, "src", named);
        } else if (named === null && cratePath !== null) {
          const sources = listRustSources(join(repoRoot, cratePath, "src"));
          if (sources.length === 1) target = sources[0];
        }
        if (target === null || !existsSync(target)) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} declares ${declaredLines} 行 but names no single resolvable Rust source file to count`,
          );
        } else {
          const actual = countSourceLines(target);
          if (actual !== declaredLines) {
            problems.push(
              `${PARITY_DOCUMENT}:${row.line} ${label} declares ${declaredLines} 行, but the file it names holds ${actual} line(s)`,
            );
          }
        }
      }

      for (const token of evidence) {
        const anchor = LINE_ANCHOR.exec(token);
        if (!anchor) continue;
        shapeAnchors += 1;
        const relative = anchor[1];
        const line = Number.parseInt(anchor[2], 10);
        // A bare file name belongs to the row's own crate; anything with a slash is root-relative.
        const absolute =
          relative.includes("/") || cratePath === null
            ? join(repoRoot, relative)
            : join(repoRoot, cratePath, "src", relative);
        if (!existsSync(absolute)) {
          problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} cites \`${token}\`, which resolves to no file`);
          continue;
        }
        const total = countSourceLines(absolute);
        if (line < 1 || line > total) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} cites \`${token}\`, but \`${relative}\` has ${total} line(s)`,
          );
          continue;
        }
        const window = readFileSync(absolute, "utf8")
          .split(/\r?\n/u)
          .slice(line - 1, line + ANCHOR_WINDOW)
          .join(" ")
          .replace(/\s+/gu, " ");
        const attributed = evidence.filter((candidate) => candidate !== token);
        if (
          attributed.length > 0 &&
          !attributed.some((candidate) => window.includes(candidate.replace(/\s+/gu, " ")))
        ) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} cites \`${token}\`, but nothing the row attributes to it is visible at \`${relative}:${line}\``,
          );
        }
      }

      if (declaresAbsent) {
        const named = evidence.filter((token) => !LINE_ANCHOR.test(token));
        const types = named.filter((token) => TYPE_NAME.test(token));
        const words = named.filter((token) => BARE_WORD.test(token));
        if (types.length === 0 && words.length === 0) {
          problems.push(
            `${PARITY_DOCUMENT}:${row.line} ${label} declares the component absent but names nothing absent; cite the identifier or the crate name that must not exist, so the claim can be re-derived from the tree`,
          );
        }
        for (const token of types) {
          const occurrence = findRustOccurrence(repoRoot, token);
          if (occurrence !== null) {
            problems.push(
              `${PARITY_DOCUMENT}:${row.line} ${label} declares \`${token}\` absent, but it appears in ${occurrence}`,
            );
          }
        }
        for (const token of words) {
          const crate = findCrateNamed(repoRoot, token);
          if (crate !== null) {
            problems.push(
              `${PARITY_DOCUMENT}:${row.line} ${label} declares no \`${token}\` implementation, but crates/${crate} exists`,
            );
          }
        }
      }
      // A bolded negative phrase is a claim as well, and the rows that carry one name the constructs
      // they say are missing: `**无 `Pausing/Paused/Recovering`**` stops being true the moment the
      // enum grows one, and that particular row is the implementation half of the section 3.2 blocker
      // about the PRD state machine. So every identifier named inside such a phrase must be absent
      // from the file the row anchors -- the claim is about *that* construct in *that* file -- falling
      // back to the crate's `src/` when the row anchors nothing. Free-form negatives ("零生产实现",
      // "零命令") name nothing to look for and are instead grounded by their anchors: `fn main() {}`
      // sitting at `main.rs:3` is what makes "zero commands" checkable, not the phrase itself.
      for (const phrase of row["真实状态"].matchAll(/\*\*([^*]+)\*\*/gu)) {
        if (!/(?:无|零|不存在)/u.test(phrase[1])) continue;
        const named = backtickedTokens(phrase[1])
          .flatMap((token) => token.split("/"))
          .map((token) => token.trim())
          .filter((token) => token !== "" && !token.includes("*"));
        if (named.length === 0) continue;
        const anchorFiles = evidence
          .map((token) => LINE_ANCHOR.exec(token))
          .filter((anchor) => anchor !== null)
          .map((anchor) =>
            anchor[1].includes("/") || cratePath === null
              ? join(repoRoot, anchor[1])
              : join(repoRoot, cratePath, "src", anchor[1]),
          );
        const scope =
          anchorFiles.length > 0
            ? anchorFiles
            : cratePath === null
              ? []
              : listRustSources(join(repoRoot, cratePath, "src"));
        for (const identifier of named) {
          for (const file of scope) {
            if (!existsSync(file)) continue;
            if (!readFileSync(file, "utf8").includes(identifier)) continue;
            problems.push(
              `${PARITY_DOCUMENT}:${row.line} ${label} names \`${identifier}\` inside a negative phrase, but it occurs in ${repositoryPath(repoRoot, file)}`,
            );
          }
        }
      }
    }
  }

  // ---- 12. HEADLINE NUMBERS
  // Section 1.1 is headed "直接回答" -- the direct answer -- and it is the part of the audit a
  // reviewer quotes, because it is the part that says whether the capability set is aligned at all.
  // Every figure in it is a *copy* of a number that some other artifact owns: the census table one
  // section below, the requirement records, the decision records, the authorization field on each
  // machine contract, the evidence registry. Until this family existed not one of those copies was
  // compared to its original, and the section repeats whole sentences of itself -- the contract
  // paragraph appears in both 1.1 and 3.2 -- so a single stale copy would have been quoted twice.
  // The rule therefore reads every occurrence in the document, not just the first, and requires the
  // ones section 1.1 is built out of to be present there: a figure that quietly disappears leaves a
  // section that still reads like an answer while asserting nothing.
  const answer = parseAnswerSection(text);
  let answerRows = 0;
  let headlineChecks = 0;

  if (!answer) {
    problems.push(`${PARITY_DOCUMENT} has no "${ANSWER_SECTION}" answer section`);
  } else if (!answer?.header) {
    problems.push(`${PARITY_DOCUMENT} section 1.1 has no path table`);
  } else if (
    answer.header.length !== ANSWER_COLUMNS.length ||
    answer.header.join("|") !== ANSWER_COLUMNS.join("|")
  ) {
    problems.push(
      `${PARITY_DOCUMENT} section 1.1 path table header is [${answer.header.join(", ")}]; expected [${ANSWER_COLUMNS.join(", ")}]`,
    );
  } else {
    for (const row of answer.malformed) {
      problems.push(
        `${PARITY_DOCUMENT}:${row.line} section 1.1 row has ${row.cells.length} cell(s), expected ${ANSWER_COLUMNS.length}`,
      );
    }
    if (answer.rows.length !== 2) {
      problems.push(
        `${PARITY_DOCUMENT} section 1.1 lists ${answer.rows.length} path(s); the claim it backs is about E2B's two core paths, so it must list exactly two`,
      );
    }
    answerRows = answer.rows.length;
    const journeys = new Set();
    for (const row of answer.rows) {
      const label = `section 1.1 row at line ${row.line}`;
      const journey = stripEmphasis(row["路径"]).trim();
      if (journey === "") {
        problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} names no path`);
      } else if (journeys.has(journey)) {
        problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} repeats the path "${journey}"`);
      } else {
        journeys.add(journey);
      }
      // The state cell is where the row says what this repository has. A row whose state cites
      // nothing reads exactly like a row whose state cites something, which is why the section is
      // listed here at all.
      if (backtickedTokens(row["本仓现状"]).length === 0) {
        problems.push(
          `${PARITY_DOCUMENT}:${row.line} ${label} states this repository's position without citing a single backticked artifact`,
        );
      }
      for (const column of ANSWER_COLUMNS) {
        for (const token of backtickedTokens(row[column])) {
          const unresolved = unresolvedCitation(repoRoot, token);
          if (unresolved !== null) {
            problems.push(`${PARITY_DOCUMENT}:${row.line} ${label} cites \`${unresolved}\`, which does not exist`);
          }
        }
      }
    }
  }

  // The originals, recomputed from the artifacts themselves. The census figures come from the matrix
  // rows rather than from the census table, so a document that restates a wrong number in both places
  // still turns red -- comparing copy to copy would only prove the two copies agree.
  const requirementRecords = readRequirementStatuses(repoRoot);
  const decisionRecords = readDecisionStatuses(repoRoot);
  const namedContracts = listNamedContracts(repoRoot);
  const apiContracts = listApiContracts(repoRoot);
  const evidenceCounts = readEvidenceCounts(repoRoot);
  const declaresFalse = namedContracts.filter(
    (relative) => readJsonFile(join(repoRoot, relative))?.implementationAuthorized === false,
  );
  const declaresNothing = namedContracts.filter((relative) => !declaresFalse.includes(relative));

  const headlineExpectations = {
    census: [rows.length, recomputed.ok, recomputed.partial, recomputed.missing, recomputed.deliberate],
    requirements: [
      requirementRecords.length,
      requirementRecords.filter((record) => record.status === "ready").length,
      requirementRecords.filter((record) => record.status === "accepted").length,
      requirementRecords.filter((record) => record.status === "draft").length,
    ],
    decisions: [decisionRecords.length],
    contracts: [namedContracts.length, declaresFalse.length],
    "evidence-partial": evidenceCounts
      ? [evidenceCounts.contracts, evidenceCounts.ids, evidenceCounts.partial, evidenceCounts.gated]
      : null,
    "evidence-gated": evidenceCounts
      ? [evidenceCounts.contracts, evidenceCounts.ids, evidenceCounts.gated]
      : null,
    "evidence-registry": evidenceCounts ? [evidenceCounts.ids] : null,
  };

  const lines = text.split(/\r?\n/u);
  const occurrences = new Map(HEADLINE_PATTERNS.map(({ id }) => [id, []]));
  // Section 3.3 is about the gates, and it quotes their assertions while explaining them ("the
  // sentence that says every ADR is proposed"), so a match there is a quotation rather than a claim
  // about the repository. Reading it would fire on a description of a finding instead of on the
  // finding -- the same reason the zero-requirement registry is exempt from its own citation scan --
  // and a gate that cries wolf is a gate somebody eventually weakens. The exemption is narrow: it is
  // the one prose section about the tooling, not the audit's findings.
  const descriptionRange = sectionRange(text, GATE_DESCRIPTION_SECTION);
  const inDescription = (lineNumber) =>
    descriptionRange !== null &&
    lineNumber >= descriptionRange.start &&
    lineNumber <= descriptionRange.end;
  for (const { id, pattern } of HEADLINE_PATTERNS) {
    const expected = headlineExpectations[id];
    for (let index = 0; index < lines.length; index += 1) {
      if (inDescription(index + 1)) continue;
      const line = deEmphasize(lines[index]);
      for (const match of line.matchAll(pattern)) {
        const lineNumber = index + 1;
        occurrences.get(id).push(lineNumber);
        headlineChecks += 1;
        const stated = match.slice(1).map((value) => Number.parseInt(value, 10));
        if (!expected) {
          problems.push(
            `${PARITY_DOCUMENT}:${lineNumber} states a ${id} figure, but the artifact it restates could not be read, so nothing refutes it`,
          );
        } else if (stated.join(",") !== expected.join(",")) {
          problems.push(
            `${PARITY_DOCUMENT}:${lineNumber} states ${id} as [${stated.join(", ")}], but the repository yields [${expected.join(", ")}]`,
          );
        }
        for (const token of backtickedTokens(line)) {
          const unresolved = unresolvedCitation(repoRoot, token);
          if (unresolved !== null) {
            problems.push(`${PARITY_DOCUMENT}:${lineNumber} cites \`${unresolved}\`, which does not exist`);
          }
        }
      }
    }
  }

  if (answer) {
    for (const id of ANSWER_REQUIRED_FIGURES) {
      const inside = occurrences.get(id).some((line) => line >= answer.start && line <= answer.end);
      if (!inside) {
        problems.push(
          `${PARITY_DOCUMENT} section 1.1 states no ${id} figure; the answer section is where that claim belongs, so either state it there or remove the pattern deliberately`,
        );
      }
    }
  }

  // "27 份 `ADR` 全部 `proposed`" carries a claim the count cannot: that *none* of them is accepted.
  // A record promoted to `accepted` leaves the count intact, so the adjective would stay on the page
  // asserting something that is no longer true -- and that adjective is a governance blocker.
  if (occurrences.get("decisions").length > 0) {
    const accepted = decisionRecords.filter((record) => record.status !== "proposed");
    if (accepted.length > 0) {
      problems.push(
        `${PARITY_DOCUMENT} states every one of the ${decisionRecords.length} \`ADR\` record(s) is \`proposed\`, but ${accepted.length} no longer is (first: \`${accepted[0].file}\` is ${accepted[0].status ?? "unstated"})`,
      );
    }
  }

  // The contract sentence carries two things the positional patterns above cannot: a count of
  // contracts that are not named `*.contract.json`, and the identity of the one that declares no
  // authorization field. Both are checked against the tree, because "22 of 23 declare false" is only
  // meaningful if the twenty-third is named -- otherwise the exception reads as an omission.
  lines.forEach((raw, index) => {
    const lineNumber = index + 1;
    const line = deEmphasize(raw);
    const unnamed = UNNAMED_CONTRACT_CLAUSE.exec(line);
    if (unnamed) {
      headlineChecks += 1;
      const stated = /^\d+$/u.test(unnamed[1])
        ? Number.parseInt(unnamed[1], 10)
        : CHINESE_COUNT_WORDS[unnamed[1]] ?? null;
      if (stated !== apiContracts.length) {
        problems.push(
          `${PARITY_DOCUMENT}:${lineNumber} states ${stated ?? unnamed[1]} unnamed machine contract(s), but ${apiContracts.length} exist under ${API_SURFACE}/`,
        );
      }
      // The document lists these in prose order, so the comparison is over sorted sets: matching
      // order would fail a sentence that merely names them the other way round, which says the rule
      // is brittle rather than that the document is wrong.
      const cited = backtickedTokens(line)
        .filter((token) => /^apis\/.+\.json$/u.test(token))
        .sort();
      if (cited.join(",") !== apiContracts.join(",")) {
        problems.push(
          `${PARITY_DOCUMENT}:${lineNumber} names unnamed machine contracts [${cited.join(", ") || "none"}], but the repository holds [${apiContracts.join(", ") || "none"}]`,
        );
      }
    }
    const exception = EXCEPTION_CONTRACT_CLAUSE.exec(line);
    if (!exception) return;
    headlineChecks += 1;
    const ordinal = Number.parseInt(exception[1], 10);
    if (ordinal !== namedContracts.length) {
      problems.push(
        `${PARITY_DOCUMENT}:${lineNumber} calls its exception contract #${ordinal}, but the repository holds ${namedContracts.length}`,
      );
    }
    const path = exception[2];
    if (!namedContracts.includes(path)) {
      problems.push(
        `${PARITY_DOCUMENT}:${lineNumber} names \`${path}\` as the contract that declares no \`implementationAuthorized\`, but it is not one`,
      );
      return;
    }
    const value = readJsonFile(join(repoRoot, path));
    if (value !== null && Object.hasOwn(value, "implementationAuthorized")) {
      problems.push(
        `${PARITY_DOCUMENT}:${lineNumber} states \`${path}\` has no \`implementationAuthorized\` field, but it declares one`,
      );
    }
    if (line.includes("runtimeImplementationAuthorizationGranted: false") &&
        value?.releaseDecision?.runtimeImplementationAuthorizationGranted !== false) {
      problems.push(
        `${PARITY_DOCUMENT}:${lineNumber} states \`${path}\` declares \`runtimeImplementationAuthorizationGranted: false\`, but it does not`,
      );
    }
    if (line.includes('releaseDecision.status: "no-go"') && value?.releaseDecision?.status !== "no-go") {
      problems.push(
        `${PARITY_DOCUMENT}:${lineNumber} states \`${path}\` declares \`releaseDecision.status: "no-go"\`, but it does not`,
      );
    }
  });

  for (const relative of declaresNothing) {
    if (!text.includes(`\`${relative}\``)) {
      problems.push(
        `${PARITY_DOCUMENT} states that ${declaresFalse.length} of ${namedContracts.length} ${CONTRACT_SUFFIX} files declare \`implementationAuthorized: false\`, but never names \`${relative}\`, the one that does not`,
      );
    }
  }
  // The bolded claim behind all of the above: no machine contract authorizes implementation. A
  // contract could declare the field false and still carry a go decision, so the assertion is checked
  // against both recognized fields on every contract rather than inferred from the count.
  for (const relative of [...namedContracts, ...apiContracts]) {
    if (authorizesImplementation(readJsonFile(join(repoRoot, relative)))) {
      problems.push(
        `${PARITY_DOCUMENT} states that no machine contract authorizes implementation, but \`${relative}\` does`,
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
    gapCount: gaps?.rows.length ?? 0,
    claimCount: claims?.rows.length ?? 0,
    coveredTests,
    workspaceTests,
    shapeRows,
    shapeAnchors,
    answerRows,
    headlineChecks,
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
    `implementation coverage: ${assessment.coveredTests} of ${assessment.workspaceTests} workspace test(s) accounted for`,
    `shape evidence: ${assessment.shapeRows} component row(s), ${assessment.shapeAnchors} line-numbered anchor(s) resolved`,
    `answer section: ${assessment.answerRows} path row(s), ${assessment.headlineChecks} restated figure(s) compared to their source`,
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
