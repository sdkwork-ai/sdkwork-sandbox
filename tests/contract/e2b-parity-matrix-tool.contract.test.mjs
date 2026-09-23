import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { join, resolve } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  ADVANTAGE_COLUMNS,
  ADVANTAGE_SECTION,
  ANSWER_COLUMNS,
  ANSWER_REQUIRED_FIGURES,
  CLAIM_ATTRIBUTION_COLUMNS,
  CLAIM_ATTRIBUTION_VERDICTS,
  CLAIM_CENSUS_COLUMNS,
  CLAIM_COLUMNS,
  CLAIM_CORRECTION_COLUMNS,
  COVERAGE_COLUMNS,
  EXCEPTION_CONTRACT_CLAUSE,
  GAP_COLUMNS,
  GAP_KINDS,
  HEADLINE_PATTERNS,
  RULE_FAMILIES,
  RULE_FAMILY_SURFACES,
  SHAPE_COLUMNS,
  UNNAMED_CONTRACT_CLAUSE,
  ZERO_REQUIREMENT_SCAN_ROOT,
  assessE2bParityMatrix,
  authorizesImplementation,
  discoverWorkspaceTests,
  formatE2bParityMatrixReport,
  listApiContracts,
  listMarkdownFiles,
  listNamedContracts,
  markdownHeadingSpans,
  parseAdvantageClaims,
  parseAnswerSection,
  parseClaimKeywords,
  parseClaimMarkers,
  parseClaimSurfaceRegistry,
  parseCoverageGaps,
  parseDeclaredRuleFamilies,
  parseE2bParityMatrixArgs,
  parseGapEvidence,
  parseImplementationCoverage,
  parseLineScopedRuleFamilies,
  parseShapeEvidence,
  parseCensus,
  parseMatrixCategories,
  parseRequirementClaimRegistry,
  parseStatusVocabulary,
  readDecisionStatuses,
  readEvidenceCounts,
  readJsonFile,
  readRequirementStatus,
  readRequirementStatuses,
  requirementOwnershipIndex,
  requirementOwnsKeyword,
  zeroRequirementClaimLines,
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

/**
 * The fixture's section 3.5: one counted claim in another document, and one correction. The document
 * it counts carries a claim, so the census has something real to recount rather than a count of zero
 * that would pass whatever the scan did.
 */
const CLAIM_SURFACE = [
  "### 3.5 跨文档零需求断言对账",
  "",
  `| ${CLAIM_CENSUS_COLUMNS.join(" | ")} |`,
  "| --- | --- | --- | --- |",
  "| 1 | `docs/product/prd/PRD.md` | `8. 尚未拆分的能力` | 1 |",
  "",
  `| ${CLAIM_CORRECTION_COLUMNS.join(" | ")} |`,
  "| --- | --- | --- | --- | --- |",
  "| 1 | `docs/product/prd/PRD.md` | Fixture Capability | `REQ-2026-0002` | `fixture-absent-probe` |",
  "",
].join("\n");

/**
 * The attribution ledger, the fixture's counterpart to the repository's own. Its one row judges the
 * one counted claim: the keyword stands for the capability, the candidate is the record a reader
 * would most suspect, and the lexical ownership of that keyword by that candidate is what the gate
 * re-derives (`capability` appears nowhere in the fixture records' needles, so the verdict holds).
 */
const ATTRIBUTION = [
  `| ${CLAIM_ATTRIBUTION_COLUMNS.join(" | ")} |`,
  "| --- | --- | --- | --- | --- | --- |",
  "| 1 | `docs/product/prd/PRD.md` | fixture capability | `capability` | 确认无承载 | `REQ-2026-0002` |",
  "",
].join("\n");

/** The fixture document the census counts. Its section 8 makes exactly one claim. */
const PRD_FIXTURE = [
  "# Fixture PRD",
  "",
  "## 1. 前言",
  "",
  "fixture prose",
  "",
  "## 8. 尚未拆分的能力",
  "",
  "| 能力 | 缺口 |",
  "| --- | --- |",
  "| fixture capability | 无 `REQ-*`；仍在拆分 |",
  "",
].join("\n");

const RUST_TEST_FILE = "crates/fixture/src/lib.rs";
const RUST_TYPE_FILE = "crates/fixture/src/model.rs";

/** A Rust source file whose single test the coverage table is expected to claim. */
function rustTestSource(testName = "works") {
  return ["#[cfg(test)]", "mod tests {", "    #[test]", `    fn ${testName}() {}`, "}", ""].join("\n");
}

/**
 * A second source file, without tests, so the crate holds two Rust sources rather than one. It gives
 * the shape table a module count that is not trivially the "sole file" case, and it gives the absence
 * check a real identifier in the tree to be refuted by.
 */
const RUST_TYPE_SOURCE = ["pub enum FixtureState {", "    Idle,", "}", ""].join("\n");

/**
 * The section 5 advantage table, the fixture's counterpart to the repository's own. Its one row
 * pairs a line anchor with the identifier the anchor should show (`works`), and cites a requirement
 * record -- the two citation forms the family resolves for every advantage row.
 */
const ADVANTAGE = [
  ADVANTAGE_SECTION,
  "",
  `| ${ADVANTAGE_COLUMNS.join(" | ")} |`,
  "| --- | --- | --- |",
  "| fixture advantage | `crates/fixture/src/lib.rs:4`（`works`）、`REQ-2026-0003` | the fixture keeps what E2B does not offer |",
  "",
].join("\n");

const COVERAGE = [
  "### 3.1 已实现面的覆盖",
  "",
  `| ${COVERAGE_COLUMNS.join(" | ")} |`,
  "| --- | --- | --- | --- |",
  `| fixture surface | \`${RUST_TEST_FILE}\` | \`${RUST_TEST_FILE}\` | \`works\` |`,
  "",
].join("\n");

/**
 * The section 1.2 shape table, the fixture's counterpart to the repository's own. It carries one row
 * per checkable form: a module count, a line count with the file named, a line-numbered anchor, a
 * bare anchor that has to resolve against the row's own crate, and a row that claims an absence by
 * naming what must not exist.
 */
const SHAPE = [
  "### 1.2 本仓当前真实形状（可点证据）",
  "",
  `| ${SHAPE_COLUMNS.join(" | ")} |`,
  "| --- | --- | --- | --- |",
  `| fixture crate | \`crates/fixture\` | 2 模块 | \`works\` is declared at \`lib.rs:4\` |`,
  `| sized crate | \`crates/fixture\` | 5 行 \`lib.rs\` | the crate is \`#[cfg(test)] mod tests {\` at \`lib.rs:1\` |`,
  `| negative claim | \`crates/fixture\` | 2 模块 | **无 \`Pausing\` 变体**（\`FixtureState\`，\`model.rs:1\`） |`,
  "| absent component | — | 不存在 | no `FixtureAbsentType` implementation exists |",
  "",
].join("\n");

/**
 * The section 1.1 answer table and its three restated figures -- the fixture's counterpart to the
 * repository's own. Every figure is derived from this fixture rather than typed into it: the census
 * figure from the three matrix rows, the requirement and decision figures from the records the
 * fixture writes, the contract figures from the two contracts it writes, the evidence figures from
 * the registry's `acknowledged` block. A fixture that hardcoded a number the gate recomputes would
 * prove the gate compares two copies of one typo.
 */
const ANSWER = [
  "### 1.1 直接回答",
  "",
  "**Not aligned.** Neither core path works.",
  "",
  `| ${ANSWER_COLUMNS.join(" | ")} |`,
  "| --- | --- | --- |",
  "| fast create | `Sandbox.create()` | no entrypoint; only `crates/fixture` is candidate-shaped |",
  "| fast deploy | `Template.build()` | `Template` has nothing behind it; no `REQ-*`〔§3.4/1〕 |",
  "",
  "Three figures:",
  "",
  "- E2B 的能力集合共 **3 项**，本仓 ✅ **0**、🟡 **1**、❌ **1**、⛔ **1**。",
  '- 2 份 `REQ-*` 中 1 份 `ready`（0 `accepted` / 1 `draft`）；1 份 `ADR` 中 1 份 `proposed`（0 份 `accepted`）；机器契约里**没有任何一份**授权实现：2 份 `*.contract.json` 中 1 份显式声明 `implementationAuthorized: false`，第 2 份 `specs/fixture-readiness.contract.json` 是发布决定记录而非能力契约，它没有该字段、但独立声明 `runtimeImplementationAuthorizationGranted: false` 且 `releaseDecision.status: "no-go"`；另有一份不以 `.contract.json` 命名的机器契约（`apis/commands/fixture-command-contract.json`）同为 `false`。',
  "- 1 份契约声明的 **3 个证据 id** 中，只有 **1 个**有 host-precondition 半产出，**2 个**仍被真实 runner 或人工评审完全阻塞。",
  "",
].join("\n");

/** The two `*.contract.json` files the fixture writes, and which of them declares the field. */
const NAMED_CONTRACTS = Object.freeze({
  declaring: "specs/fixture-alpha.contract.json",
  excepted: "specs/fixture-readiness.contract.json",
});

/** The one machine contract the fixture writes that is not named `*.contract.json`. */
const API_CONTRACT = "apis/commands/fixture-command-contract.json";

/** The registry whose `acknowledged` block the answer section's evidence figures restate. */
const EVIDENCE_COUNTS = Object.freeze({
  sandbox_contracts_with_requirements: 1,
  sandbox_total_distinct_evidence_ids: 3,
  sandbox_host_precondition_partial: 1,
  sandbox_fully_gated: 2,
});

/**
 * The line that states how many rule families this gate implements, inside the audit document. The
 * document is a self-description surface that cannot be scoped by blocks -- it names this gate and the
 * field gate in adjacent table rows -- so it is read line by line, and the counter lives on a line of
 * its own. The field gate's own line is here to prove the reader does not credit this gate with it.
 *
 * The two evidence lines are the phrasings the answer section does *not* use: they carry the same
 * figures in the other two wordings the document uses, so the fixture exercises all three.
 */
const GATE_DESCRIPTION = [
  `The matrix gate (\`tools/check-sandbox-e2b-parity-matrix.mjs\`) holds ${RULE_FAMILIES.length} 条规则族.`,
  "The field gate (`tools/check-sandbox-e2b-field-parity.mjs`) holds 10 条规则族.",
  `Evidence accounting: 1 份契约声明的 3 个证据 id 中 2 个无产出者.`,
  `The registry: 3 个证据 id 的机器可读注册表 is \`specs/sandbox-real-evidence-registry.json\`.`,
].join("\n");

/** The prose that describes this gate itself, in the two languages it is written in. */
function gateSurfacesFixture(ruleFamilyWord = "twelve", chineseWord = "十二") {
  return {
    rootReadme: `# Fixture Repository\n\nThe gate holds ${ruleFamilyWord} rule families.\n`,
    toolsReadme: `${ruleFamilyWord[0].toUpperCase()}${ruleFamilyWord.slice(1)} rule families:\n`,
    gateZeroView: `# Gate 0\n\n${chineseWord}条规则：词表、编号与形状。\n`,
  };
}

function buildDocument({
  vocabulary = VOCABULARY,
  answer = ANSWER,
  census = CENSUS,
  matrix = MATRIX,
  coverage = COVERAGE,
  gaps = GAPS,
  claims = CLAIMS,
  surface = CLAIM_SURFACE,
  shape = SHAPE,
  advantage = ADVANTAGE,
  attribution = ATTRIBUTION,
  description = GATE_DESCRIPTION,
} = {}) {
  return [
    "# E2B 能力对齐审计",
    "",
    "## 0. 基准快照与状态口径",
    "",
    vocabulary,
    answer,
    shape,
    census,
    matrix,
    "## 3. 测试覆盖矩阵",
    "",
    coverage,
    gaps,
    "",
    description,
    "",
    claims,
    "",
    surface,
    "",
    attribution,
    "",
    advantage,
    "",
  ].join("\n");
}

/**
 * Build a throwaway repository shaped like the real one for the checks this gate performs:
 *
 *   <tmp>/sdkwork-fixture/
 *     README.md
 *     tools/README.md
 *     docs/architecture/views/gate-zero-current-state.md
 *     docs/INDEX.yaml
 *     docs/product/requirements/REQ-2026-0002-fixture.md          (status: draft)
 *     docs/product/requirements/REQ-2026-0003-fixture-ready.md    (status: ready)
 *     docs/architecture/decisions/ADR-20260728-fixture-decision.md
 *     docs/architecture/tech/TECH-fixture-surface.md
 *     docs/architecture/tech/TECH_ARCHITECTURE.md
 *     docs/architecture/tech/README.md
 *     docs/architecture/tech/TECH-e2b-capability-parity.md
 *     crates/fixture/src/lib.rs                                   (one #[test])
 *     crates/fixture/src/model.rs                                 (one type, no test)
 */
function createFixture({
  document = buildDocument(),
  surfaces = gateSurfacesFixture(),
  rustSource = rustTestSource(),
  registerInIndex = true,
  registerInEntry = true,
  decisionStatus = "proposed",
  contractValues = {},
  registryCounts = EVIDENCE_COUNTS,
} = {}) {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-e2b-parity-"));
  const repo = path.join(base, "sdkwork-fixture");
  const techDirectory = path.join(repo, "docs", "architecture", "tech");
  const requirementsDirectory = path.join(repo, "docs", "product", "requirements");
  mkdirSync(requirementsDirectory, { recursive: true });
  mkdirSync(path.join(repo, "docs", "product", "prd"), { recursive: true });
  writeFileSync(path.join(repo, "docs", "product", "prd", "PRD.md"), PRD_FIXTURE);
  mkdirSync(path.join(repo, "docs", "architecture", "decisions"), { recursive: true });
  mkdirSync(path.join(repo, "docs", "architecture", "views"), { recursive: true });
  mkdirSync(techDirectory, { recursive: true });
  mkdirSync(path.join(repo, "tools"), { recursive: true });
  mkdirSync(path.join(repo, "crates", "fixture", "src"), { recursive: true });
  writeFileSync(path.join(repo, RUST_TEST_FILE), rustSource);
  writeFileSync(path.join(repo, RUST_TYPE_FILE), RUST_TYPE_SOURCE);
  writeFileSync(path.join(repo, "README.md"), surfaces.rootReadme);
  writeFileSync(path.join(repo, "tools", "README.md"), surfaces.toolsReadme);
  writeFileSync(
    path.join(repo, "docs", "architecture", "views", "gate-zero-current-state.md"),
    surfaces.gateZeroView,
  );
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
    `# fixture\n\nStatus: ${decisionStatus}\n`,
  );
  // The machine contracts and the evidence registry the answer section's figures restate. They are
  // written as real JSON rather than stubbed readers, so the fixture exercises the same discovery the
  // gate runs against the repository.
  const contractDefaults = {
    [NAMED_CONTRACTS.declaring]: {
      schemaVersion: 1,
      kind: "sdkwork.sandbox.fixture-contract",
      implementationAuthorized: false,
    },
    [NAMED_CONTRACTS.excepted]: {
      schemaVersion: 1,
      kind: "sdkwork.sandbox.fixture-release-decision",
      releaseDecision: { status: "no-go", runtimeImplementationAuthorizationGranted: false },
    },
    [API_CONTRACT]: {
      schemaVersion: 1,
      kind: "sdkwork.sandbox.fixture-api-contract",
      implementationAuthorized: false,
    },
    ["specs/sandbox-real-evidence-registry.json"]: {
      schemaVersion: 1,
      kind: "sdkwork.sandbox.real-evidence-producer-registry",
      acknowledged: registryCounts,
    },
  };
  for (const [relative, value] of Object.entries({ ...contractDefaults, ...contractValues })) {
    const absolute = path.join(repo, relative);
    mkdirSync(path.dirname(absolute), { recursive: true });
    writeFileSync(absolute, `${JSON.stringify(value, null, 2)}\n`);
  }
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

/**
 * The answer section with one figure edited. The anchor is asserted to be present first: a negative
 * case whose anchor has drifted silently edits nothing, and a test that mutates nothing and then
 * expects a finding fails for the wrong reason -- or, worse, passes because some unrelated finding
 * happened to be present.
 */
function answerWith(from, to) {
  assert.ok(ANSWER.includes(from), `the answer fixture no longer contains: ${from}`);
  return ANSWER.replace(from, to);
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
  // Six gaps were closed and moved to the closure record below the table: the PRD state machine
  // marking (traceability gate family 5), the metric family join (family 6), the capability
  // matrix join (field gate family 8), the section 5 advantage claims (this gate's shape family,
  // extended), the per-item attribution of the cross-document assertions (section 3.5's
  // attribution ledger), and the no-real-provider-consumption blocker (lifted by the 2026-09-24
  // approvals, which moved implementation into the roadmap's hands). Only this one remains,
  // genuinely open.
  assert.equal(assessment.gapCount, 1);

  const gaps = parseCoverageGaps(readFileSync(path.join(repoRoot, PARITY_DOC), "utf8"));
  assert.deepEqual(gaps.header, [...GAP_COLUMNS]);
  assert.equal(gaps.rows.length, 1);
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

test("a claim count that disagrees with the document it counts is rejected", () => {
  const surface = CLAIM_SURFACE.replace(
    "| `8. 尚未拆分的能力` | 1 |",
    "| `8. 尚未拆分的能力` | 3 |",
  );

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "declares 3 claim(s) in `docs/product/prd/PRD.md` `8. 尚未拆分的能力`, which holds 1",
  );
});

test("a census row naming a document the repository does not have is rejected", () => {
  const surface = CLAIM_SURFACE.replace(
    "| 1 | `docs/product/prd/PRD.md` | `8.",
    "| 1 | `docs/product/prd/PRD-absent.md` | `8.",
  );

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "names `docs/product/prd/PRD-absent.md`, which the repository does not have",
  );
});

test("a census row naming a section the document does not have is rejected", () => {
  const surface = CLAIM_SURFACE.replace(
    "| `8. 尚未拆分的能力` | 1 |",
    "| `9. No Such Section` | 1 |",
  );

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "counts a section `9. No Such Section` that `docs/product/prd/PRD.md` does not have",
  );
});

test("a claim no census row counts is rejected", () => {
  // The census counts an empty section instead, so the claim in section 8 is registered nowhere.
  const surface = CLAIM_SURFACE.replace("| `8. 尚未拆分的能力` | 1 |", "| `1. 前言` | 0 |");

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "docs/product/prd/PRD.md:11 asserts a capability is unowned, and no §3.5 census row counts it",
  );
});

test("a census that lists no document is rejected", () => {
  const surface = CLAIM_SURFACE.replace(
    "| 1 | `docs/product/prd/PRD.md` | `8. 尚未拆分的能力` | 1 |\n",
    "",
  );

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "section 3.5 census lists no document, which asserts the claim is made nowhere but this document",
  );
});

test("a census row that breaks the numbering is rejected", () => {
  const surface = CLAIM_SURFACE.replace(
    "| 1 | `docs/product/prd/PRD.md` | `8.",
    "| 2 | `docs/product/prd/PRD.md` | `8.",
  );

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "breaks the census numbering; expected 1",
  );
});

test("a census table with the wrong columns is reported rather than misread", () => {
  const surface = CLAIM_SURFACE.replace(
    `| ${CLAIM_CENSUS_COLUMNS.join(" | ")} |`,
    "| # | 文档 | 断言数 |",
  );

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "section 3.5 has no census table; expected columns [#, 文档, 段落, 断言数]",
  );
});

test("a missing cross-document census is reported rather than thrown", () => {
  expectProblem(
    inspect({ document: buildDocument({ surface: "" }) }),
    'has no "### 3.5" cross-document claim census',
  );
});

test("a refuted claim whose carrying requirement does not exist is rejected", () => {
  const surface = CLAIM_SURFACE.replace(
    "`REQ-2026-0002` | `fixture-absent-probe`",
    "`REQ-2026-9999` | `fixture-absent-probe`",
  );

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "blames the corrected claim on `REQ-2026-9999`, which has no record in docs/product/requirements",
  );
});

test("a refuted claim whose stale phrasing is still present is rejected", () => {
  // `仍在拆分` is exactly what the fixture document still says, so the correction is unproved.
  const surface = CLAIM_SURFACE.replace("`fixture-absent-probe`", "`仍在拆分`");

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "records `仍在拆分` as corrected, but `docs/product/prd/PRD.md` still contains it",
  );
});

test("a correction ledger with no row is rejected", () => {
  const surface = CLAIM_SURFACE.replace(
    "| 1 | `docs/product/prd/PRD.md` | Fixture Capability | `REQ-2026-0002` | `fixture-absent-probe` |\n",
    "",
  );

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "records no corrected claim, which asserts no document ever asserted this falsely",
  );
});

test("a correction row naming no probe cannot be refuted and is rejected", () => {
  const surface = CLAIM_SURFACE.replace("`fixture-absent-probe`", "见散文");

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "names no backticked probe, so nothing can refute the correction",
  );
});

test("a correction row naming no carrying requirement is rejected", () => {
  const surface = CLAIM_SURFACE.replace("`REQ-2026-0002`", "—");

  expectProblem(
    inspect({ document: buildDocument({ surface }) }),
    "names no `REQ-*`, so nothing carries the corrected claim",
  );
});

test("a keyword matched only inside a longer word does not refute a claim", () => {
  // The regression this pins: `port` is a substring of `transport`, and a substring matcher reads a
  // record about transport as owning the port-exposure capability. No record does today, so the
  // defect was latent rather than absent -- which is why it needs a case rather than a memory.
  const transport = { needle: "req-2026-0002 sandbox-transport-hardening 交付传输层加固" };

  assert.equal(requirementOwnsKeyword(transport, "port"), false);
  assert.equal(requirementOwnsKeyword(transport, "transport"), true);
  assert.equal(requirementOwnsKeyword({ needle: "req-2026-0002 x port y" }, "port"), true);
  assert.equal(requirementOwnsKeyword({ needle: "req-2026-0002 fixture" }, "fixture"), true);
  // A keyword that is itself hyphenated still matches on its own boundaries.
  assert.equal(
    requirementOwnsKeyword({ needle: "req-2026-0002 runtime-pool" }, "runtime-pool"),
    true,
  );
});

test("the repository's own cross-document census recount matches its declarations", () => {
  const assessment = assessE2bParityMatrix({ repoRoot });

  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
  // Five documents, 19 claims: the widened phrasing set (无独立 `REQ-*`) added the Auto Pause and
  // MCP assertions in PRD-capabilities, the MCP row in PRD section 8, and the Port Exposure row of
  // PRD-sandbox-surfaces to the four documents the narrower set counted.
  assert.equal(assessment.claimSurfaceCount, 5);
  assert.equal(assessment.claimSurfaceClaims, 18);

  const surface = parseClaimSurfaceRegistry(readFileSync(path.join(repoRoot, PARITY_DOC), "utf8"));
  assert.deepEqual(surface.census.header, [...CLAIM_CENSUS_COLUMNS]);
  assert.deepEqual(surface.corrections.header, [...CLAIM_CORRECTION_COLUMNS]);
  assert.deepEqual(surface.attribution.header, [...CLAIM_ATTRIBUTION_COLUMNS]);
  assert.equal(surface.census.malformed.length, 0);
  // Two corrections on record: the benchmark-capability claim and the SDK-family claim, both
  // retired into the ledger with probes instead of being edited silently.
  assert.equal(surface.corrections.rows.length, 2);
  assert.equal(surface.attribution.rows.length, 18);

  // The census is the whole account: every document outside this one that makes the claim is
  // registered. This restates the gate's completeness scan from outside the gate.
  const registered = new Set(surface.census.rows.map((row) => row["文档"]));
  for (const relative of listMarkdownFiles(repoRoot, ZERO_REQUIREMENT_SCAN_ROOT)) {
    if (relative === PARITY_DOC) continue;
    const lines = zeroRequirementClaimLines(readFileSync(path.join(repoRoot, relative), "utf8"));
    if (lines.length === 0) continue;
    assert.ok(registered.has(`\`${relative}\``), `${relative} makes a claim no census row counts`);
  }
});

test("the repository's own attribution ledger judges every counted claim", () => {
  const assessment = assessE2bParityMatrix({ repoRoot });
  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));

  const surface = parseClaimSurfaceRegistry(readFileSync(path.join(repoRoot, PARITY_DOC), "utf8"));
  const verdicts = new Set(surface.attribution.rows.map((row) => row["判定"]));
  for (const verdict of verdicts) {
    assert.ok(CLAIM_ATTRIBUTION_VERDICTS.includes(verdict), `unknown verdict ${verdict}`);
  }
  // Every counted capability is either confirmed unowned with named candidates, marked as a
  // definitional sentence, or pointed at the correction ledger -- none left unjudged.
  const unowned = surface.attribution.rows.filter((row) => row["判定"] === "确认无承载");
  assert.ok(unowned.length >= 14, `expected the bulk to be confirmed unowned, got ${unowned.length}`);
  for (const row of unowned) {
    assert.ok(/REQ-\d{4}-\d{4}/u.test(row["已核对候选"]), `row ${row["#"]} cites no candidate`);
    assert.ok(row["关键词"].trim() !== "", `row ${row["#"]} names no keyword`);
  }
});

test("a missing attribution ledger is reported rather than passed", () => {
  expectProblem(
    inspect({ document: buildDocument({ attribution: "" }) }),
    "has no attribution ledger",
  );
});

test("an attribution table that judges nothing is rejected", () => {
  const attribution = [
    `| ${CLAIM_ATTRIBUTION_COLUMNS.join(" | ")} |`,
    "| --- | --- | --- | --- | --- | --- |",
    "",
  ].join("\n");

  expectProblem(inspect({ document: buildDocument({ attribution }) }), "attributes no counted claim");
});

test("an attribution row with an unknown verdict is rejected", () => {
  const attribution = ATTRIBUTION.replace("确认无承载", "已由 REQ 承载");

  expectProblem(inspect({ document: buildDocument({ attribution }) }), 'declares verdict "已由 REQ 承载"');
});

test("an attribution row that names no keyword is rejected", () => {
  const attribution = ATTRIBUTION.replace("| `capability` | 确认无承载 |", "| — | 确认无承载 |");

  expectProblem(inspect({ document: buildDocument({ attribution }) }), "names no keyword");
});

test("an attribution row citing no checked record is rejected", () => {
  const attribution = ATTRIBUTION.replace(" | `REQ-2026-0002` |", " | — |");

  expectProblem(inspect({ document: buildDocument({ attribution }) }), "cites no checked requirement record");
});

test("an attribution row whose candidate does not resolve is rejected", () => {
  const attribution = ATTRIBUTION.replace("`REQ-2026-0002`", "`REQ-2026-9999`");

  expectProblem(
    inspect({ document: buildDocument({ attribution }) }),
    "`REQ-2026-9999`, which has no record in",
  );
});

test("an attribution row whose keyword is owned by its candidate is rejected", () => {
  // The refutation that keeps the ledger honest: the fixture record's slug contains `fixture`, so
  // citing it as checked-and-unowned for the keyword `fixture` is self-contradicting.
  const attribution = ATTRIBUTION.replace("| `capability` |", "| `fixture` |");

  expectProblem(
    inspect({ document: buildDocument({ attribution }) }),
    "the checked candidate `REQ-2026-0002` owns it",
  );
});

test("an attribution row claiming a refuted capability without a ledger entry is rejected", () => {
  const attribution = ATTRIBUTION.replace(
    "| 1 | `docs/product/prd/PRD.md` | fixture capability | `capability` | 确认无承载 | `REQ-2026-0002` |",
    "| 1 | `docs/product/prd/PRD.md` | fixture capability | — | 已证伪，见更正账 | — |",
  );

  expectProblem(
    inspect({ document: buildDocument({ attribution }) }),
    "holds no `fixture capability` entry",
  );
});

test("an attribution count that disagrees with the census is rejected", () => {
  const attribution = ATTRIBUTION.replace(
    "`REQ-2026-0002` |",
    "`REQ-2026-0002` |\n| 2 | `docs/product/prd/PRD.md` | another counted claim | `capability` | 确认无承载 | `REQ-2026-0003` |",
  );

  expectProblem(
    inspect({ document: buildDocument({ attribution }) }),
    "attributes 2 claim(s) in `docs/product/prd/PRD.md`, but its census row declares 1",
  );
});

test("an attribution row for a document no census row counts is rejected", () => {
  const attribution = ATTRIBUTION.replace(
    "`REQ-2026-0002` |",
    "`REQ-2026-0002` |\n| 2 | `docs/architecture/tech/TECH-fixture-surface.md` | another counted claim | `capability` | 确认无承载 | `REQ-2026-0003` |",
  );

  expectProblem(
    inspect({ document: buildDocument({ attribution }) }),
    "attributes claims in `docs/architecture/tech/TECH-fixture-surface.md`, which no census row counts",
  );
});

test("a broken attribution numbering is rejected", () => {
  const attribution = ATTRIBUTION.replace("| 1 | `docs/product/prd/PRD.md` | fixture capability", "| 7 | `docs/product/prd/PRD.md` | fixture capability");

  expectProblem(
    inspect({ document: buildDocument({ attribution }) }),
    "breaks the attribution numbering; expected 1",
  );
});

test("the attribution reader is total on the real document and on one without the section", () => {
  const real = parseClaimSurfaceRegistry(readFileSync(path.join(repoRoot, PARITY_DOC), "utf8"));
  assert.equal(real.attribution.rows.length, 18);
  // A document with no section at all parses to null, the same contract the census reader states.
  assert.equal(parseClaimSurfaceRegistry("# nothing to see\n"), null);
});

test("the cross-document census reader is total, and heading spans do not nest", () => {
  assert.ok(parseClaimSurfaceRegistry(readFileSync(path.join(repoRoot, PARITY_DOC), "utf8")));
  assert.equal(parseClaimSurfaceRegistry(buildDocument({ surface: "" })), null);
  assert.equal(markdownHeadingSpans("").length, 0);

  const spans = markdownHeadingSpans(PRD_FIXTURE);
  const preamble = spans.find((span) => span.title === "1. 前言");
  const later = spans.find((span) => span.title === "8. 尚未拆分的能力");
  assert.ok(preamble && later);
  // A shallower heading ends the deeper one, and a sibling ends its predecessor.
  assert.equal(preamble.end, later.start - 1);
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

test("every requirement in this repository declares a readable status", () => {
  const directory = path.join(repoRoot, "docs/product/requirements");
  const names = readdirSync(directory).filter((name) => /^REQ-\d{4}-\d{4}-.+\.md$/u.test(name));

  assert.equal(names.length, 28);
  const ready = [];
  for (const name of names) {
    const record = readRequirementStatus(repoRoot, name.slice(0, 13));
    assert.ok(record, `${name} has no readable status, so the blocker check cannot classify it`);
    assert.notEqual(record.status, null, `${name} has no readable status`);
    if (record.status === "ready") ready.push(name);
  }
  // The first transition (2026-09-24) promoted exactly the three provider-side requirements. A new
  // `ready` requirement is fine, but section 3.2's 治理阻塞 rows must be re-triaged when it happens,
  // so the count is pinned to make that re-triage deliberate rather than accidental.
  assert.deepEqual(ready.sort(), [
    "REQ-2026-0003-secure-local-provider.md",
    "REQ-2026-0007-sandbox-command-execution-contract.md",
    "REQ-2026-0008-firecracker-sandbox-provider.md",
  ]);
});

// ---- rule 9: IMPLEMENTATION COVERAGE

test("the fixture's coverage table accounts for every test the fixture workspace declares", () => {
  const assessment = inspect();

  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
  assert.equal(assessment.workspaceTests, 1);
  assert.equal(assessment.coveredTests, 1);
});

test("the repository's own coverage table accounts for every test the workspace declares", () => {
  const assessment = assessE2bParityMatrix({ repoRoot });

  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
  assert.equal(assessment.workspaceTests, 95);
  assert.equal(assessment.coveredTests, 95);

  const discovered = discoverWorkspaceTests(repoRoot);
  let runnable = 0;
  let ignored = 0;
  for (const tests of discovered.values()) {
    for (const entry of tests) {
      if (entry.ignored) ignored += 1;
      else runnable += 1;
    }
  }
  // The two readings the audit quotes have to agree with the code: 90 declared, 89 of them
  // runnable because one declares it needs an external PostgreSQL.
  assert.equal(runnable + ignored, 95);
  assert.equal(ignored, 1);
});

test("a test the workspace declares but the coverage table omits is rejected", () => {
  // The table still cites a case, so every per-row check passes and only the two-way accounting
  // catches the omission -- which is the direction that actually failed here: two repository crates
  // and 19 of the workspace's 68 tests were absent while the table claimed to list them all.
  const rustSource = rustTestSource().replace(
    "    fn works() {}",
    "    fn works() {}\n\n    #[test]\n    fn second() {}",
  );

  expectProblem(inspect({ rustSource }), "does not account for `second`");
});

test("a cited test name the cited test file does not declare is rejected", () => {
  const coverage = COVERAGE.replace("| `works` |", "| `works`、`imaginary` |");

  expectProblem(inspect({ document: buildDocument({ coverage }) }), "cites `imaginary`, which");
});

test("a cited implementation path that does not exist is rejected", () => {
  const coverage = COVERAGE.replace(
    `| \`${RUST_TEST_FILE}\` |`,
    "| `crates/fixture/src/absent.rs` |",
  );

  expectProblem(
    inspect({ document: buildDocument({ coverage }) }),
    "`crates/fixture/src/absent.rs`, which does not exist",
  );
});

test("a line anchor past the end of the cited file is rejected", () => {
  const coverage = COVERAGE.replace(`| \`${RUST_TEST_FILE}\` |`, `| \`${RUST_TEST_FILE}:9999\` |`);

  expectProblem(inspect({ document: buildDocument({ coverage }) }), "has 6 line(s)");
});

test("a cited test file that declares no test is rejected", () => {
  expectProblem(
    inspect({ rustSource: "pub fn plain() {}\n" }),
    `names \`${RUST_TEST_FILE}\` as a test file, but a file under crates declares no \`#[test]\` at that path`,
  );
});

test("the same case cited twice is rejected", () => {
  const coverage = COVERAGE.replace("| `works` |", "| `works`、`works` |");

  expectProblem(inspect({ document: buildDocument({ coverage }) }), "cites `works` a second time");
});

test("a coverage table with the wrong columns reports the header and stops", () => {
  const coverage = COVERAGE.replace(
    `| ${COVERAGE_COLUMNS.join(" | ")} |`,
    "| 实现面 | 实现点 | 测试全名 |",
  );

  expectProblem(
    inspect({ document: buildDocument({ coverage }) }),
    `expected [${COVERAGE_COLUMNS.join(", ")}]`,
  );
});

test("a test declared with attribute arguments is discovered, not dropped", () => {
  // The first draft of this rule required `]` immediately after `test`, so
  // `#[tokio::test(flavor = "multi_thread", worker_threads = 4)]` was invisible and the gate
  // counted 67 tests where the workspace holds 68. A narrow pattern reports a coverage table as
  // complete while a real test is missing from it, which is the same defect as the character class
  // that once reported 36 uncovered operations instead of 19.
  const rustSource = [
    "#[cfg(test)]",
    "mod tests {",
    "    #[test]",
    "    fn works() {}",
    "",
    '    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]',
    "    async fn async_works() {}",
    "}",
    "",
  ].join("\n");
  const fixture = createFixture({ rustSource });
  try {
    assert.deepEqual(
      discoverWorkspaceTests(fixture.repo)
        .get(RUST_TEST_FILE)
        .map((entry) => entry.name),
      ["works", "async_works"],
    );
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("an ignored test is recorded as ignored rather than dropped", () => {
  const discovered = discoverWorkspaceTests(repoRoot);
  const ignored = [...discovered]
    .flatMap(([file, tests]) => tests.map((entry) => ({ ...entry, file })))
    .filter((entry) => entry.ignored);

  assert.deepEqual(
    ignored.map((entry) => entry.name),
    ["sandbox_postgres_repository_enforces_durable_lifecycle_contract"],
  );
});

// ---- rule 10: SELF-DESCRIPTION

test("every surface describing this gate declares the rule families it implements", () => {
  assert.equal(RULE_FAMILIES.length, 12);
  assert.equal(new Set(RULE_FAMILIES).size, RULE_FAMILIES.length, "family keys must be unique");

  for (const surface of RULE_FAMILY_SURFACES) {
    const text = readFileSync(join(repoRoot, surface.path), "utf8");
    if (surface.lineScoped === true) {
      // This surface describes both gates inside one table, so it is read line by line: every line
      // that names this gate and states a count is a claim, and all of them must agree.
      const readings = parseLineScopedRuleFamilies(text, "check-sandbox-e2b-parity-matrix.mjs");
      assert.ok(readings.length > 0, `${surface.id} (${surface.path}) must state the count`);
      for (const reading of readings) {
        assert.equal(
          reading.value,
          RULE_FAMILIES.length,
          `${surface.id} (${surface.path}) line ${reading.line} must declare ${RULE_FAMILIES.length} rule families`,
        );
      }
      continue;
    }
    assert.equal(
      parseDeclaredRuleFamilies(text, surface.language, { own: surface.own === true }),
      RULE_FAMILIES.length,
      `${surface.id} (${surface.path}) must declare ${RULE_FAMILIES.length} rule families`,
    );
  }
});

test("a surface declaring the wrong number of rule families is rejected", () => {
  expectProblem(
    inspect({ surfaces: gateSurfacesFixture("eight", "八") }),
    `declares 8 rule families, this gate implements ${RULE_FAMILIES.length}`,
  );
});

test("a surface declaring no rule-family count is rejected", () => {
  expectProblem(
    inspect({
      surfaces: { ...gateSurfacesFixture(), toolsReadme: "# Tooling\n\nThis paragraph states no count.\n" },
    }),
    "declares no rule-family count",
  );
});

test("a missing surface is reported rather than skipped", () => {
  const fixture = createFixture();
  try {
    rmSync(join(fixture.repo, "tools", "README.md"));
    const assessment = assessE2bParityMatrix({ repoRoot: fixture.repo });
    assert.equal(assessment.ok, false);
    assert.ok(
      assessment.problems.some((problem) =>
        problem.includes("the tools README (tools/README.md) is missing"),
      ),
      formatE2bParityMatrixReport(assessment),
    );
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("the family count is read where this gate is described, not where another gate is", () => {
  // `tools/README.md` describes this gate above the field gate, and a count read from the wrong
  // section is attributed to the wrong gate.
  const twoGates = [
    "`check-sandbox-e2b-parity-matrix.mjs` keeps the matrix honest. Ten rule families:",
    "",
    "`check-sandbox-e2b-field-parity.mjs` keeps the baseline the matrix rests on.",
    "",
    "Ten rule families:",
  ].join("\n");
  assert.equal(parseDeclaredRuleFamilies(twoGates, "english"), 10);

  // A count that stands outside the place this gate is described is not this gate's.
  assert.equal(
    parseDeclaredRuleFamilies(
      ["Seven rule families:", "", "`check-sandbox-e2b-parity-matrix.mjs` is named with no count."].join("\n"),
      "english",
    ),
    null,
  );
});

test("a section is not credited with the next section's number", () => {
  // `README.md`'s matrix and field paragraphs sit one block apart. Reading the naming block plus the
  // following block unconditionally let the matrix section -- which states no count of its own --
  // pass by borrowing the field section's "ten", which is the misattribution the scoping exists to
  // prevent. A following block that names another gate is that gate's section, not a continuation.
  const adjacent = [
    "`check-sandbox-e2b-parity-matrix.mjs` keeps the matrix honest and states no count.",
    "",
    "`check-sandbox-e2b-field-parity.mjs` holds ten rule families.",
  ].join("\n");
  assert.equal(parseDeclaredRuleFamilies(adjacent, "english"), null);

  // The widening is still allowed when the next block introduces no gate of its own, because a
  // count written as a standalone line lands in the following paragraph by ordinary prose habit.
  const standaloneLine = [
    "`check-sandbox-e2b-parity-matrix.mjs` keeps the matrix honest.",
    "",
    "Ten rule families:",
  ].join("\n");
  assert.equal(parseDeclaredRuleFamilies(standaloneLine, "english"), 10);
});

test("a usage fence is not where the count lives", () => {
  // The section's own usage fence names the gate file *after* the sentence that declares the count.
  // Treating the fence as the naming block started the scope one block too late, so the gate
  // reported "declares no rule-family count" against a section that states it plainly.
  const withFence = [
    "`check-sandbox-e2b-parity-matrix.mjs` keeps the matrix honest. Ten rule families:",
    "",
    "```bash",
    "node tools/check-sandbox-e2b-parity-matrix.mjs",
    "```",
    "",
    "A trailing paragraph with no count.",
  ].join("\n");
  assert.equal(parseDeclaredRuleFamilies(withFence, "english"), 10);
});

// ---- rule family 11: shape evidence (section 1.2)

test("the repository's own shape table resolves and its sizes recompute", () => {
  const shape = parseShapeEvidence(readFileSync(path.join(repoRoot, PARITY_DOC), "utf8"));
  assert.ok(shape?.header, "section 1.2 must carry a table");
  assert.equal(shape.header.join("|"), SHAPE_COLUMNS.join("|"));
  assert.equal(shape.rows.length, 11);

  const assessment = assessE2bParityMatrix({ repoRoot });
  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
  assert.equal(assessment.shapeRows, 11);
  // Ten lines in the real table point the reader at a numbered line (the Local Provider row cites
  // its two production modules and a test module; the Command Executor row cites its port and its
  // fingerprint function). Each is resolved into its file and checked to still carry the construct
  // the row names.
  assert.equal(assessment.shapeAnchors, 10);
});

test("a module count that disagrees with the crate's sources is rejected", () => {
  const shape = SHAPE.replace("| 2 模块 |", "| 4 模块 |");

  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    "declares 4 模块, but `crates/fixture/src` holds 2 Rust source file(s)",
  );
});

test("a line count that disagrees with the file it names is rejected", () => {
  const shape = SHAPE.replace("| 5 行 `lib.rs` |", "| 9 行 `lib.rs` |");

  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    "declares 9 行, but the file it names holds 5 line(s)",
  );
});

test("a size that names no resolvable file is rejected", () => {
  const shape = SHAPE.replace("| 5 行 `lib.rs` |", "| 5 行 |");

  // The crate holds two sources, so "5 行" with no file named has no single referent. Guessing one
  // would make the figure unfalsifiable, which is the state this family exists to end.
  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    "declares 5 行 but names no single resolvable Rust source file to count",
  );
});

test("a cited shape path that does not exist is rejected", () => {
  const shape = SHAPE.replace("`crates/fixture`", "`crates/absent`");

  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    "`crates/absent`, which does not exist",
  );
});

test("a line anchor past the end of its file is rejected", () => {
  const shape = SHAPE.replace("`lib.rs:4`", "`lib.rs:9999`");

  expectProblem(inspect({ document: buildDocument({ shape }) }), "`lib.rs` has 5 line(s)");
});

test("a line anchor that resolves but points at unrelated code is rejected", () => {
  // The quiet half of this rot. The number still lands inside the file, so nothing looks broken until
  // the reader follows it and finds a plausible declaration that has nothing to do with the row.
  const shape = SHAPE.replace("at `lib.rs:1`", "at `lib.rs:5`");

  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    "nothing the row attributes to it is visible at `lib.rs:5`",
  );
});

test("a shape row that cites no evidence is rejected", () => {
  const shape = SHAPE.replace(
    "no `FixtureAbsentType` implementation exists",
    "nothing of the sort is implemented here",
  );

  expectProblem(inspect({ document: buildDocument({ shape }) }), "cites no backticked evidence");
});

test("a row that declares a component absent without naming what is absent is rejected", () => {
  const shape = SHAPE.replace(
    "no `FixtureAbsentType` implementation exists",
    "nothing of the sort beyond `REQ-*`",
  );

  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    "declares the component absent but names nothing absent",
  );
});

test("an absence claim refuted by a crate directory is rejected", () => {
  const shape = SHAPE.replace("no `FixtureAbsentType` implementation exists", "no `fixture` crate exists");

  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    "declares no `fixture` implementation, but crates/fixture exists",
  );
});

test("an absence claim refuted by a source identifier is rejected", () => {
  // The other half of the absence check: a PascalCase token is looked up in `crates/**/*.rs`, so a
  // row may only claim an identifier absent if the tree agrees.
  const shape = SHAPE.replace("no `FixtureAbsentType` implementation exists", "no `FixtureState` enum exists");

  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    "declares `FixtureState` absent, but it appears in crates/fixture/src/model.rs",
  );
});

test("an identifier named inside a negative phrase is re-derived from the anchored file", () => {
  // The fixture says the enum has no `Pausing` variant. Swapping in a variant the enum really does
  // declare must fail -- scope is the anchored file, so this is a check on that construct in that
  // file, not a search of the repository for a word.
  const shape = SHAPE.replace("**无 `Pausing` 变体**", "**无 `Idle` 变体**");

  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    "names `Idle` inside a negative phrase, but it occurs in crates/fixture/src/model.rs",
  );
});

test("a cited glob that matches nothing is rejected", () => {
  const shape = SHAPE.replace("`works` is declared at `lib.rs:4`", "`works` is declared in `crates/**/*.toml`");

  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    "`crates/**/*.toml`, which does not exist",
  );
});

test("a shape table with the wrong columns reports the header and stops", () => {
  const shape = SHAPE.replace(`| ${SHAPE_COLUMNS.join(" | ")} |`, "| 组件 | 位置 | 规模 | 状态 |");

  expectProblem(
    inspect({ document: buildDocument({ shape }) }),
    `expected [${SHAPE_COLUMNS.join(", ")}]`,
  );
});

test("a shape row with the wrong cell count is rejected", () => {
  const shape = SHAPE.replace("| absent component | — | 不存在 |", "| absent component | — | 不存在 | extra |");

  expectProblem(inspect({ document: buildDocument({ shape }) }), "shape-evidence row has 5 cell(s)");
});

test("a shape section that describes no component is rejected", () => {
  const shape = [
    "### 1.2 本仓当前真实形状（可点证据）",
    "",
    `| ${SHAPE_COLUMNS.join(" | ")} |`,
    "| --- | --- | --- | --- |",
    "",
  ].join("\n");

  expectProblem(inspect({ document: buildDocument({ shape }) }), "describes no component");
});

test("a missing shape section is reported rather than thrown", () => {
  expectProblem(
    inspect({ document: buildDocument({ shape: "" }) }),
    'has no "### 1.2" shape-evidence section',
  );
});

test("parseShapeEvidence is total on the real document and on one without the section", () => {
  const real = readFileSync(path.join(repoRoot, PARITY_DOC), "utf8");
  assert.equal(parseShapeEvidence(real).rows.length, 11);
  assert.equal(parseShapeEvidence("# nothing to see\n"), null);
});

// ---- rule family 11: advantage claims (section 5)

test("the repository's own advantage table resolves every citation it makes", () => {
  const advantages = parseAdvantageClaims(readFileSync(path.join(repoRoot, PARITY_DOC), "utf8"));
  assert.ok(advantages?.header, "section 5 must carry a table");
  assert.equal(advantages.header.join("|"), ADVANTAGE_COLUMNS.join("|"));
  assert.equal(advantages.rows.length, 9);

  const assessment = assessE2bParityMatrix({ repoRoot });
  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
  assert.equal(assessment.advantageRows, 9);
  // Four lines across the three rows that cite numbered positions: identity.rs:97, model.rs:276,
  // provider.rs:55 and capability.rs:2. Each is resolved into its file, kept inside its bounds and
  // paired with another citation the row makes that is visible at the anchor.
  assert.equal(assessment.advantageAnchors, 4);
});

test("a missing advantage section is reported rather than thrown", () => {
  expectProblem(
    inspect({ document: buildDocument({ advantage: "" }) }),
    `has no "${ADVANTAGE_SECTION}" advantage section`,
  );
});

test("an advantage table that describes no row is rejected", () => {
  const advantage = [
    ADVANTAGE_SECTION,
    "",
    `| ${ADVANTAGE_COLUMNS.join(" | ")} |`,
    "| --- | --- | --- |",
    "",
  ].join("\n");

  expectProblem(inspect({ document: buildDocument({ advantage }) }), "claims no advantage");
});

test("an advantage table with the wrong columns reports the header", () => {
  const advantage = ADVANTAGE.replace(
    `| ${ADVANTAGE_COLUMNS.join(" | ")} |`,
    "| 能力 | 证据 | 理由 |",
  );

  expectProblem(
    inspect({ document: buildDocument({ advantage }) }),
    `expected [${ADVANTAGE_COLUMNS.join(", ")}]`,
  );
});

test("an advantage row that cites nothing openable is rejected", () => {
  const advantage = ADVANTAGE.replace(
    "| fixture advantage | `crates/fixture/src/lib.rs:4`（`works`）、`REQ-2026-0003` |",
    "| fixture advantage | nothing E2B could not also claim |",
  );

  expectProblem(inspect({ document: buildDocument({ advantage }) }), "cites no backticked evidence");
});

test("an advantage row with the wrong cell count is rejected", () => {
  const advantage = ADVANTAGE.replace(
    "| fixture advantage | `crates/fixture/src/lib.rs:4`（`works`）、`REQ-2026-0003` |",
    "| fixture advantage | `crates/fixture/src/lib.rs:4`（`works`）、`REQ-2026-0003` | extra |",
  );

  expectProblem(inspect({ document: buildDocument({ advantage }) }), "advantage row has 4 cell(s)");
});

test("an advantage row that names no capability is rejected", () => {
  const advantage = ADVANTAGE.replace(
    "| fixture advantage | `crates/fixture/src/lib.rs:4`（`works`）、`REQ-2026-0003` |",
    "|  | `crates/fixture/src/lib.rs:4`（`works`）、`REQ-2026-0003` |",
  );

  expectProblem(inspect({ document: buildDocument({ advantage }) }), "names no capability");
});

test("an advantage row whose evidence names nothing checkable is rejected", () => {
  // `SomeFixtureWord` is neither a repository path, a Rust file, a record id, nor a snake_case
  // identifier, so the row asserts an advantage nothing in the tree could refute.
  const advantage = ADVANTAGE.replace(
    "| fixture advantage | `crates/fixture/src/lib.rs:4`（`works`）、`REQ-2026-0003` |",
    "| fixture advantage | `SomeFixtureWord` |",
  );

  expectProblem(inspect({ document: buildDocument({ advantage }) }), "names no checkable anchor");
});

test("an advantage row citing a bare Rust file name is rejected", () => {
  // The defect the real table carried: `provider.rs:50` names no crate, so it survives every crate
  // rename by pointing at nothing in particular.
  const advantage = ADVANTAGE.replace("`crates/fixture/src/lib.rs:4`", "`lib.rs:4`");

  expectProblem(inspect({ document: buildDocument({ advantage }) }), "cites the bare file name `lib.rs:4`");
});

test("an advantage row citing a path that does not exist is rejected", () => {
  const advantage = ADVANTAGE.replace("`crates/fixture/src/lib.rs:4`", "`crates/fixture/src/absent.rs:4`");

  expectProblem(
    inspect({ document: buildDocument({ advantage }) }),
    "`crates/fixture/src/absent.rs:4`, which resolves to no file",
  );
});

test("an advantage row citing a non-anchor path that does not exist is rejected", () => {
  // The path branch and the anchor branch are different judgements: a token with no line number is
  // existence-checked as a file, so `specs/absent-thing.contract.json` must fail on existence even
  // though no anchor is involved.
  const advantage = ADVANTAGE.replace("`REQ-2026-0003`", "`specs/absent-thing.contract.json`");

  expectProblem(
    inspect({ document: buildDocument({ advantage }) }),
    "`specs/absent-thing.contract.json`, which does not exist",
  );
});

test("an advantage line anchor past the end of its file is rejected", () => {
  const advantage = ADVANTAGE.replace("`crates/fixture/src/lib.rs:4`", "`crates/fixture/src/lib.rs:9999`");

  expectProblem(inspect({ document: buildDocument({ advantage }) }), "has 5 line(s)");
});

test("an advantage line anchor that resolves but points at unrelated code is rejected", () => {
  // The quiet half of the rot, which the real table carried twice: `identity.rs:88` and
  // `model.rs:24` both still resolved inside their files while the constructs they name had moved.
  // The anchor moves to the fixture's second source file, which the window shows is an enum
  // declaration -- nothing the row cites (`works`, the requirement) is visible there.
  const advantage = ADVANTAGE.replace("`crates/fixture/src/lib.rs:4`", "`crates/fixture/src/model.rs:1`");

  expectProblem(
    inspect({ document: buildDocument({ advantage }) }),
    "nothing else the row cites is visible at `crates/fixture/src/model.rs:1`",
  );
});

test("an advantage row citing a requirement that has no record is rejected", () => {
  const advantage = ADVANTAGE.replace("`REQ-2026-0003`", "`REQ-2026-9999`");

  expectProblem(
    inspect({ document: buildDocument({ advantage }) }),
    "`REQ-2026-9999`, which has no record in",
  );
});

test("an advantage row citing an identifier the tree does not carry is rejected", () => {
  const advantage = ADVANTAGE.replace(
    "| fixture advantage | `crates/fixture/src/lib.rs:4`（`works`）、`REQ-2026-0003` |",
    "| fixture advantage | `crates/fixture/src/lib.rs:4`（`works`）、`absent_identifier_name` |",
  );

  expectProblem(
    inspect({ document: buildDocument({ advantage }) }),
    "`absent_identifier_name`, which occurs nowhere under crates/ or database/",
  );
});

test("parseAdvantageClaims is total on the real document and on one without the section", () => {
  const real = readFileSync(path.join(repoRoot, PARITY_DOC), "utf8");
  assert.equal(parseAdvantageClaims(real).rows.length, 9);
  assert.equal(parseAdvantageClaims("# nothing to see\n"), null);
});


// ---- the audit document is a self-description surface, read line by line

test("the audit document states this gate's rule-family count where it names the gate", () => {
  const real = readFileSync(path.join(repoRoot, PARITY_DOC), "utf8");
  const readings = parseLineScopedRuleFamilies(real, "check-sandbox-e2b-parity-matrix.mjs");

  assert.ok(readings.length > 0, "the document must state how many families this gate implements");
  for (const reading of readings) {
    assert.equal(reading.value, RULE_FAMILIES.length, `line ${reading.line} disagrees`);
  }
});

test("a line-scoped surface stating the wrong count is rejected", () => {
  const description = GATE_DESCRIPTION.replace(`${RULE_FAMILIES.length} 条规则族`, "9 条规则族");

  expectProblem(
    inspect({ document: buildDocument({ description }) }),
    `declares 9 rule families, this gate implements ${RULE_FAMILIES.length}`,
  );
});

test("a line-scoped surface that never states a count is rejected", () => {
  expectProblem(
    inspect({
      document: buildDocument({
        description: "`tools/check-sandbox-e2b-parity-matrix.mjs` keeps the audit honest.",
      }),
    }),
    "names this gate but never states how many rule families it implements",
  );
});

test("the line-scoped reader keeps each gate's count on its own line", () => {
  const text = [
    "The matrix gate (`tools/check-sandbox-e2b-parity-matrix.mjs`) holds 11 条规则族.",
    "The field gate (`tools/check-sandbox-e2b-field-parity.mjs`) holds 10 条规则族.",
  ].join("\n");

  // No borrowing: the field gate's ten sits on the very next line and must not be attributed here.
  assert.deepEqual(parseLineScopedRuleFamilies(text, "check-sandbox-e2b-parity-matrix.mjs"), [
    { value: 11, line: 1 },
  ]);
});

test("an ordinal reference is not read as a declaration of the count", () => {
  // "this is why the 9th rule family exists" names a family, not a total. Both spellings have to be
  // excluded: with no space the numeral follows `第` directly, and with a space `\d+` would otherwise
  // start at the second digit of `第 11`.
  const text = [
    "The field gate (`tools/check-sandbox-e2b-field-parity.mjs`) holds 10 条规则族.",
    "`check-sandbox-e2b-parity-matrix.mjs` — this is why the 第 4 条规则族 exists.",
    "`check-sandbox-e2b-parity-matrix.mjs` — this is why the 第 11 条规则族 exists.",
  ].join("\n");

  assert.deepEqual(parseLineScopedRuleFamilies(text, "check-sandbox-e2b-parity-matrix.mjs"), []);
});

test("arguments are parsed strictly", () => {
  assert.deepEqual(parseE2bParityMatrixArgs([]), { root: process.cwd(), json: false });
  assert.equal(parseE2bParityMatrixArgs(["--json"]).json, true);
  assert.equal(parseE2bParityMatrixArgs(["--root", "."]).root, resolve(process.cwd()));
  assert.throws(() => parseE2bParityMatrixArgs(["--root"]), /--root requires a directory/u);
  assert.throws(() => parseE2bParityMatrixArgs(["--nope"]), /unsupported argument/u);
});

// ---- rule family 12: headline numbers (section 1.1)

test("the answer-section reader is total", () => {
  assert.equal(parseAnswerSection("# nothing that looks like the answer section\n"), null);
  const parsed = parseAnswerSection(ANSWER);
  assert.deepEqual(parsed.header, [...ANSWER_COLUMNS]);
  assert.equal(parsed.rows.length, 2);
  assert.equal(parsed.malformed.length, 0);
});

test("the restated-figure patterns are a closed set covering every figure section 1.1 must state", () => {
  const ids = HEADLINE_PATTERNS.map((entry) => entry.id);
  assert.equal(new Set(ids).size, ids.length, "pattern ids must be unique");
  for (const id of ANSWER_REQUIRED_FIGURES) {
    assert.ok(ids.includes(id), `section 1.1's required figure ${id} has no pattern behind it`);
  }
  for (const { id, pattern } of HEADLINE_PATTERNS) {
    assert.ok(pattern.global, `${id} must be global, or the scan only ever reads the first occurrence`);
  }
});

test("the contract-clause readers match only their own phrasings", () => {
  assert.equal(UNNAMED_CONTRACT_CLAUSE.exec("另有 4 份不以 `.contract.json` 命名的机器契约")[1], "4");
  assert.equal(UNNAMED_CONTRACT_CLAUSE.exec("另有一份不以 `.contract.json` 命名的机器契约")[1], "一");
  assert.equal(UNNAMED_CONTRACT_CLAUSE.exec("另有 37 个契约"), null);
  const exception = EXCEPTION_CONTRACT_CLAUSE.exec("第 23 份 `specs/a.contract.json` 是发布决定记录");
  assert.deepEqual([exception[1], exception[2]], ["23", "specs/a.contract.json"]);
  // `条` is a rule family, `份` is a file: an ordinal reference must not read as an exception.
  assert.equal(EXCEPTION_CONTRACT_CLAUSE.exec("第 11 条规则族"), null);
});

test("authorization is recognized on either recognized field and on neither false one", () => {
  assert.equal(authorizesImplementation({ implementationAuthorized: true }), true);
  assert.equal(
    authorizesImplementation({ releaseDecision: { runtimeImplementationAuthorizationGranted: true } }),
    true,
  );
  assert.equal(authorizesImplementation({ implementationAuthorized: false }), false);
  assert.equal(
    authorizesImplementation({
      releaseDecision: { runtimeImplementationAuthorizationGranted: false, status: "no-go" },
    }),
    false,
  );
  assert.equal(authorizesImplementation(null), false);
  assert.equal(authorizesImplementation("true"), false);
});

test("an unreadable json file reads as absent rather than throwing", () => {
  assert.equal(readJsonFile(join(repoRoot, "specs", "definitely-absent-contract.json")), null);
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-e2b-parity-json-"));
  try {
    const broken = join(base, "broken.json");
    writeFileSync(broken, "{ this is not json");
    assert.equal(readJsonFile(broken), null);
  } finally {
    rmSync(base, { recursive: true, force: true });
  }
});

test("the readers discover the fixture's contracts, records and evidence counts", () => {
  const fixture = createFixture();
  try {
    assert.deepEqual(
      listNamedContracts(fixture.repo),
      [NAMED_CONTRACTS.declaring, NAMED_CONTRACTS.excepted].sort(),
    );
    assert.deepEqual(listApiContracts(fixture.repo), [API_CONTRACT]);
    assert.deepEqual(readEvidenceCounts(fixture.repo), {
      contracts: EVIDENCE_COUNTS.sandbox_contracts_with_requirements,
      ids: EVIDENCE_COUNTS.sandbox_total_distinct_evidence_ids,
      partial: EVIDENCE_COUNTS.sandbox_host_precondition_partial,
      gated: EVIDENCE_COUNTS.sandbox_fully_gated,
    });
    assert.deepEqual(readRequirementStatuses(fixture.repo).map((record) => record.status), [
      "draft",
      "ready",
    ]);
    assert.deepEqual(readDecisionStatuses(fixture.repo).map((record) => record.status), [
      "proposed",
    ]);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("the repository holds exactly the machine contracts, records and evidence counts section 1.1 states", () => {
  const named = listNamedContracts(repoRoot);
  assert.equal(named.length, 23);
  const api = listApiContracts(repoRoot);
  assert.equal(api.length, 2);
  assert.deepEqual(readEvidenceCounts(repoRoot), {
    contracts: 8,
    ids: 127,
    partial: 2,
    gated: 125,
  });
  const authorized = [...named, ...api].filter((relative) =>
    authorizesImplementation(readJsonFile(join(repoRoot, relative))),
  );
  // The 2026-09-24 approvals authorized the local host boundary and, with REQ-2026-0007 ready,
  // the shared command contract; every other machine contract remains closed.
  assert.deepEqual(authorized, [
    "specs/sandbox-local-provider-host-boundary.contract.json",
    "apis/commands/sandbox-command-contract.json",
  ]);
  const requirements = readRequirementStatuses(repoRoot);
  assert.equal(requirements.length, 28);
  assert.equal(requirements.filter((record) => record.status === "ready").length, 3);
  assert.equal(requirements.filter((record) => record.status === "accepted").length, 5);
  assert.equal(requirements.filter((record) => record.status === "draft").length, 20);
  const decisions = readDecisionStatuses(repoRoot);
  assert.equal(decisions.length, 28);
  assert.equal(decisions.filter((record) => record.status === "proposed").length, 25);
  assert.equal(decisions.filter((record) => record.status === "accepted").length, 3);
});

test("the repository's own answer section compares every restated figure", () => {
  const assessment = assessE2bParityMatrix({ repoRoot });

  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
  assert.equal(assessment.answerRows, 2);
  // Pinned on purpose: a pattern narrowed until it stops matching lowers this count, and a section
  // that quietly loses a claim is exactly what the family exists to catch.
  assert.equal(assessment.headlineChecks, 14);
});

test("a missing answer section is rejected", () => {
  expectProblem(
    inspect({ document: buildDocument({ answer: "### 1.9 other section" }) }),
    'has no "### 1.1" answer section',
  );
});

test("an answer table with the wrong columns is rejected at the header", () => {
  const answer = ANSWER.replace(`| ${ANSWER_COLUMNS.join(" | ")} |`, "| 路径 | 本仓现状 |");

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "section 1.1 path table header is [路径, 本仓现状]; expected [路径, E2B 的形态, 本仓现状]",
  );
});

test("an answer table that stops listing E2B's two core paths is rejected", () => {
  const answer = answerWith(
    "| fast deploy | `Template.build()` | `Template` has nothing behind it; no `REQ-*`〔§3.4/1〕 |",
    "| fast deploy | `Template.build()` | only `crates/fixture` is cited |\n| third path | `x` | cited `crates/fixture` |",
  );

  expectProblem(inspect({ document: buildDocument({ answer }) }), "lists 3 path(s)");
});

test("an answer row that repeats a path is rejected", () => {
  const answer = answerWith("| fast deploy |", "| fast create |");

  expectProblem(inspect({ document: buildDocument({ answer }) }), 'repeats the path "fast create"');
});

test("an answer row stating this repository's position without evidence is rejected", () => {
  const answer = answerWith(
    "| no entrypoint; only `crates/fixture` is candidate-shaped |",
    "| there is simply no entrypoint |",
  );

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "states this repository's position without citing a single backtick",
  );
});

test("an answer row citing a path that does not exist is rejected", () => {
  const answer = answerWith(
    "`crates/fixture` is candidate-shaped",
    "`crates/absent-crate` is candidate-shaped",
  );

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "cites `crates/absent-crate`, which does not exist",
  );
});

test("a headline line citing a gate that does not exist is rejected", () => {
  const answer = answerWith("- 2 份 `REQ-*` 中", "- See `check-sandbox-absent-gate.mjs`. 2 份 `REQ-*` 中");

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "cites `check-sandbox-absent-gate.mjs`, which does not exist",
  );
});

test("an answer row with the wrong number of cells is rejected", () => {
  const answer = answerWith(
    "| fast create | `Sandbox.create()` | no entrypoint; only `crates/fixture` is candidate-shaped |",
    "| fast create | `Sandbox.create()` | no entrypoint | and a fourth cell |",
  );

  expectProblem(inspect({ document: buildDocument({ answer }) }), "row has 4 cell(s), expected 3");
});

test("a census figure that disagrees with the matrix is rejected", () => {
  const answer = answerWith("- E2B 的能力集合共 **3 项**", "- E2B 的能力集合共 **4 项**");

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "states census as [4, 0, 1, 1, 1], but the repository yields [3, 0, 1, 1, 1]",
  );
});

test("a census figure is compared to the matrix, not to the census table above it", () => {
  // Both copies say four. Comparing copy to copy would only prove they agree; the matrix holds three
  // rows, so the round trip through the census table must not launder the number.
  const census = CENSUS.replace("| **合计** | **3** |", "| **合计** | **4** |");
  const answer = answerWith("- E2B 的能力集合共 **3 项**", "- E2B 的能力集合共 **4 项**");
  const assessment = inspect({ document: buildDocument({ answer, census }) });

  assert.ok(
    assessment.problems.some((problem) =>
      problem.includes("states census as [4, 0, 1, 1, 1], but the repository yields [3, 0, 1, 1, 1]"),
    ),
    formatE2bParityMatrixReport(assessment),
  );
});

test("a requirement figure that disagrees with the records is rejected", () => {
  const answer = answerWith(
    "2 份 `REQ-*` 中 1 份 `ready`（0 `accepted` / 1 `draft`）",
    "3 份 `REQ-*` 中 1 份 `ready`（0 `accepted` / 2 `draft`）",
  );

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "states requirements as [3, 1, 0, 2], but the repository yields [2, 1, 0, 1]",
  );
});

test("a decision count that disagrees with the records is rejected", () => {
  const answer = answerWith(
    "1 份 `ADR` 中 1 份 `proposed`（0 份 `accepted`）",
    "2 份 `ADR` 中 1 份 `proposed`（0 份 `accepted`）",
  );

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "states decisions as [2, 1, 0], but the repository yields [1, 1, 0]",
  );
});

test("a decision promoted out of proposed refutes the breakdown the count cannot carry", () => {
  // The breakdown replaced the "全部 proposed" adjective (2026-09-24): promoting the fixture's one
  // decision to `accepted` must move every number the sentence states, not just hide behind a total.
  expectProblem(
    inspect({ decisionStatus: "accepted" }),
    "states decisions as [1, 1, 0], but the repository yields [1, 0, 1]",
  );
});

test("a contract count that disagrees with the tree is rejected", () => {
  const answer = answerWith("2 份 `*.contract.json` 中 1 份", "3 份 `*.contract.json` 中 1 份");

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "states contracts as [3, 1], but the repository yields [2, 1]",
  );
});

test("a declared-false count that disagrees with the tree is rejected", () => {
  const answer = answerWith(
    "2 份 `*.contract.json` 中 1 份显式声明",
    "2 份 `*.contract.json` 中 2 份显式声明",
  );

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "states contracts as [2, 2], but the repository yields [2, 1]",
  );
});

test("a contract whose fields authorize implementation must be named by the document", () => {
  // The authorization ratchet replaced the blanket "none authorizes" claim (2026-09-24): once a
  // contract's fields authorize, the audit has to state that authorization, not stay silent.
  expectProblem(
    inspect({
      contractValues: { [NAMED_CONTRACTS.declaring]: { implementationAuthorized: true } },
    }),
    "never names `specs/fixture-alpha.contract.json`, whose fields authorize implementation",
  );
});

test("a contract that would rather not be named is a finding, not an omission", () => {
  expectProblem(
    inspect({
      contractValues: { [NAMED_CONTRACTS.declaring]: { implementationAuthorized: true } },
    }),
    "but never names `specs/fixture-alpha.contract.json`, the one that does not",
  );
});

test("an evidence figure that disagrees with the registry is rejected", () => {
  const answer = answerWith(
    "1 份契约声明的 **3 个证据 id** 中，只有 **1 个**有 host-precondition 半产出，**2 个**仍被",
    "1 份契约声明的 **4 个证据 id** 中，只有 **1 个**有 host-precondition 半产出，**3 个**仍被",
  );

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "states evidence-partial as [1, 4, 1, 3], but the repository yields [1, 3, 1, 2]",
  );
});

test("editing the registry without editing the document is rejected", () => {
  expectProblem(
    inspect({
      registryCounts: {
        ...EVIDENCE_COUNTS,
        sandbox_total_distinct_evidence_ids: 4,
        sandbox_fully_gated: 3,
      },
    }),
    "states evidence-partial as [1, 3, 1, 2], but the repository yields [1, 4, 1, 3]",
  );
});

test("a figure that appears only outside section 1.1 is rejected as missing from the answer", () => {
  const answer = ANSWER.split("\n")
    .filter((line) => !line.includes("能力集合共"))
    .join("\n");

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "section 1.1 states no census figure",
  );
});

test("removing the named exception is rejected rather than read as a shorter sentence", () => {
  const answer = ANSWER.replace(
    "，第 2 份 `specs/fixture-readiness.contract.json` 是发布决定记录而非能力契约，它没有该字段、但独立声明 `runtimeImplementationAuthorizationGranted: false` 且 `releaseDecision.status: \"no-go\"`",
    "",
  );
  assert.ok(answer !== ANSWER, "the exception sentence must actually be removed");

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "but never names `specs/fixture-readiness.contract.json`, the one that does not",
  );
});

test("an exception that does declare the field is rejected", () => {
  expectProblem(
    inspect({
      contractValues: {
        [NAMED_CONTRACTS.excepted]: {
          releaseDecision: { status: "no-go", runtimeImplementationAuthorizationGranted: false },
          implementationAuthorized: false,
        },
      },
    }),
    "states `specs/fixture-readiness.contract.json` has no `implementationAuthorized` field, but it declares one",
  );
});

test("an exception whose release decision is not a no-go is rejected", () => {
  expectProblem(
    inspect({
      contractValues: {
        [NAMED_CONTRACTS.excepted]: {
          releaseDecision: { status: "go", runtimeImplementationAuthorizationGranted: false },
        },
      },
    }),
    'states `specs/fixture-readiness.contract.json` declares `releaseDecision.status: "no-go"`, but it does not',
  );
});

test("an exception whose ordinal disagrees with the count is rejected", () => {
  const answer = answerWith(
    "第 2 份 `specs/fixture-readiness.contract.json`",
    "第 3 份 `specs/fixture-readiness.contract.json`",
  );

  expectProblem(inspect({ document: buildDocument({ answer }) }), "calls its exception contract #3");
});

test("a figure quoted inside the gate-description section is a quotation, not a claim", () => {
  // Section 3.3 explains the gates and quotes the sentences they check. Without the exemption this
  // line would be read as a claim that the repository holds ninety-nine decision records.
  const description = [
    "### 3.3 the gates and their cases",
    "",
    GATE_DESCRIPTION,
    "",
    "This family checks the sentence that reads 99 份 `ADR` 全部 `proposed`.",
    "",
  ].join("\n");
  const assessment = inspect({ document: buildDocument({ description }) });

  assert.equal(assessment.ok, true, formatE2bParityMatrixReport(assessment));
});

test("a figure whose source cannot be read is reported rather than passed", () => {
  // No `acknowledged` block: the registry exists but yields nothing, so the figures cannot be
  // refuted. Reporting that is the difference between a check and a decoration.
  expectProblem(
    inspect({
      contractValues: {
        ["specs/sandbox-real-evidence-registry.json"]: { schemaVersion: 1, kind: "fixture" },
      },
    }),
    "states a evidence-partial figure, but the artifact it restates could not be read",
  );
});

test("an unnamed-contract count that disagrees with the tree is rejected", () => {
  const answer = answerWith("另有一份不以", "另有两份不以");

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "states 2 unnamed machine contract(s), but 1 exist under apis/",
  );
});

test("an unnamed-contract list that does not match the tree is rejected", () => {
  const answer = answerWith(
    "`apis/commands/fixture-command-contract.json`）同为",
    "`apis/commands/fixture-other-contract.json`）同为",
  );

  expectProblem(
    inspect({ document: buildDocument({ answer }) }),
    "names unnamed machine contracts [apis/commands/fixture-other-contract.json], but the repository holds [apis/commands/fixture-command-contract.json]",
  );
});
