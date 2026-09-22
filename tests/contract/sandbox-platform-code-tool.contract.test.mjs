import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  ARCHITECTURE_ENTRY,
  DOCS_INDEX,
  PLATFORM_DOCUMENT,
  PLATFORM_MARKER_IDS,
  PLATFORM_IDS,
  TECH_README,
  assessPlatformRegime,
  assessPlatformRegimeAtRoot,
  classifyCitation,
  collectPlatformMarkers,
  formatPlatformRegimeReport,
  parsePlatformRegimeArgs,
  splitTableRow,
} from "../../tools/check-sandbox-platform-code.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

/// A minimal but fully conforming platform document. Every rule family below mutates exactly one
/// part of this document, so a failure pinpoints the rule that broke.
function platformDocument({
  vocabulary = PLATFORM_IDS,
  matrix = PLATFORM_IDS.map((platform) => `| \`${platform}\` | \`unmeasured\` | 见 \`README.md\` |`),
  census = 0,
  declarationRows = ["| （无） | | | |"],
  gates = [
    "| 路径 | `tools/gate-a.mjs` | `portability:paths:check` |",
    "| shell | `tools/gate-b.mjs` | `portability:shell:check` |",
    "| 平台代码 | `tools/check-sandbox-platform-code.mjs` | `portability:platform:check` |",
  ],
} = {}) {
  return [
    "# 平台支持",
    "",
    "### 1.1 平台词汇",
    "",
    "| 平台 id | 含义 |",
    "| --- | --- |",
    ...vocabulary.map((platform) => `| \`${platform}\` | 平台 |`),
    "",
    "### 1.2 平台支持矩阵",
    "",
    "| 平台 | 状态 | 证据与判据 |",
    "| --- | --- | --- |",
    ...matrix,
    "",
    "### 3.1 平台条件代码声明",
    "",
    `当前声明：${census}`,
    "",
    "| 文件 | 标记 | 平台 | 理由 |",
    "| --- | --- | --- | --- |",
    ...declarationRows,
    "",
    "### 4.1 可移植性门禁",
    "",
    "| 门禁 | 脚本 | package.json 脚本名 |",
    "| --- | --- | --- |",
    ...gates,
    "",
  ].join("\n");
}

const GATE_SCRIPTS = {
  "portability:paths:check": "node tools/gate-a.mjs --root .",
  "portability:shell:check": "node tools/gate-b.mjs --root .",
  "portability:platform:check": "node tools/check-sandbox-platform-code.mjs",
};

/// Stub gate scripts the fixture's `### 4.1` table points at, so the fixture does not depend on the
/// real repository's layout.
const GATE_STUBS = [
  "tools/gate-a.mjs",
  "tools/gate-b.mjs",
  "tools/check-sandbox-platform-code.mjs",
];

/// Writes a throwaway repository shaped just enough for `assessPlatformRegimeAtRoot`.
function writeFixture({
  document = platformDocument(),
  scripts = GATE_SCRIPTS,
  rustSources = {},
  indexHasDocument = true,
  architectureHasDocument = true,
  readmeHasDocument = true,
} = {}) {
  const root = mkdtempSync(path.join(tmpdir(), "sdkwork-platform-"));
  const write = (relative, content) => {
    const full = path.join(root, relative);
    mkdirSync(path.dirname(full), { recursive: true });
    writeFileSync(full, content, "utf8");
  };
  write(PLATFORM_DOCUMENT, document);
  write("README.md", "# fixture repository root\n");
  write("package.json", JSON.stringify({ name: "fixture", scripts }, null, 2));
  write(ARCHITECTURE_ENTRY, architectureHasDocument ? "see TECH-platform-support.md\n" : "nothing here\n");
  write(TECH_README, readmeHasDocument ? "see TECH-platform-support.md\n" : "nothing here\n");
  write(DOCS_INDEX, indexHasDocument ? "path: docs/architecture/tech/TECH-platform-support.md\n" : "entries: []\n");
  for (const stub of GATE_STUBS) {
    write(stub, "// stub\n");
  }
  for (const [relative, content] of Object.entries(rustSources)) {
    write(relative, content);
  }
  return root;
}

function withFixture(options, assertion) {
  const root = writeFixture(options);
  try {
    assertion(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function problemsOf(root) {
  return assessPlatformRegimeAtRoot(root).problems;
}

function assertProblemMatching(problems, pattern) {
  assert.ok(
    problems.some((problem) => pattern.test(problem)),
    `expected a problem matching ${pattern}, got:\n${problems.join("\n")}`,
  );
}

// --- The real repository is the primary fixture -------------------------------------------------

test("the repository's own platform regime passes", () => {
  const result = assessPlatformRegimeAtRoot(repoRoot);
  assert.deepEqual(result.problems, []);
  assert.equal(result.markerCount, 0);
});

test("the rendered report prints every platform and its status", () => {
  const rendered = formatPlatformRegimeReport(assessPlatformRegimeAtRoot(repoRoot));
  assert.match(rendered, /sandbox platform regime passed/u);
  for (const platform of PLATFORM_IDS) {
    assert.match(rendered, new RegExp(platform, "u"));
  }
});

// --- Rule family: document structure -----------------------------------------------------------

test("a missing section is reported by name", () => {
  const document = platformDocument().replace("### 4.1 可移植性门禁", "### 4.2 换个标题");
  withFixture({ document }, (root) => {
    assertProblemMatching(problemsOf(root), /missing the "### 4\.1 可移植性门禁" section/u);
  });
});

test("a platform absent from the vocabulary is reported", () => {
  withFixture({ document: platformDocument({ vocabulary: PLATFORM_IDS.slice(1) }) }, (root) => {
    assertProblemMatching(problemsOf(root), /does not declare platform windows-x64/u);
  });
});

test("a platform outside the fixed vocabulary is rejected", () => {
  withFixture({ document: platformDocument({ vocabulary: [...PLATFORM_IDS, "freebsd-x64"] }) }, (root) => {
    assertProblemMatching(problemsOf(root), /declares platform freebsd-x64, which is not in the tool/u);
  });
});

test("a platform missing from the matrix is reported", () => {
  const matrix = PLATFORM_IDS.slice(1).map((platform) => `| \`${platform}\` | \`unmeasured\` | 见 \`README.md\` |`);
  withFixture({ document: platformDocument({ matrix }) }, (root) => {
    assertProblemMatching(problemsOf(root), /has no row for platform windows-x64/u);
  });
});

test("an unknown platform row is reported", () => {
  const matrix = [...PLATFORM_IDS, "freebsd-x64"].map(
    (platform) => `| \`${platform}\` | \`unmeasured\` | 见 \`README.md\` |`,
  );
  withFixture({ document: platformDocument({ matrix }) }, (root) => {
    assertProblemMatching(problemsOf(root), /has an unknown platform row freebsd-x64/u);
  });
});

test("a status outside the vocabulary is reported", () => {
  const matrix = PLATFORM_IDS.map(
    (platform) => `| \`${platform}\` | \`${platform === "windows-x64" ? "probably-fine" : "unmeasured"}\` | 见 \`README.md\` |`,
  );
  withFixture({ document: platformDocument({ matrix }) }, (root) => {
    assertProblemMatching(problemsOf(root), /uses status "probably-fine"/u);
  });
});

// --- Rule family: citations ---------------------------------------------------------------------

test("a row citing nothing checkable is reported as unanchored", () => {
  const matrix = PLATFORM_IDS.map(
    (platform) =>
      `| \`${platform}\` | \`unmeasured\` | ${platform === "windows-x64" ? "没有可解析的引证" : "见 `README.md`"} |`,
  );
  withFixture({ document: platformDocument({ matrix }) }, (root) => {
    assertProblemMatching(problemsOf(root), /cites no repository path and no host-capability id/u);
  });
});

test("a repository path that does not resolve is reported", () => {
  const matrix = PLATFORM_IDS.map(
    (platform) =>
      `| \`${platform}\` | \`unmeasured\` | 见 \`${platform === "windows-x64" ? "docs/nope.md" : "README.md"}\` |`,
  );
  withFixture({ document: platformDocument({ matrix }) }, (root) => {
    assertProblemMatching(problemsOf(root), /cites docs\/nope\.md, which does not resolve/u);
  });
});

test("a capability id outside the probe vocabulary is reported", () => {
  const matrix = PLATFORM_IDS.map(
    (platform) =>
      `| \`${platform}\` | \`unmeasured\` | \`${platform === "windows-x64" ? "cgroup.made-up" : "cgroup.v2-mounted"}\` |`,
  );
  withFixture({ document: platformDocument({ matrix }) }, (root) => {
    assertProblemMatching(problemsOf(root), /cites capability cgroup\.made-up, which is not in the probe vocabulary/u);
  });
});

test("a real capability id anchored by a real path passes", () => {
  const matrix = PLATFORM_IDS.map(
    (platform) => `| \`${platform}\` | \`unmeasured\` | \`cgroup.v2-mounted\` 见 \`README.md\` |`,
  );
  withFixture({ document: platformDocument({ matrix }) }, (root) => {
    assert.deepEqual(problemsOf(root), []);
  });
});

test("a verified claim without a requirement, decision or document path is reported", () => {
  const matrix = PLATFORM_IDS.map(
    (platform) =>
      `| \`${platform}\` | \`${platform === "windows-x64" ? "verified" : "unmeasured"}\` | \`cgroup.v2-mounted\` |`,
  );
  withFixture({ document: platformDocument({ matrix }) }, (root) => {
    assertProblemMatching(problemsOf(root), /claims "verified" without citing a requirement, decision or document path/u);
  });
});

// --- Regression: the two parser bugs found on the real document ---------------------------------

test("a pipe inside an inline-code span does not split the table row", () => {
  const cells = splitTableRow("| `linux-x64-wsl2` | `partial` | denied: `namespace.mount|pid|uts` tail `docs/a.md` |");
  assert.equal(cells.length, 3);
  assert.match(cells[2], /namespace\.mount\|pid\|uts/u);
  assert.match(cells[2], /docs\/a\.md/u);
});

test("citations after a pipe span are still checked", () => {
  // Before the fix the evidence cell ended at the first `|` inside code, so `docs/ghost.md` was
  // never inspected and the gate passed while the document pointed at a file that does not exist.
  const matrix = PLATFORM_IDS.map(
    (platform) =>
      `| \`${platform}\` | \`unmeasured\` | \`namespace.mount|pid\` then \`${platform === "windows-x64" ? "docs/ghost.md" : "README.md"}\` |`,
  );
  withFixture({ document: platformDocument({ matrix }) }, (root) => {
    assertProblemMatching(problemsOf(root), /cites docs\/ghost\.md, which does not resolve/u);
  });
});

test("a placeholder declaration row is not read as a declaration", () => {
  withFixture({ document: platformDocument({ census: 0, declarationRows: ["| （无） | | | |"] }) }, (root) => {
    assert.deepEqual(problemsOf(root), []);
  });
});

test("classifyCitation separates paths, capability ids, host observations and free-form text", () => {
  assert.equal(classifyCitation("docs/INDEX.yaml"), "path");
  assert.equal(classifyCitation("tools/check-sandbox-platform-code.mjs"), "path");
  assert.equal(classifyCitation("cgroup.v2-mounted"), "capability");
  assert.equal(classifyCitation("/etc/os-release"), "host");
  assert.equal(classifyCitation("https://example.com"), "external");
  assert.equal(classifyCitation("#5-复核方式"), "external");
  assert.equal(classifyCitation("unshare failed: Operation not permitted"), "freeform");
});

// --- Rule family: platform-conditional code -----------------------------------------------------

test("an undeclared platform marker in the source is reported", () => {
  withFixture({ rustSources: { "crates/alpha/src/lib.rs": "#[cfg(windows)]\nfn only_windows() {}\n" } }, (root) => {
    const problems = problemsOf(root);
    assertProblemMatching(problems, /PLATFORM-CODE-UNDECLARED crates\/alpha\/src\/lib\.rs:1 uses marker "cfg-windows"/u);
  });
});

test("a declared marker that matches the source is accepted", () => {
  const declarationRows = ["| `crates/alpha/src/lib.rs` | `cfg-windows` | `windows-x64` | 只在该平台的 Provider 内落地 |"];
  withFixture(
    {
      document: platformDocument({ census: 1, declarationRows }),
      rustSources: { "crates/alpha/src/lib.rs": "#[cfg(windows)]\nfn only_windows() {}\n" },
    },
    (root) => {
      assert.deepEqual(problemsOf(root), []);
    },
  );
});

test("a declaration for code that no longer exists is reported", () => {
  const declarationRows = ["| `crates/alpha/src/lib.rs` | `cfg-windows` | `windows-x64` | 只在该平台的 Provider 内落地 |"];
  withFixture({ document: platformDocument({ census: 1, declarationRows }) }, (root) => {
    assertProblemMatching(problemsOf(root), /declares crates\/alpha\/src\/lib\.rs \/ cfg-windows, which no longer appears/u);
  });
});

test("a declaration naming an unknown marker is reported", () => {
  const declarationRows = ["| `crates/alpha/src/lib.rs` | `cfg-solaris` | `windows-x64` | 理由足够长可以接受 |"];
  withFixture(
    {
      document: platformDocument({ census: 1, declarationRows }),
      rustSources: { "crates/alpha/src/lib.rs": "#[cfg(windows)]\nfn only_windows() {}\n" },
    },
    (root) => {
      assertProblemMatching(problemsOf(root), /declares marker "cfg-solaris", which is not one of the known markers/u);
    },
  );
});

test("a declaration without a platform from the vocabulary is reported", () => {
  const declarationRows = ["| `crates/alpha/src/lib.rs` | `cfg-windows` | `beos` | 理由足够长可以接受 |"];
  withFixture(
    {
      document: platformDocument({ census: 1, declarationRows }),
      rustSources: { "crates/alpha/src/lib.rs": "#[cfg(windows)]\nfn only_windows() {}\n" },
    },
    (root) => {
      assertProblemMatching(problemsOf(root), /for platform "beos", which is outside the vocabulary/u);
    },
  );
});

test("a declaration without a usable reason is reported", () => {
  const declarationRows = ["| `crates/alpha/src/lib.rs` | `cfg-windows` | `windows-x64` | 短 |"];
  withFixture(
    {
      document: platformDocument({ census: 1, declarationRows }),
      rustSources: { "crates/alpha/src/lib.rs": "#[cfg(windows)]\nfn only_windows() {}\n" },
    },
    (root) => {
      assertProblemMatching(problemsOf(root), /declares crates\/alpha\/src\/lib\.rs without a usable reason/u);
    },
  );
});

test("an understated census is reported against the real marker count", () => {
  const declarationRows = ["| `crates/alpha/src/lib.rs` | `cfg-windows` | `windows-x64` | 只在该平台的 Provider 内落地 |"];
  withFixture(
    {
      document: platformDocument({ census: 0, declarationRows }),
      rustSources: { "crates/alpha/src/lib.rs": "#[cfg(windows)]\nfn only_windows() {}\n" },
    },
    (root) => {
      assertProblemMatching(problemsOf(root), /states 当前声明：0 but the source carries 1 platform marker/u);
    },
  );
});

test("a missing census line is reported", () => {
  const document = platformDocument().replace("当前声明：0", "声明数量见上");
  withFixture({ document }, (root) => {
    assertProblemMatching(problemsOf(root), /must state 当前声明：N/u);
  });
});

test("markers are collected from crates only, and test code counts", () => {
  const markers = collectPlatformMarkers(repoRoot);
  assert.deepEqual(markers, []);
  withFixture(
    {
      rustSources: {
        "crates/alpha/src/tests.rs": "    #[cfg(unix)]\n    fn t() {}\n",
        "crates/alpha/tests/integration.rs": "#[cfg(unix)]\nfn t() {}\n",
      },
    },
    (root) => {
      const found = collectPlatformMarkers(root);
      assert.equal(found.length, 1, "src/ counts, the crate's tests/ directory does not");
      assert.equal(found[0].path, "crates/alpha/src/tests.rs");
      assert.equal(found[0].line, 1);
    },
  );
});

test("every marker id is discoverable through the exported vocabulary", () => {
  assert.ok(PLATFORM_MARKER_IDS.includes("cfg-windows"));
  assert.ok(PLATFORM_MARKER_IDS.includes("path-separator"));
  assert.equal(new Set(PLATFORM_MARKER_IDS).size, PLATFORM_MARKER_IDS.length);
});

// --- Rule family: the gates that back the claim -------------------------------------------------

test("a gate script that does not exist is reported", () => {
  const gates = ["| 路径 | `../sdkwork-specs/tools/check-not-there.mjs` | `portability:paths:check` |"];
  withFixture({ document: platformDocument({ gates }) }, (root) => {
    assertProblemMatching(problemsOf(root), /names gate script .*check-not-there\.mjs, which does not exist/u);
  });
});

test("a gate script that is not wired into package.json is reported", () => {
  const scripts = { ...GATE_SCRIPTS };
  delete scripts["portability:shell:check"];
  withFixture({ scripts }, (root) => {
    assertProblemMatching(problemsOf(root), /names package\.json script "portability:shell:check", which is not declared/u);
  });
});

test("a package.json script that invokes the wrong tool is reported", () => {
  const scripts = { ...GATE_SCRIPTS, "portability:platform:check": "node tools/something-else.mjs" };
  withFixture({ scripts }, (root) => {
    assertProblemMatching(problemsOf(root), /does not invoke tools\/check-sandbox-platform-code\.mjs/u);
  });
});

test("a document naming no gate at all is reported", () => {
  withFixture({ document: platformDocument({ gates: [] }) }, (root) => {
    assertProblemMatching(problemsOf(root), /must name at least one portability gate/u);
  });
});

// --- Rule family: registration ------------------------------------------------------------------

test("each unregistered location is reported separately", () => {
  withFixture({ architectureHasDocument: false, readmeHasDocument: false, indexHasDocument: false }, (root) => {
    const problems = problemsOf(root);
    assertProblemMatching(problems, new RegExp(`${ARCHITECTURE_ENTRY} does not link`, "u"));
    assertProblemMatching(problems, new RegExp(`${TECH_README} does not link`, "u"));
    assertProblemMatching(problems, new RegExp(`${DOCS_INDEX} does not register`, "u"));
  });
});

test("a missing document is reported instead of throwing", () => {
  const root = mkdtempSync(path.join(tmpdir(), "sdkwork-platform-empty-"));
  try {
    const problems = problemsOf(root);
    assert.equal(problems.length, 1);
    assert.match(problems[0], /does not exist/u);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

// --- Argument handling --------------------------------------------------------------------------

test("arguments parse --root and --json and reject anything else", () => {
  assert.deepEqual(parsePlatformRegimeArgs([]), { root: null, json: false });
  assert.deepEqual(parsePlatformRegimeArgs(["--json"]), { root: null, json: true });
  assert.deepEqual(parsePlatformRegimeArgs(["--root", "/tmp/x"]), { root: "/tmp/x", json: false });
  assert.throws(() => parsePlatformRegimeArgs(["--root"]), /--root requires a directory/u);
  assert.throws(() => parsePlatformRegimeArgs(["--verbose"]), /unsupported argument --verbose/u);
});

test("injected content is assessed from the provided values", () => {
  // The document, the package manifest and the three registration sites are all passed in, which is
  // what lets the rule families be tested without a second checkout. Only the cited paths are read.
  withFixture({}, (root) => {
    const result = assessPlatformRegime({
      root,
      documentContent: platformDocument(),
      markers: [],
      packageJson: { scripts: GATE_SCRIPTS },
      indexContent: "TECH-platform-support.md",
      architectureEntry: "TECH-platform-support.md",
      techReadme: "TECH-platform-support.md",
      hostCapabilityIds: ["cgroup.v2-mounted"],
    });
    assert.deepEqual(result.problems, []);
    assert.equal(result.platformCount, PLATFORM_IDS.length);
  });
});
