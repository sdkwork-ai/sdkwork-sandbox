# sdkwork-intelligence-sandbox-repository-sqlx

PostgreSQL L4 adapter for the Service-owned `SandboxSessionRepository` port. Lifecycle Operation order is stored explicitly through zero-based `sandbox_operation_sequence`; timestamps are not replay authority.

## Public API

The crate exports `SqlxSandboxSessionRepository`, `SdkworkUtilsSandboxProviderAllocationProtector`, `SandboxProviderAllocationKeySource`, and the redacted `SandboxProviderAllocationKey` secret carrier. It does not export database records as Domain/API projections.

## Required SDK Surface

None. This component consumes Rust ports and SDKWork database/utilities crates only.

## Configuration

Composition injects an already constructed PostgreSQL `sdkwork_database_sqlx::DatabasePool` and a Secret-backed key source. The adapter does not construct pools, read environment variables, run migrations, or discover keys from ordinary config.

## Deployment Profile And Runtime Target Behavior

The adapter is valid only for PostgreSQL `authoritative-server` and `server-test` profiles. Memory persistence remains test/candidate-only; SQLite is rejected and is not a Server fallback.

## Security

`SandboxProviderAllocationRef` is encrypted with context-bound HKDF-SHA256 plus AES-256-GCM through `sdkwork-utils-rust`. Ciphertext Debug output is redacted. Key material, invalid constructor inputs, and derived AES keys use zeroizing carriers. Key IDs accept only 1..=128-byte printable ASCII and are independently constrained by the PostgreSQL migration. Stable Operation sequence and Snapshot invariant validation reject corrupted lifecycle combinations before decryption. Key material must originate from the future Service Host Secret Port.

## Extension Points

Implement `SandboxProviderAllocationKeySource` for an approved Secret/KMS adapter. Rotation retains old key ID/version lookup until every active Runtime Binding has been re-encrypted or destroyed. The current port is synchronous: production composition must inject a short-lived locally resolved key handle with refresh outside Tokio worker execution, or obtain human approval for an async-port evolution before integrating a remote KMS.

## Verification

```bash
cargo test -p sdkwork-intelligence-sandbox-repository-sqlx
cargo clippy -p sdkwork-intelligence-sandbox-repository-sqlx --all-targets -- -D warnings
node ../sdkwork-specs/tools/check-database-framework-standard.mjs --root .
```

Live PostgreSQL verification is explicit and remains ignored in the default suite:

```bash
node tools/testing/sandbox-postgres-evidence.mjs --postgres-major 17

# Equivalent manual flow:
SDKWORK_DATABASE_URL=<server-test-url> cargo run --manifest-path ../sdkwork-database/Cargo.toml --locked -p sdkwork-database-cli -- --app-root . init
SDKWORK_DATABASE_URL=<server-test-url> SDKWORK_DATABASE_TEST_POSTGRES_URL=<same-server-test-url> cargo test -p sdkwork-intelligence-sandbox-repository-sqlx --test postgres_repository --locked -- --ignored --nocapture
```

The pre-provisioned database and same-named schema must use a canonical `sdkwork_ai_test_<run_id>` identity with the `sdkwork_ai_test` role. Initialize the empty schema only through `sdkwork-database-cli`; do not apply the SQL asset through a repository-local migration runner. `SDKWORK_DATABASE_URL` remains the sole `PoolBuilder` connection input, while `SDKWORK_DATABASE_TEST_POSTGRES_URL` is a destructive-test safety latch. The test requires both values to match exactly before constructing a pool or running its initial `TRUNCATE`, does not echo either value on mismatch, and cannot silently target the default development database.
