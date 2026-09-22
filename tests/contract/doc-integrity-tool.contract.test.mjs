import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  assessDocumentationIntegrity,
  blankInlineCode,
  findDeadLinks,
  findUnrunnableCommands,
  formatDocumentationIntegrityReport,
  parseDocumentationIntegrityArgs,
  splitDocument,
} from "../../tools/check-sandbox-doc-integrity.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const gatePath = path.join(repoRoot, "tools/check-sandbox-doc-integrity.mjs");

/**
 * Build a throwaway multi-repository layout:
 *
 *   <base>/sdkwork-<name>/          <- repoRoot
 *   <base>/sdkwork-specs/tools/     <- only visible through the multi-repository root
 */
function createFixture({ name = "fixture", documents = {}, extraFiles = {}, packageScripts = {} } = {}) {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-doc-integrity-"));
  const tree = path.join(base, `sdkwork-${name}`);
  mkdirSync(tree, { recursive: true });
  writeFileSync(
    path.join(tree, "package.json"),
    `${JSON.stringify({ name: `sdkwork-${name}`, scripts: packageScripts }, null, 2)}\n`,
  );
  for (const [relativePath, body] of Object.entries(documents)) {
    const full = path.join(tree, relativePath);
    mkdirSync(path.dirname(full), { recursive: true });
    writeFileSync(full, body);
  }
  for (const [relativePath, body] of Object.entries(extraFiles)) {
    const full = path.join(base, relativePath);
    mkdirSync(path.dirname(full), { recursive: true });
    writeFileSync(full, body);
  }
  return { base, tree };
}

function withFixture(options, run) {
  const fixture = createFixture(options);
  try {
    return run(fixture);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
}

test("splitDocument separates prose from fenced blocks and preserves line numbers", () => {
  const markdown = ["# Title", "", "```bash", "echo one", "echo two", "```", "", "tail"].join("\n");
  const { prose, blocks } = splitDocument(markdown);

  assert.equal(blocks.length, 1);
  assert.equal(blocks[0].language, "bash");
  assert.equal(blocks[0].body, "echo one\necho two");
  assert.equal(blocks[0].startLine, 4, "the first body line is line 4 of the document");
  assert.equal(prose.split("\n").length, markdown.split("\n").length, "line structure is preserved");
  assert.doesNotMatch(prose, /echo one/u, "code is removed from prose");
  assert.match(prose, /tail/u);
});

test("splitDocument reports an unterminated fence instead of silently dropping it", () => {
  const { blocks } = splitDocument("# Title\n\n```bash\necho one\n");
  assert.equal(blocks.length, 1);
  assert.equal(blocks[0].unterminated, true);
});

test("blankInlineCode hides inline code spans but keeps the character offsets", () => {
  const prose = "see [`SPEC.md`](../x.md) and `[fake](nope.md)` here";
  const blanked = blankInlineCode(prose);

  assert.equal(blanked.length, prose.length);
  // The link target survives: only the inline-code span inside the link text is blanked, and the
  // link itself must still be resolvable.
  assert.match(blanked, /\(\.\.\/x\.md\)/u);
  // A link-shaped example that lives entirely inside backticks is not a link.
  assert.doesNotMatch(blanked, /nope\.md/u);
});

test("findDeadLinks accepts resolvable targets and skips external, anchor and scheme links", () => {
  const documentPath = path.join(repoRoot, "docs", "fixture.md");
  const failures = findDeadLinks(
    documentPath,
    [
      "[spec](../sdkwork-specs/API_SPEC.md)",
      "[anchor](#section)",
      "[external](https://example.com/x.md)",
      "[mail](mailto:a@b.c)",
      "[query](PRD.md?plain=1#x)",
    ].join("\n\n"),
  );

  assert.deepEqual(
    failures.map((failure) => failure.target),
    [
      "../sdkwork-specs/API_SPEC.md",
      "PRD.md?plain=1#x",
    ],
    "only the two relative paths are probed, and both resolve from docs/",
  );
});

test("findDeadLinks reports a link one directory level short, with its line number", () => {
  // This is the exact 2026-09-22 defect: docs/guides/<role>/README.md reached for
  // ../../../sdkwork-specs, which resolves to <repo>/sdkwork-specs.
  const documentPath = path.join(repoRoot, "docs", "guides", "fixture", "README.md");
  const failures = findDeadLinks(
    documentPath,
    ["# Guide", "", "See [`API_SPEC`](../../../sdkwork-specs/API_SPEC.md)."].join("\n"),
  );

  assert.equal(failures.length, 1);
  assert.equal(failures[0].reason, "dead-relative-link");
  assert.equal(failures[0].line, 3);
  assert.match(failures[0].message, /does not exist/u);
});

test("findDeadLinks ignores link-shaped text inside a fenced code block", () => {
  const documentPath = path.join(repoRoot, "docs", "fixture.md");
  const failures = findDeadLinks(
    documentPath,
    ["# Title", "", "```text", "[example](does-not-exist.md)", "```"].join("\n"),
  );

  assert.deepEqual(failures, [], "markdown does not render links inside a fenced block");
});

test("the forbidden formatting command is caught in every spelling, with its line number", () => {
  const fixtureBlock = ["# Title", "", "```bash", "cargo fmt --all", "cargo fmt --all -- --check", "```"].join("\n");
  const failures = findUnrunnableCommands({
    documentPath: path.join(repoRoot, "docs", "fixture.md"),
    markdown: fixtureBlock,
    repoRoot,
    multiRepoRoot: path.dirname(repoRoot),
  });

  assert.equal(failures.length, 2);
  assert.deepEqual(
    failures.map((failure) => failure.reason),
    ["forbidden-fmt-scope", "forbidden-fmt-scope"],
  );
  assert.deepEqual(
    failures.map((failure) => failure.line),
    [4, 5],
  );
  assert.match(failures[0].message, /cargo fmt --check/u);
});

test("prose that forbids the formatting command is not mistaken for a prescription", () => {
  // The repository documents the prohibition in prose. A substring scan over the whole
  // document reports it as a violation; only a fenced-block scan is correct.
  const proseOnly = [
    "# Title",
    "",
    "Use `cargo fmt --check`, never `cargo fmt --all -- --check`, because `--all` also formats",
    "local path dependencies owned by sibling repositories.",
  ].join("\n");
  const failures = findUnrunnableCommands({
    documentPath: path.join(repoRoot, "docs", "fixture.md"),
    markdown: proseOnly,
    repoRoot,
    multiRepoRoot: path.dirname(repoRoot),
  });

  assert.deepEqual(failures, []);
});

test("a node target that resolves from the crate directory or the multi-repository root is accepted", () => {
  withFixture(
    {
      extraFiles: { "sdkwork-specs/tools/real.mjs": "// real tool\n" },
      documents: {
        // `sdkwork-specs` is a sibling of the repository, not a child of it, so a crate README sits
        // three levels below the checkout root and must write `../../../sdkwork-specs/...`.
        "crates/leaf/README.md": [
          "# Leaf",
          "",
          "```bash",
          "node ../../../sdkwork-specs/tools/real.mjs --root ../..",
          "```",
        ].join("\n"),
        "README.md": [
          "# Root",
          "",
          "```bash",
          "node ../sdkwork-specs/tools/real.mjs --root .",
          "```",
        ].join("\n"),
        "deployments/webserver/README.md": [
          "# Deployments",
          "",
          "```bash",
          "node sdkwork-specs/tools/real.mjs <sdkwork-space-root>",
          "```",
        ].join("\n"),
      },
    },
    ({ tree }) => {
      const assessment = assessDocumentationIntegrity({ repoRoot: tree });
      assert.deepEqual(assessment.failures, [], "all three anchor conventions must be honoured");
      assert.equal(assessment.ok, true);
    },
  );
});

test("a node target that resolves only from the document's parent directory is rejected", () => {
  // `../../sdkwork-specs/tools/real.mjs` written in `crates/leaf/README.md` resolves from
  // `crates/` — a directory the document does not occupy — but not from the document's own
  // directory, the repository root or the checkout root. Accepting it admitted a verification
  // command whose tool path and `--root` implied different working directories, so the tool
  // audited the checkout root and reported success while checking nothing.
  withFixture(
    {
      extraFiles: { "sdkwork-specs/tools/real.mjs": "// real tool\n" },
      documents: {
        "crates/leaf/README.md": [
          "# Leaf",
          "",
          "```bash",
          "node ../../sdkwork-specs/tools/real.mjs --root ../..",
          "```",
        ].join("\n"),
      },
    },
    ({ tree }) => {
      const assessment = assessDocumentationIntegrity({ repoRoot: tree });
      assert.equal(assessment.ok, false);
      assert.deepEqual(
        assessment.failures.map((failure) => failure.reason),
        ["unrunnable-command-target"],
      );
      assert.equal(assessment.failures[0].line, 4);
    },
  );
});

test("a node target that resolves from nowhere is reported", () => {
  withFixture(
    {
      documents: {
        "docs/guides/developer/README.md": [
          "# Developer Guide",
          "",
          "```bash",
          "node scripts/documentation-checker.mjs",
          "```",
        ].join("\n"),
      },
    },
    ({ tree }) => {
      const assessment = assessDocumentationIntegrity({ repoRoot: tree });
      assert.equal(assessment.ok, false);
      assert.equal(assessment.failures.length, 1);
      assert.equal(assessment.failures[0].reason, "unrunnable-command-target");
      assert.equal(assessment.failures[0].document, "docs/guides/developer/README.md");
      assert.equal(assessment.failures[0].line, 4);
      assert.match(formatDocumentationIntegrityReport(assessment), /unrunnable-command-target/u);
    },
  );
});

test("a glob target only needs its directory to exist", () => {
  withFixture(
    {
      documents: {
        "docs/README.md": ["# Docs", "", "```bash", "node --test tests/contract/*.test.mjs", "```"].join("\n"),
      },
    },
    ({ tree }) => {
      const missing = assessDocumentationIntegrity({ repoRoot: tree });
      assert.equal(missing.ok, false);
      assert.equal(missing.failures[0].reason, "unrunnable-command-target");

      mkdirSync(path.join(tree, "tests", "contract"), { recursive: true });
      const present = assessDocumentationIntegrity({ repoRoot: tree });
      assert.deepEqual(present.failures, []);

      // The directory exists but holds nothing: still runnable syntax, so still accepted.
      writeFileSync(path.join(tree, "tests", "contract", "x.contract.test.mjs"), "// noop\n");
      assert.equal(assessDocumentationIntegrity({ repoRoot: tree }).ok, true);
    },
  );
});

test("an unknown pnpm script is reported and a declared one is not", () => {
  withFixture(
    {
      packageScripts: { "db:validate": "node validate.mjs" },
      documents: {
        "docs/README.md": [
          "# Docs",
          "",
          "```bash",
          "pnpm run db:validate",
          "pnpm run db:bogus",
          "```",
        ].join("\n"),
      },
    },
    ({ tree }) => {
      const assessment = assessDocumentationIntegrity({ repoRoot: tree });
      assert.equal(assessment.failures.length, 1);
      assert.equal(assessment.failures[0].reason, "unknown-package-script");
      assert.match(assessment.failures[0].message, /db:bogus/u);
    },
  );
});

test("commands are only audited in live documents, never in point-in-time evidence records", () => {
  const body = ["# Record", "", "```bash", "cargo fmt --all -- --check", "node scripts/gone.mjs", "```"].join("\n");
  withFixture(
    {
      documents: {
        "docs/changelogs/CHANGELOG-2026-01-01.md": body,
        "docs/engineering/reviews/REVIEW-20260101-x.md": body,
        "docs/releases/RELEASE-v0.0.1.md": body,
        "docs/archive/README.md": body,
      },
    },
    ({ tree }) => {
      const assessment = assessDocumentationIntegrity({ repoRoot: tree });
      assert.deepEqual(assessment.failures, [], "recorded commands and links are historical facts");
      assert.equal(assessment.liveDocumentsChecked, 0);
    },
  );
});

test("links are still audited inside evidence records, because navigation must work everywhere", () => {
  withFixture(
    {
      documents: {
        "docs/releases/RELEASE-v0.0.1.md": ["# Release", "", "- [PRD](docs/product/prd/PRD.md)"].join("\n"),
      },
    },
    ({ tree }) => {
      const assessment = assessDocumentationIntegrity({ repoRoot: tree });
      assert.equal(assessment.failures.length, 1);
      assert.equal(assessment.failures[0].reason, "dead-relative-link");
    },
  );
});

test("the assessment is never vacuous", () => {
  withFixture({ documents: {} }, ({ tree }) => {
    assert.throws(() => assessDocumentationIntegrity({ repoRoot: tree }), /vacuous/u);
  });
});

test("--root audits another tree and the CLI exits 1 on a defect and 0 on a clean tree", () => {
  const options = parseDocumentationIntegrityArgs(["--json", "--root", "."]);
  assert.equal(options.root, repoRoot);
  assert.throws(() => parseDocumentationIntegrityArgs(["--root"]), /requires a directory/u);
  assert.throws(() => parseDocumentationIntegrityArgs(["--wat"]), /unsupported argument/u);

  withFixture(
    {
      documents: {
        "docs/README.md": ["# Clean", "", "```bash", "cargo fmt --check", "```"].join("\n"),
      },
    },
    ({ tree }) => {
      const clean = execFileSync(process.execPath, [gatePath, "--root", tree], { encoding: "utf8" });
      assert.match(clean, /Documentation integrity: /u);

      writeFileSync(
        path.join(tree, "docs", "README.md"),
        ["# Dirty", "", "```bash", "cargo fmt --all -- --check", "```"].join("\n"),
      );
      let exitCode = 0;
      let stderr = "";
      try {
        execFileSync(process.execPath, [gatePath, "--root", tree], { encoding: "utf8" });
      } catch (error) {
        exitCode = error.status;
        stderr = String(error.stdout);
      }
      assert.equal(exitCode, 1, "the CLI must exit 1 when a prescription is unrunnable");
      assert.match(stderr, /forbidden-fmt-scope/u);
      assert.match(stderr, /docs\/README\.md:4/u);
    },
  );
});

test("a shell entry point with a directory component is checked like a node target", () => {
  withFixture(
    {
      documents: {
        "docs/runbooks/deploy.md": [
          "# Runbook",
          "",
          "```bash",
          "bin/doctor.sh --environment staging",
          "./bin/backup.sh create --environment production",
          "bin/gone.sh --now",
          "```",
        ].join("\n"),
      },
    },
    ({ tree }) => {
      mkdirSync(path.join(tree, "bin"), { recursive: true });
      writeFileSync(path.join(tree, "bin", "doctor.sh"), "#!/bin/sh\nexit 0\n");
      writeFileSync(path.join(tree, "bin", "backup.sh"), "#!/bin/sh\nexit 0\n");

      const assessment = assessDocumentationIntegrity({ repoRoot: tree });
      assert.equal(assessment.ok, false);
      assert.equal(assessment.failures.length, 1, "only the missing entry point is reported");
      assert.equal(assessment.failures[0].reason, "unrunnable-command-target");
      assert.equal(assessment.failures[0].line, 6);
      assert.match(assessment.failures[0].message, /bin\/gone\.sh/u);
      assert.equal(assessment.scriptTargetsChecked, 3);
    },
  );
});

test("a shell reference that names no location is neither checked nor reported", () => {
  // A bare basename, an absolute path and a variable-prefixed path do not claim to live in this
  // checkout. Treating them as repository-relative paths would report three false positives and
  // train the reader to ignore the gate.
  withFixture(
    {
      documents: {
        "docs/runbooks/deploy.md": [
          "# Runbook",
          "",
          "```bash",
          "install.sh --prefix /opt",
          "/usr/local/bin/render.sh",
          "$SDKWORK_BIN/entry.sh",
          "```",
        ].join("\n"),
      },
    },
    ({ tree }) => {
      const assessment = assessDocumentationIntegrity({ repoRoot: tree });
      assert.deepEqual(assessment.failures, []);
      assert.equal(assessment.scriptTargetsChecked, 0);
    },
  );
});

test("a shell entry point reached through the multi-repository root is accepted", () => {
  withFixture(
    {
      extraFiles: { "sdkwork-specs/tools/align.sh": "#!/bin/sh\nexit 0\n" },
      documents: {
        "docs/runbooks/deploy.md": [
          "# Runbook",
          "",
          "```bash",
          "sdkwork-specs/tools/align.sh --root .",
          "```",
        ].join("\n"),
      },
    },
    ({ tree }) => {
      const assessment = assessDocumentationIntegrity({ repoRoot: tree });
      assert.deepEqual(assessment.failures, []);
      assert.equal(assessment.scriptTargetsChecked, 1);
    },
  );
});

test("the repository itself is clean and the checked counts are non-trivial", () => {
  const assessment = assessDocumentationIntegrity({ repoRoot });
  assert.equal(assessment.ok, true, formatDocumentationIntegrityReport(assessment));
  assert.ok(assessment.linksChecked > 400, `expected many links, saw ${assessment.linksChecked}`);
  assert.ok(assessment.liveDocumentsChecked > 100);
  assert.ok(
    assessment.scriptTargetsChecked > 200,
    `expected every prescribed script target to be counted, saw ${assessment.scriptTargetsChecked}`,
  );
  assert.equal(readFileSync(path.join(repoRoot, "Cargo.toml"), "utf8").length > 0, true);
});
