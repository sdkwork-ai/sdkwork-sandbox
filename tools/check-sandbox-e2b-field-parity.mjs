#!/usr/bin/env node
/**
 * Static gate: the E2B capability baseline must be fully evidenced, and the audit document must
 * agree with it row for row.
 *
 * Why this exists. `docs/architecture/tech/TECH-e2b-capability-parity.md` judges 78 E2B
 * capabilities, and `tools/check-sandbox-e2b-parity-matrix.mjs` proves the document is
 * *internally* consistent. Neither one proves the baseline itself was ever read. Until this gate,
 * 23 of the 78 rows were marked `基准仅索引`: judged from the documentation index alone, without
 * a single field name verified against the page that defines it. That is the failure mode a
 * parity audit is most exposed to -- the matrix adds up, the census adds up, and the whole thing
 * rests on a page title. `specs/sandbox-e2b-capability-baseline.json` replaces the recollection
 * with a captured inventory carrying per-source provenance, and this gate keeps the two in step.
 *
 * Ten deterministic rule families:
 *
 *   1. BASELINE SHAPE. The artifact declares the expected `kind`, a supported `schemaVersion`,
 *      and non-empty `sources`, `categories`, and `rows` arrays.
 *   2. PROVENANCE. Every source carries a unique id, a unique absolute https url, one of the
 *      declared kinds, a positive byte count, and a 64-hex sha256. A source without provenance is
 *      an assertion, not evidence.
 *   3. ROW EVIDENCE. Matrix rows are numbered 1..N, ascending, each exactly once, and every row
 *      names at least one source that resolves, plus at least one extracted surface item: an API
 *      operationId or schema field, a CLI command form, or a page section heading. Documentary
 *      capabilities (region, compliance, BYOC) legitimately evidence themselves with headings;
 *      what is rejected is a row with no evidence of any kind.
 *   4. CATEGORY ALIGNMENT. The category index covers every row exactly once, each declared
 *      `rowCount` equals the rows it owns, and the categories sum to the row total.
 *   5. DOCUMENT JOIN. The audit document's matrix rows and its per-category census must equal the
 *      baseline's, so a row cannot be re-scoped in prose without the baseline moving with it.
 *   6. RATCHET. Rows still marked baseline-incomplete must not exceed the recorded ceiling, and
 *      the ceiling must equal the recorded list, so an improvement is recorded rather than
 *      silently taken and a regression fails. This is the rule that stops the baseline from
 *      quietly decaying back to page titles.
 *   7. OPERATION COVERAGE. Every operation in the captured OpenAPI document is accounted for
 *      exactly once: either an operationId cited by some matrix row, or an entry in
 *      `operationCoverage.unjudged` carrying the reason no row judges it. Row evidence may not
 *      cite an operation the document does not define. Two measurement mistakes sit behind the
 *      current reading of 70 of 71. A `[A-Za-z0-9_]+` scan first reported 36 unjudged, because
 *      pointing at rows 36/38/40/45-49 depends on operationIds containing dots
 *      (`filesystem.Filesystem.Stat`, `process.Process.Start`), which that character class drops
 *      silently. The next pass reported 19, of which 18 were accounting errors rather than gaps:
 *      those operations were the field-level evidence for rows that already judge the capability
 *      -- E2B's whole Templates REST lifecycle belongs to row 26 (`e2b template init`/`build`/
 *      `deploy`), alias to row 29, `GET /envs` to row 1, `GET /metrics` to row 65. Only
 *      `GET /health` genuinely has no home: a control-plane liveness probe is not a sandbox
 *      capability, so it stays recorded with that reason. Accounting for the surface is what
 *      turns an absent judgement into a fact on file and a mis-attributed one into a citation.
 *   8. REGISTRATION. The baseline must be linked from the audit document and from `specs/README.md`,
 *      so a reader arrives at the evidence instead of a claim.
 *   9. TEST INVENTORY. The counts the audit document states about the test suite must be true. Two
 *      are checked: the contract suite, recomputed from `tests/contract/*.test.mjs` itself, and the
 *      Rust workspace reading, which cannot be derived statically and so is compared against the
 *      measurement the baseline records alongside the command that produced it. The document said
 *      `406 pass / 0 fail` while the suite held 481, and `63 passed / 1 ignored` while
 *      `cargo test --workspace` reported 67: a coverage section that overstates or understates the
 *      suite is a falsehood about the only part of this audit that executes.
 *  10. SELF-DESCRIPTION. Every surface that describes this gate must declare the same number of
 *      rule families that the gate implements. The root README said "seven" for as long as
 *      operation coverage had existed, because the paragraph was never revisited when the family
 *      was added. A gate that understates its own coverage is making exactly the claim this gate
 *      exists to reject, so the count is derived from the rule-family registry rather than typed.
 *
 * Usage:
 *   node tools/check-sandbox-e2b-field-parity.mjs
 *   node tools/check-sandbox-e2b-field-parity.mjs --json
 *   node tools/check-sandbox-e2b-field-parity.mjs --root <dir>
 */

import { readFileSync, readdirSync, existsSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const BASELINE_PATH = "specs/sandbox-e2b-capability-baseline.json";
export const PARITY_DOCUMENT = "docs/architecture/tech/TECH-e2b-capability-parity.md";
export const SPECS_README = "specs/README.md";
export const EXPECTED_KIND = "sdkwork.sandbox.e2b-capability-baseline";
export const SUPPORTED_SCHEMA_VERSIONS = Object.freeze([1]);
export const SOURCE_KINDS = Object.freeze(["openapi", "index", "doc-page"]);
/** The marker a row carries while it rests on the documentation index instead of a captured page. */
export const BASELINE_INCOMPLETE_MARKER = "基准仅索引";

/**
 * The rule families this gate implements, in the order the header documents them. This list -- not
 * the prose -- is the authority for the count every registration surface declares, and every
 * finding must name one of these keys, so a family cannot be added, renamed or dropped without the
 * descriptions moving with it.
 */
export const RULE_FAMILIES = Object.freeze([
  "baseline-shape",
  "provenance",
  "row-evidence",
  "category-alignment",
  "document-join",
  "ratchet",
  "operation-coverage",
  "registration",
  "test-inventory",
  "self-description",
]);

/** The prose surfaces that describe this gate, and the language each states the count in. */
export const RULE_FAMILY_SURFACES = Object.freeze([
  // `own` marks the surface that *is* this gate: it cannot confuse its own count with another
  // gate's, so the count may sit in any of its comment blocks.
  { id: "gate header", path: "tools/check-sandbox-e2b-field-parity.mjs", language: "english", own: true },
  { id: "tools README", path: "tools/README.md", language: "english" },
  { id: "root README", path: "README.md", language: "english" },
  { id: "Gate 0 view", path: "docs/architecture/views/gate-zero-current-state.md", language: "chinese" },
]);

const ENGLISH_COUNT_WORDS = Object.freeze({
  one: 1,
  two: 2,
  three: 3,
  four: 4,
  five: 5,
  six: 6,
  seven: 7,
  eight: 8,
  nine: 9,
  ten: 10,
  eleven: 11,
  twelve: 12,
});

const CHINESE_COUNT_WORDS = Object.freeze({
  一: 1,
  二: 2,
  三: 3,
  四: 4,
  五: 5,
  六: 6,
  七: 7,
  八: 8,
  九: 9,
  十: 10,
  十一: 11,
  十二: 12,
});

/** The name this gate is addressed by, used to scope a declared count to where the gate is described. */
export const GATE_FILE_NAME = "check-sandbox-e2b-field-parity.mjs";

/**
 * The span of a document that describes *this* gate. A surface may describe several gates, and the
 * first count in such a file belongs to whichever gate is described first: `tools/README.md` now
 * carries a matrix-gate section above this gate's section, so reading the file's first count
 * attributed seven families to a gate that implements ten. The span is the last block naming this
 * gate plus the block that follows it, because a count introduced as a standalone line ("Ten rule
 * families:") lands in the next paragraph by ordinary prose habit. A surface that *is* this gate
 * carries no ambiguity -- its comment blocks are separated by ` *` lines, and the block that names
 * the file (the usage lines) is not where the count lives -- so it opts out of scoping.
 */
function declarationScope(text, { own = false } = {}) {
  const source = String(text);
  if (own) return source;
  const blocks = source.split(/\n\s*\n/u);
  const index = blocks.findLastIndex((block) => block.includes(GATE_FILE_NAME));
  if (index === -1) return source;
  return index + 1 < blocks.length ? `${blocks[index]}\n\n${blocks[index + 1]}` : blocks[index];
}

/**
 * The number a document declares for this gate's rule families, or null when it declares none.
 * The alternation is anchored on the count words themselves: a looser `(\w+)\s+rule famil` reads
 * "the gate then holds seven rule families" as "holds" and reports an unrecognized count, which is
 * the parser blaming the document for its own greediness. At most one adjective may sit between the
 * count and the noun, so "Eight deterministic rule families" parses like "seven rule families".
 */
export function parseDeclaredRuleFamilies(text, language, { own = false } = {}) {
  const source = declarationScope(text, { own });
  if (language === "chinese") {
    const match = /([一二三四五六七八九十]+)\s*条规则/.exec(source);
    return match ? CHINESE_COUNT_WORDS[match[1]] ?? null : null;
  }
  const pattern = new RegExp(
    `(${Object.keys(ENGLISH_COUNT_WORDS).join("|")})(?:\\s+[A-Za-z]+)?\\s+rule famil(?:y|ies)`,
    "i",
  );
  const match = pattern.exec(source);
  return match ? ENGLISH_COUNT_WORDS[match[1].toLowerCase()] ?? null : null;
}

const SHA256 = /^[0-9a-f]{64}$/;
const HEX = /^[0-9a-f]+$/;

function readText(repoRoot, relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

/** Read a file that a rule treats as optional, returning null instead of throwing when it is gone. */
function safeReadText(absolutePath) {
  try {
    return readFileSync(absolutePath, "utf8");
  } catch {
    return null;
  }
}

function parseJson(repoRoot, relativePath) {
  return JSON.parse(readText(repoRoot, relativePath));
}

/**
 * Matrix rows of the audit document, bounded to its per-capability section so the census and
 * coverage tables that follow cannot be mistaken for matrix rows.
 */
export function parseDocumentRows(documentText) {
  const lines = documentText.split(/\r?\n/);
  const start = lines.findIndex((line) => /^##\s*2\.\s/.test(line));
  if (start === -1) throw new Error("the audit document has no section 2");
  let end = lines.length;
  for (let index = start + 1; index < lines.length; index += 1) {
    if (/^##\s*3\.\s/.test(lines[index])) {
      end = index;
      break;
    }
  }
  const rows = [];
  const categoryCounts = new Map();
  let category = null;
  for (let index = start; index < end; index += 1) {
    const line = lines[index];
    const heading = line.match(/^###\s*2\.(\d+)\s+(.+)$/);
    if (heading) {
      category = heading[2].trim();
      continue;
    }
    const match = line.match(/^\|\s*(\d+)\s*\|/);
    if (!match || !category) continue;
    const number = Number(match[1]);
    rows.push({
      row: number,
      category,
      baselineIncomplete: line.includes(BASELINE_INCOMPLETE_MARKER),
    });
    categoryCounts.set(category, (categoryCounts.get(category) ?? 0) + 1);
  }
  return { rows, categoryCounts };
}

/** Census counts declared in the document's own totals table. */
export function parseDeclaredCensus(documentText) {
  const found = new Map();
  let total = null;
  for (const line of documentText.split(/\r?\n/)) {
    const match = line.match(/^\|\s*(?:\*\*)?(.+?)(?:\*\*)?\s*\|\s*\*{0,2}(\d+)\*{0,2}\s*\|/);
    if (!match) continue;
    const label = match[1].trim();
    const count = Number(match[2]);
    if (/^合计$/.test(label) || /^\*\*合计\*\*$/.test(label)) {
      total = count;
      continue;
    }
    if (["Sandbox 生命周期", "持久化（Pause / Resume）", "Snapshot 与 Fork", "Template", "Filesystem", "Volumes", "Commands 与 Process", "PTY", "Code Interpreter", "Network", "Secrets 与 IAM", "Metrics 与 Telemetry", "CLI", "SDK", "MCP Gateway", "平台与部署", "Agent 框架集成"].includes(label)) {
      found.set(label, count);
    }
  }
  return { perCategory: found, total };
}

export function assessE2bFieldParity({ repoRoot = "." } = {}) {
  const root = resolve(repoRoot);
  const findings = [];
  const add = (rule, message) => findings.push({ rule, message });

  for (const [relativePath, label] of [
    [BASELINE_PATH, "baseline"],
    [PARITY_DOCUMENT, "audit document"],
    [SPECS_README, "specs README"],
  ]) {
    if (!existsSync(join(root, relativePath))) {
      add("baseline-shape", `missing ${label}: ${relativePath}`);
      return { ok: false, findings, summary: null };
    }
  }

  let baseline;
  try {
    baseline = parseJson(root, BASELINE_PATH);
  } catch (error) {
    add("baseline-shape", `${BASELINE_PATH} is not valid JSON: ${error.message}`);
    return { ok: false, findings, summary: null };
  }

  // ---- 1. BASELINE SHAPE
  if (baseline.kind !== EXPECTED_KIND) {
    add("baseline-shape", `kind is "${baseline.kind}", expected "${EXPECTED_KIND}"`);
  }
  if (!SUPPORTED_SCHEMA_VERSIONS.includes(baseline.schemaVersion)) {
    add("baseline-shape", `unsupported schemaVersion ${baseline.schemaVersion}`);
  }
  for (const key of ["sources", "categories", "rows"]) {
    if (!Array.isArray(baseline[key]) || baseline[key].length === 0) {
      add("baseline-shape", `baseline.${key} must be a non-empty array`);
    }
  }
  if (findings.length) return { ok: false, findings, summary: null };

  // ---- 2. PROVENANCE
  const sourceIds = new Set();
  const sourceUrls = new Set();
  for (const source of baseline.sources) {
    const label = `source ${source.id ?? "<no id>"}`;
    if (!source.id || typeof source.id !== "string") {
      add("provenance", "a source has no id");
      continue;
    }
    if (sourceIds.has(source.id)) add("provenance", `${label}: duplicate id`);
    sourceIds.add(source.id);
    if (!/^https:\/\//.test(source.url ?? "")) add("provenance", `${label}: url must be absolute https`);
    else if (sourceUrls.has(source.url)) add("provenance", `${label}: duplicate url ${source.url}`);
    else sourceUrls.add(source.url);
    if (!SOURCE_KINDS.includes(source.kind)) {
      add("provenance", `${label}: kind "${source.kind}" is not one of ${SOURCE_KINDS.join(", ")}`);
    }
    if (!Number.isInteger(source.bytes) || source.bytes <= 0) {
      add("provenance", `${label}: bytes must be a positive integer`);
    }
    if (!SHA256.test(source.sha256 ?? "")) {
      add("provenance", `${label}: sha256 must be 64 lowercase hex characters`);
    }
  }

  // ---- 3. ROW EVIDENCE
  const baselineRows = new Map();
  const expectedCount = baseline.rows.length;
  for (const [index, row] of baseline.rows.entries()) {
    const label = `row ${row.row ?? "<no number>"}`;
    if (!Number.isInteger(row.row)) {
      add("row-evidence", `baseline row at index ${index} has no integer row number`);
      continue;
    }
    if (baselineRows.has(row.row)) add("row-evidence", `${label}: duplicate row number`);
    baselineRows.set(row.row, row);
    const surface = (row.e2bFields ?? []).length + (row.e2bClis ?? []).length + (row.e2bFacts ?? []).length;
    if (surface === 0) {
      add("row-evidence", `${label}: no extracted surface (e2bFields, e2bClis, e2bFacts are all empty)`);
    }
    if (!Array.isArray(row.sourceIds) || row.sourceIds.length === 0) {
      add("row-evidence", `${label}: names no source`);
    } else {
      for (const id of row.sourceIds) {
        if (!sourceIds.has(id)) add("row-evidence", `${label}: sourceId "${id}" does not resolve`);
      }
    }
  }
  for (let number = 1; number <= expectedCount; number += 1) {
    if (!baselineRows.has(number)) add("row-evidence", `row ${number} is missing from the baseline`);
  }
  const numbers = baseline.rows.map((row) => row.row);
  for (let index = 1; index < numbers.length; index += 1) {
    if (numbers[index] <= numbers[index - 1]) {
      add("row-evidence", `rows must ascend: ${numbers[index - 1]} is followed by ${numbers[index]}`);
      break;
    }
  }

  // ---- 4. CATEGORY ALIGNMENT
  const ownedByCategory = new Map();
  for (const row of baseline.rows) {
    if (!ownedByCategory.has(row.category)) ownedByCategory.set(row.category, []);
    ownedByCategory.get(row.category).push(row.row);
  }
  let categoryTotal = 0;
  for (const category of baseline.categories) {
    const owned = ownedByCategory.get(category.id) ?? [];
    if (!owned.length) {
      add("category-alignment", `category "${category.id}" owns no row`);
    }
    if (category.rowCount !== owned.length) {
      add(
        "category-alignment",
        `category "${category.id}" declares rowCount ${category.rowCount} but owns ${owned.length}`,
      );
    }
    const declared = [...(category.rows ?? [])].sort((a, b) => a - b);
    const actual = [...owned].sort((a, b) => a - b);
    if (declared.join(",") !== actual.join(",")) {
      add("category-alignment", `category "${category.id}" row list does not match its members`);
    }
    categoryTotal += category.rowCount ?? 0;
  }
  if (categoryTotal !== expectedCount) {
    add("category-alignment", `categories total ${categoryTotal} but the baseline has ${expectedCount} rows`);
  }

  // ---- 5. DOCUMENT JOIN
  const documentText = readText(root, PARITY_DOCUMENT);
  const { rows: documentRows, categoryCounts } = parseDocumentRows(documentText);
  const documentNumbers = documentRows.map((row) => row.row);
  if (documentNumbers.length !== expectedCount) {
    add(
      "document-join",
      `the audit document has ${documentNumbers.length} matrix rows, the baseline has ${expectedCount}`,
    );
  }
  for (const row of documentRows) {
    if (!baselineRows.has(row.row)) {
      add("document-join", `document row ${row.row} has no baseline entry`);
    }
  }
  const headingToId = new Map(baseline.categories.map((c) => [c.documentHeading, c.id]));
  for (const [heading, count] of categoryCounts) {
    const id = headingToId.get(heading);
    if (!id) {
      add("document-join", `document subsection "${heading}" is not a baseline category`);
      continue;
    }
    const owned = ownedByCategory.get(id) ?? [];
    if (count !== owned.length) {
      add(
        "document-join",
        `document "${heading}" lists ${count} rows, baseline category "${id}" has ${owned.length}`,
      );
    }
  }
  for (const category of baseline.categories) {
    if (!categoryCounts.has(category.documentHeading)) {
      add(
        "document-join",
        `baseline category "${category.id}" expects a document subsection "${category.documentHeading}" that is absent`,
      );
    }
  }
  const census = parseDeclaredCensus(documentText);
  for (const [heading, declared] of census.perCategory) {
    const id = headingToId.get(heading);
    if (!id) continue;
    const owned = ownedByCategory.get(id) ?? [];
    if (declared !== owned.length) {
      add(
        "document-join",
        `census row "${heading}" declares ${declared}, baseline category "${id}" has ${owned.length}`,
      );
    }
  }
  if (census.total !== null && census.total !== expectedCount) {
    add("document-join", `census total declares ${census.total}, the baseline has ${expectedCount}`);
  }

  // ---- 6. RATCHET
  const ratchet = baseline.ratchet ?? {};
  const declaredIncomplete = [...(ratchet.indexOnlyRows ?? [])].sort((a, b) => a - b);
  const markedRows = documentRows.filter((row) => row.baselineIncomplete).map((row) => row.row).sort((a, b) => a - b);
  if (!Number.isInteger(ratchet.maxIndexOnlyRows) || ratchet.maxIndexOnlyRows < 0) {
    add("ratchet", "ratchet.maxIndexOnlyRows must be a non-negative integer");
  } else if (markedRows.length > ratchet.maxIndexOnlyRows) {
    add(
      "ratchet",
      `the audit document marks ${markedRows.length} row(s) baseline-incomplete, above the ceiling ${ratchet.maxIndexOnlyRows}`,
    );
  }
  if (markedRows.join(",") !== declaredIncomplete.join(",")) {
    add(
      "ratchet",
      `rows marked "${BASELINE_INCOMPLETE_MARKER}" are [${markedRows.join(",")}] but the ratchet records [${declaredIncomplete.join(",")}]`,
    );
  }
  if (declaredIncomplete.length > (ratchet.maxIndexOnlyRows ?? -1)) {
    add("ratchet", "the recorded incomplete-row list exceeds the recorded ceiling");
  }

  // ---- 7. OPERATION COVERAGE
  const coverage = baseline.operationCoverage;
  if (!coverage || typeof coverage !== "object" || Array.isArray(coverage)) {
    add("operation-coverage", "baseline.operationCoverage must be an object");
  } else {
    if (!sourceIds.has(coverage.sourceId)) {
      add("operation-coverage", `operationCoverage.sourceId "${coverage.sourceId}" does not resolve to a source`);
    }
    const operations = Array.isArray(coverage.operations) ? coverage.operations : [];
    if (operations.length === 0) {
      add("operation-coverage", "operationCoverage.operations must be a non-empty array");
    }
    const unique = [...new Set(operations)];
    if (unique.length !== operations.length) {
      add("operation-coverage", "operationCoverage.operations contains duplicate operation ids");
    }
    if ([...unique].sort().join(",") !== unique.join(",")) {
      add("operation-coverage", "operationCoverage.operations must be sorted");
    }
    const declaredCount = coverage.documentedOperations;
    if (!Number.isInteger(declaredCount) || declaredCount !== operations.length) {
      add(
        "operation-coverage",
        `operationCoverage.documentedOperations declares ${declaredCount}, the operations list has ${operations.length}`,
      );
    }
    const known = new Set(unique);
    const cited = new Set();
    for (const row of baseline.rows) {
      for (const field of row.e2bFields ?? []) {
        const match = /^[A-Z]+ \S.*\[([A-Za-z0-9_.]+)\]$/.exec(String(field));
        if (match) cited.add(match[1]);
      }
    }
    for (const id of cited) {
      if (!known.has(id)) {
        add("operation-coverage", `row evidence cites operation "${id}", which operationCoverage does not define`);
      }
    }
    const unjudged = coverage.unjudged ?? {};
    for (const [id, reason] of Object.entries(unjudged)) {
      if (!known.has(id)) {
        add("operation-coverage", `operationCoverage.unjudged names "${id}", which is not a documented operation`);
      } else if (cited.has(id)) {
        add("operation-coverage", `operation "${id}" is recorded unjudged but a row cites it`);
      }
      if (typeof reason !== "string" || reason.trim().length < 10) {
        add("operation-coverage", `operation "${id}" is recorded unjudged without a usable reason`);
      }
    }
    for (const id of unique) {
      if (!cited.has(id) && !Object.hasOwn(unjudged, id)) {
        add("operation-coverage", `operation "${id}" is neither cited by a row nor recorded unjudged`);
      }
    }
  }

  // ---- 8. REGISTRATION
  for (const [relativePath, label] of [
    [PARITY_DOCUMENT, "the audit document"],
    [SPECS_README, "specs/README.md"],
  ]) {
    const text = readText(root, relativePath);
    if (!text.includes("sandbox-e2b-capability-baseline.json")) {
      add("registration", `${label} does not link ${BASELINE_PATH}`);
    }
  }
  if (!HEX.test(baseline.capturedAt ?? "")) {
    // capture timestamp is only checked for presence of an ISO date; the shape matters, not the zone
    if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(baseline.capturedAt ?? "")) {
      add("baseline-shape", `capturedAt "${baseline.capturedAt}" is not an ISO-8601 UTC timestamp`);
    }
  }

  // ---- 9. TEST INVENTORY
  const inventory = baseline.testInventory;
  if (!inventory || typeof inventory !== "object" || Array.isArray(inventory)) {
    add("test-inventory", "baseline.testInventory must be an object");
  } else {
    let files = null;
    try {
      files = readdirSync(join(root, "tests", "contract"))
        .filter((name) => name.endsWith(".test.mjs"))
        .sort();
    } catch (error) {
      add("test-inventory", `cannot enumerate tests/contract: ${error.message}`);
    }
    if (files) {
      const countTests = (file) =>
        readText(root, join("tests", "contract", file))
          .split(/\r?\n/)
          .filter((line) => /^test\(/.test(line)).length;
      const total = files.reduce((sum, file) => sum + countTests(file), 0);
      if (inventory.files !== files.length) {
        add(
          "test-inventory",
          `testInventory.files declares ${inventory.files}, tests/contract holds ${files.length} .test.mjs file(s)`,
        );
      }
      if (inventory.tests !== total) {
        add(
          "test-inventory",
          `testInventory.tests declares ${inventory.tests}, the suite declares ${total} test(s)`,
        );
      }
      for (const entry of inventory.auditSuites ?? []) {
        const file = basename(String(entry.file ?? ""));
        if (!files.includes(file)) {
          add("test-inventory", `testInventory.auditSuites names "${entry.file}", which is not in tests/contract`);
          continue;
        }
        const actual = countTests(file);
        if (entry.tests !== actual) {
          add("test-inventory", `testInventory records ${entry.tests} test(s) for ${file}, which declares ${actual}`);
        }
      }
    }
    // The audit document quotes the suite size. A reading that no longer matches the suite is a
    // documentation falsehood of exactly the kind this rule exists to catch: it said 406 while the
    // suite had grown to 481.
    const quoted = /`(\d+) pass \/ (\d+) fail`/.exec(documentText);
    if (!quoted) {
      add("test-inventory", "the audit document declares no `<n> pass / <n> fail` contract-suite reading");
    } else {
      if (Number(quoted[1]) !== inventory.tests) {
        add(
          "test-inventory",
          `the audit document declares ${quoted[1]} passing contract test(s), the baseline records ${inventory.tests}`,
        );
      }
      if (Number(quoted[2]) !== 0) {
        add("test-inventory", `the audit document declares ${quoted[2]} failing contract test(s), which must be 0`);
      }
    }

    // The Rust reading cannot be derived from the tree -- it needs a build and a test run -- so the
    // baseline records the measurement next to the command that produced it, and the document must
    // agree with that recording. Same honesty model as the source hashes: a statement captured at a
    // moment, not a digest recomputed offline.
    const rust = inventory.rustWorkspace;
    if (rust === undefined) {
      add("test-inventory", "testInventory.rustWorkspace must record the `cargo test --workspace` reading");
    } else if (
      typeof rust !== "object" ||
      Array.isArray(rust) ||
      !Number.isInteger(rust.passed) ||
      rust.passed < 0 ||
      !Number.isInteger(rust.ignored) ||
      rust.ignored < 0 ||
      rust.failed !== 0 ||
      typeof rust.command !== "string" ||
      !rust.command.includes("cargo test")
    ) {
      add("test-inventory", "testInventory.rustWorkspace must carry `command`, a non-negative `passed`/`ignored` and `failed: 0`");
    } else if (rust.passed + rust.ignored <= 0) {
      add("test-inventory", "testInventory.rustWorkspace records no executed and no ignored test");
    } else {
      const rustQuoted = /`(\d+) passed \/ (\d+) ignored`/.exec(documentText);
      if (!rustQuoted) {
        add("test-inventory", "the audit document declares no `<n> passed / <n> ignored` Rust reading");
      } else if (Number(rustQuoted[1]) !== rust.passed || Number(rustQuoted[2]) !== rust.ignored) {
        add(
          "test-inventory",
          `the audit document declares \`${rustQuoted[1]} passed / ${rustQuoted[2]} ignored\`, the baseline records ` +
            `\`${rust.passed} passed / ${rust.ignored} ignored\` for \`${rust.command}\``,
        );
      }
    }
  }

  // ---- 10. SELF-DESCRIPTION
  // Every surface that describes this gate states how many rule families it implements. That count
  // is derived from RULE_FAMILIES, so a family added without a description -- or a description left
  // behind by a family that moved -- is a finding rather than an overstatement nobody reads twice.
  const selfSource = safeReadText(fileURLToPath(import.meta.url));
  for (const surface of RULE_FAMILY_SURFACES) {
    const text = surface.id === "gate header" ? selfSource : safeReadText(join(root, surface.path));
    if (text === null) {
      add("self-description", `the ${surface.id} (${surface.path}) is missing, so it describes no rule families`);
      continue;
    }
    const declared = parseDeclaredRuleFamilies(text, surface.language, { own: surface.own === true });
    if (declared === null) {
      add(
        "self-description",
        `the ${surface.id} (${surface.path}) declares no rule-family count; it must say how many of the ${RULE_FAMILIES.length} families this gate implements`,
      );
      continue;
    }
    if (declared !== RULE_FAMILIES.length) {
      add(
        "self-description",
        `the ${surface.id} declares ${declared} rule families, this gate implements ${RULE_FAMILIES.length}`,
      );
    }
  }
  for (const { rule } of [...findings]) {
    if (!RULE_FAMILIES.includes(rule)) {
      add("self-description", `a finding names undeclared rule family "${rule}"`);
    }
  }

  const summary = {
    rows: expectedCount,
    categories: baseline.categories.length,
    sources: baseline.sources.length,
    rowsVerified: baseline.rows.filter(
      (row) => (row.e2bFields ?? []).length + (row.e2bClis ?? []).length + (row.e2bFacts ?? []).length > 0,
    ).length,
    baselineIncompleteRows: markedRows.length,
    ceiling: ratchet.maxIndexOnlyRows ?? null,
    documentedOperations: baseline.operationCoverage?.operations?.length ?? null,
    unjudgedOperations: Object.keys(baseline.operationCoverage?.unjudged ?? {}).length,
    contractTests: baseline.testInventory?.tests ?? null,
    rustTests: baseline.testInventory?.rustWorkspace?.passed ?? null,
    ruleFamilies: RULE_FAMILIES.length,
    capturedAt: baseline.capturedAt ?? null,
  };
  return { ok: findings.length === 0, findings, summary };
}

export function formatE2bFieldParityReport(assessment) {
  const lines = [];
  if (assessment.ok) {
    const s = assessment.summary;
    lines.push(
      `sandbox E2B field parity: ${s.rows} row(s) fully evidenced across ${s.categories} categor(ies) from ${s.sources} captured source(s); ${s.baselineIncompleteRows} row(s) rest on the documentation index (ceiling ${s.ceiling}); captured ${s.capturedAt}`,
    );
    lines.push(
      `  E2B OpenAPI surface: ${s.documentedOperations} operation(s), ${s.documentedOperations - s.unjudgedOperations} judged by a row, ${s.unjudgedOperations} recorded unjudged`,
    );
    lines.push(
      `  Declared suite size: ${s.contractTests} contract test(s) recomputed from tests/contract, ${s.rustTests} Rust test(s) recorded from \`cargo test --workspace\`; ${s.ruleFamilies} rule families declared consistently in 4 surface(s)`,
    );
    return `${lines.join("\n")}\n`;
  }
  lines.push(`sandbox E2B field parity: ${assessment.findings.length} finding(s)`);
  for (const finding of assessment.findings) {
    lines.push(`  [${finding.rule}] ${finding.message}`);
  }
  return `${lines.join("\n")}\n`;
}

export function parseE2bFieldParityArgs(argv) {
  const options = { root: ".", json: false };
  for (let index = 0; index < argv.length; ) {
    const argument = argv[index];
    if (argument === "--json") {
      options.json = true;
      index += 1;
    } else if (argument === "--root") {
      const value = argv[index + 1];
      if (!value) throw new Error("--root requires a directory");
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
    const options = parseE2bFieldParityArgs(process.argv.slice(2));
    const assessment = assessE2bFieldParity({ repoRoot: options.root });
    process.stdout.write(
      options.json ? `${JSON.stringify(assessment, null, 2)}\n` : formatE2bFieldParityReport(assessment),
    );
    if (!assessment.ok) {
      process.exitCode = 1;
    }
  } catch (error) {
    process.stderr.write(`sandbox E2B field parity check failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
