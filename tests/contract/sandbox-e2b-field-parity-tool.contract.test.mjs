import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  BASELINE_INCOMPLETE_MARKER,
  BASELINE_PATH,
  EXPECTED_KIND,
  PARITY_DOCUMENT,
  RULE_FAMILIES,
  RULE_FAMILY_SURFACES,
  SOURCE_KINDS,
  SPECS_README,
  SUPPORTED_SCHEMA_VERSIONS,
  assessE2bFieldParity,
  formatE2bFieldParityReport,
  parseDeclaredCensus,
  parseDeclaredRuleFamilies,
  parseDocumentRows,
  parseE2bFieldParityArgs,
} from "../../tools/check-sandbox-e2b-field-parity.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const REAL_SOURCES = 101;
const REAL_CATEGORIES = 17;
const REAL_ROWS = 78;
const REAL_CAPTURED_AT = "2026-09-22T09:36:31Z";
const REAL_DOCUMENTED_OPERATIONS = 71;
const REAL_CONTRACT_FILES = 40;
const REAL_CONTRACT_TESTS = 533;
const REAL_RUST_WORKSPACE = { command: "cargo test --workspace", passed: 67, failed: 0, ignored: 1 };
const REAL_RULE_FAMILIES = 10;
/** The only E2B OpenAPI operation no matrix row judges. Recorded, not hidden. */
const REAL_UNJUDGED_OPERATIONS = ["getHealth"];
/** Rows that gained field-level operation evidence when the 18 mis-accounted operations were attached. */
const REAL_OPERATION_ATTACHMENTS = [
  { row: 1, operation: "GET /envs [getEnvVars]" },
  { row: 26, operation: "POST /templates [postTemplates]" },
  { row: 26, operation: "GET /templates/{templateID}/builds/{buildID}/status [getTemplateBuildStatus]" },
  { row: 26, operation: "GET /templates/{templateID}/files/{hash} [getTemplateFile]" },
  { row: 29, operation: "GET /templates/aliases/{alias} [getTemplatesAlias]" },
  { row: 65, operation: "GET /metrics [getMetrics]" },
];
const REAL_HEADINGS = [
  "Sandbox 生命周期",
  "持久化（Pause / Resume）",
  "Snapshot 与 Fork",
  "Template",
  "Filesystem",
  "Volumes",
  "Commands 与 Process",
  "PTY",
  "Code Interpreter",
  "Network",
  "Secrets 与 IAM",
  "Metrics 与 Telemetry",
  "CLI",
  "SDK",
  "MCP Gateway",
  "平台与部署",
  "Agent 框架集成",
];
/** The 23 rows that were judged from the documentation index alone before the baseline existed. */
const RETIRED_INDEX_ONLY_ROWS = [8, 10, 30, 37, 38, 39, 40, 43, 44, 48, 49, 51, 52, 53, 56, 59, 60, 61, 63, 64, 66, 67, 70];

function realBaseline() {
  return JSON.parse(readFileSync(join(repoRoot, BASELINE_PATH), "utf8"));
}

// ---------------------------------------------------------------------------
// Fixture: a throwaway repository shaped like the real one for the checks this
// gate performs.
//
//   <tmp>/sdkwork-fixture/
//     specs/sandbox-e2b-capability-baseline.json
//     specs/README.md
//     docs/architecture/tech/TECH-e2b-capability-parity.md
//
// Four rows across two categories, exercising all three surface kinds
// (e2bFields, e2bClis, e2bFacts). Both category headings are taken from the
// gate's census allowlist so the census sub-rule is live too.
// ---------------------------------------------------------------------------

function baselineFixture() {
  const hex = (char) => char.repeat(64);
  return {
    schemaVersion: 1,
    kind: EXPECTED_KIND,
    capturedAt: "2026-09-22T09:36:31Z",
    ratchet: { maxIndexOnlyRows: 0, indexOnlyRows: [], note: "fixture" },
    testInventory: {
      suiteGlob: "tests/contract/*.test.mjs",
      files: 2,
      tests: 5,
      auditSuites: [{ file: "tests/contract/fixture-a.test.mjs", tests: 3 }],
      rustWorkspace: {
        command: "cargo test --workspace",
        passed: 2,
        failed: 0,
        ignored: 1,
        note: "fixture",
      },
      note: "fixture",
    },
    operationCoverage: {
      sourceId: "e2b-openapi",
      documentedOperations: 3,
      operations: ["getSandbox", "listSandboxes", "postSandboxes"],
      unjudged: { getSandbox: "No fixture row judges reading a single sandbox record." },
      note: "fixture",
    },
    sources: [
      { id: "e2b-openapi", url: "https://docs.e2b.dev/openapi-public.yaml", kind: "openapi", bytes: 166293, sha256: hex("a") },
      { id: "e2b-lifecycle", url: "https://docs.e2b.dev/sandbox/lifecycle", kind: "doc-page", bytes: 1024, sha256: hex("b") },
      { id: "e2b-index", url: "https://docs.e2b.dev/llms.txt", kind: "index", bytes: 4096, sha256: hex("c") },
    ],
    categories: [
      { id: "lifecycle", documentHeading: "Sandbox 生命周期", rowCount: 3, rows: [1, 2, 3] },
      { id: "network", documentHeading: "Network", rowCount: 1, rows: [4] },
    ],
    rows: [
      { row: 1, category: "lifecycle", sourceIds: ["e2b-lifecycle"], e2bFields: ["POST /sandboxes [postSandboxes]"] },
      { row: 2, category: "lifecycle", sourceIds: ["e2b-openapi"], e2bFields: ["GET /sandboxes [listSandboxes]"] },
      { row: 3, category: "lifecycle", sourceIds: ["e2b-lifecycle"], e2bClis: ["e2b sandbox list"] },
      { row: 4, category: "network", sourceIds: ["e2b-openapi"], e2bFacts: ["egress allowlist"] },
    ],
  };
}

function documentFixture() {
  return [
    "# E2B 能力对齐审计",
    "",
    "基准：`specs/sandbox-e2b-capability-baseline.json`",
    "",
    "### 1.3 分类计数",
    "",
    "| E2B 分类 | 行数 |",
    "| --- | --- |",
    "| Sandbox 生命周期 | 3 |",
    "| Network | 1 |",
    "| **合计** | **4** |",
    "",
    "## 2. 逐项对照",
    "",
    "### 2.1 Sandbox 生命周期",
    "",
    "| # | E2B | mapping | 状态 | 证据 |",
    "| --- | --- | --- | --- | --- |",
    "| 1 | a1 | A | 🟡 | ev |",
    "| 2 | a2 | B | ❌ | ev |",
    "| 3 | a3 | C | ❌ | ev |",
    "",
    "### 2.2 Network",
    "",
    "| # | E2B | mapping | 状态 | 证据 |",
    "| --- | --- | --- | --- | --- |",
    "| 4 | d1 | D | 🟡 | ev |",
    "",
    "## 3. 测试覆盖矩阵",
    "",
    "`node --test tests/contract/*.test.mjs`：`5 pass / 0 fail`。",
    "",
    "`cargo test --workspace`：`2 passed / 1 ignored`。",
    "",
  ].join("\n");
}

function readmeFixture() {
  return `# Repository Contracts\n\n\`sandbox-e2b-capability-baseline.json\` is the captured baseline authority.\n`;
}

/** The prose that describes the gate itself, in the two languages it is written in. */
function gateSurfacesFixture(ruleFamilyWord = "ten", chineseWord = "十") {
  return {
    rootReadme: `# Fixture Repository\n\nThe gate holds ${ruleFamilyWord} rule families.\n`,
    toolsReadme: `${ruleFamilyWord[0].toUpperCase()}${ruleFamilyWord.slice(1)} rule families:\n`,
    gateZeroView: `# Gate 0\n\n${chineseWord}条规则：基准形状、逐来源 provenance。\n`,
  };
}

function writeTestSuite(dir, contents) {
  mkdirSync(dir, { recursive: true });
  for (const [name, source] of Object.entries(contents)) {
    writeFileSync(join(dir, name), source);
  }
}

/** Two files, five tests: three in the audited suite, two in a neighbour. */
function testSuiteFixture() {
  const lines = (count, label) =>
    Array.from({ length: count }, (_, index) => `test("${label} ${index}", () => {});`).join("\n") + "\n";
  return {
    "fixture-a.test.mjs": lines(3, "audited"),
    "fixture-b.test.mjs": lines(2, "neighbour"),
  };
}

function createFixture({
  baseline = baselineFixture(),
  document = documentFixture(),
  readme = readmeFixture(),
  surfaces = gateSurfacesFixture(),
  testSuite = testSuiteFixture(),
  mutate,
} = {}) {
  const base = mkdtempSync(join(tmpdir(), "sdkwork-e2b-field-parity-"));
  const repo = join(base, "sdkwork-fixture");
  mkdirSync(join(repo, "specs"), { recursive: true });
  mkdirSync(join(repo, "docs", "architecture", "tech"), { recursive: true });
  mkdirSync(join(repo, "docs", "architecture", "views"), { recursive: true });
  mkdirSync(join(repo, "tools"), { recursive: true });
  const paths = {
    baseline: join(repo, "specs", "sandbox-e2b-capability-baseline.json"),
    document: join(repo, "docs", "architecture", "tech", "TECH-e2b-capability-parity.md"),
    readme: join(repo, "specs", "README.md"),
    rootReadme: join(repo, "README.md"),
    toolsReadme: join(repo, "tools", "README.md"),
    gateZeroView: join(repo, "docs", "architecture", "views", "gate-zero-current-state.md"),
    tests: join(repo, "tests", "contract"),
  };
  writeFileSync(paths.baseline, `${JSON.stringify(baseline, null, 2)}\n`);
  writeFileSync(paths.document, document);
  writeFileSync(paths.readme, readme);
  writeFileSync(paths.rootReadme, surfaces.rootReadme);
  writeFileSync(paths.toolsReadme, surfaces.toolsReadme);
  writeFileSync(paths.gateZeroView, surfaces.gateZeroView);
  writeTestSuite(paths.tests, testSuite);
  if (mutate) mutate({ repo, paths });
  return { base, repo, paths };
}

function inspect(options) {
  const fixture = createFixture(options);
  try {
    return assessE2bFieldParity({ repoRoot: fixture.repo });
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
}

function expectRule(options, rule, fragment) {
  const assessment = inspect(options);
  assert.equal(assessment.ok, false, `expected the gate to redden for ${rule}`);
  const matching = assessment.findings.filter((finding) => finding.rule === rule);
  assert.ok(
    matching.length > 0,
    `expected a ${rule} finding, got:\n${formatE2bFieldParityReport(assessment)}`,
  );
  if (fragment !== undefined) {
    assert.ok(
      matching.some((finding) => finding.message.includes(fragment)),
      `expected a ${rule} finding mentioning "${fragment}", got:\n${formatE2bFieldParityReport(assessment)}`,
    );
  }
}

// ---------------------------------------------------------------------------
// The real corpus
// ---------------------------------------------------------------------------

test("the repository's own E2B baseline is fully evidenced and agrees with its audit document", () => {
  const assessment = assessE2bFieldParity({ repoRoot });

  assert.equal(assessment.ok, true, formatE2bFieldParityReport(assessment));
  assert.deepEqual(assessment.summary, {
    rows: REAL_ROWS,
    categories: REAL_CATEGORIES,
    sources: REAL_SOURCES,
    rowsVerified: REAL_ROWS,
    baselineIncompleteRows: 0,
    ceiling: 0,
    documentedOperations: REAL_DOCUMENTED_OPERATIONS,
    unjudgedOperations: REAL_UNJUDGED_OPERATIONS.length,
    contractTests: REAL_CONTRACT_TESTS,
    rustTests: REAL_RUST_WORKSPACE.passed,
    ruleFamilies: REAL_RULE_FAMILIES,
    capturedAt: REAL_CAPTURED_AT,
  });
});

test("the gate's report names the coverage it verified", () => {
  const report = formatE2bFieldParityReport(assessE2bFieldParity({ repoRoot }));

  assert.match(report, /78 row\(s\) fully evidenced/);
  assert.match(report, /17 categor\(ies\)/);
  assert.match(report, /101 captured source\(s\)/);
  assert.match(report, /0 row\(s\) rest on the documentation index \(ceiling 0\)/);
  assert.match(report, /71 operation\(s\), 70 judged by a row, 1 recorded unjudged/);
  assert.match(report, /533 contract test\(s\) recomputed from tests\/contract, 67 Rust test\(s\) recorded/);
  assert.match(report, /10 rule families declared consistently in 4 surface\(s\)/);
});

test("every E2B OpenAPI operation is accounted for, judged or recorded unjudged", () => {
  const baseline = realBaseline();
  const coverage = baseline.operationCoverage;

  assert.equal(coverage.sourceId, "e2b-openapi-public");
  assert.ok(baseline.sources.some((source) => source.id === coverage.sourceId && source.kind === "openapi"));
  assert.equal(coverage.documentedOperations, REAL_DOCUMENTED_OPERATIONS);
  assert.deepEqual(coverage.operations, [...coverage.operations].sort(), "operations must be sorted");
  assert.equal(new Set(coverage.operations).size, coverage.operations.length, "operations must be unique");

  const cited = new Set();
  for (const row of baseline.rows) {
    for (const field of row.e2bFields ?? []) {
      const match = /^[A-Z]+ \S.*\[([A-Za-z0-9_.]+)\]$/.exec(String(field));
      if (match) cited.add(match[1]);
    }
  }
  assert.deepEqual(
    Object.keys(coverage.unjudged).sort(),
    REAL_UNJUDGED_OPERATIONS,
    "the recorded unjudged set is the contract; changing it is a deliberate act",
  );
  for (const id of coverage.operations) {
    const judged = cited.has(id);
    const recorded = Object.hasOwn(coverage.unjudged, id);
    assert.ok(judged || recorded, `operation ${id} is neither judged nor recorded unjudged`);
    assert.ok(!(judged && recorded), `operation ${id} is both judged and recorded unjudged`);
  }
  for (const id of cited) {
    assert.ok(coverage.operations.includes(id), `row cites ${id}, absent from the documented operation list`);
  }
  for (const reason of Object.values(coverage.unjudged)) {
    assert.ok(typeof reason === "string" && reason.trim().length >= 10, "every unjudged op needs a real reason");
  }
});

test("only the control-plane liveness probe is left unjudged, and it carries a reason", () => {
  const { unjudged } = realBaseline().operationCoverage;

  assert.deepEqual(Object.keys(unjudged), ["getHealth"], "the recorded unjudged set is the contract");
  const reason = unjudged.getHealth;
  assert.ok(typeof reason === "string" && reason.trim().length >= 10);
  assert.match(reason, /liveness|健康|存活|readiness/i, "the reason must say what it is");
  assert.match(reason, /REQ-|row|Row/, "the reason must point at where the equivalent surface lives");
});

test("the 18 mis-accounted operations are attached as evidence to the rows that judge them", () => {
  const { rows } = realBaseline();
  const fieldsOf = (rowNumber) => rows.find((row) => row.row === rowNumber).e2bFields;

  for (const { row, operation } of REAL_OPERATION_ATTACHMENTS) {
    assert.ok(
      fieldsOf(row).includes(operation),
      `row ${row} must cite "${operation}" as the field-level evidence for the capability it judges`,
    );
  }
  // Row 26 carries the whole Templates REST lifecycle, so it must be the widest template row.
  const templateOps = fieldsOf(26).filter((field) => /\[[A-Za-z0-9_.]*(Template|Templates)[A-Za-z0-9_.]*\]$/.test(field));
  assert.equal(templateOps.length, 15, "row 26 owns template CRUD, the build pipeline and build output");
  assert.equal(
    fieldsOf(29).filter((field) => /\[getTemplatesAlias\]$/.test(field)).length,
    1,
    "alias belongs to the tagging/versioning row, not the lifecycle row",
  );
});

test("the baseline declares its identity, capture time and provenance contract", () => {
  const baseline = realBaseline();

  assert.equal(baseline.kind, EXPECTED_KIND);
  assert.equal(baseline.schemaVersion, 1);
  assert.ok(SUPPORTED_SCHEMA_VERSIONS.includes(baseline.schemaVersion));
  assert.match(baseline.capturedAt, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/);
  assert.match(baseline.recaptureCommand, /target\/e2b-baseline/);
  assert.match(baseline.recaptureCommand, /docs\.e2b\.dev/);
});

test("every captured source carries unique provenance of a declared kind", () => {
  const { sources } = realBaseline();
  const ids = new Set();
  const urls = new Set();

  for (const source of sources) {
    assert.equal(typeof source.id, "string", "every source needs an id");
    assert.ok(!ids.has(source.id), `duplicate source id ${source.id}`);
    ids.add(source.id);
    assert.match(source.url, /^https:\/\//, `${source.id} must be an absolute https url`);
    assert.ok(!urls.has(source.url), `duplicate source url ${source.url}`);
    urls.add(source.url);
    assert.ok(SOURCE_KINDS.includes(source.kind), `${source.id} kind ${source.kind} is not declared`);
    assert.ok(Number.isInteger(source.bytes) && source.bytes > 0, `${source.id} needs a positive byte count`);
    assert.match(source.sha256, /^[0-9a-f]{64}$/, `${source.id} needs a 64-hex sha256`);
  }
  assert.equal(sources.length, REAL_SOURCES);
});

test("every baseline row ascends 1..78, names a resolved source, and is backed by extracted surface", () => {
  const baseline = realBaseline();
  const { rows, sources, categories } = baseline;
  const sourceIds = new Set(sources.map((source) => source.id));
  const categoryIds = new Set(categories.map((category) => category.id));

  assert.equal(rows.length, REAL_ROWS);
  assert.deepEqual(
    rows.map((row) => row.row),
    Array.from({ length: REAL_ROWS }, (_, index) => index + 1),
    "rows must be numbered 1..78 ascending with no gaps",
  );
  for (const row of rows) {
    assert.ok(categoryIds.has(row.category), `row ${row.row} names unknown category ${row.category}`);
    assert.ok(Array.isArray(row.sourceIds) && row.sourceIds.length > 0, `row ${row.row} names no source`);
    for (const id of row.sourceIds) {
      assert.ok(sourceIds.has(id), `row ${row.row} names unresolvable source ${id}`);
    }
    const surface =
      (row.e2bFields ?? []).length + (row.e2bClis ?? []).length + (row.e2bFacts ?? []).length;
    assert.ok(surface > 0, `row ${row.row} carries no extracted surface`);
  }
});

test("the category index partitions the rows and matches the document headings", () => {
  const { categories, rows } = realBaseline();

  assert.deepEqual(
    categories.map((category) => category.documentHeading),
    REAL_HEADINGS,
    "the category headings are the contract with the audit document",
  );
  let total = 0;
  for (const category of categories) {
    const owned = rows.filter((row) => row.category === category.id).map((row) => row.row).sort((a, b) => a - b);
    assert.equal(category.rowCount, owned.length, `category ${category.id} rowCount`);
    assert.deepEqual(
      [...category.rows].sort((a, b) => a - b),
      owned,
      `category ${category.id} row list is not its members`,
    );
    total += category.rowCount;
  }
  assert.equal(total, REAL_ROWS);
});

test("no row rests on the documentation index, and the 23 retired rows are recorded", () => {
  const baseline = realBaseline();
  const retired = baseline.rows.filter((row) => row.previouslyIndexOnly).map((row) => row.row).sort((a, b) => a - b);

  assert.equal(baseline.ratchet.maxIndexOnlyRows, 0);
  assert.deepEqual(baseline.ratchet.indexOnlyRows, []);
  assert.deepEqual(retired, RETIRED_INDEX_ONLY_ROWS, "the ratchet must remember the rows it retired");
  assert.equal(
    readFileSync(join(repoRoot, PARITY_DOCUMENT), "utf8").includes(BASELINE_INCOMPLETE_MARKER),
    true,
    "the retired marker should survive only as the legend entry",
  );
  const { rows } = parseDocumentRows(readFileSync(join(repoRoot, PARITY_DOCUMENT), "utf8"));
  assert.equal(rows.filter((row) => row.baselineIncomplete).length, 0, "no matrix row may carry the marker");
});

test("the metrics family is evidenced at field level rather than by index title", () => {
  const { rows } = realBaseline();
  const sandboxMetrics = rows.find((row) => row.row === 65);
  const teamMetrics = rows.find((row) => row.row === 66);

  assert.ok(sandboxMetrics.e2bFields.includes("GET /sandboxes/{sandboxID}/metrics [getSandboxMetrics]"));
  assert.ok(teamMetrics.e2bFields.includes("GET /teams/{teamID}/metrics [getTeamMetrics]"));
  assert.equal(teamMetrics.previouslyIndexOnly, true, "row 66 is one of the rows the baseline upgraded");
});

test("the baseline is registered from both the audit document and specs/README.md", () => {
  assert.ok(readFileSync(join(repoRoot, PARITY_DOCUMENT), "utf8").includes("sandbox-e2b-capability-baseline.json"));
  assert.ok(readFileSync(join(repoRoot, SPECS_README), "utf8").includes("sandbox-e2b-capability-baseline.json"));
});

test("the baseline records the test suite it lives in, recomputed from the files", () => {
  const { testInventory: inventory } = realBaseline();
  const suiteDir = join(repoRoot, "tests", "contract");
  const files = readdirSync(suiteDir)
    .filter((name) => name.endsWith(".test.mjs"))
    .sort();
  const countTests = (file) =>
    readFileSync(join(suiteDir, file), "utf8")
      .split(/\r?\n/)
      .filter((line) => /^test\(/.test(line)).length;

  assert.equal(files.length, REAL_CONTRACT_FILES, "the number of contract suites is the contract");
  assert.equal(inventory.files, files.length);
  assert.equal(inventory.tests, REAL_CONTRACT_TESTS);
  assert.equal(
    files.reduce((sum, file) => sum + countTests(file), 0),
    inventory.tests,
    "the recorded suite size must equal a fresh recount of the files",
  );
  for (const entry of inventory.auditSuites) {
    const file = path.basename(entry.file);
    assert.ok(files.includes(file), `${entry.file} must exist`);
    assert.equal(countTests(file), entry.tests, `${file} declares ${countTests(file)} test(s)`);
  }
  assert.equal(inventory.auditSuites.length, 2, "both E2B audit suites are covered by this gate");
});

test("the Rust reading is recorded next to the command that produced it", () => {
  const { rustWorkspace: rust } = realBaseline().testInventory;

  assert.equal(rust.command, REAL_RUST_WORKSPACE.command);
  assert.equal(rust.passed, REAL_RUST_WORKSPACE.passed);
  assert.equal(rust.failed, REAL_RUST_WORKSPACE.failed);
  assert.equal(rust.ignored, REAL_RUST_WORKSPACE.ignored);
  assert.match(rust.measuredAt, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/, "a measurement needs a timestamp");
  assert.match(rust.note, /PostgreSQL/, "the ignored test must say what it is waiting for");
});

test("the audit document's declared test readings match the suite and the recording", () => {
  const text = readFileSync(join(repoRoot, PARITY_DOCUMENT), "utf8");
  const contract = /`(\d+) pass \/ (\d+) fail`/.exec(text);
  const rust = /`(\d+) passed \/ (\d+) ignored`/.exec(text);

  assert.ok(contract, "the coverage section must quote the contract-suite reading");
  assert.equal(Number(contract[1]), REAL_CONTRACT_TESTS);
  assert.equal(Number(contract[2]), 0);
  assert.ok(rust, "the coverage section must quote the Rust reading");
  assert.equal(Number(rust[1]), REAL_RUST_WORKSPACE.passed);
  assert.equal(Number(rust[2]), REAL_RUST_WORKSPACE.ignored);
});

test("every surface describing this gate declares the rule families it implements", () => {
  const header = readFileSync(join(repoRoot, RULE_FAMILY_SURFACES[0].path), "utf8");

  assert.equal(RULE_FAMILIES.length, REAL_RULE_FAMILIES);
  assert.equal(new Set(RULE_FAMILIES).size, RULE_FAMILIES.length, "family keys must be unique");
  for (const surface of RULE_FAMILY_SURFACES) {
    const text = readFileSync(join(repoRoot, surface.path), "utf8");
    assert.equal(
      parseDeclaredRuleFamilies(text, surface.language, { own: surface.own === true }),
      RULE_FAMILIES.length,
      `${surface.id} (${surface.path}) must declare ${RULE_FAMILIES.length} rule families`,
    );
  }
  // The registry is the authority: a family added here without a numbered header entry fails. The
  // header is prose, so either separator is accepted for a compound family name.
  for (const family of RULE_FAMILIES) {
    const candidates = [family.toUpperCase(), family.toUpperCase().replace(/-/g, " ")];
    assert.ok(
      candidates.some((title) => header.includes(title)),
      `the gate header must document the ${family} family (expected ${candidates.join(" or ")})`,
    );
  }
});

test("parseDeclaredRuleFamilies reads the count and ignores the word standing before it", () => {
  assert.equal(parseDeclaredRuleFamilies("Ten deterministic rule families:", "english"), 10);
  assert.equal(parseDeclaredRuleFamilies("The gate then holds seven rule families.", "english"), 7);
  assert.equal(parseDeclaredRuleFamilies("one rule family", "english"), 1);
  assert.equal(parseDeclaredRuleFamilies("十条规则：基准形状。", "chinese"), 10);
  assert.equal(parseDeclaredRuleFamilies("八条规则", "chinese"), 8);
  assert.equal(parseDeclaredRuleFamilies("十二条规则", "chinese"), 12);
  assert.equal(parseDeclaredRuleFamilies("nothing declares a count here", "english"), null);
  // The first version of this parser read "proves every rule family" as the count "proves".
  assert.equal(parseDeclaredRuleFamilies("proves every rule family can go red", "english"), null);
  assert.equal(parseDeclaredRuleFamilies("三条规则", "chinese"), 3);
});

test("a count is read where this gate is described, not where another gate is", () => {
  // `tools/README.md` describes the matrix gate above this one, and the first count in the file
  // used to be read as this gate's -- seven families attributed to a gate implementing ten.
  const twoGates = [
    "`check-sandbox-e2b-parity-matrix.mjs` keeps the matrix honest. Seven rule families:",
    "",
    "`check-sandbox-e2b-field-parity.mjs` keeps the baseline the matrix rests on.",
    "",
    "Ten rule families:",
  ].join("\n");

  assert.equal(parseDeclaredRuleFamilies(twoGates, "english"), 10);

  // The same shape in Chinese, with the stranger count first.
  const chinese = [
    "`check-sandbox-e2b-parity-matrix.mjs` 读矩阵。七条规则族：",
    "",
    "`check-sandbox-e2b-field-parity.mjs` 读基准。",
    "",
    "十条规则：基准形状。",
  ].join("\n");

  assert.equal(parseDeclaredRuleFamilies(chinese, "chinese"), 10);
  // A count that stands outside the place this gate is described is not this gate's.
  assert.equal(
    parseDeclaredRuleFamilies(
      ["Seven rule families:", "", "`check-sandbox-e2b-field-parity.mjs` is named with no count."].join(
        "\n",
      ),
      "english",
    ),
    null,
  );
});

test("the assessment is deterministic across repeated runs", () => {
  const first = JSON.stringify(assessE2bFieldParity({ repoRoot }));
  const second = JSON.stringify(assessE2bFieldParity({ repoRoot }));

  assert.equal(first, second, "repeated regression runs must produce identical output");
});

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

test("a consistent fixture passes", () => {
  const assessment = inspect();

  assert.equal(assessment.ok, true, formatE2bFieldParityReport(assessment));
  assert.deepEqual(assessment.summary, {
    rows: 4,
    categories: 2,
    sources: 3,
    rowsVerified: 4,
    baselineIncompleteRows: 0,
    ceiling: 0,
    documentedOperations: 3,
    unjudgedOperations: 1,
    contractTests: 5,
    rustTests: 2,
    ruleFamilies: 10,
    capturedAt: "2026-09-22T09:36:31Z",
  });
});

test("a missing repository is reported rather than thrown", () => {
  const base = mkdtempSync(join(tmpdir(), "sdkwork-e2b-field-parity-empty-"));
  try {
    const assessment = assessE2bFieldParity({ repoRoot: base });
    assert.equal(assessment.ok, false);
    assert.ok(assessment.findings.some((finding) => finding.message.includes("missing baseline")));
    assert.equal(assessment.summary, null);
  } finally {
    rmSync(base, { recursive: true, force: true });
  }
});

test("a baseline that is not valid JSON is reported rather than thrown", () => {
  const fixture = createFixture();
  try {
    writeFileSync(fixture.paths.baseline, "{ not json");
    const assessment = assessE2bFieldParity({ repoRoot: fixture.repo });
    assert.equal(assessment.ok, false);
    assert.ok(assessment.findings.some((finding) => finding.message.includes("is not valid JSON")));
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

// ---------------------------------------------------------------------------
// Mutation self-proof: every one of the ten rule families must be able to
// fail. A rule that cannot redden is dead code, and a green gate proves nothing.
// ---------------------------------------------------------------------------

test("every rule family reddens on its own mutation while the unmutated control stays green", () => {
  const control = inspect();
  assert.equal(control.ok, true, "the control group must stay green");

  const cases = [
    // 1. baseline-shape
    { rule: "baseline-shape", baseline: () => ({ ...baselineFixture(), kind: "wrong.kind" }) },
    { rule: "baseline-shape", baseline: () => ({ ...baselineFixture(), schemaVersion: 2 }) },
    { rule: "baseline-shape", baseline: () => ({ ...baselineFixture(), sources: [] }) },
    // 2. provenance
    {
      rule: "provenance",
      baseline: () => {
        const next = baselineFixture();
        next.sources[1].id = next.sources[0].id;
        return next;
      },
    },
    {
      rule: "provenance",
      baseline: () => {
        const next = baselineFixture();
        next.sources[1].url = "http://insecure.example/x";
        return next;
      },
    },
    {
      rule: "provenance",
      baseline: () => {
        const next = baselineFixture();
        next.sources[1].kind = "pdf";
        return next;
      },
    },
    {
      rule: "provenance",
      baseline: () => {
        const next = baselineFixture();
        next.sources[1].bytes = 0;
        return next;
      },
    },
    {
      rule: "provenance",
      baseline: () => {
        const next = baselineFixture();
        next.sources[1].sha256 = "abc";
        return next;
      },
    },
    // 3. row-evidence
    {
      rule: "row-evidence",
      baseline: () => {
        const next = baselineFixture();
        next.rows[0].e2bFields = [];
        return next;
      },
    },
    {
      rule: "row-evidence",
      baseline: () => {
        const next = baselineFixture();
        next.rows[0].sourceIds = [];
        return next;
      },
    },
    {
      rule: "row-evidence",
      baseline: () => {
        const next = baselineFixture();
        next.rows[0].sourceIds = ["e2b-not-captured"];
        return next;
      },
    },
    {
      rule: "row-evidence",
      baseline: () => {
        const next = baselineFixture();
        next.rows[3].row = 2;
        return next;
      },
    },
    {
      rule: "row-evidence",
      baseline: () => {
        const next = baselineFixture();
        next.rows.splice(1, 1);
        return next;
      },
    },
    // 4. category-alignment
    {
      rule: "category-alignment",
      baseline: () => {
        const next = baselineFixture();
        next.categories[1].rowCount = 2;
        return next;
      },
    },
    {
      rule: "category-alignment",
      baseline: () => {
        const next = baselineFixture();
        next.categories[0].rows = [1, 2];
        return next;
      },
    },
    {
      rule: "category-alignment",
      baseline: () => {
        const next = baselineFixture();
        next.categories.push({ id: "orphan", documentHeading: "PTY", rowCount: 0, rows: [] });
        return next;
      },
    },
    // 5. document-join
    { rule: "document-join", document: () => documentFixture().replace("| 4 | d1 | D | 🟡 | ev |\n", "") },
    {
      rule: "document-join",
      document: () =>
        documentFixture().replace(
          "## 3. 测试覆盖矩阵",
          "### 2.3 PTY\n\n| # | E2B | mapping | 状态 | 证据 |\n| --- | --- | --- | --- | --- |\n| 5 | p1 | P | ❌ | ev |\n\n## 3. 测试覆盖矩阵",
        ),
    },
    { rule: "document-join", document: () => documentFixture().replace("**4**", "**5**") },
    { rule: "document-join", document: () => documentFixture().replace("| Network | 1 |", "| Network | 3 |") },
    // 6. ratchet
    {
      rule: "ratchet",
      document: () => documentFixture().replace("| 1 | a1 | A | 🟡 | ev |", "| 1 | a1 | A | 🟡 | ev（基准仅索引）|"),
    },
    {
      rule: "ratchet",
      baseline: () => {
        const next = baselineFixture();
        next.ratchet = { maxIndexOnlyRows: -1, indexOnlyRows: [] };
        return next;
      },
    },
    {
      rule: "ratchet",
      baseline: () => {
        const next = baselineFixture();
        next.ratchet = { maxIndexOnlyRows: 1, indexOnlyRows: [2] };
        return next;
      },
    },
    // 7. operation-coverage
    {
      rule: "operation-coverage",
      baseline: () => {
        const next = baselineFixture();
        next.operationCoverage.operations = ["getSandbox", "listSandboxes", "postSandboxes", "deleteSandbox"];
        next.operationCoverage.documentedOperations = 4;
        return next;
      },
    },
    {
      rule: "operation-coverage",
      baseline: () => {
        const next = baselineFixture();
        next.operationCoverage.unjudged = {};
        return next;
      },
    },
    {
      rule: "operation-coverage",
      baseline: () => {
        const next = baselineFixture();
        next.operationCoverage.unjudged = { listSandboxes: "a row already judges this one" };
        return next;
      },
    },
    {
      rule: "operation-coverage",
      baseline: () => {
        const next = baselineFixture();
        next.operationCoverage.unjudged = { getSandbox: "short" };
        return next;
      },
    },
    {
      rule: "operation-coverage",
      baseline: () => {
        const next = baselineFixture();
        next.operationCoverage.sourceId = "e2b-not-a-source";
        return next;
      },
    },
    {
      rule: "operation-coverage",
      baseline: () => {
        const next = baselineFixture();
        next.rows[0].e2bFields = ["POST /sandboxes [postSandboxesUndocumented]"];
        return next;
      },
    },
    {
      rule: "operation-coverage",
      baseline: () => {
        const next = baselineFixture();
        next.operationCoverage.documentedOperations = 99;
        return next;
      },
    },
    // 8. registration
    { rule: "registration", readme: () => "# Repository Contracts\n\nnothing links the baseline here.\n" },
    { rule: "registration", document: () => documentFixture().replace("`specs/sandbox-e2b-capability-baseline.json`", "`specs/other.json`") },
    // 9. test-inventory
    {
      rule: "test-inventory",
      baseline: () => {
        const next = baselineFixture();
        next.testInventory.tests = 99;
        return next;
      },
    },
    {
      rule: "test-inventory",
      baseline: () => {
        const next = baselineFixture();
        next.testInventory.files = 7;
        return next;
      },
    },
    {
      rule: "test-inventory",
      baseline: () => {
        const next = baselineFixture();
        next.testInventory.auditSuites[0].tests = 4;
        return next;
      },
    },
    {
      rule: "test-inventory",
      baseline: () => {
        const next = baselineFixture();
        delete next.testInventory.rustWorkspace;
        return next;
      },
    },
    {
      rule: "test-inventory",
      baseline: () => {
        const next = baselineFixture();
        next.testInventory.rustWorkspace.failed = 1;
        return next;
      },
    },
    // A new suite appears on disk without the recorded size moving: the count is recomputed, not
    // trusted, so growing the suite cannot leave the recorded inventory stale.
    {
      rule: "test-inventory",
      mutate: ({ paths }) => writeFileSync(join(paths.tests, "extra.test.mjs"), 'test("extra", () => {});\n'),
    },
    { rule: "test-inventory", document: () => documentFixture().replace("`5 pass / 0 fail`", "`7 pass / 0 fail`") },
    { rule: "test-inventory", document: () => documentFixture().replace("`5 pass / 0 fail`", "`5 pass / 2 fail`") },
    { rule: "test-inventory", document: () => documentFixture().replace("`5 pass / 0 fail`", "no reading at all") },
    { rule: "test-inventory", document: () => documentFixture().replace("`2 passed / 1 ignored`", "`3 passed / 1 ignored`") },
    { rule: "test-inventory", document: () => documentFixture().replace("`2 passed / 1 ignored`", "`2 passed / 2 ignored`") },
    // 10. self-description
    { rule: "self-description", surfaces: gateSurfacesFixture("nine") },
    { rule: "self-description", surfaces: gateSurfacesFixture("ten", "八") },
    {
      rule: "self-description",
      surfaces: { ...gateSurfacesFixture(), rootReadme: "# Fixture Repository\n\nno count is declared here.\n" },
    },
    {
      rule: "self-description",
      mutate: ({ paths }) => rmSync(paths.toolsReadme),
    },
  ];

  for (const [index, entry] of cases.entries()) {
    const options = {};
    if (entry.baseline) options.baseline = entry.baseline();
    if (entry.document) options.document = entry.document();
    if (entry.readme) options.readme = entry.readme();
    if (entry.surfaces) options.surfaces = entry.surfaces;
    if (entry.mutate) options.mutate = entry.mutate;
    const label = `${entry.rule} mutation #${index}`;
    const assessment = inspect(options);
    assert.equal(assessment.ok, false, `${label} stayed green`);
    assert.ok(
      assessment.findings.some((finding) => finding.rule === entry.rule),
      `${label} produced no ${entry.rule} finding:\n${formatE2bFieldParityReport(assessment)}`,
    );
  }

  assert.equal(inspect().ok, true, "the control group must still be green after the mutations");
});

test("targeted rule messages are stable", () => {
  expectRule({ document: documentFixture().replace("| 2 | a2 | B | ❌ | ev |\n", "") }, "document-join", "matrix rows");

  const misalignedRows = baselineFixture();
  misalignedRows.categories[0].rowCount = 9;
  expectRule({ baseline: misalignedRows }, "category-alignment", "declares rowCount 9");

  const evidenceless = baselineFixture();
  evidenceless.rows[0].e2bFields = [];
  expectRule({ baseline: evidenceless }, "row-evidence", "no extracted surface");

  const staleCount = baselineFixture();
  staleCount.testInventory.tests = 9;
  expectRule({ baseline: staleCount }, "test-inventory", "the suite declares 5 test(s)");

  expectRule({ surfaces: gateSurfacesFixture("nine") }, "self-description", "this gate implements 10");
  expectRule({ document: (() => documentFixture().replace("`5 pass / 0 fail`", "no reading"))() }, "test-inventory", "declares no");
});

// ---------------------------------------------------------------------------
// Parsers and arguments
// ---------------------------------------------------------------------------

test("parseDocumentRows is bounded to section 2 and joins rows to their heading", () => {
  const { rows, categoryCounts } = parseDocumentRows(documentFixture());

  assert.deepEqual(
    rows.map((row) => [row.row, row.category]),
    [
      [1, "Sandbox 生命周期"],
      [2, "Sandbox 生命周期"],
      [3, "Sandbox 生命周期"],
      [4, "Network"],
    ],
  );
  assert.deepEqual([...categoryCounts.entries()], [
    ["Sandbox 生命周期", 3],
    ["Network", 1],
  ]);
});

test("parseDocumentRows rejects a document with no section 2", () => {
  assert.throws(() => parseDocumentRows("# nothing\n"), /no section 2/u);
});

test("parseDeclaredCensus reads the total row and the known category labels", () => {
  const census = parseDeclaredCensus(documentFixture());

  assert.equal(census.total, 4);
  assert.equal(census.perCategory.get("Sandbox 生命周期"), 3);
  assert.equal(census.perCategory.get("Network"), 1);
});

test("arguments are parsed strictly", () => {
  assert.deepEqual(parseE2bFieldParityArgs([]), { root: ".", json: false });
  assert.equal(parseE2bFieldParityArgs(["--json"]).json, true);
  assert.equal(parseE2bFieldParityArgs(["--root", "."]).root, resolve(process.cwd()));
  assert.throws(() => parseE2bFieldParityArgs(["--root"]), /--root requires a directory/u);
  assert.throws(() => parseE2bFieldParityArgs(["--nope"]), /unsupported argument/u);
});

test("the gate's declared contract constants are stable", () => {
  assert.equal(BASELINE_PATH, "specs/sandbox-e2b-capability-baseline.json");
  assert.equal(PARITY_DOCUMENT, "docs/architecture/tech/TECH-e2b-capability-parity.md");
  assert.equal(SPECS_README, "specs/README.md");
  assert.equal(EXPECTED_KIND, "sdkwork.sandbox.e2b-capability-baseline");
  assert.deepEqual(SOURCE_KINDS, ["openapi", "index", "doc-page"]);
  assert.equal(BASELINE_INCOMPLETE_MARKER, "基准仅索引");
});
