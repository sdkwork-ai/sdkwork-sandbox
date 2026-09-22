import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  assessRequirementTraceability,
  formatRequirementTraceabilityReport,
  parseRequirementTraceabilityArgs,
  readCapabilityMatrix,
  readLocalAuthorities,
} from "../../tools/check-sandbox-requirement-traceability.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const CAPABILITY_MATRIX = `# SDKWork Sandbox 能力与生命周期需求

## 11. 能力对齐矩阵 (Capability Alignment Matrix)

本产品以成熟 microVM Agent Runtime 的公开能力集合为对齐基线。

| # | 能力 | 承载与状态 |
| --- | --- | --- |
| 1 | Alpha | \`REQ-2026-0001\`（候选实现） |
| 2 | Beta | **无**；见 PRD 第 3 节 |

## 12. 运行模式与隔离等级映射

其余正文。
`;

/**
 * Build a throwaway repository that owns one requirement record and one decision record, so the
 * gate has authorities to resolve against:
 *
 *   <tmp>/sdkwork-fixture/
 *     docs/product/requirements/REQ-2026-0001-alpha.md
 *     docs/architecture/decisions/ADR-20260101-alpha-decision.md
 *     docs/product/prd/PRD-capabilities.md
 *     README.md                     <- keeps both records from being orphans
 */
function createFixture({ documents = {}, requirements = {}, decisions = {}, capabilityMatrix } = {}) {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-req-traceability-"));
  const repo = path.join(base, "sdkwork-fixture");

  const write = (relativePath, contents) => {
    const absolute = path.join(repo, relativePath);
    mkdirSync(path.dirname(absolute), { recursive: true });
    writeFileSync(absolute, contents);
  };

  for (const [id, contents] of Object.entries(requirements)) {
    write(path.join("docs", "product", "requirements", `${id}-record.md`), contents);
  }
  for (const [id, contents] of Object.entries(decisions)) {
    write(path.join("docs", "architecture", "decisions", `${id}.md`), contents);
  }
  write(
    path.join("docs", "product", "prd", "PRD-capabilities.md"),
    capabilityMatrix ?? CAPABILITY_MATRIX,
  );
  for (const [relativePath, contents] of Object.entries(documents)) {
    write(relativePath, contents);
  }

  return { base, repo };
}

const DEFAULT_REQUIREMENTS = { "REQ-2026-0001": "# Alpha\n\nBody.\n" };
const DEFAULT_DECISIONS = { "ADR-20260101-alpha-decision": "# Alpha decision\n\nBody.\n" };
/** The index that keeps both authority records cited, exactly as PRD.md does upstream. */
const DEFAULT_INDEX = "See REQ-2026-0001 and ADR-20260101-alpha-decision.\n";

function inspect(overrides = {}) {
  const fixture = createFixture({
    requirements: DEFAULT_REQUIREMENTS,
    decisions: DEFAULT_DECISIONS,
    documents: { "README.md": DEFAULT_INDEX, ...overrides.documents },
    ...("capabilityMatrix" in overrides ? { capabilityMatrix: overrides.capabilityMatrix } : {}),
    ...("requirements" in overrides ? { requirements: overrides.requirements } : {}),
    ...("decisions" in overrides ? { decisions: overrides.decisions } : {}),
  });
  try {
    return assessRequirementTraceability({ repoRoot: fixture.repo });
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
}

function failuresFor(assessment, reason) {
  return assessment.failures.filter((failure) => failure.reason === reason);
}

test("the repository's own traceability chain resolves and the gate is not vacuous", () => {
  const assessment = assessRequirementTraceability({ repoRoot });

  assert.equal(assessment.ok, true, formatRequirementTraceabilityReport(assessment));
  assert.deepEqual(assessment.failures, []);
  assert.ok(
    assessment.liveDocumentsChecked >= 100,
    `expected a live document corpus, saw ${assessment.liveDocumentsChecked}`,
  );
  assert.ok(
    assessment.requirementReferencesChecked >= 500,
    `the gate must not be vacuous; saw ${assessment.requirementReferencesChecked} requirement references`,
  );
  assert.ok(
    assessment.decisionReferencesChecked >= 100,
    `the gate must not be vacuous; saw ${assessment.decisionReferencesChecked} decision references`,
  );
  assert.equal(assessment.requirementsOnRecord, 27);
  assert.equal(assessment.decisionsOnRecord, 27);
  assert.equal(assessment.capabilityRowsUnattributed, 0);
});

test("the census states what the capability matrix literally says", () => {
  const assessment = assessRequirementTraceability({ repoRoot });

  assert.equal(assessment.capabilityRows, 34);
  assert.equal(assessment.capabilityRowsCitingRequirement, 17);
  assert.equal(assessment.capabilityRowsNoCarrier, 14);
  assert.equal(assessment.capabilityRowsBackReference, 3);
  const report = formatRequirementTraceabilityReport(assessment);
  assert.match(report, /Capability alignment census/u);
  assert.match(report, /17 cite a `REQ-\*`/u);
  assert.match(report, /14 are marked `无`/u);
});

test("the census buckets partition the matrix so the line cannot sum past its own row count", () => {
  const assessment = assessRequirementTraceability({ repoRoot });

  const partition =
    assessment.capabilityRowsCitingRequirement +
    assessment.capabilityRowsNoCarrier +
    assessment.capabilityRowsBackReference +
    assessment.capabilityRowsUnattributed;
  assert.equal(
    partition,
    assessment.capabilityRows,
    "a row classified twice or not at all makes the printed census self-contradictory",
  );
  // `Snapshot` and `Egress Policy` both cite a requirement and qualify it with a scoped `无`
  // caveat. They must be counted once, under "citing", and named in the caveat sub-count.
  assert.equal(assessment.capabilityRowsCitingWithScopeCaveat, 2);
  assert.deepEqual(
    assessment.capabilityRowsCitingWithScopeCaveatDetail.map((row) => row.capability),
    ["Snapshot", "Egress Policy"],
  );
  const report = formatRequirementTraceabilityReport(assessment);
  assert.match(
    report,
    /2 of the citing row\(s\) qualify the carrier with a scoped `无` caveat \(7 `Snapshot`, 16 `Egress Policy`\)/u,
  );
});

test("a row that cites a requirement and carries a scoped caveat is classified once, not twice", () => {
  const assessment = inspect({
    capabilityMatrix: CAPABILITY_MATRIX.replace(
      "| 1 | Alpha | `REQ-2026-0001`（候选实现） |",
      "| 1 | Alpha | `REQ-2026-0001`；`shared` 模式无 `REQ-*` |",
    ),
  });

  assert.equal(assessment.ok, true, formatRequirementTraceabilityReport(assessment));
  assert.equal(
    assessment.capabilityRowsCitingRequirement +
      assessment.capabilityRowsNoCarrier +
      assessment.capabilityRowsBackReference +
      assessment.capabilityRowsUnattributed,
    assessment.capabilityRows,
  );
  assert.equal(assessment.capabilityRowsCitingWithScopeCaveat, 1);
});

test("a fixture with a cited requirement and decision is accepted", () => {
  const assessment = inspect();

  assert.equal(assessment.ok, true, formatRequirementTraceabilityReport(assessment));
  assert.equal(assessment.requirementsOnRecord, 1);
  assert.equal(assessment.decisionsOnRecord, 1);
});

test("a requirement id that resolves to no record is rejected with its exact location", () => {
  const assessment = inspect({
    documents: { "docs/guide.md": "Second line.\nSee REQ-2026-9999 for detail.\n" },
  });

  assert.equal(assessment.ok, false);
  const failures = failuresFor(assessment, "dangling-id");
  assert.equal(failures.length, 1);
  assert.equal(failures[0].document, "docs/guide.md");
  assert.equal(failures[0].line, 2);
  assert.match(failures[0].message, /REQ-2026-9999/u);
  assert.match(formatRequirementTraceabilityReport(assessment), /dangling-id/u);
});

test("a cross-repository requirement id is accepted when a sibling path qualifies it", () => {
  const assessment = inspect({
    documents: {
      "docs/guide.md":
        "See `sdkwork-agents/docs/product/requirements/REQ-2026-0730-hybrid.md`.\n",
    },
  });

  assert.equal(assessment.ok, true, formatRequirementTraceabilityReport(assessment));
  assert.equal(failuresFor(assessment, "dangling-id").length, 0);
  assert.equal(failuresFor(assessment, "unqualified-cross-repository-id").length, 0);
});

test("a sibling path mentioned in an earlier clause does not qualify a later bare id", () => {
  // Regression: an earlier build tested the whole line prefix for a sibling path, so long prose
  // that explains the rule before violating it went silently green — this very repository's
  // tools/README.md did exactly that.
  const assessment = inspect({
    documents: {
      "docs/guide.md":
        "Qualify cross-repository ids with a sibling path such as `sdkwork-agents/...`, and " +
        "write the owning record's path rather than a bare number: `Kernel REQ-2026-0001` " +
        "resolves to the wrong record.\n",
    },
  });

  assert.equal(assessment.ok, false);
  const failures = failuresFor(assessment, "unqualified-cross-repository-id");
  assert.equal(failures.length, 1);
  assert.equal(failures[0].document, "docs/guide.md");
  assert.equal(failures[0].line, 1);
});

test("a cross-repository requirement id written as a path is accepted", () => {
  const assessment = inspect({
    documents: {
      "docs/guide.md":
        "See `sdkwork-agents/docs/product/requirements/REQ-2026-0730-hybrid.md` for the record.\n",
    },
  });

  assert.equal(assessment.ok, true, formatRequirementTraceabilityReport(assessment));
  assert.equal(failuresFor(assessment, "dangling-id").length, 0);
  assert.equal(failuresFor(assessment, "unqualified-cross-repository-id").length, 0);
});

test("another repository's requirement id cannot be written bare", () => {
  // This is the 2026-09-22 regression in PLAN-2026-0002: `Kernel REQ-2026-0002` also resolves to
  // this repository's own REQ-2026-0002, so the sentence silently changed meaning.
  const assessment = inspect({
    documents: { "docs/plan.md": "Kernel REQ-2026-0001 blocks the handoff.\n" },
  });

  assert.equal(assessment.ok, false);
  const failures = failuresFor(assessment, "unqualified-cross-repository-id");
  assert.equal(failures.length, 1);
  assert.equal(failures[0].document, "docs/plan.md");
  assert.equal(failures[0].line, 1);
  assert.match(failures[0].message, /cannot be resolved to this repository/u);
  // The id itself resolves locally, so it must not also be reported as dangling.
  assert.equal(failuresFor(assessment, "dangling-id").length, 0);
});

test("an abbreviated decision id is accepted when the line anchors a real decision file", () => {
  const assessment = inspect({
    documents: {
      "docs/guide.md":
        "Decision: [ADR-20260101: Alpha](architecture/decisions/ADR-20260101-alpha-decision.md).\n",
    },
  });

  assert.equal(assessment.ok, true, formatRequirementTraceabilityReport(assessment));
  assert.equal(failuresFor(assessment, "dangling-id").length, 0);
});

test("an abbreviated decision id with no anchor to a real file is rejected", () => {
  // This is the 2026-09-22 regression in MIG-2026-0003, which cited an id that matched nothing.
  const assessment = inspect({
    documents: { "docs/guide.md": "按 ADR-20260101-alpha-bogus 决策：\n" },
  });

  assert.equal(assessment.ok, false);
  const failures = failuresFor(assessment, "dangling-id");
  assert.equal(failures.length, 1);
  assert.equal(failures[0].document, "docs/guide.md");
  assert.equal(failures[0].line, 1);
  assert.match(failures[0].message, /ADR-20260101-alpha-bogus/u);
});

test("an authority record no live document cites is an orphan", () => {
  const assessment = inspect({
    requirements: {
      "REQ-2026-0001": "# Alpha\n\nBody.\n",
      "REQ-2026-0002": "# Beta\n\nBody.\n",
    },
  });

  assert.equal(assessment.ok, false);
  const failures = failuresFor(assessment, "orphan-authority");
  assert.equal(failures.length, 1);
  assert.equal(
    failures[0].document,
    "docs/product/requirements/REQ-2026-0002-record.md",
  );
  assert.match(failures[0].message, /REQ-2026-0002 is referenced by no live document/u);
});

test("a record citing only itself is still an orphan", () => {
  const assessment = inspect({
    requirements: {
      "REQ-2026-0001": "# Alpha\n\nBody.\n",
      "REQ-2026-0002": "# Beta\n\nThis record is REQ-2026-0002 and cites nothing else.\n",
    },
  });

  assert.equal(assessment.ok, false);
  const failures = failuresFor(assessment, "orphan-authority");
  assert.equal(failures.length, 1);
  assert.match(failures[0].message, /REQ-2026-0002 is referenced by no live document/u);
});

test("a historical evidence record is exempt from the resolution rules", () => {
  const assessment = inspect({
    documents: {
      "docs/engineering/reviews/REVIEW-20260101-historic.md":
        "Recorded at the time: REQ-2026-9999 and ADR-20260101-alpha-gone.\n",
    },
  });

  assert.equal(assessment.ok, true, formatRequirementTraceabilityReport(assessment));
});

test("a capability row with no classification is rejected", () => {
  const assessment = inspect({
    capabilityMatrix: CAPABILITY_MATRIX.replace(
      "| 2 | Beta | **无**；见 PRD 第 3 节 |",
      "| 2 | Beta | 见 PRD 第 3 节 |",
    ),
  });

  assert.equal(assessment.ok, false);
  const failures = failuresFor(assessment, "unclassified-capability-row");
  assert.equal(failures.length, 1);
  assert.equal(failures[0].document, "docs/product/prd/PRD-capabilities.md");
  assert.match(failures[0].message, /Beta/u);
});

test("a capability row that inherits the row above with 同上 is accepted", () => {
  const assessment = inspect({
    capabilityMatrix: CAPABILITY_MATRIX.replace(
      "| 2 | Beta | **无**；见 PRD 第 3 节 |",
      "| 2 | Beta | 同上 |",
    ),
  });

  assert.equal(assessment.ok, true, formatRequirementTraceabilityReport(assessment));
  assert.equal(assessment.capabilityRowsBackReference, 1);
  assert.equal(assessment.capabilityRowsUnattributed, 0);
});

test("capability rows must be numbered contiguously from 1", () => {
  const assessment = inspect({
    capabilityMatrix: CAPABILITY_MATRIX.replace("| 2 | Beta", "| 3 | Beta"),
  });

  assert.equal(assessment.ok, false);
  const failures = failuresFor(assessment, "capability-row-numbering");
  assert.equal(failures.length, 1);
  assert.match(failures[0].message, /contiguously from 1/u);
});

test("a requirement record whose file name carries no id is rejected", () => {
  const assessment = inspect({
    documents: {
      "docs/product/requirements/REQ-2026-broken.md": "# Broken record\n\nBody.\n",
    },
  });

  assert.equal(assessment.ok, false);
  const failures = failuresFor(assessment, "malformed-authority-filename");
  assert.equal(failures.length, 1);
  assert.equal(
    failures[0].document,
    "docs/product/requirements/REQ-2026-broken.md",
  );
  assert.match(failures[0].message, /must carry its REQ-####-#### id/u);
});

test("a file under the requirements directory with no REQ- prefix is not a record", () => {
  const assessment = inspect({
    documents: { "docs/product/requirements/notes.md": "Free-form notes.\n" },
  });

  assert.equal(assessment.ok, true, formatRequirementTraceabilityReport(assessment));
  assert.equal(assessment.requirementsOnRecord, 1);
});

test("the capability matrix reader reports an explicit failure when the section is absent", () => {
  const fixture = createFixture({
    requirements: DEFAULT_REQUIREMENTS,
    decisions: DEFAULT_DECISIONS,
    documents: { "README.md": DEFAULT_INDEX },
    capabilityMatrix: "# PRD\n\nNo matrix here.\n",
  });
  try {
    const matrix = readCapabilityMatrix({ repoRoot: fixture.repo });
    assert.equal(matrix.rows.length, 0);
    assert.equal(matrix.failures.length, 1);
    assert.equal(matrix.failures[0].reason, "missing-capability-matrix");
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("authority discovery reads both record families", () => {
  const fixture = createFixture({
    requirements: DEFAULT_REQUIREMENTS,
    decisions: DEFAULT_DECISIONS,
    documents: { "README.md": DEFAULT_INDEX },
  });
  try {
    const authority = readLocalAuthorities({ repoRoot: fixture.repo });
    assert.deepEqual([...authority.requirements.keys()], ["REQ-2026-0001"]);
    assert.deepEqual([...authority.decisions.keys()], ["ADR-20260101-alpha-decision"]);
    assert.deepEqual(authority.failures, []);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("argument parsing accepts --json and --root and rejects anything else", () => {
  const probeRoot = path.join(tmpdir(), "sdkwork-traceability-root");

  assert.deepEqual(parseRequirementTraceabilityArgs([]), { json: false, root: repoRoot });
  assert.deepEqual(parseRequirementTraceabilityArgs(["--json"]), { json: true, root: repoRoot });
  assert.equal(parseRequirementTraceabilityArgs(["--root", probeRoot]).root, path.resolve(probeRoot));
  assert.throws(() => parseRequirementTraceabilityArgs(["--root"]), /--root requires/u);
  assert.throws(() => parseRequirementTraceabilityArgs(["--nope"]), /unsupported argument/u);
});

test("the success report states the counts it actually checked", () => {
  const assessment = assessRequirementTraceability({ repoRoot });
  const report = formatRequirementTraceabilityReport(assessment);

  assert.match(report, new RegExp(`${assessment.requirementReferencesChecked} requirement`, "u"));
  assert.match(report, new RegExp(`${assessment.decisionReferencesChecked} decision`, "u"));
  assert.match(report, new RegExp(`${assessment.liveDocumentsChecked} live document`, "u"));
});
