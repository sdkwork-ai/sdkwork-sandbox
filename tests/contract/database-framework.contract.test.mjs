import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { validateDatabaseFramework } from "../../../sdkwork-specs/tools/check-database-framework-standard.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const engine = "postgres";

const manifest = JSON.parse(
  readFileSync(path.join(repoRoot, "database/database.manifest.json"), "utf8"),
);

const baselineDirectory = path.join(repoRoot, `database/ddl/baseline/${engine}`);
const migrationDirectory = path.join(repoRoot, `database/migrations/${engine}`);

/** The single bootstrap anchor required by DATABASE_FRAMEWORK_SPEC section 7.5. */
const canonicalBaselineName = `0001_${manifest.moduleId}_baseline.sql`;

/**
 * The effective installed schema, not merely the bootstrap anchor.
 *
 * DATABASE_FRAMEWORK_SPEC section 7.5 states that a fresh install applies the baseline
 * followed by every ordered migration, so "the baseline alone is not the complete active
 * table inventory". Asserting only against the baseline would therefore stop covering the
 * real installed schema the moment the first post-baseline migration lands, and asserting
 * only against `migrations/postgres/*.up.sql` would cover nothing at initialization state,
 * where section 5.1 permits that tree to be empty. This helper reads both, in apply order.
 */
function readEffectiveInstalledSchema() {
  const baselinePath = path.join(baselineDirectory, canonicalBaselineName);
  assert.ok(
    existsSync(baselinePath),
    `the ${manifest.baselineStrategy} manifest requires the canonical baseline at database/ddl/baseline/${engine}/${canonicalBaselineName}`,
  );

  const orderedMigrations = existsSync(migrationDirectory)
    ? readdirSync(migrationDirectory)
        .filter((name) => name.endsWith(".up.sql"))
        .sort((left, right) => left.localeCompare(right, "en"))
    : [];

  return [baselinePath, ...orderedMigrations.map((name) => path.join(migrationDirectory, name))]
    .map((file) => readFileSync(file, "utf8"))
    .join("\n");
}

test("canonical database framework validator accepts the Sandbox contract", () => {
  const result = validateDatabaseFramework(repoRoot);

  assert.equal(result.skipped, false);
  assert.deepEqual(result.failures, []);
  assert.equal(result.ok, true);
});

test("Sandbox database is PostgreSQL authoritative-server only", () => {
  const manifest = JSON.parse(
    readFileSync(path.join(repoRoot, "database/database.manifest.json"), "utf8"),
  );

  assert.equal(manifest.schemaVersion, 2);
  assert.equal(manifest.databaseRole, "authoritative-server");
  assert.deepEqual(manifest.engines, ["postgres"]);
  assert.equal(manifest.defaultEngine, "postgres");
  assert.equal(manifest.tablePrefix, "sandbox_");
  assert.equal(manifest.lifecycle.autoMigrate, false);
});

test("Sandbox lifecycle authority registers the exact four owned tables", () => {
  const registry = JSON.parse(
    readFileSync(path.join(repoRoot, "database/contract/table-registry.json"), "utf8"),
  );
  const registeredTables = registry.tables.map((entry) => entry.table_name);

  assert.deepEqual(registeredTables, [
    "sandbox_session",
    "sandbox_session_operation",
    "sandbox_runtime_binding",
    "sandbox_session_lease",
  ]);
  assert.ok(registry.tables.every((entry) => entry.system_of_record === true));
});

/**
 * Extract one table's DDL block so assertions are table-scoped.
 *
 * The lifecycle tables deliberately repeat constraint shapes — `sandbox_session`,
 * `sandbox_session_lease` and the operation/binding foreign keys all carry
 * `(tenant_id, sandbox_session_id)`. A schema-wide regex therefore keeps matching after
 * the constraint is dropped from the table under test, which makes the assertion look
 * like coverage while proving nothing. Scoping each assertion to its own block is what
 * makes the test fail when a specific table regresses.
 */
function readTableDefinition(schema, tableName) {
  const match = schema.match(
    new RegExp(`CREATE TABLE(?: IF NOT EXISTS)? ${tableName} \\(([\\s\\S]*?)\\n\\);`, "u"),
  );
  assert.ok(match, `the effective installed schema must define table ${tableName}`);
  return match[1];
}

test("the effective installed schema preserves tenant-leading identity, operation order, CAS, encryption, and fencing constraints", () => {
  const schema = readEffectiveInstalledSchema();

  const session = readTableDefinition(schema, "sandbox_session");
  assert.match(session, /PRIMARY KEY \(tenant_id, sandbox_session_id\)/u);
  assert.match(session, /\bversion BIGINT NOT NULL\b/u);

  const operation = readTableDefinition(schema, "sandbox_session_operation");
  assert.match(operation, /PRIMARY KEY \(tenant_id, sandbox_operation_id\)/u);
  assert.match(
    operation,
    /UNIQUE \(\s*tenant_id, sandbox_session_id, sandbox_operation_sequence\s*\)/u,
  );
  assert.match(operation, /CHECK \(sandbox_operation_sequence >= 0\)/u);

  const binding = readTableDefinition(schema, "sandbox_runtime_binding");
  assert.match(binding, /sandbox_allocation_ciphertext TEXT/u);
  assert.match(binding, /sandbox_allocation_key_version BIGINT/u);
  assert.ok(binding.includes("sandbox_allocation_key_id ~ '^[!-~]+$'"));

  const lease = readTableDefinition(schema, "sandbox_session_lease");
  assert.match(lease, /sandbox_fencing_token BIGINT NOT NULL DEFAULT 0/u);

  assert.doesNotMatch(schema, /CREATE TABLE\s+(?:agent_workspace|agent_session)\b/iu);
  assert.doesNotMatch(schema, /\bsqlite\b/iu);
});

test("the initialization state commits exactly one canonically named bootstrap baseline", () => {
  // DATABASE_FRAMEWORK_SPEC section 7.5 limits initialization-state debt to `competing-baseline`:
  // more than one `0001_*_baseline.sql`, a primary baseline whose name is not
  // `0001_<moduleId>_baseline.sql`, or a baseline supplement that is not a retired stub.
  const baselineFiles = readdirSync(baselineDirectory)
    .filter((name) => name.endsWith(".sql"))
    .sort();
  const primaryBaselines = baselineFiles.filter((name) => /^0001_.*_baseline\.sql$/iu.test(name));

  assert.deepEqual(
    primaryBaselines,
    [canonicalBaselineName],
    `ddl/baseline/${engine} must commit exactly one primary baseline named ${canonicalBaselineName}`,
  );

  for (const supplement of baselineFiles.filter((name) => !primaryBaselines.includes(name))) {
    // A non-primary baseline may only exist as a retired stub: provenance comments are allowed,
    // a second competing definition of an owned table is not.
    const stub = readFileSync(path.join(baselineDirectory, supplement), "utf8");
    assert.doesNotMatch(
      stub,
      /CREATE TABLE/iu,
      `${supplement} is a baseline supplement and must be a retired stub without CREATE TABLE`,
    );
  }

  // Section 7.5 also forbids classifying an ordered migration as debt on its own: the tree may be
  // empty at initialization state and must not be forced non-empty to satisfy a test.
  const declaredStrategy = manifest.baselineStrategy;
  assert.ok(
    ["migrations-only", "baseline-plus-migrations", "baseline-only-dev"].includes(declaredStrategy),
    `manifest baselineStrategy must be a declared strategy, found '${declaredStrategy}'`,
  );
  if (declaredStrategy === "migrations-only") {
    assert.ok(
      readdirSync(migrationDirectory).some((name) => name.endsWith(".up.sql")),
      "a migrations-only root must provide at least one ordered .up.sql migration",
    );
  }
});
