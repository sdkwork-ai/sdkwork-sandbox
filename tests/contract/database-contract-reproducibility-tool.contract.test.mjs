import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  assessDatabaseContractReproducibility,
  formatDatabaseContractReproducibilityReport,
  normaliseText,
  parseDatabaseContractReproducibilityArgs,
  parseRegisteredCommand,
  redirectRoot,
} from "../../tools/check-sandbox-database-contract-reproducibility.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");

/**
 * The stub generator stands in for `materialize-database-contract-from-baseline.mjs`. It resolves
 * `--root` against its working directory exactly as the real tool does, so the fixture exercises
 * the same redirect contract.
 */
const STUB_GENERATOR = `import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

const args = process.argv.slice(2);
const valueOf = (flag) => args[args.indexOf(flag) + 1];
const root = valueOf("--root");
const baseline = valueOf("--baseline");
// The real generator reads the baseline that its arguments name, resolved from the working
// directory. Modelling that here is what makes the missing-baseline case meaningful.
if (baseline && !existsSync(path.resolve(baseline))) {
  process.stderr.write("baseline '" + baseline + "' does not exist\\n");
  process.exit(2);
}
const contractDirectory = path.join(root, "database/contract");
const schema = process.env.SDKWORK_STUB_SCHEMA ?? "materialized: schema";
const registry = process.env.SDKWORK_STUB_REGISTRY ?? "materialized: registry";
const prefix = process.env.SDKWORK_STUB_PREFIX ?? "materialized: prefix";
mkdirSync(contractDirectory, { recursive: true });
writeFileSync(path.join(contractDirectory, "schema.yaml"), schema + "\\n");
writeFileSync(path.join(contractDirectory, "table-registry.json"), registry + "\\n");
writeFileSync(path.join(contractDirectory, "prefix-registry.json"), prefix + "\\n");
process.stdout.write(readFileSync(path.join(contractDirectory, "schema.yaml"), "utf8"));
`;

const REGISTERED_COMMAND =
  "node tools/stub-materialize.mjs --root . --baseline database/ddl/baseline/postgres/0001_stub_baseline.sql --module-id stub --owner stub-team --prefixes stub_";

function createFixture({
  committedSchema = "materialized: schema",
  committedRegistry = "materialized: registry",
  committedPrefix = "materialized: prefix",
  command = REGISTERED_COMMAND,
  includeGenerator = true,
  includeManifest = true,
  includeBaseline = true,
} = {}) {
  const base = mkdtempSync(path.join(tmpdir(), "sdkwork-contract-repro-"));
  const repo = path.join(base, "sdkwork-fixture");
  mkdirSync(path.join(repo, "tools"), { recursive: true });
  mkdirSync(path.join(repo, "database/contract"), { recursive: true });
  mkdirSync(path.join(repo, "database/ddl/baseline/postgres"), { recursive: true });
  mkdirSync(path.join(repo, "database/contract"), { recursive: true });

  writeFileSync(
    path.join(repo, "package.json"),
    `${JSON.stringify({ name: "sdkwork-fixture", private: true, scripts: { "db:materialize:contract": command } }, null, 2)}\n`,
    "utf8",
  );
  if (includeGenerator) {
    writeFileSync(path.join(repo, "tools/stub-materialize.mjs"), STUB_GENERATOR, "utf8");
  }
  if (includeManifest) {
    writeFileSync(
      path.join(repo, "database/database.manifest.json"),
      `${JSON.stringify({ schemaVersion: 1, moduleId: "stub" }, null, 2)}\n`,
      "utf8",
    );
  }
  if (includeBaseline) {
    writeFileSync(
      path.join(repo, "database/ddl/baseline/postgres/0001_stub_baseline.sql"),
      "-- stub baseline\n",
      "utf8",
    );
  }
  writeFileSync(path.join(repo, "database/contract/schema.yaml"), `${committedSchema}\n`, "utf8");
  writeFileSync(
    path.join(repo, "database/contract/table-registry.json"),
    `${committedRegistry}\n`,
    "utf8",
  );
  writeFileSync(
    path.join(repo, "database/contract/prefix-registry.json"),
    `${committedPrefix}\n`,
    "utf8",
  );

  return { base, repo };
}

function withFixture(options, body) {
  const fixture = createFixture(options);
  try {
    return body(fixture.repo, fixture.base);
  } finally {
    rmSync(fixture.base, { recursive: true, force: true });
  }
}

test("the registered command is split into a tool and its arguments", () => {
  const parsed = parseRegisteredCommand(REGISTERED_COMMAND);
  assert.equal(parsed.tool, "tools/stub-materialize.mjs");
  assert.deepEqual(parsed.args, [
    "--root",
    ".",
    "--baseline",
    "database/ddl/baseline/postgres/0001_stub_baseline.sql",
    "--module-id",
    "stub",
    "--owner",
    "stub-team",
    "--prefixes",
    "stub_",
  ]);
  assert.throws(() => parseRegisteredCommand("./tools/stub-materialize.mjs"), /must be a 'node/u);
});

test("redirectRoot replaces an existing --root value and appends one when absent", () => {
  const replaced = redirectRoot(["--root", ".", "--module-id", "stub"], "target/contract-reproducibility");
  assert.deepEqual(replaced, [
    "--root",
    "target/contract-reproducibility",
    "--module-id",
    "stub",
  ]);

  const appended = redirectRoot(["--module-id", "stub"], "target/contract-reproducibility");
  assert.deepEqual(appended, [
    "--module-id",
    "stub",
    "--root",
    "target/contract-reproducibility",
  ]);

  assert.throws(() => redirectRoot(["--module-id", "stub", "--root"], "x"), /--root with no value/u);
});

test("normaliseText makes comparisons insensitive to checkout line endings", () => {
  assert.equal(normaliseText("a\r\nb\r\n"), normaliseText("a\nb\n"));
  assert.notEqual(normaliseText("a\nb\n"), normaliseText("a\nc\n"));
});

test("the real repository contract is reproducible from the registered command", () => {
  const assessment = assessDatabaseContractReproducibility({ repoRoot });
  assert.equal(assessment.ok, true, formatDatabaseContractReproducibilityReport(assessment));
  assert.deepEqual(assessment.filesChecked, [
    "schema.yaml",
    "prefix-registry.json",
    "table-registry.json",
  ]);
  assert.match(assessment.command, /materialize-database-contract-from-baseline\.mjs/u);
  assert.equal(
    existsSync(path.join(repoRoot, "target/contract-reproducibility")),
    false,
    "the temporary copy must be removed before returning",
  );
});

test("a committed artifact the generator does not produce is rejected", () => {
  // This is the exact 2026-09-22 defect: table-registry.json carried extra fields the generator
  // never emits, so the registered command silently deleted authored data on every run.
  withFixture({ committedRegistry: '{\n  "schemaVersion": 1,\n  "extra": true\n}' }, (fixtureRepo) => {
    const assessment = assessDatabaseContractReproducibility({ repoRoot: fixtureRepo });

    assert.equal(assessment.ok, false);
    assert.equal(assessment.failures.length, 1);
    assert.equal(assessment.failures[0].reason, "non-reproducible-contract");
    assert.equal(assessment.failures[0].file, "database/contract/table-registry.json");
    assert.match(assessment.failures[0].message, /first difference at line 1/u);
    assert.match(formatDatabaseContractReproducibilityReport(assessment), /db:materialize:contract/u);
  });
});

test("the temporary copy is removed even when the artifact is not reproducible", () => {
  withFixture({ committedSchema: "hand-edited: schema" }, (fixtureRepo) => {
    assessDatabaseContractReproducibility({ repoRoot: fixtureRepo });
    assert.equal(existsSync(path.join(fixtureRepo, "target/contract-reproducibility")), false);
  });
});

test("a registered command whose tool is missing fails closed", () => {
  withFixture({ includeGenerator: false }, (fixtureRepo) => {
    const assessment = assessDatabaseContractReproducibility({ repoRoot: fixtureRepo });
    assert.equal(assessment.ok, false);
    assert.equal(assessment.failures[0].reason, "missing-generator");
    assert.equal(assessment.filesChecked.length, 0);
  });
});

test("a missing materialization input is reported instead of silently passing", () => {
  withFixture({ includeManifest: false }, (fixtureRepo) => {
    const assessment = assessDatabaseContractReproducibility({ repoRoot: fixtureRepo });
    assert.equal(assessment.ok, false);
    assert.equal(assessment.failures[0].reason, "materialization-failed");
    assert.match(assessment.failures[0].message, /database\/database\.manifest\.json/u);
  });

  withFixture({ includeBaseline: false }, (fixtureRepo) => {
    const assessment = assessDatabaseContractReproducibility({ repoRoot: fixtureRepo });
    assert.equal(assessment.ok, false);
    assert.equal(assessment.failures[0].reason, "materialization-failed");
    assert.match(assessment.failures[0].message, /ddl\/baseline/u);
  });
});

test("a generated file that the repository does not commit is reported", () => {
  withFixture({}, (fixtureRepo) => {
    rmSync(path.join(fixtureRepo, "database/contract/schema.yaml"));
    const assessment = assessDatabaseContractReproducibility({ repoRoot: fixtureRepo });
    assert.equal(assessment.ok, false);
    assert.equal(assessment.failures[0].reason, "missing-generated-file");
    assert.equal(assessment.failures[0].file, "database/contract/schema.yaml");
  });
});

test("a repository without the registered command cannot claim reproducibility", () => {
  withFixture({ command: undefined }, (fixtureRepo) => {
    rmSync(path.join(fixtureRepo, "package.json"));
    writeFileSync(path.join(fixtureRepo, "package.json"), '{"name":"sdkwork-fixture"}\n', "utf8");
    assert.throws(
      () => assessDatabaseContractReproducibility({ repoRoot: fixtureRepo }),
      /no 'db:materialize:contract' script/u,
    );
  });
});

test("a repository without package.json cannot claim reproducibility", () => {
  withFixture({}, (fixtureRepo) => {
    rmSync(path.join(fixtureRepo, "package.json"));
    assert.throws(
      () => assessDatabaseContractReproducibility({ repoRoot: fixtureRepo }),
      /package\.json is missing/u,
    );
  });
});

test("line-ending differences alone are not reported as drift", () => {
  withFixture({ committedSchema: "materialized: schema" }, (fixtureRepo) => {
    // Rewrite the committed artifacts with CRLF, as a Windows checkout would produce.
    for (const name of ["schema.yaml", "table-registry.json", "prefix-registry.json"]) {
      const file = path.join(fixtureRepo, "database/contract", name);
      writeFileSync(file, readFileSync(file, "utf8").replace(/\n/gu, "\r\n"), "utf8");
    }
    const assessment = assessDatabaseContractReproducibility({ repoRoot: fixtureRepo });
    assert.equal(assessment.ok, true, formatDatabaseContractReproducibilityReport(assessment));
  });
});

test("the CLI exits non-zero on drift and zero on a reproducible contract", () => {
  const cli = path.join(repoRoot, "tools/check-sandbox-database-contract-reproducibility.mjs");
  // Without --root the CLI inspects its own repository, resolved from its own location rather than
  // the working directory, so a fixture must be named explicitly.
  const run = (root) => {
    try {
      const stdout = execFileSync(process.execPath, [cli, "--root", root], { encoding: "utf8" });
      return { status: 0, stdout };
    } catch (error) {
      return { status: error.status, stdout: error.stdout ?? "", stderr: error.stderr ?? "" };
    }
  };

  assert.equal(run(repoRoot).status, 0);
  const ownRepository = execFileSync(process.execPath, [cli], {
    cwd: tmpdir(),
    encoding: "utf8",
  });
  assert.match(ownRepository, /reproduces 3 committed artifact/u);

  withFixture({ committedPrefix: "hand-edited: prefix" }, (fixtureRepo) => {
    const result = run(fixtureRepo);
    assert.equal(result.status, 1);
    assert.match(result.stdout, /non-reproducible-contract/u);
    assert.match(result.stdout, /prefix-registry\.json/u);
  });
});

test("argument parsing accepts --root and --json and rejects anything else", () => {
  assert.deepEqual(parseDatabaseContractReproducibilityArgs([]).json, false);
  assert.equal(parseDatabaseContractReproducibilityArgs(["--json"]).json, true);
  assert.equal(parseDatabaseContractReproducibilityArgs(["--root", "."]).root, repoRoot);
  assert.throws(() => parseDatabaseContractReproducibilityArgs(["--root"]), /requires a directory/u);
  assert.throws(() => parseDatabaseContractReproducibilityArgs(["--strict"]), /unsupported argument/u);
});
