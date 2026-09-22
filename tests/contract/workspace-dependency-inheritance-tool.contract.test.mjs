import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  assessWorkspaceDependencyInheritance,
  classifyDependencySection,
  formatWorkspaceDependencyInheritanceReport,
  parseMemberDependencyEntries,
  parseWorkspaceDependencyInheritanceArgs,
} from "../../tools/check-sandbox-workspace-dependency-inheritance.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

const ROOT_MANIFEST = [
  "[workspace]",
  'members = ["crates/app"]',
  "",
  "[workspace.dependencies]",
  'leaf = { path = "../sibling/leaf" }',
  'serde = "1"',
  "",
  "[workspace.package]",
  'edition = "2021"',
  'rust-version = "1.85"',
  'version = "0.1.0"',
  'license = "AGPL-3.0-or-later"',
  "",
].join("\n");

/**
 * Build a throwaway single-repository workspace:
 *
 *   <tmp>/sdkwork-fixture/
 *     Cargo.toml            <- workspace root and dependency authority
 *     crates/app/Cargo.toml <- the member under audit
 */
function createFixture({ rootManifest = ROOT_MANIFEST, memberManifest }) {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-dep-inheritance-"));
  const repo = path.join(base, "sdkwork-fixture");
  const member = path.join(repo, "crates", "app");
  mkdirSync(member, { recursive: true });
  writeFileSync(path.join(repo, "Cargo.toml"), rootManifest);
  writeFileSync(path.join(member, "Cargo.toml"), memberManifest);
  return { base, repo, memberManifestPath: path.join(member, "Cargo.toml") };
}

function inspect(memberManifest, rootManifest) {
  const fixture = createFixture({ rootManifest, memberManifest });
  try {
    return assessWorkspaceDependencyInheritance({ repoRoot: fixture.repo });
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
}

const MEMBER_PREAMBLE = [
  "[package]",
  'name = "app"',
  "version.workspace = true",
  "edition.workspace = true",
  "",
].join("\n");

test("the repository's own manifests all inherit from the root table", () => {
  const assessment = assessWorkspaceDependencyInheritance({ repoRoot });

  assert.equal(assessment.ok, true, formatWorkspaceDependencyInheritanceReport(assessment));
  assert.deepEqual(assessment.failures, []);
  assert.ok(
    assessment.memberManifestsChecked >= 8,
    `expected the workspace to own member manifests, saw ${assessment.memberManifestsChecked}`,
  );
  assert.ok(
    assessment.entriesChecked >= 20,
    `the gate must not be vacuous; saw ${assessment.entriesChecked} entries`,
  );
  assert.equal(assessment.entriesChecked, assessment.inheritedChecked);
  for (const key of ["axum", "tokio", "sqlx", "sdkwork-utils-rust"]) {
    assert.ok(assessment.authorityKeys.includes(key), `root table must declare ${key}`);
  }
});

test("a member-local third-party version is rejected with the exact location", () => {
  // This is the 2026-09-22 regression: `axum = "0.8"` lived in the member manifest while
  // check-rust-manifest-standard.mjs reported PASS, because it only inspects [package] and [lints].
  const assessment = inspect(
    `${MEMBER_PREAMBLE}[dependencies]\naxum = "0.8"\nserde = { workspace = true }\n`,
  );

  assert.equal(assessment.ok, false);
  assert.equal(assessment.failures.length, 1);
  const [failure] = assessment.failures;
  assert.equal(failure.reason, "member-local-declaration");
  assert.equal(failure.crate, "axum");
  assert.equal(failure.manifest, path.join("crates", "app", "Cargo.toml"));
  assert.equal(failure.line, 6);
  assert.match(failure.message, /RUST_CODE_SPEC\.md section 14/u);
  assert.match(formatWorkspaceDependencyInheritanceReport(assessment), /member-local-declaration/u);
});

test("a member-local path dependency is rejected too", () => {
  const assessment = inspect(
    `${MEMBER_PREAMBLE}[dependencies]\nleaf = { path = "../../sibling/leaf" }\n`,
  );

  assert.equal(assessment.ok, false);
  assert.equal(assessment.failures[0].reason, "member-local-declaration");
  assert.equal(assessment.failures[0].crate, "leaf");
});

test("both inherited spellings are accepted", () => {
  const assessment = inspect(
    [
      MEMBER_PREAMBLE,
      "[dependencies]",
      "serde = { workspace = true }",
      "leaf.workspace = true",
      "",
      "[dev-dependencies]",
      "serde = { workspace = true, features = [] }",
      "",
    ].join("\n"),
  );

  assert.equal(assessment.ok, true, formatWorkspaceDependencyInheritanceReport(assessment));
  assert.equal(assessment.entriesChecked, 3);
  assert.equal(assessment.inheritedChecked, 3);
});

test("inheriting a key the root table never declares is rejected", () => {
  const assessment = inspect(`${MEMBER_PREAMBLE}[dependencies]\nhyper = { workspace = true }\n`);

  assert.equal(assessment.ok, false);
  assert.equal(assessment.failures.length, 1);
  assert.equal(assessment.failures[0].reason, "missing-workspace-declaration");
  assert.equal(assessment.failures[0].crate, "hyper");
  assert.match(assessment.failures[0].message, /NAMING_SPEC\.md section 3\.2 rule 6/u);
});

test("the [dependencies.crate] section form is classified and enforced", () => {
  assert.deepEqual(classifyDependencySection("dependencies.axum"), {
    kind: "table-entry",
    crate: "axum",
  });
  assert.deepEqual(classifyDependencySection("target.'cfg(unix)'.dependencies"), { kind: "table" });

  const inherited = inspect(
    `${MEMBER_PREAMBLE}[dependencies.serde]\nworkspace = true\n`,
  );
  assert.equal(inherited.ok, true, formatWorkspaceDependencyInheritanceReport(inherited));

  const local = inspect(`${MEMBER_PREAMBLE}[dependencies.axum]\nversion = "0.8"\n`);
  assert.equal(local.ok, false);
  assert.equal(local.failures[0].reason, "member-local-declaration");
  assert.equal(local.failures[0].crate, "axum");
});

test("target-specific dependency tables are in scope", () => {
  const assessment = inspect(
    [
      MEMBER_PREAMBLE,
      "[target.'cfg(windows)'.dependencies]",
      'winapi = "0.3"',
      "",
      "[target.'cfg(unix)'.dev-dependencies]",
      "serde = { workspace = true }",
      "",
    ].join("\n"),
  );

  assert.equal(assessment.ok, false);
  assert.equal(assessment.failures.length, 1);
  assert.equal(assessment.failures[0].crate, "winapi");
  assert.equal(assessment.failures[0].line, 7);
});

test("[package] inheritance fields are never mistaken for dependencies", () => {
  // A naive scan that greps `.workspace = true` reports edition / rust-version / version /
  // license as missing workspace-dependency keys. Those inherit from [workspace.package]
  // instead, so a checker that cannot tell the two tables apart produces false positives on
  // every crate in the repository.
  const fixture = createFixture({
    memberManifest: `${MEMBER_PREAMBLE}[dependencies]\nserde = { workspace = true }\n`,
  });
  try {
    const parsed = parseMemberDependencyEntries(fixture.memberManifestPath);
    assert.deepEqual(
      parsed.map((entry) => entry.crate),
      ["serde"],
      "only dependency tables may produce entries",
    );
    assert.equal(parsed[0].line, MEMBER_PREAMBLE.split("\n").length + 1);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }

  const assessment = assessWorkspaceDependencyInheritance({ repoRoot });
  assert.deepEqual(
    assessment.failures.filter((item) => item.crate === "edition"),
    [],
    "edition inherits from [workspace.package], not [workspace.dependencies]",
  );
});

test("[patch.*] and [workspace.dependencies] are out of member scope", () => {
  assert.equal(classifyDependencySection("patch.crates-io"), null);
  assert.equal(classifyDependencySection("workspace.dependencies"), null);
  assert.equal(classifyDependencySection("package"), null);
  assert.equal(classifyDependencySection("lints"), null);
});

test("a manifest without a root [workspace.dependencies] table is refused, not silently passed", () => {
  const fixture = createFixture({
    rootManifest: `${ROOT_MANIFEST.split("[workspace.dependencies]")[0]}[workspace.package]\nedition = "2021"\n`,
    memberManifest: `${MEMBER_PREAMBLE}[dependencies]\nserde = { workspace = true }\n`,
  });
  try {
    assert.throws(
      () => assessWorkspaceDependencyInheritance({ repoRoot: fixture.repo }),
      /\[workspace\.dependencies\] table is empty or missing/u,
    );
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("argument parsing accepts --json and rejects anything else", () => {
  assert.deepEqual(parseWorkspaceDependencyInheritanceArgs([]), { json: false });
  assert.deepEqual(parseWorkspaceDependencyInheritanceArgs(["--json"]), { json: true });
  assert.throws(
    () => parseWorkspaceDependencyInheritanceArgs(["--all"]),
    /unsupported argument: --all/u,
  );
  assert.throws(() => parseWorkspaceDependencyInheritanceArgs(["extra"]), /unsupported argument/u);
});

test("the success report states the counts it actually checked", () => {
  const assessment = assessWorkspaceDependencyInheritance({ repoRoot });
  const report = formatWorkspaceDependencyInheritanceReport(assessment);

  assert.match(report, new RegExp(`${assessment.entriesChecked} member dependency`,"u"));
  assert.match(report, new RegExp(`${assessment.authorityKeys.length} key`, "u"));
});
