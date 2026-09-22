import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  assessE2bParityMatrix,
  formatE2bParityMatrixReport,
  parseE2bParityMatrixArgs,
  parseCensus,
  parseMatrixCategories,
  parseStatusVocabulary,
} from "../../tools/check-sandbox-e2b-parity-matrix.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const PARITY_DOC = "docs/architecture/tech/TECH-e2b-capability-parity.md";
const ARCH_ENTRY = "docs/architecture/tech/TECH_ARCHITECTURE.md";
const TECH_README = "docs/architecture/tech/README.md";
const DOCS_INDEX = "docs/INDEX.yaml";

const VOCABULARY = [
  "### 0.2 状态口径",
  "",
  "| 标记 | 含义 |",
  "| --- | --- |",
  "| ✅ 完整对齐 | equivalent |",
  "| 🟡 部分 / 形态不同 | partial |",
  "| ❌ 未实现 | missing |",
  "| ⛔ 刻意不做 | deliberate |",
  "",
].join("\n");

const CENSUS = [
  "### 1.3 分类四态计数",
  "",
  "| E2B 分类 | 行数 | ✅ | 🟡 | ❌ | ⛔ |",
  "| --- | --- | --- | --- | --- | --- |",
  "| Alpha | 1 | 0 | 1 | 0 | 0 |",
  "| Beta | 2 | 0 | 0 | 1 | 1 |",
  "| **合计** | **3** | **0** | **1** | **1** | **1** |",
  "",
].join("\n");

const MATRIX = [
  "## 2. 逐项对照",
  "",
  "### 2.1 Alpha",
  "",
  "| # | E2B | mapping | 状态 | 证据 |",
  "| --- | --- | --- | --- | --- |",
  "| 1 | a1 | candidate | 🟡 | REQ-2026-0002 |",
  "",
  "### 2.2 Beta",
  "",
  "| # | E2B | mapping | 状态 | 证据 |",
  "| --- | --- | --- | --- | --- |",
  "| 2 | b1 | none | ❌ | — |",
  "| 3 | b2 | none | ⛔ | PRD.md 非目标原话 |",
  "",
].join("\n");

function buildDocument({ vocabulary = VOCABULARY, census = CENSUS, matrix = MATRIX } = {}) {
  return ["# E2B 能力对齐审计", "", "## 0. 基准快照与状态口径", "", vocabulary, census, matrix, "## 3. 测试覆盖矩阵", ""].join("\n");
}

/**
 * Build a throwaway repository shaped like the real one for the checks this gate performs:
 *
 *   <tmp>/sdkwork-fixture/
 *     docs/INDEX.yaml
 *     docs/product/requirements/REQ-2026-0002-fixture.md
 *     docs/architecture/decisions/ADR-20260728-fixture-decision.md
 *     docs/architecture/tech/TECH_ARCHITECTURE.md
 *     docs/architecture/tech/README.md
 *     docs/architecture/tech/TECH-e2b-capability-parity.md
 */
function createFixture({ document = buildDocument(), registerInIndex = true, registerInEntry = true } = {}) {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-e2b-parity-"));
  const repo = path.join(base, "sdkwork-fixture");
  const techDirectory = path.join(repo, "docs", "architecture", "tech");
  mkdirSync(path.join(repo, "docs", "product", "requirements"), { recursive: true });
  mkdirSync(path.join(repo, "docs", "architecture", "decisions"), { recursive: true });
  mkdirSync(techDirectory, { recursive: true });
  writeFileSync(path.join(repo, "docs", "product", "requirements", "REQ-2026-0002-fixture.md"), "# fixture\n");
  writeFileSync(
    path.join(repo, "docs", "architecture", "decisions", "ADR-20260728-fixture-decision.md"),
    "# fixture\n",
  );
  writeFileSync(path.join(techDirectory, "TECH-e2b-capability-parity.md"), document);
  writeFileSync(
    path.join(techDirectory, "TECH_ARCHITECTURE.md"),
    registerInEntry ? "- [E2B 能力对齐审计](TECH-e2b-capability-parity.md)\n" : "- nothing\n",
  );
  writeFileSync(
    path.join(techDirectory, "README.md"),
    registerInEntry ? "- [E2B capability parity audit](TECH-e2b-capability-parity.md)\n" : "- nothing\n",
  );
  writeFileSync(
    path.join(repo, "docs", "INDEX.yaml"),
    registerInIndex ? `    path: ${PARITY_DOC}\n` : "    path: other.md\n",
  );
  return { base, repo };
}

function inspect(options) {
  const fixture = createFixture(options);
  try {
    return assessE2bParityMatrix({ repoRoot: fixture.repo });
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
}

function expectProblem(assessment, fragment) {
  assert.equal(assessment.ok, false, formatE2bParityMatrixReport(assessment));
  assert.ok(
    assessment.problems.some((problem) => problem.includes(fragment)),
    `expected a problem mentioning "${fragment}", got:\n${formatE2bParityMatrixReport(assessment)}`,
  );
}

test("the repository's own parity document is consistent", () => {
  const assessment = assessE2bParityMatrix({ repoRoot });

  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
  assert.equal(assessment.categoryCount, 17);
  assert.equal(assessment.rowCount, 78);
  assert.deepEqual(assessment.totals, { rows: 78, ok: 0, partial: 16, missing: 60, deliberate: 2 });
  assert.deepEqual(assessment.declaredMarkers, ["✅", "🟡", "❌", "⛔"]);
});

test("the repository's own census is a partition of its matrix", () => {
  const { totals } = assessE2bParityMatrix({ repoRoot });

  assert.equal(
    totals.ok + totals.partial + totals.missing + totals.deliberate,
    totals.rows,
    "the census total row must partition the matrix",
  );
});

test("a consistent fixture passes", () => {
  const assessment = inspect();

  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
  assert.equal(assessment.rowCount, 3);
  assert.deepEqual(assessment.totals, { rows: 3, ok: 0, partial: 1, missing: 1, deliberate: 1 });
});

test("a missing document is reported rather than thrown", () => {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-e2b-parity-empty-"));
  try {
    const assessment = assessE2bParityMatrix({ repoRoot: base });
    assert.equal(assessment.ok, false);
    assert.ok(assessment.problems.some((problem) => problem.includes("does not exist")));
  } finally {
    rmSync(base, { recursive: true, force: true });
  }
});

test("an undeclared status marker is rejected", () => {
  const vocabulary = VOCABULARY.replace("| ❌ 未实现 | missing |\n", "");
  expectProblem(inspect({ document: buildDocument({ vocabulary }) }), "does not declare the status marker ❌");
});

test("a status outside the vocabulary is rejected", () => {
  const matrix = MATRIX.replace("| 1 | a1 | candidate | 🟡 |", "| 1 | a1 | candidate | 部分 |");
  expectProblem(inspect({ document: buildDocument({ matrix }) }), "is not one of");
});

test("an empty status cell is rejected", () => {
  const matrix = MATRIX.replace("| 1 | a1 | candidate | 🟡 |", "| 1 | a1 | candidate |  |");
  expectProblem(inspect({ document: buildDocument({ matrix }) }), "is not one of");
});

test("a gap in row numbering is rejected", () => {
  const matrix = MATRIX.replace("| 3 | b2 | none | ⛔ |", "| 4 | b2 | none | ⛔ |");
  expectProblem(inspect({ document: buildDocument({ matrix }) }), "numbering breaks");
});

test("a row with the wrong cell count is rejected", () => {
  const matrix = MATRIX.replace("| 1 | a1 | candidate | 🟡 | REQ-2026-0002 |", "| 1 | a1 | 🟡 | REQ-2026-0002 |");
  expectProblem(inspect({ document: buildDocument({ matrix }) }), "must have 5");
});

test("a per-category count that disagrees with the matrix is rejected", () => {
  const census = CENSUS.replace("| Alpha | 1 | 0 | 1 | 0 | 0 |", "| Alpha | 1 | 1 | 0 | 0 | 0 |");
  expectProblem(
    inspect({ document: buildDocument({ census }) }),
    'census for "Alpha" declares ok=1, but the matrix contains 0',
  );
});

test("a total row that disagrees with the category rows is rejected", () => {
  const census = CENSUS.replace(
    "| **合计** | **3** | **0** | **1** | **1** | **1** |",
    "| **合计** | **3** | **1** | **1** | **1** | **1** |",
  );
  expectProblem(inspect({ document: buildDocument({ census }) }), "category rows sum to 0");
});

test("a census category that does not match its matrix subsection is rejected", () => {
  const census = CENSUS.replace("| Alpha | 1 |", "| Gamma | 1 |");
  expectProblem(inspect({ document: buildDocument({ census }) }), 'census category 1 is "Gamma"');
});

test("a matrix subsection without a census row is rejected", () => {
  const matrix = `${MATRIX}\n### 2.3 Gamma\n\n| # | E2B | mapping | 状态 | 证据 |\n| --- | --- | --- | --- | --- |\n| 4 | g1 | none | ❌ | — |\n`;
  expectProblem(
    inspect({ document: buildDocument({ matrix }) }),
    "matrix subsection(s) but",
  );
});

test("a dangling requirement citation is rejected", () => {
  const matrix = MATRIX.replace("REQ-2026-0002", "REQ-2026-9999");
  expectProblem(
    inspect({ document: buildDocument({ matrix }) }),
    "REQ-2026-9999, which resolves to no requirement record",
  );
});

test("a dangling decision citation is rejected", () => {
  const matrix = MATRIX.replace("REQ-2026-0002", "ADR-20260728-fixture-missing");
  expectProblem(
    inspect({ document: buildDocument({ matrix }) }),
    "ADR-20260728-fixture-missing, which resolves to no decision record",
  );
});

test("a cross-repository requirement written inside a sibling path is accepted", () => {
  const matrix = MATRIX.replace("REQ-2026-0002", "`sdkwork-kernel/docs/REQ-2026-9999-x.md`");
  const assessment = inspect({ document: buildDocument({ matrix }) });

  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
});

test("an unregistered document is rejected", () => {
  expectProblem(inspect({ registerInIndex: false }), "does not register");
});

test("an unlinked document is rejected", () => {
  expectProblem(inspect({ registerInEntry: false }), "does not link");
});

test("parsers are total on the real document", () => {
  const text = [
    "# doc",
    VOCABULARY,
    CENSUS,
    MATRIX,
  ].join("\n");

  assert.deepEqual(parseStatusVocabulary(text), ["✅", "🟡", "❌", "⛔"]);
  const categories = parseMatrixCategories(text);
  assert.equal(categories.length, 2);
  assert.deepEqual(
    categories.map((category) => category.title),
    ["Alpha", "Beta"],
  );
  const census = parseCensus(text);
  assert.equal(census.categories.length, 2);
  assert.deepEqual(census.total, { rows: 3, ok: 0, partial: 1, missing: 1, deliberate: 1 });
});

test("a census without a total row is rejected", () => {
  const census = CENSUS.replace("| **合计** | **3** | **0** | **1** | **1** | **1** |\n", "");
  expectProblem(inspect({ document: buildDocument({ census }) }), "declares no total row");
});

test("a census row with a non-numeric state count is reported, not dropped", () => {
  const census = CENSUS.replace("| Alpha | 1 | 0 | 1 | 0 | 0 |", "| Alpha | 1 | 0 | one | 0 | 0 |");
  expectProblem(inspect({ document: buildDocument({ census }) }), "has a non-numeric state count");
});

test("arguments are parsed strictly", () => {
  assert.deepEqual(parseE2bParityMatrixArgs([]), { root: process.cwd(), json: false });
  assert.equal(parseE2bParityMatrixArgs(["--json"]).json, true);
  assert.equal(parseE2bParityMatrixArgs(["--root", "."]).root, resolve(process.cwd()));
  assert.throws(() => parseE2bParityMatrixArgs(["--root"]), /--root requires a directory/u);
  assert.throws(() => parseE2bParityMatrixArgs(["--nope"]), /unsupported argument/u);
});
