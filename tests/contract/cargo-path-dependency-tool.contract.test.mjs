import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  assessCargoPathDependencies,
  discoverManifests,
  formatCargoPathDependencyReport,
  parseCargoPathDependencyArgs,
  parsePathDependencies,
} from "../../tools/check-sandbox-cargo-path-dependencies.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

/**
 * Build a throwaway multi-repository workspace:
 *
 *   <tmp>/sdkwork-space/                 <- workspace root
 *     sdkwork-sibling/crates/leaf/       <- a resolvable sibling repository package
 *     sdkwork-fixture/                   <- the repository under audit
 *       Cargo.toml
 *       crates/app/Cargo.toml
 */
function createFixtureWorkspace({ rootManifest }) {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-cargo-paths-"));
  const workspaceRoot = path.join(base, "sdkwork-space");
  const siblingLeaf = path.join(workspaceRoot, "sdkwork-sibling", "crates", "leaf");
  const repo = path.join(workspaceRoot, "sdkwork-fixture");
  const appCrate = path.join(repo, "crates", "app");

  mkdirSync(siblingLeaf, { recursive: true });
  mkdirSync(appCrate, { recursive: true });
  writeFileSync(path.join(siblingLeaf, "Cargo.toml"), '[package]\nname = "leaf"\nversion = "0.1.0"\n');
  writeFileSync(
    path.join(appCrate, "Cargo.toml"),
    '[package]\nname = "app"\nversion = "0.1.0"\n\n[lib]\npath = "src/lib.rs"\n',
  );
  writeFileSync(path.join(repo, "Cargo.toml"), rootManifest);

  return { base, workspaceRoot, repo };
}

const HEALTHY_MANIFEST = [
  "[workspace]",
  'members = ["crates/app"]',
  "",
  "[workspace.dependencies]",
  'leaf = { path = "../sdkwork-sibling/crates/leaf" }',
  "",
].join("\n");

test("the repository's own manifests all resolve", () => {
  const assessment = assessCargoPathDependencies({ repoRoot });

  assert.equal(assessment.ok, true, formatCargoPathDependencyReport(assessment));
  assert.deepEqual(assessment.failures, []);
  assert.ok(assessment.manifestsChecked >= 8, "the workspace owns at least eight manifests");
  assert.ok(assessment.dependenciesChecked >= 6, "the workspace declares path dependencies");
  assert.ok(assessment.dependenciesChecked > 0, "the gate must not be vacuous");
});

test("discovery finds owned manifests and skips vendored or generated trees", () => {
  const manifests = discoverManifests(repoRoot).map((file) => path.relative(repoRoot, file));

  assert.ok(manifests.includes("Cargo.toml"));
  assert.ok(manifests.includes(path.join("crates", "sdkwork-sandbox-provider-spi", "Cargo.toml")));
  assert.ok(
    manifests.every((file) => !file.includes("target") && !file.includes("node_modules")),
    "vendored and generated trees must not be scanned",
  );
});

test("[lib] path is a source path, not a dependency path", () => {
  const fixture = createFixtureWorkspace({ rootManifest: HEALTHY_MANIFEST });
  try {
    const declared = parsePathDependencies(path.join(fixture.repo, "crates", "app", "Cargo.toml"));
    assert.deepEqual(declared, [], "a [lib] path must never be reported as a path dependency");
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("a healthy sibling-repository dependency passes", () => {
  const fixture = createFixtureWorkspace({ rootManifest: HEALTHY_MANIFEST });
  try {
    const assessment = assessCargoPathDependencies({
      repoRoot: fixture.repo,
      workspaceRoot: fixture.workspaceRoot,
    });
    assert.equal(assessment.ok, true, formatCargoPathDependencyReport(assessment));
    assert.equal(assessment.dependenciesChecked, 1);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("the 2026-09-22 regression shape is rejected", () => {
  // One surplus '..' retargeted the dependency from <workspace>/sdkwork-web-framework to
  // <drive root>/sdkwork-web-framework, which broke `cargo metadata` for the entire workspace
  // while every static gate stayed green. The escaped location did not exist, so the resolved
  // failure is `unresolved`; either reason is a rejection, which is what matters.
  const fixture = createFixtureWorkspace({
    rootManifest: HEALTHY_MANIFEST.replace(
      '"../sdkwork-sibling/crates/leaf"',
      '"../../../sdkwork-sibling/crates/leaf"',
    ),
  });
  try {
    const assessment = assessCargoPathDependencies({
      repoRoot: fixture.repo,
      workspaceRoot: fixture.workspaceRoot,
    });

    assert.equal(assessment.ok, false);
    assert.equal(assessment.failures.length, 1);
    assert.ok(
      ["unresolved", "escapes-workspace"].includes(assessment.failures[0].reason),
      `expected a rejection, got '${assessment.failures[0].reason}'`,
    );
    assert.equal(assessment.failures[0].crate, "leaf");
    assert.equal(assessment.failures[0].line, 5);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("a dependency that escapes the workspace root is rejected even when its target exists", () => {
  const fixture = createFixtureWorkspace({ rootManifest: HEALTHY_MANIFEST });
  try {
    // Land the escaped target inside the fixture's own scratch directory, outside the workspace
    // root, and give it a real manifest so only the containment rule can catch it.
    const escapedLeaf = path.join(fixture.base, "outside", "crates", "leaf");
    mkdirSync(escapedLeaf, { recursive: true });
    writeFileSync(
      path.join(escapedLeaf, "Cargo.toml"),
      '[package]\nname = "leaf"\nversion = "0.1.0"\n',
    );
    writeFileSync(
      path.join(fixture.repo, "Cargo.toml"),
      HEALTHY_MANIFEST.replace(
        '"../sdkwork-sibling/crates/leaf"',
        '"../../outside/crates/leaf"',
      ),
    );

    const assessment = assessCargoPathDependencies({
      repoRoot: fixture.repo,
      workspaceRoot: fixture.workspaceRoot,
    });

    assert.equal(assessment.ok, false);
    assert.equal(assessment.failures.length, 1);
    assert.equal(assessment.failures[0].reason, "escapes-workspace");
    assert.match(formatCargoPathDependencyReport(assessment), /escapes-workspace/u);
    assert.match(assessment.failures[0].message, /outside the workspace root/u);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("a nonexistent path dependency is rejected", () => {
  const fixture = createFixtureWorkspace({
    rootManifest: HEALTHY_MANIFEST.replace("sdkwork-sibling", "sdkwork-absent"),
  });
  try {
    const assessment = assessCargoPathDependencies({
      repoRoot: fixture.repo,
      workspaceRoot: fixture.workspaceRoot,
    });

    assert.equal(assessment.ok, false);
    assert.equal(assessment.failures[0].reason, "unresolved");
    assert.match(assessment.failures[0].message, /does not exist/u);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
});

test("a path dependency that is not a Cargo package is rejected", () => {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-cargo-paths-"));
  const workspaceRoot = path.join(base, "sdkwork-space");
  const repo = path.join(workspaceRoot, "sdkwork-fixture");
  try {
    // The declared target directory exists but carries no manifest, so only the package rule
    // can catch it.
    mkdirSync(path.join(workspaceRoot, "sdkwork-sibling", "crates", "leaf"), { recursive: true });
    mkdirSync(repo, { recursive: true });
    writeFileSync(path.join(repo, "Cargo.toml"), HEALTHY_MANIFEST);

    const assessment = assessCargoPathDependencies({ repoRoot: repo, workspaceRoot });
    assert.equal(assessment.ok, false);
    assert.equal(assessment.failures[0].reason, "not-a-package");
    assert.match(assessment.failures[0].message, /no Cargo\.toml/u);
  } finally {
    rmSync(base, { recursive: true, force: true });
  }
});

test("an empty manifest set is a hard error rather than a vacuous pass", () => {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-cargo-paths-empty-"));
  try {
    assert.throws(
      () => assessCargoPathDependencies({ repoRoot: base, workspaceRoot: base }),
      /no Cargo\.toml found/u,
    );
  } finally {
    rmSync(base, { recursive: true, force: true });
  }
});

test("argument parsing accepts --json and rejects anything else", () => {
  assert.deepEqual(parseCargoPathDependencyArgs([]), { json: false });
  assert.deepEqual(parseCargoPathDependencyArgs(["--json"]), { json: true });
  assert.throws(() => parseCargoPathDependencyArgs(["--all"]), /unsupported argument/u);
});
