# PostgreSQL Baseline

Purpose: the committed bootstrap anchor for the Sandbox PostgreSQL lifecycle.

Owner: SDKWork Runtime Platform.

Strategy: `baseline-plus-migrations`, declared by `database/database.manifest.json#baselineStrategy`.

The canonical primary baseline is `0001_sandbox_baseline.sql`, named `0001_<moduleId>_baseline.sql` for `moduleId: sandbox` as required by `DATABASE_FRAMEWORK_SPEC.md` section 7.5. It carries the full lifecycle DDL and the `sdkwork:migration` metadata header, including the statement/lock timeout contract the repository enforces per transaction.

The primary baseline is an **immutable bootstrap anchor**. It `MUST NOT` be rewritten to absorb later migrations, and no second `0001_*_baseline.sql` may be added. Any additional `.sql` file placed here `MUST` be a retired stub that retains provenance comments but contains no `CREATE TABLE`.

A fresh install applies this baseline followed by every ordered `database/migrations/postgres/*.up.sql` migration. The baseline alone is therefore **not** the complete active table inventory: the baseline and the ordered migrations jointly define the installed schema, so neither may be rewritten to make the other appear complete. An empty migration tree is valid at initialization state (`DATABASE_FRAMEWORK_SPEC.md` section 5.1) and is not debt by itself.

Authoritative contract: `database/contract/schema.yaml`. `pnpm run db:materialize:contract` materializes that contract from this baseline file, and `node ../sdkwork-specs/tools/verify-database-initialization-state.mjs` verifies the initialization state.
