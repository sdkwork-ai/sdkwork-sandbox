import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  CLAIM_COLUMNS,
  GAP_COLUMNS,
  GAP_KINDS,
  assessE2bParityMatrix,
  formatE2bParityMatrixReport,
  parseClaimKeywords,
  parseClaimMarkers,
  parseCoverageGaps,
  parseE2bParityMatrixArgs,
  parseGapEvidence,
  parseCensus,
  parseMatrixCategories,
  parseRequirementClaimRegistry,
  parseStatusVocabulary,
  readRequirementStatus,
  requirementOwnershipIndex,
} from "../../tools/check-sandbox-e2b-parity-matrix.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const PARITY_DOC = "docs/architecture/tech/TECH-e2b-capability-parity.md";
const ARCH_ENTRY = "docs/architecture/tech/TECH_ARCHITECTURE.md";
const TECH_README = "docs/architecture/tech/README.md";
const DOCS_INDEX = "docs/INDEX.yaml";
const FIXTURE_SURFACE = "docs/architecture/tech/TECH-fixture-surface.md";
const FIXTURE_MISSING = "docs/architecture/tech/TECH-fixture-absent.md";

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
  "| 2 | b1 | none | ❌ | 无 `REQ-*`〔§3.4/1〕 |",
  "| 3 | b2 | none | ⛔ | 无 `REQ-*`〔§3.4/2〕 |",
  "",
].join("\n");

const GAPS = [
  "### 3.2 覆盖空档",
  "",
  `| ${GAP_COLUMNS.join(" | ")} |`,
  "| --- | --- | --- | --- | --- |",
  `| 1 | **unimplemented surface** | 治理阻塞 | \`REQ-2026-0002\` | blocked on a requirement that is still draft |`,
  `| 2 | **ungated artifact** | 缺门禁 | \`${FIXTURE_SURFACE}\` | the artifact exists and nothing reads it |`,
  "",
].join("\n");

const CLAIMS = [
  "### 3.4 零需求断言",
  "",
  `| ${CLAIM_COLUMNS.join(" | ")} |`,
  "| --- | --- | --- | --- |",
  "| 1 | fixture capability | `template` | unowned |",
  "| 2 | fixture second capability | `snapshot` | unowned |",
  "",
].join("\n");

function buildDocument({
  vocabulary = VOCABULARY,
  census = CENSUS,
  matrix = MATRIX,
  gaps = GAPS,
  claims = CLAIMS,
} = {}) {
  return [
    "# E2B 能力对齐审计",
    "",
    "## 0. 基准快照与状态口径",
    "",
    vocabulary,
    census,
    matrix,
    "## 3. 测试覆盖矩阵",
    "",
    "### 3.1 已实现面的覆盖",
    "",
    "| 实现面 | 实现点 | 测试全名 |",
    "| --- | --- | --- |",
    "| fixture | `crates/fixture/src/lib.rs` | `fixture::tests::works` |",
    "",
    gaps,
    "",
    claims,
    "",
  ].join("\n");
}

/**
 * Build a throwaway repository shaped like the real one for the checks this gate performs:
 *
 *   <tmp>/sdkwork-fixture/
 *     docs/INDEX.yaml
 *     docs/product/requirements/REQ-2026-0002-fixture.md          (status: draft)
 *     docs/product/requirements/REQ-2026-0003-fixture-ready.md    (status: ready)
 *     docs/architecture/decisions/ADR-20260728-fixture-decision.md
 *     docs/architecture/tech/TECH-fixture-surface.md
 *     docs/architecture/tech/TECH_ARCHITECTURE.md
 *     docs/architecture/tech/README.md
 *     docs/architecture/tech/TECH-e2b-capability-parity.md
 */
function createFixture({ document = buildDocument(), registerInIndex = true, registerInEntry = true } = {}) {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-e2b-parity-"));
  const repo = path.join(base, "sdkwork-fixture");
  const techDirectory = path.join(repo, "docs", "architecture", "tech");
  const requirementsDirectory = path.join(repo, "docs", "product", "requirements");
  mkdirSync(requirementsDirectory, { recursive: true });
  mkdirSync(path.join(repo, "docs", "architecture", "decisions"), { recursive: true });
  mkdirSync(techDirectory, { recursive: true });
  writeFileSync(
    path.join(requirementsDirectory, "REQ-2026-0002-fixture.md"),
    "# REQ-2026-0002\n\nid: REQ-2026-0002\n\nstatus: draft\n",
  );
  writeFileSync(
    path.join(requirementsDirectory, "REQ-2026-0003-fixture-ready.md"),
    "# REQ-2026-0003\n\nid: REQ-2026-0003\n\nstatus: ready\n",
  );
  writeFileSync(
    path.join(repo, "docs", "architecture", "decisions", "ADR-20260728-fixture-decision.md"),
    "# fixture\n",
  );
  writeFileSync(path.join(techDirectory, "TECH-fixture-surface.md"), "# fixture surface\n");
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

test("the repository's own residual-gap table is typed and every claim holds", () => {
  const assessment = assessE2bParityMatrix({ repoRoot });

  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
  assert.equal(assessment.gapCount, 6);

  const gaps = parseCoverageGaps(readFileSync(path.join(repoRoot, PARITY_DOC), "utf8"));
  assert.deepEqual(gaps.header, [...GAP_COLUMNS]);
  assert.equal(gaps.rows.length, 6);
  assert.equal(gaps.malformed.length, 0);
  for (const row of gaps.rows) {
    assert.ok(GAP_KINDS.includes(row["性质"]), `gap row ${row.line} declares kind ${row["性质"]}`);
    assert.notEqual(row["取证"].trim(), "", `gap row ${row.line} names no artifact`);
  }
});

test("a gap table with the wrong columns is rejected at the header", () => {
  const untyped = [
    "### 3.2 覆盖空档",
    "",
    "| 优先级 | 空档 | 说明 |",
    "| --- | --- | --- |",
    "| 1 | **unimplemented surface** | blocked |",
    "",
  ].join("\n");

  // One header mismatch means every cell below is read at the wrong offset, so the per-row
  // findings are artefacts of the misparse. The gate must report the root cause once.
  const assessment = inspect({ document: buildDocument({ gaps: untyped }) });

  assert.equal(assessment.ok, false);
  assert.deepEqual(assessment.problems, [
    `${PARITY_DOC} section 3.2 gap table header is [优先级, 空档, 说明]; expected [${GAP_COLUMNS.join(", ")}]`,
  ]);
});

test("a gap row with the wrong cell count is rejected", () => {
  const gaps = GAPS.replace(
    `| 2 | **ungated artifact** | 缺门禁 | \`${FIXTURE_SURFACE}\` | the artifact exists and nothing reads it |`,
    `| 2 | **ungated artifact** | 缺门禁 | \`${FIXTURE_SURFACE}\` |`,
  );

  expectProblem(
    inspect({ document: buildDocument({ gaps }) }),
    "coverage-gap row has 4 cell(s), expected 5",
  );
});

test("a gap section that declares no gap is rejected", () => {
  const gaps = [
    "### 3.2 覆盖空档",
    "",
    `| ${GAP_COLUMNS.join(" | ")} |`,
    "| --- | --- | --- | --- | --- |",
    "",
  ].join("\n");

  expectProblem(
    inspect({ document: buildDocument({ gaps }) }),
    "declares no coverage gap, which asserts the audit is complete",
  );
});

test("a gap row whose priority breaks the sequence is rejected", () => {
  const gaps = GAPS.replace("| 2 | **ungated artifact**", "| 3 | **ungated artifact**");

  expectProblem(
    inspect({ document: buildDocument({ gaps }) }),
    "breaks the priority sequence; expected 2",
  );
});

test("a gap kind outside the vocabulary is rejected", () => {
  const gaps = GAPS.replace("| 缺门禁 |", "| 缺文档 |");

  expectProblem(inspect({ document: buildDocument({ gaps }) }), 'declares kind "缺文档"');
});

test("a gap row naming no artifact cannot be checked and is rejected", () => {
  const gaps = GAPS.replace(`\`${FIXTURE_SURFACE}\``, "见散文");

  expectProblem(inspect({ document: buildDocument({ gaps }) }), "names no backticked artifact");
});

test("a gap row that declares an existing artifact missing is rejected", () => {
  const gaps = GAPS.replace("| 缺门禁 |", "| 缺产物 |");

  expectProblem(
    inspect({ document: buildDocument({ gaps }) }),
    `declares \`${FIXTURE_SURFACE}\` missing, but it exists`,
  );
});

test("a gap row that declares an absent artifact ungated is rejected", () => {
  const gaps = GAPS.replace(FIXTURE_SURFACE, FIXTURE_MISSING);

  expectProblem(
    inspect({ document: buildDocument({ gaps }) }),
    `declares \`${FIXTURE_MISSING}\` ungated, but ${FIXTURE_MISSING} does not exist`,
  );
});

test("a blocker that names no requirement is rejected", () => {
  // The row still names a backticked artifact, so it clears the evidence check and reaches the
  // kind-specific rule: a blocker with no requirement is one nobody can lift.
  const gaps = GAPS.replace("`REQ-2026-0002`", `\`${DOCS_INDEX}\``);

  expectProblem(
    inspect({ document: buildDocument({ gaps }) }),
    "is 治理阻塞 but names no `REQ-*`",
  );
});

test("a blocker blamed on a requirement that does not exist is rejected", () => {
  const gaps = GAPS.replace("REQ-2026-0002", "REQ-2026-9999");

  expectProblem(
    inspect({ document: buildDocument({ gaps }) }),
    "`REQ-2026-9999`, which has no record in docs/product/requirements",
  );
});

test("a blocker blamed on a requirement that is already ready is rejected", () => {
  const gaps = GAPS.replace("REQ-2026-0002", "REQ-2026-0003");

  expectProblem(
    inspect({ document: buildDocument({ gaps }) }),
    "is already ready; re-triage the gap",
  );
});

test("the repository's own zero-requirement registry is refuted-clean and fully cited", () => {
  const assessment = assessE2bParityMatrix({ repoRoot });

  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
  assert.equal(assessment.claimCount, 7);

  const registry = parseRequirementClaimRegistry(readFileSync(path.join(repoRoot, PARITY_DOC), "utf8"));
  assert.deepEqual(registry.header, [...CLAIM_COLUMNS]);
  assert.equal(registry.rows.length, 7);
  assert.equal(registry.malformed.length, 0);
  for (const row of registry.rows) {
    assert.ok(parseClaimKeywords(row["关键词"]).length > 0, `registry row ${row["#"]} names no keyword`);
    assert.notEqual(row["主题"].trim(), "", `registry row ${row["#"]} names no subject`);
  }
});

test("a capability registered as unowned is refuted by a requirement record", () => {
  // The claim dies the moment a record claims the capability in its id, slug or title.
  const claims = CLAIMS.replace("| `snapshot` |", "| `snapshot` `fixture` |");

  expectProblem(
    inspect({ document: buildDocument({ claims }) }),
    "claims no requirement owns `fixture`, but `docs/product/requirements/REQ-2026-0002-fixture.md` claims it",
  );
});

test("the ownership reading takes the id, slug and title but not the record body", () => {
  const index = requirementOwnershipIndex(repoRoot);
  const record = index.find((entry) => entry.id === "REQ-2026-0019");

  assert.ok(record, "REQ-2026-0019 must be in the ownership index");
  assert.match(record.needle, /runtime-pool-and-fast-allocation/);
  // The body talks about snapshots; the record does not own the Snapshot capability, and reading
  // the body would make the registry permanently red for the wrong reason.
  assert.doesNotMatch(record.needle, /snapshot|template|dockerfile/);
});

test("a zero-requirement claim outside the registry must cite a row", () => {
  const matrix = MATRIX.replace(" 无 `REQ-*`〔§3.4/1〕", " 无 `REQ-*`");

  expectProblem(
    inspect({ document: buildDocument({ matrix }) }),
    "asserts a capability is unowned but cites no §3.4 row",
  );
});

test("a citation to a registry row that does not exist is rejected", () => {
  const matrix = MATRIX.replace("〔§3.4/1〕", "〔§3.4/9〕");

  expectProblem(
    inspect({ document: buildDocument({ matrix }) }),
    "cites §3.4 row 9, which does not exist",
  );
});

test("a registry row that nothing cites is rejected", () => {
  const matrix = MATRIX.replace("〔§3.4/2〕", "〔§3.4/1〕");

  expectProblem(
    inspect({ document: buildDocument({ matrix }) }),
    "section 3.4 row 2 is cited nowhere",
  );
});

test("an empty zero-requirement registry is rejected", () => {
  const claims = [
    "### 3.4 零需求断言",
    "",
    `| ${CLAIM_COLUMNS.join(" | ")} |`,
    "| --- | --- | --- | --- |",
    "",
  ].join("\n");

  expectProblem(
    inspect({ document: buildDocument({ claims }) }),
    "registers no zero-requirement claim, which asserts that every capability has a requirement",
  );
});

test("a missing zero-requirement registry is reported rather than thrown", () => {
  expectProblem(
    inspect({ document: buildDocument({ claims: "" }) }),
    'has no "### 3.4" zero-requirement claim registry',
  );
});

test("a registry with the wrong columns is rejected at the header", () => {
  const claims = CLAIMS.replace("| # | 主题 | 关键词 | 说明 |", "| # | 主题 | 说明 |");

  expectProblem(
    inspect({ document: buildDocument({ claims }) }),
    "section 3.4 registry header is [#, 主题, 说明]; expected [#, 主题, 关键词, 说明]",
  );
});

test("a registry row that breaks the claim numbering is rejected", () => {
  const claims = CLAIMS.replace("| 2 | fixture second capability", "| 3 | fixture second capability");

  expectProblem(
    inspect({ document: buildDocument({ claims }) }),
    "breaks the claim numbering; expected 2",
  );
});

test("a registry row with no keyword cannot be refuted and is rejected", () => {
  const claims = CLAIMS.replace("| `snapshot` |", "| 见散文 |");

  expectProblem(
    inspect({ document: buildDocument({ claims }) }),
    "names no backticked keyword, so nothing can refute it",
  );
});

test("a registry keyword that cannot be matched against a record is rejected", () => {
  const claims = CLAIMS.replace("| `snapshot` |", "| `Snapshot / Fork` |");

  expectProblem(
    inspect({ document: buildDocument({ claims }) }),
    'declares keyword "snapshot / fork", which is not a lowercase token',
  );
});

test("parseClaimMarkers reads every citation on a line and parseClaimKeywords lowercases", () => {
  assert.deepEqual(parseClaimMarkers("无 `REQ-*`〔§3.4/1〕 与 〔§3.4/12〕"), [1, 12]);
  assert.deepEqual(parseClaimMarkers("无 `REQ-*`"), []);
  assert.deepEqual(parseClaimKeywords("`Template` `fromTemplate`"), ["template", "fromtemplate"]);
  assert.deepEqual(parseClaimKeywords("见散文"), []);
});

test("parseGapEvidence separates requirement ids from repository paths", () => {
  assert.deepEqual(parseGapEvidence("`REQ-2026-0002` `docs/a/b.md` `no-slash` `two words/x`"), {
    requirements: ["REQ-2026-0002"],
    paths: ["docs/a/b.md"],
  });
  assert.deepEqual(parseGapEvidence("—"), { requirements: [], paths: [] });
});

test("readRequirementStatus reads the record instead of the citation", () => {
  const fixture = createFixture();
  try {
    assert.deepEqual(readRequirementStatus(fixture.repo, "REQ-2026-0002"), {
      file: "docs/product/requirements/REQ-2026-0002-fixture.md",
      status: "draft",
    });
    assert.equal(readRequirementStatus(fixture.repo, "REQ-2026-0003").status, "ready");
    assert.equal(readRequirementStatus(fixture.repo, "REQ-2026-0001"), null);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("every requirement in this repository declares a readable status, and none is ready", () => {
  const directory = path.join(repoRoot, "docs/product/requirements");
  const names = readdirSync(directory).filter((name) => /^REQ-\d{4}-\d{4}-.+\.md$/u.test(name));

  assert.equal(names.length, 27);
  for (const name of names) {
    const record = readRequirementStatus(repoRoot, name.slice(0, 13));
    assert.ok(record, `${name} has no readable status, so the blocker check cannot classify it`);
    assert.notEqual(record.status, null, `${name} has no readable status`);
    // A `ready` requirement is what lifts the section 3.2 blockers and activates the runtime
    // mechanisms the README keeps inactive. If this assertion fails, re-triage section 3.2 rather
    // than relaxing the assertion.
    assert.notEqual(
      record.status,
      "ready",
      `${name} is ready; section 3.2's 治理阻塞 rows must be re-triaged`,
    );
  }
});

test("arguments are parsed strictly", () => {
  assert.deepEqual(parseE2bParityMatrixArgs([]), { root: process.cwd(), json: false });
  assert.equal(parseE2bParityMatrixArgs(["--json"]).json, true);
  assert.equal(parseE2bParityMatrixArgs(["--root", "."]).root, resolve(process.cwd()));
  assert.throws(() => parseE2bParityMatrixArgs(["--root"]), /--root requires a directory/u);
  assert.throws(() => parseE2bParityMatrixArgs(["--nope"]), /unsupported argument/u);
});
