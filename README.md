# SDKWork Sandbox

repository-kind: application

SDKWork Sandbox is the execution-environment application for SDKWork agents. It provides the product and architecture boundary for local and remote Runtime, `SandboxSession`, Workspace Attachment, Sandbox Provider, resource policy, terminal access, snapshots, events, and operational telemetry. `sdkwork-agents` remains the authority for `AgentWorkspace` and `AgentSession`; Kernel maps authorized IDs into `SandboxWorkspaceId` and `SandboxSessionId`. The repository contains Provider-neutral lifecycle, Memory Repository, PostgreSQL Repository, encrypted Provider recovery metadata, Lease/Fencing, and transient reconciliation candidates. No production Sandbox Provider, Service Host composition, Runtime API, or release deployment is implemented yet.

## Status

- Lifecycle, PostgreSQL persistence, and allocation key rotation: verified candidates including live PostgreSQL migration, concurrency, recovery, and re-encryption evidence; pending human architecture/security review and production operations gates
- Application code: `sandbox`
- Primary language: Rust
- Primary app surface: repository root (planned CLI and service host)
- Current phase: V1 lifecycle core after accepted Phase 0 foundation

## Documentation

- [Documentation index](docs/README.md)
- [Product PRD](docs/product/prd/PRD.md)
- [Technical architecture](docs/architecture/tech/TECH_ARCHITECTURE.md)
- [Application surfaces](apps/README.md)
- [Repository contracts](specs/README.md)

## Active Layout

The complete SDKWork top-level directory dictionary is initialized so capability ownership is explicit. `crates/`, `apis/`, `database/`, `docs/`, `specs/`, and `tests/` are active. Provider SPI, lifecycle service, non-production Memory Repository, PostgreSQL Repository candidate, bounded allocation key re-encryption, authoritative-server database assets, draft Sandbox event/command contracts, Local/Firecracker Provider gates, Local Host, Host Broker, Firecracker Artifact, Workspace Block Device, Network/Resource Isolation, Multi-tenant Scheduling, Node Trust, Quota/Capacity Persistence, Runtime Pool, Lifecycle Hot State/Idempotency, Workspace Runtime Transaction, Standalone Data Residency/Recovery, Internal Control Plane, Interactive Terminal, Runtime Secret Projection, Cloud Data Residency/Recovery, Cross-Repository Version Compatibility, and Service Host Bootstrap/Profile/Capability Gate 0 contracts are present. REQ-2026-0021 composes allocation, attachment, command, durable checkpoint handoff, compensation, sanitization and release across Local and Firecracker lanes while Agents remains Revision authority. REQ-2026-0022 separately governs an all-data Local claim across BirdCoder, Agents, Kernel, Sandbox, Workspace, database, cache, log, secret, backup and purge authorities; neither `standalone` nor Local Provider selection proves device locality. REQ-2026-0025 keeps Secret values outside BirdCoder/Agents/Kernel/Sandbox control-plane contracts and separates Local device authority from region-bound Cloud authority; REQ-2026-0026 keeps Cloud residency/recovery claims tied to explicit region/storage/replication/restore evidence; REQ-2026-0027 keeps four-repository release identity and multi-dimensional compatibility explicit; none of these gates authorizes runtime mechanisms. Service Host now resolves 18 fail-closed Gate dependencies: Workspace Runtime Transaction is common to all lanes, while Standalone Data Residency applies only to `sandbox_standalone_local`. REQ-2026-0018 still blocks quota/capacity persistence on the `tenant_id TEXT` to positive `BIGINT` migration, and REQ-2026-0020 still blocks lifecycle history retention and migration policy; until it is approved, repository reads are bounded by the `MAX_SANDBOX_SESSION_OPERATIONS` safety bound and fail closed above it. All new runtime mechanisms, cross-repository integration, API/SDK, storage/KMS, production profiles and deployment remain inactive and unauthorized. `sdks/`, `jobs/`, `tools/`, `plugins/`, `examples/`, `etc/`, `deployments/`, and `scripts/` remain inactive until their owning requirement is ready.

Rust components live under `crates/`. The root is the primary application surface, so `apps/README.md` indexes the root and records that no secondary client surface exists yet. API contracts will be authored under `apis/` before route, SDK, or gateway implementation begins.

## Workspace

This repository is independently buildable from its own root and does not depend on `sdkwork-kernel` or `sdkwork-agents`. The cross-repository direction is `sdkwork-agents -> sdkwork-kernel -> sdkwork-sandbox`; sibling Sandbox dependencies are declared once in the Kernel root `Cargo.toml`, and Kernel member crates consume them through Cargo workspace dependencies.

## Verification

```bash
node tools/check-sandbox-cargo-path-dependencies.mjs
node tools/check-sandbox-workspace-dependency-inheritance.mjs
node tools/check-sandbox-doc-integrity.mjs
node tools/check-sandbox-component-contract-alignment.mjs
node tools/check-sandbox-requirement-traceability.mjs
node tools/check-sandbox-e2b-parity-matrix.mjs
node ../sdkwork-specs/tools/check-workspace-path-portability.mjs --root .
node ../sdkwork-specs/tools/check-shell-portability.mjs --root .
node tools/check-sandbox-platform-code.mjs
node tools/check-sandbox-database-contract-reproducibility.mjs
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node --test tests/contract/*.test.mjs
node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root .
node ../sdkwork-specs/tools/check-workspace-packages-layout.mjs --root . --mode enforce
node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root . --strict
node ../sdkwork-specs/tools/audit-repository-baseline.mjs --root .
node tools/check-sandbox-evidence-traceability.mjs
node tools/check-sandbox-human-review-signoff.mjs
node tools/check-sandbox-commercial-readiness.mjs
```

`pnpm run check` runs this chain; the individual gates are also exposed as `pnpm run cargo:paths:check`, `cargo:deps:check`, `docs:integrity:check`, `portability:paths:check`, `portability:shell:check`, `portability:platform:check`, `component:contracts:check`, `specs:port-bindings:check`, `requirement:traceability:check`, `parity:matrix:check`, `rust:fmt:check`, `gate0:evidence:check`, `gate0:signoff:check` and `gate0:readiness:check`.

Use `cargo fmt --check`, never `cargo fmt --all -- --check`. `--all` also formats local path dependencies, so it reports and writes formatting diffs owned by sibling repositories such as `sdkwork-web-framework`, `sdkwork-utils` and `sdkwork-database`, which this repository must not edit. `cargo fmt --check` covers every Sandbox workspace member and leaves sibling repositories alone. Run it through `pnpm run rust:fmt:check`.

`node tools/check-sandbox-cargo-path-dependencies.mjs` runs first because a single surplus `..` in a manifest path dependency breaks `cargo metadata` for the whole workspace, which makes every other cargo command unrunnable; the static gate reports the offending manifest, line and resolved path instead.

`node tools/check-sandbox-workspace-dependency-inheritance.mjs` enforces `RUST_CODE_SPEC.md` section 14 and `NAMING_SPEC.md` section 3.2 rule 6: a third-party version belongs in the root `[workspace.dependencies]` table and every member inherits it with `workspace = true`. `check-rust-manifest-standard.mjs` does not cover this, so a member-local `axum = "0.8"` passes there.

`node tools/check-sandbox-doc-integrity.mjs` checks that every relative markdown link resolves and that every command in a live document's fenced code blocks is runnable as written. `check-repository-docs-standard.mjs` validates document structure and path ownership, not link resolvability or command executability, so fourteen dead links and four prescriptions naming non-existent `scripts/*-checker.mjs` entrypoints survived every gate until 2026-09-22. A `node <path>.mjs` target must resolve from the document's own directory, the repository root or the checkout root; the document's *parent* is not a candidate, because `crates/sdkwork-intelligence-sandbox-repository-sqlx/README.md` prescribed `node ../../sdkwork-specs/tools/check-database-framework-standard.mjs --root ../..`, which resolved only from `crates/` — where `--root ../..` names the checkout root, so the tool reported `Database framework standard skipped (no database/ directory)` and exited 0. The gate accepts a path that resolves from an unintended directory and never requires the command to pass, so a prescription that silently checks nothing reads exactly like one that works. Point-in-time evidence records under `docs/changelogs/`, `docs/engineering/reviews/`, `docs/releases/` and `docs/archive/` are exempt from the command rules: a recorded command is a fact about the past, a prescription is an instruction that must work today.

`node tools/check-sandbox-component-contract-alignment.mjs` derives its rules from `COMPONENT_SPEC.md` prose rather than copying a spec list, and checks that `component.languages` is backed by authored source, that authored source references its language and code-style specs, and that a `rust-api-assembly` references every spec its MUST sentence names and declares `component.surface: "api-assembly"`. No other validator covers these invariants: a root contract declaring `typescript` with no TypeScript source, an assembly contract missing `WEB_BACKEND_SPEC.md`, and a repository adapter missing `CODE_STYLE_SPEC.md` all passed every gate until 2026-09-22.

`pnpm run specs:port-bindings:check` runs the strict form of `check-component-port-bindings.mjs`. Under `--strict` that tool requires `contracts.layerRole` on every component spec that declares authored source (`APPLICATION_LAYERED_ARCHITECTURE_SPEC.md` section 2 makes it a MUST for new composable modules), and the repository root `specs/component.spec.json` was added on 2026-08-30 without it. All eight crate-level component specs declare their role correctly, so the file that violated the rule was the one describing the whole application root. Nothing caught it because no CI exists, this repository's own `_sdkwork:check` chain did not run the tool, and `check-sandbox-doc-integrity.mjs` only requires a prescribed command to be *runnable*, never to *pass*: `--strict` was documented in the Gate 0 command set, the developer guide, the delivery plan and several requirement verification blocks while silently failing. Review packets dated 2026-07-28 and 2026-07-31 record it as PASS, and that is consistent — the root component spec did not exist yet.

`node tools/check-sandbox-database-contract-reproducibility.mjs` re-runs the `db:materialize:contract` command against a throwaway copy of its inputs and fails when the committed `database/contract/` artifacts are not what that command produces. `DATABASE_FRAMEWORK_SPEC.md` section 6.2 makes those three files generated artifacts, and regeneration *deletes* unknown fields rather than adding them, so a non-reproducible registry means an operator silently loses authored data on every run; the committed `table-registry.json` carried exactly such fields until 2026-09-22. `check-database-framework-standard.mjs` and `verify-database-initialization-state.mjs` validate the contract's shape against the baseline but never re-run the generator.

`node tools/check-sandbox-requirement-traceability.mjs` enforces the traceability chain `DOCUMENTATION_SPEC.md` section 28 requires and the id-resolution rule `REQUIREMENTS_SPEC.md` section 6 states: every `REQ-*` and `ADR-*` token in a live document resolves to a record, or is qualified on the same line by a sibling-repository path; no requirement or decision record is cited by nothing but itself; and every row of the `PRD-capabilities.md` section 11 matrix is classified as carried by a `REQ-*`, marked `无`, or inheriting the row above. It also prints the capability census, so this repository's answer to `PRD.md` section 6's "能力对齐完整性" metric is one command instead of a hand diff of two tables. Bare cross-repository ids are the case it exists for. The delivery plan cited `sdkwork-kernel/docs/product/requirements/REQ-2026-0002-distributed-execution-placement-control-plane.md` and `sdkwork-birdcoder/docs/product/requirements/REQ-2026-0006-hybrid-local-cloud-agent-execution.md` as bare `REQ-2026-0002` and `REQ-2026-0006`, and both numbers also name this repository's own lifecycle-core and key-rotation requirements, so the references looked resolvable while pointing at the wrong records. No validator under `../sdkwork-specs/tools/` covered any of this.

Commercial release preflight must additionally run `node tools/check-sandbox-commercial-readiness.mjs --require-go`. It intentionally fails while the repository decision remains `NO-GO`; a green Gate 0 contract suite is not evidence of a runnable Sandbox product.

`node tools/check-sandbox-e2b-parity-matrix.mjs` keeps the E2B capability-parity matrix (`docs/architecture/tech/TECH-e2b-capability-parity.md`) honest: the four status markers must be the declared vocabulary, matrix rows must be numbered `1..N` with five cells each, the census categories must correspond in order to the matrix subsections, and the per-category counts, the declared total row and the recomputed totals must all agree. Markdown emphasis is stripped before parsing, because `| **合计** | **78** | ... |` left unstripped parsed as no total row at all — the gate then skipped every total-row assertion and printed the recomputed sum as though it had verified the declared one. The same run resolves every `REQ-*`/`ADR-*` token in the document and requires it to be linked from `TECH_ARCHITECTURE.md` and the tech README and registered in `docs/INDEX.yaml`. Current reading: 17 categories, 78 rows, census `✅ 0 | 🟡 16 | ❌ 60 | ⛔ 2`.

`node tools/check-sandbox-platform-code.mjs` keeps `docs/architecture/tech/TECH-platform-support.md` — which platforms can actually carry this repository's control plane and its execution environment, and on what evidence — machine-checked. It covers the one platform invariant the two upstream gates do not: `check-workspace-path-portability.mjs` checks machine-specific absolute paths and `check-shell-portability.mjs` checks shell syntax, but nothing inspected `#[cfg(windows)]`, `std::process::Command`, `libc::` or `std::path::MAIN_SEPARATOR`, so a runtime backend could have become single-platform with every gate still green. It also closed a wiring gap of its own: both upstream gates passed against this repository on 2026-09-22 but appeared in no `package.json` script, so `DEPENDENCY_MANAGEMENT_SPEC.md` section 1's cross-platform requirement had been satisfied by accident rather than by execution. The gate requires every matrix row to cite a resolving repository path or a capability id from the probe vocabulary (imported, not restated), every platform marker in `crates/*/src` to be declared with its platform and reason, and every gate the document names to be wired in `package.json`. Current reading: 5 platforms, 0 platform-conditional markers.

Live PostgreSQL 16/17 migration, repository, encryption and backup/restore evidence is available through the disposable loopback-only runner:

```bash
node tools/testing/sandbox-postgres-evidence.mjs --postgres-major 16
node tools/testing/sandbox-postgres-evidence.mjs --postgres-major 17
```

Global standards remain authoritative under `../sdkwork-specs/`; this repository links to them and does not copy their bodies.
