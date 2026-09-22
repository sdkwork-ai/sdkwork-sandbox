import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import {
  assessComponentContractAlignment,
  discoverComponentSpecs,
  hasAuthoredSource,
} from "../../tools/check-sandbox-component-contract-alignment.mjs";
const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const toolPath = path.join(repoRoot, "tools/check-sandbox-component-contract-alignment.mjs");

const LEAF = "crates/sdkwork-fixture-leaf";
const REPO_NAME = "sdkwork-fixture";

/** The spec files every healthy fixture references; they are created under the repository root. */
const REF_SPECS = [
  "CODE_STYLE_SPEC.md",
  "NAMING_SPEC.md",
  "RUST_CODE_SPEC.md",
  "TYPESCRIPT_CODE_SPEC.md",
];

function createFixture({ spec, extraFiles = {}, componentRel = LEAF, repoName = REPO_NAME } = {}) {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-component-contract-"));
  const fixtureRepo = path.join(base, repoName);
  const componentRoot = path.join(fixtureRepo, componentRel);
  mkdirSync(path.join(componentRoot, "specs"), { recursive: true });
  writeFileSync(
    path.join(componentRoot, "specs", "component.spec.json"),
    JSON.stringify(spec, null, 2),
  );
  for (const name of REF_SPECS) {
    const target = path.join(fixtureRepo, "refs", name);
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, "# fixture reference spec\n");
  }
  for (const [rel, content] of Object.entries(extraFiles)) {
    const target = path.join(fixtureRepo, rel);
    mkdirSync(path.dirname(target), { recursive: true });
    writeFileSync(target, content);
  }
  return { base, repoRoot: fixtureRepo, componentRoot };
}

function healthySpec(overrides = {}) {
  return {
    component: {
      name: "sdkwork-fixture-leaf",
      type: "rust-crate",
      root: `${REPO_NAME}/${LEAF}`,
      domain: "intelligence",
      capability: "fixture",
      languages: ["rust"],
      manifests: ["Cargo.toml"],
      ...overrides.component,
    },
    canonicalSpecs:
      overrides.canonicalSpecs ??
      ["CODE_STYLE_SPEC.md", "NAMING_SPEC.md", "RUST_CODE_SPEC.md"].map((file) => ({
        file,
        path: `../../refs/${file}`,
      })),
    contracts: overrides.contracts ?? {},
  };
}

/** Files that make a fixture component genuinely own authored Rust source. */
function healthyExtras() {
  return {
    [`${LEAF}/Cargo.toml`]: '[package]\nname = "sdkwork-fixture-leaf"\nversion = "0.1.0"\n',
    [`${LEAF}/src/lib.rs`]: "pub fn fixture() {}\n",
  };
}

function assess(fixture) {
  return assessComponentContractAlignment({ repoRoot: fixture.repoRoot });
}

function withFixture(options, run) {
  const fixture = createFixture(options);
  try {
    return run(fixture);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
}

test("discovery finds every component spec in this repository", () => {
  const specs = discoverComponentSpecs(repoRoot);
  assert.ok(specs.length >= 9, `expected at least 9 component specs, saw ${specs.length}`);
  for (const specPath of specs) {
    assert.equal(path.basename(specPath), "component.spec.json");
    assert.equal(path.basename(path.dirname(specPath)), "specs");
  }
  const labels = specs.map((specPath) => {
    const rel = path
      .relative(repoRoot, path.dirname(path.dirname(specPath)))
      .split(path.sep)
      .join("/");
    return rel === "" ? "." : rel;
  });
  assert.ok(labels.includes("."), "the repository root contract must be discovered");
  assert.ok(labels.includes("crates/sdkwork-api-sandbox-assembly"));
});

test("discovery skips generated and vendored trees", () => {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-component-discovery-"));
  try {
    for (const skipped of ["node_modules", "target", ".workbuddy", "dist"]) {
      const dir = path.join(base, skipped, "pkg", "specs");
      mkdirSync(dir, { recursive: true });
      writeFileSync(path.join(dir, "component.spec.json"), "{}");
    }
    const kept = path.join(base, "crates", "kept", "specs");
    mkdirSync(kept, { recursive: true });
    writeFileSync(path.join(kept, "component.spec.json"), "{}");
    assert.deepEqual(discoverComponentSpecs(base), [
      path.join(base, "crates", "kept", "specs", "component.spec.json"),
    ]);
  } finally {
    rmSync(base, { recursive: true, force: true });
  }
});

test("authored-source detection reads extensions and ignores generated trees", () => {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-component-source-"));
  try {
    assert.equal(hasAuthoredSource(base, [".rs"]), false, "empty directory authors nothing");
    mkdirSync(path.join(base, "node_modules", "dep"), { recursive: true });
    writeFileSync(path.join(base, "node_modules", "dep", "index.ts"), "");
    assert.equal(
      hasAuthoredSource(base, [".ts"]),
      false,
      "a vendored dependency must not make the component look like a TypeScript author",
    );
    mkdirSync(path.join(base, "src"), { recursive: true });
    writeFileSync(path.join(base, "src", "lib.rs"), "");
    assert.equal(hasAuthoredSource(base, [".rs"]), true);
    assert.equal(hasAuthoredSource(base, [".dart"]), false);
  } finally {
    rmSync(base, { recursive: true, force: true });
  }
});

test("a healthy component passes every rule", () => {
  withFixture({ spec: healthySpec(), extraFiles: healthyExtras() }, (fixture) => {
    const assessment = assess(fixture);
    assert.equal(assessment.ok, true, JSON.stringify(assessment.failures, null, 2));
    assert.equal(assessment.componentsChecked, 1);
    assert.equal(assessment.components[0].ownsAuthoredSource, true);
  });
});

test("a canonical spec path that does not resolve is rejected", () => {
  const spec = healthySpec();
  spec.canonicalSpecs[0].path = "../../refs/GONE_SPEC.md";
  spec.canonicalSpecs[0].file = "GONE_SPEC.md";
  withFixture({ spec, extraFiles: healthyExtras() }, (fixture) => {
    const assessment = assess(fixture);
    assert.equal(assessment.ok, false);
    const failure = assessment.failures.find((f) => f.reason === "unresolved-canonical-spec");
    assert.ok(failure, "a dangling canonical spec path must be reported");
    assert.match(failure.message, /GONE_SPEC\.md/u);
  });
});

test("a canonical spec whose file name contradicts its path is rejected", () => {
  const spec = healthySpec();
  spec.canonicalSpecs[0].file = "WRONG_NAME_SPEC.md";
  withFixture({ spec, extraFiles: healthyExtras() }, (fixture) => {
    const assessment = assess(fixture);
    assert.equal(assessment.ok, false);
    assert.ok(assessment.failures.some((f) => f.reason === "canonical-spec-name-mismatch"));
  });
});

test("a manifest that does not exist is rejected", () => {
  const spec = healthySpec();
  spec.component.manifests = ["Cargo.toml", "missing-manifest.json"];
  withFixture({ spec, extraFiles: healthyExtras() }, (fixture) => {
    const assessment = assess(fixture);
    assert.equal(assessment.ok, false);
    const failure = assessment.failures.find((f) => f.reason === "missing-manifest");
    assert.ok(failure);
    assert.match(failure.message, /missing-manifest\.json/u);
  });
});

test("a component.root that does not describe the real location is rejected", () => {
  const spec = healthySpec();
  spec.component.root = `${REPO_NAME}/crates/somewhere-else`;
  withFixture({ spec, extraFiles: healthyExtras() }, (fixture) => {
    const assessment = assess(fixture);
    assert.equal(assessment.ok, false);
    const failure = assessment.failures.find((f) => f.reason === "stale-component-root");
    assert.ok(failure);
    assert.match(failure.message, /crates\/sdkwork-fixture-leaf/u);
  });
});

test("either sanctioned root spelling is accepted", () => {
  const spec = healthySpec();
  spec.component.root = LEAF;
  withFixture({ spec, extraFiles: healthyExtras() }, (fixture) => {
    assert.equal(assess(fixture).ok, true, "COMPONENT_SPEC.md section 4 spells this root crates/<name>/");
  });
});

test("a missing required component field is rejected", () => {
  const spec = healthySpec();
  delete spec.component.domain;
  withFixture({ spec, extraFiles: healthyExtras() }, (fixture) => {
    const assessment = assess(fixture);
    assert.equal(assessment.ok, false);
    const failure = assessment.failures.find((f) => f.reason === "missing-required-field");
    assert.ok(failure);
    assert.match(failure.message, /component\.domain/u);
  });
});

test("authored source without CODE_STYLE_SPEC.md and NAMING_SPEC.md is rejected", () => {
  const spec = healthySpec();
  spec.canonicalSpecs = spec.canonicalSpecs.filter(
    (entry) => entry.file !== "CODE_STYLE_SPEC.md" && entry.file !== "NAMING_SPEC.md",
  );
  withFixture({ spec, extraFiles: healthyExtras() }, (fixture) => {
    const assessment = assess(fixture);
    assert.equal(assessment.ok, false);
    const missing = assessment.failures.filter((f) => f.reason === "missing-base-spec");
    assert.equal(missing.length, 2, "both base specs are required once the component authors source");
  });
});

test("a language declared with no authored source is rejected", () => {
  // Regression for the 2026-09-22 finding: the root contract declared `typescript` while the
  // repository had no TypeScript source at all, and nothing failed.
  const spec = healthySpec();
  spec.component.languages = ["rust", "typescript"];
  spec.canonicalSpecs = [
    ...spec.canonicalSpecs,
    { file: "TYPESCRIPT_CODE_SPEC.md", path: "../../refs/TYPESCRIPT_CODE_SPEC.md" },
  ];
  withFixture({ spec, extraFiles: healthyExtras() }, (fixture) => {
    const assessment = assess(fixture);
    assert.equal(assessment.ok, false);
    const failure = assessment.failures.find((f) => f.reason === "false-language-declaration");
    assert.ok(failure, "listing the language spec must not excuse a phantom language");
    assert.match(failure.message, /typescript/u);
  });
});

test("a declared language that authors source without its language spec is rejected", () => {
  const spec = healthySpec();
  spec.canonicalSpecs = spec.canonicalSpecs.filter((entry) => entry.file !== "RUST_CODE_SPEC.md");
  withFixture({ spec, extraFiles: healthyExtras() }, (fixture) => {
    const assessment = assess(fixture);
    assert.equal(assessment.ok, false);
    const failure = assessment.failures.find((f) => f.reason === "missing-language-spec");
    assert.ok(failure);
    assert.match(failure.message, /RUST_CODE_SPEC\.md/u);
  });
});

test("a rust-api-assembly that violates its MUST sentence is rejected on every clause", () => {
  const name = "sdkwork-api-fixture-assembly";
  const rel = `crates/${name}`;
  const spec = {
    component: {
      name,
      type: "rust-api-assembly",
      root: `${REPO_NAME}/${rel}`,
      domain: "application",
      capability: "api-assembly",
      languages: ["rust"],
      // assembly-manifest.json deliberately absent
      manifests: ["Cargo.toml"],
      surface: "backend-service",
    },
    canonicalSpecs: ["CODE_STYLE_SPEC.md", "NAMING_SPEC.md", "RUST_CODE_SPEC.md"].map((file) => ({
      file,
      path: `../../refs/${file}`,
    })),
    contracts: { layerRole: "backend-service" },
  };
  withFixture(
    {
      spec,
      componentRel: rel,
      extraFiles: {
        [`${rel}/Cargo.toml`]: `[package]\nname = "${name}"\nversion = "0.1.0"\n`,
        [`${rel}/src/lib.rs`]: "pub fn fixture() {}\n",
      },
    },
    (fixture) => {
      const assessment = assess(fixture);
      assert.equal(assessment.ok, false);
      const reasons = new Set(assessment.failures.map((f) => f.reason));
      assert.ok(reasons.has("assembly-surface"), "surface must be api-assembly");
      assert.ok(reasons.has("assembly-layer-role"), "layerRole must be runtime-composition");
      assert.ok(reasons.has("assembly-manifest"), "assembly-manifest.json must be owned");
      assert.ok(reasons.has("assembly-required-spec"), "the seven MUST specs are required");
      assert.ok(!reasons.has("assembly-name"), "the name is valid and must not be reported");
      assert.ok(!reasons.has("assembly-root"), "the location is valid and must not be reported");
    },
  );
});

test("a rust-api-assembly whose Cargo package name diverges is rejected", () => {
  const name = "sdkwork-api-fixture-assembly";
  const rel = `crates/${name}`;
  const spec = {
    component: {
      name,
      type: "rust-api-assembly",
      root: `${REPO_NAME}/${rel}`,
      domain: "application",
      capability: "api-assembly",
      languages: ["rust"],
      manifests: ["Cargo.toml", "assembly-manifest.json"],
      surface: "api-assembly",
    },
    canonicalSpecs: [
      "API_ASSEMBLY_SPEC.md",
      "APPLICATION_GATEWAY_SPEC.md",
      "WEB_FRAMEWORK_SPEC.md",
      "WEB_BACKEND_SPEC.md",
      "RUST_CODE_SPEC.md",
      "APP_RUNTIME_TOPOLOGY_SPEC.md",
      "TEST_SPEC.md",
    ].map((file) => ({ file, path: `../../../refs/${file}` })),
    contracts: { layerRole: "runtime-composition" },
  };
  withFixture(
    {
      spec,
      componentRel: rel,
      extraFiles: {
        [`${rel}/Cargo.toml`]: `[package]\nname = "something-else"\nversion = "0.1.0"\n`,
        [`${rel}/src/lib.rs`]: "pub fn fixture() {}\n",
        [`${rel}/assembly-manifest.json`]: "{}\n",
      },
    },
    (fixture) => {
      const assessment = assess(fixture);
      assert.equal(assessment.ok, false);
      const failure = assessment.failures.find((f) => f.reason === "assembly-package-name");
      assert.ok(failure);
      assert.match(failure.message, /something-else/u);
    },
  );
});

test("this repository satisfies every rule", () => {
  const assessment = assessComponentContractAlignment({ repoRoot });
  assert.equal(
    assessment.ok,
    true,
    JSON.stringify(assessment.failures, null, 2),
  );
  assert.ok(assessment.componentsChecked >= 9);
});

test("the CLI exits non-zero and names the offending component", () => {
  const spec = healthySpec();
  spec.component.languages = ["rust", "typescript"];
  withFixture({ spec, extraFiles: healthyExtras() }, (fixture) => {
    const result = spawnSync(
      process.execPath,
      [toolPath, "--root", fixture.repoRoot, "--json"],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 1, result.stderr || result.stdout);
    const output = JSON.parse(result.stdout);
    assert.equal(output.ok, false);
    assert.ok(output.failures.some((f) => f.reason === "false-language-declaration"));
  });
});

test("a crate directory without a component spec is rejected", () => {
  // R7, forward direction (the F-07 shape): an authored crate that dropped
  // out of the component-contract system -- exactly how
  // `sdkwork-api-sandbox-assembly` went missing from the human module
  // inventory -- must be reported, not discovered by whoever re-reads lists.
  withFixture({ spec: healthySpec(), extraFiles: healthyExtras() }, (fixture) => {
    mkdirSync(path.join(fixture.repoRoot, "crates", "sdkwork-fixture-orphan"), {
      recursive: true,
    });
    writeFileSync(
      path.join(fixture.repoRoot, "crates", "sdkwork-fixture-orphan", "Cargo.toml"),
      '[package]\nname = "sdkwork-fixture-orphan"\nversion = "0.1.0"\n',
    );
    const assessment = assess(fixture);
    assert.equal(assessment.ok, false);
    const failure = assessment.failures.find((f) => f.reason === "missing-crate-component-spec");
    assert.ok(failure, JSON.stringify(assessment.failures, null, 2));
    assert.match(failure.message, /sdkwork-fixture-orphan/u);
    assert.equal(assessment.cratesChecked, 2);
  });
});

test("a component spec whose crate owns no Cargo.toml is rejected", () => {
  // R7, reverse direction: a spec under crates/ that describes no crate is a
  // stale contract, not a small one.
  withFixture(
    {
      spec: healthySpec({
        component: { languages: [] },
      }),
      extraFiles: {},
    },
    (fixture) => {
      const assessment = assess(fixture);
      assert.equal(assessment.ok, false);
      const failure = assessment.failures.find((f) => f.reason === "crate-spec-without-crate");
      assert.ok(failure, JSON.stringify(assessment.failures, null, 2));
      assert.match(failure.message, /owns no Cargo\.toml/u);
    },
  );
});

test("a crates/ directory with neither Cargo.toml nor spec is ignored by R7", () => {
  // Control: a bare directory (no Cargo.toml) is not a crate, so the
  // reconciliation must not demand a component spec for it.
  withFixture({ spec: healthySpec(), extraFiles: healthyExtras() }, (fixture) => {
    mkdirSync(path.join(fixture.repoRoot, "crates", "sdkwork-fixture-empty"), {
      recursive: true,
    });
    const assessment = assess(fixture);
    assert.equal(assessment.ok, true, JSON.stringify(assessment.failures, null, 2));
    assert.equal(assessment.cratesChecked, 1);
  });
});
