# REVIEW-20260728: Sandbox PostgreSQL Persistence Verification

Status: conditional-pass

Requirement: [REQ-2026-0005](../../product/requirements/REQ-2026-0005-durable-sandbox-session-repository-and-reconciliation.md)

Decision: [ADR-20260728-postgresql-sandbox-lifecycle-persistence-and-reconciliation](../../architecture/decisions/ADR-20260728-postgresql-sandbox-lifecycle-persistence-and-reconciliation.md)

Owner: SDKWork Runtime Platform

Date: 2026-07-28

Updated: 2026-07-30

## Scope

本 Review 验证 PostgreSQL-authoritative `SandboxSession` Persistence、受保护的 `SandboxRuntimeBinding` Recovery Metadata、Tenant-scoped Lease/Fencing、有界 Provider Operation Timeout 与 `Starting`/`Stopping`/`Destroying` Reconciler 候选实现。固定产品术语继续使用 `Runtime`、`Session`、`Workspace`、`Sandbox` 与 `Provider`；实现类型使用 `Sandbox*`，存在歧义的字段/变量使用 `sandbox_*`，共享 `TenantId`、`OperationId`、`RuntimeCapability` 与 `IsolationAssurance` 保持 SDKWork Canon。

## Environment

- Windows host with Docker Engine `28.0.4`.
- Ephemeral `postgres:16-alpine`, image digest `postgres@sha256:20edbde7749f822887a1a022ad526fde0a47d6b2be9a8364433605cf65099416`.
- Ephemeral `postgres:17-alpine`, image digest `postgres@sha256:742f40ea20b9ff2ff31db5458d127452988a2164df9e17441e191f3b72252193`.
- Each run used an internally generated canonical `sdkwork_ai_test_<run_id>` database and same-named schema with role `sdkwork_ai_test` and a Docker-selected loopback-only host port.
- Database initialized only through `sdkwork-database-cli`; Repository tests constructed the pool through `sdkwork-database-sqlx::PoolBuilder` from the explicit test URL.
- The exact temporary container and all generated test databases/dumps were deleted after verification.

## Evidence

| Command / Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` and `cargo fmt --package <each Sandbox workspace member> -- --check` | PASS for the full workspace and all 7 Sandbox crates on the final 2026-07-30 verification baseline. |
| `cargo test --workspace --locked` | PASS: 44 Rust tests, including the destructive-test URL guard; the explicit live PostgreSQL test remains intentionally ignored in the default suite because it requires an external database. |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS. |
| `node --test tests/contract/database-framework.contract.test.mjs` | PASS: 4 database contract tests, including stable Tenant+Session Operation Sequence constraints. |
| `node --test tests/contract/postgres-evidence-tool.contract.test.mjs` | PASS: 6 tests cover the supported 16/17 matrix, canonical disposable identity, loopback-only binding, count/plaintext validation, URL redaction and argument-array cleanup scope. |
| `cargo run --manifest-path ../sdkwork-database/Cargo.toml --locked -p sdkwork-database-cli -- --app-root . init` against an empty database | PASS: 1 migration applied. Immediate second init applied 0 migrations. |
| `cargo run --manifest-path ../sdkwork-database/Cargo.toml --locked -p sdkwork-database-cli -- --app-root . status` | PASS: `module=sandbox engine=postgres status=clean pending_migrations=0`. |
| `cargo run --manifest-path ../sdkwork-database/Cargo.toml --locked -p sdkwork-database-cli -- --app-root . drift-check` | PASS: `drift check passed`. |
| `node tools/testing/sandbox-postgres-evidence.mjs --postgres-major 16` and `--postgres-major 17` | PASS on both supported majors: exact pre-pool URL equality, migration `1/0`, clean status, zero drift and 1 live Repository test. The runner did not mutate process-global environment and removed each exact disposable container. |
| Persisted invariant tamper regression | PASS: `Destroyed` with Binding, `Starting` without InProgress Start, `Starting` without Binding Intent, and `Failed` with unmatched Failure all returned `InvalidStoredData`; Capture and Restore apply the same fail-closed validation, and the legal record was restored unchanged. |
| Exact reconciliation boundary | PASS: page sizes `0/201` return `InvalidPageRequest`; a final page exactly equal to `page_size` has no continuation when no successor exists. |
| Start Intent ordering regression | PASS: no Provider Allocate occurs before `Starting`、In-progress Start Operation and no-allocation Binding Intent are durably saved; Retry Start clears an old Allocation while the aggregate remains in its stable state. |
| Allocation persistence failure injection | PASS: after Allocate succeeded and Allocation Save failed, Provider cleanup ran, persistence retained recoverable `Starting` plus no-allocation Intent, and Reconciler acquired fencing token `2`, reallocated, and started only `allocation-2`. |
| Lease authority failure injection | PASS: Renew failure, Save `LeaseConflict`, and successful-business Release failure return `SandboxLifecycleError::LeaseLost`; an existing Provider failure remains authoritative when Release also fails. |
| Stale reconciliation candidate | PASS: after Lease acquisition the Service reloads the authoritative Session; a candidate already advanced to `Running` causes no Allocate/Start side effect and returns the current stable state. |
| Fencing token saturation | PASS: Memory and live PostgreSQL return `LeaseConflict` at `9223372036854775807`; neither wraps nor reports temporary Lease unavailability. |
| PostgreSQL backup/restore using `pg_dump --format=custom` and `pg_restore` | PASS on PostgreSQL 16 and 17: restored Session/Operation/Binding/Lease counts were `11/20/9/11`; plaintext Allocation Reference matches were `0`. |
| Database, Component, Layering, Identity Naming, Documentation, Packages Layout, and Repository Baseline validators | PASS. |
| Kernel `cargo fmt --manifest-path sdkwork-agent-kernel/Cargo.toml -- --check` and `cargo check --offline -p sdkwork-agent-kernel` | PASS. The Kernel lock change only adds Tokio to the Sandbox Service dependency entry. |
| Kernel `cargo test --offline -p sdkwork-agent-kernel` | PASS: 160 library tests plus all component contract tests; 2 doc tests passed and 1 remained intentionally ignored. |
| Kernel `cargo clippy --offline -p sdkwork-agent-kernel --all-targets -- -D warnings` | PASS. |
| Agents `cargo check --locked -p sdkwork-intelligence-agents-service` | PASS after the reviewed lock refresh; the Agents lock change only adds Tokio to the Sandbox Service dependency entry. |
| Agents `cargo test --locked -p sdkwork-intelligence-agents-service` | PASS: 287 tests; 5 live PostgreSQL tests remained intentionally ignored because `SDKWORK_DATABASE_TEST_POSTGRES_URL` was not provided. |
| Agents `cargo tree --offline -p sdkwork-intelligence-agents-service -i sdkwork-intelligence-sandbox-service` | PASS: confirms `sdkwork-intelligence-agents-service -> sdkwork-agent-kernel -> sdkwork-intelligence-sandbox-service`. |

The live integration test verifies empty-schema materialization, stable zero-based Operation ordering, aggregate round-trip, persisted State/Failure/Binding invariant rejection before decryption, Tenant denial, Operation conflict, Version CAS, encrypted Allocation at rest, simultaneous Lease competition, expiry takeover, monotonic and saturation-safe `SandboxFencingToken`, stale Lease Renew/Release/Save denial, bounded reconciliation query-plan execution, and recovery through a newly constructed Repository/Service instance.

## Security And Reliability Findings

- PostgreSQL stores Ciphertext, Key ID, Key Version, and Crypto Version; the Provider Allocation plaintext did not appear in restored database rows.
- Each Allocate/Start/Stop/Destroy renews `SandboxSessionLease` first, carries the current `sandbox_fencing_token`, and is bounded by a timeout no greater than half the Lease duration. Post-acquisition Renew/Save-Lease/Release failures close as `LeaseLost`; an existing Provider/Readiness failure is not overwritten by a concurrent Release failure.
- Memory and PostgreSQL repositories enforce the same Tenant, CAS, Lease, Fencing, and token-saturation behavior; Memory remains test-only and is not a production fallback.
- `sandbox_operation_sequence` is the lifecycle replay authority; transaction timestamps and random Operation IDs are not used to infer Aggregate order. Snapshot Capture and Restore validate the replayed state and Binding/Allocation matrix before encryption or decryption, including the rule that every persisted `Starting` owns a recoverable Binding Intent.
- Retry Start never persists `Starting` with an old Allocation. It destroys the old Allocation while the aggregate is stable, then atomically persists the new no-allocation Intent; cleanup failure records Typed `Failed(Cleanup)` while retaining the old Binding for explicit recovery.
- Reconciler scans only explicit Tenant scope with page size `1..=200`, uses a Tenant-partitioned ordered Memory index or PostgreSQL keyset query, and returns continuation only after a bounded successor probe; active Lease ownership produces `SandboxLifecycleError::LeaseUnavailable` without Provider effects. After acquisition it reloads the authoritative Session and never calls a Provider from the pre-Lease candidate snapshot.
- Kernel maps `SandboxLifecycleError::LeaseUnavailable` and `SandboxLifecycleError::LeaseLost` to retryable Runtime conflicts. Repository unavailability remains retryable but internal; persisted-data, protection, and engine-integrity failures remain non-retryable internal errors without leaking storage or cryptographic detail.

## Remaining Gates

- Human architecture/security review is still required for public naming, database ownership, encryption/key lifecycle, Provider Fencing contract, and the cross-repository Kernel/Agents integration.
- Real Local and Firecracker Sandbox Providers must persist and reject stale Fencing Tokens; the current Fake Provider proves request propagation only, and Docker is deferred.
- Service Host Secret/KMS wiring, key rotation/re-encryption, multi-replica soak, failover, load/SLO, PITR/retention, production backup automation, monitoring/alerting, and release rollback evidence are not delivered by this requirement.
- HTTP/internal API, generated SDK, IAM authorization, quota/metering, events/outbox/audit, deployable profiles, and commercial operations remain separate Ready Requirements.

## Conclusion

`REQ-2026-0005` passes its defined repository-local PostgreSQL persistence/recovery scope and is accepted at that scope, including reproducible live PostgreSQL 16/17 evidence. This acceptance supports further integration but does not approve a production Provider, production deployment, or commercial release.
