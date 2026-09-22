# sdkwork-sandbox Documentation

Purpose: route product, architecture, engineering, integration, operations, release, and historical documentation for SDKWork Sandbox.

Owner: SDKWork Runtime Platform maintainers.

Allowed content: Canon PRD and technical architecture, stable `REQ-*`/`ADR-*`/`PLAN-*`/`REVIEW-*` working records, guides, runbooks, changelogs, migrations, releases, domain extensions, and archives. Forbidden content: copied global standards, sole-source machine contracts, generated SDK transports, runtime state, credentials, and private environment values.

## Audience Routing

| I am… | Read first | Then read |
| --- | --- | --- |
| Product or business | [product/prd/PRD.md](product/prd/PRD.md) | [product/requirements/](product/requirements/) |
| Architect | [architecture/tech/TECH_ARCHITECTURE.md](architecture/tech/TECH_ARCHITECTURE.md) | [architecture/decisions/](architecture/decisions/) |
| Developer | [guides/developer/README.md](guides/developer/README.md) | [engineering/plans/](engineering/plans/) |
| Operator | [guides/operator/README.md](guides/operator/README.md) | [runbooks/](runbooks/) |
| Integrator | [guides/integrator/README.md](guides/integrator/README.md) | repository `apis/` and `sdks/` |
| Agent | [../AGENTS.md](../AGENTS.md) | [INDEX.yaml](INDEX.yaml) |

## Canon Documents

| Document | Path |
| --- | --- |
| Product PRD | [product/prd/PRD.md](product/prd/PRD.md) |
| Technical architecture | [architecture/tech/TECH_ARCHITECTURE.md](architecture/tech/TECH_ARCHITECTURE.md) |

## Related Specs

- `DOCUMENTATION_SPEC.md`
- `SDKWORK_WORKSPACE_SPEC.md`
- `REQUIREMENTS_SPEC.md`
- `ARCHITECTURE_DECISION_SPEC.md`

## Active Delivery Focus

[PLAN-2026-0001: Local And Firecracker Sandbox Provider Delivery](engineering/plans/PLAN-2026-0001-local-and-firecracker-provider-delivery.md) fixes the Provider delivery order as Local, shared Command/Terminal conformance, then Firecracker. [PLAN-2026-0002: Commercial Cloud Agent Runtime Delivery](engineering/plans/PLAN-2026-0002-commercial-cloud-agent-runtime-delivery.md) extends that sequence through trusted-node scheduling, optional Runtime Pool acceleration, durable Workspace Checkpoint handoff, Kernel integration, operations, and release evidence. [REVIEW-20260731: Sandbox Commercial Readiness Gap Audit](engineering/reviews/REVIEW-20260731-sandbox-commercial-readiness-gap-audit.md) records the current four-repository No-Go decision and the additional missing internal-control-plane, interactive-terminal, Secret-projection, cloud-data-governance, and compatibility gates. REQ-2026-0025 now supplies the value-free Runtime Secret Projection Gate 0 candidate, but no Secret Authority or projection mechanism is approved. REQ-2026-0026 now supplies the Cloud Data Residency/Recovery Gate 0 candidate, but no region, replication, backup or restore mechanism is approved. REQ-2026-0027 now supplies the immutable cross-repository release-set compatibility Gate 0 candidate, but no Release Authority, registry, mixed-version evidence, rollout or rollback mechanism is approved. Docker remains deferred. The Service Host draft Bootstrap/Composition contracts resolve common, Local, Cold Firecracker, Cloud Firecracker, Command/Terminal and optional Pool dependencies with fail-closed status rules. REQ-2026-0021 composes Workspace Revision authorization, allocation, attachment, execution, checkpoint, compensation, sanitization and release across Local and Firecracker lanes; Agents alone promotes Workspace Revisions. REQ-2026-0022 adds the Local-only four-repository data-residency/recovery Gate: `standalone` and Local Provider selection do not prove device locality, and an all-data claim remains unavailable until every declared store, transfer, backup, restore and purge path has real evidence. All REQ-2026-0010 through REQ-2026-0027 runtime gates remain draft and disabled. REQ-2026-0018 still requires the SQL subject migration, REQ-2026-0020 still requires approved lifecycle retention/migration, and no static Gate 0 evidence is a production runtime or commercial claim. The PRD Canon now additionally fixes the runtime execution model (shared execution runtime / namespace sandbox / MicroVM tiers), the sandbox capability surfaces (filesystem, process, PTY, network modes, egress policy, port exposure, in-sandbox agent runtime, MCP, skills, SDK), the capability-alignment matrix with per-row gate status, and the functional, performance, resource and concurrency acceptance criteria with their benchmark and chaos test requirements. Every item listed there remains unauthorized until its own `REQ-*` reaches `ready`; Template, Snapshot, Fork, port exposure, agent runtime, skills, SDK, the lightest execution tier and the benchmark suite currently have no `REQ-*` at all. The Provider and cloud gate backlog is now indexed at [Human Review Sign-Off Backlog](engineering/human-review-signoff-backlog.md), whose contract-gated packet list and status coherence are enforced by `tools/check-sandbox-human-review-signoff.mjs`; that gate refuses a contract-named packet that does not exist, a required reviewer role that is never asked to sign, a pending packet sitting beside an approved outcome or a `ready`/`accepted` requirement, and any `implementationAuthorized` set before sign-off. `tools/check-sandbox-evidence-traceability.mjs` snapshots every evidence id the Gate 0 contracts require and fails on drift in either direction, which makes the number of required-but-unproduced evidence ids explicit rather than assumed; only the two host-precondition ids have a partial producer, and host facts are an input to real-runner conformance, never a substitute for it.

## Verification

```bash
node tools/check-sandbox-cargo-path-dependencies.mjs
node tools/check-sandbox-workspace-dependency-inheritance.mjs
node tools/check-sandbox-doc-integrity.mjs
node tools/check-sandbox-component-contract-alignment.mjs
node tools/check-sandbox-database-contract-reproducibility.mjs
node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root .
node ../sdkwork-specs/tools/check-database-framework-standard.mjs --root .
node ../sdkwork-specs/tools/verify-database-initialization-state.mjs --root .
node tools/check-sandbox-commercial-readiness.mjs
node tools/check-sandbox-evidence-traceability.mjs
node tools/check-sandbox-human-review-signoff.mjs
```

The path-dependency gate runs first because a manifest path dependency with one surplus `..` breaks `cargo metadata` for the whole workspace, which makes `cargo fmt`, `cargo check`, `cargo clippy` and `cargo test` unrunnable while every static validator stays green. The same reasoning applies to the formatting command: use `cargo fmt --check`, never `cargo fmt --all -- --check`, because `--all` also formats local path dependencies owned by sibling repositories.

`check-sandbox-doc-integrity.mjs` closes the other documentation blind spot: `check-repository-docs-standard.mjs` validates document structure and path ownership, never link resolvability or command executability, so fourteen dead relative links and four fenced-code prescriptions naming non-existent `scripts/*-checker.mjs` entrypoints survived every gate until 2026-09-22. The gate exempts point-in-time evidence under `docs/changelogs/`, `docs/engineering/reviews/`, `docs/releases/` and `docs/archive/` from the command rules — a recorded command is a fact about the past, a prescription is an instruction that must run today.

`check-sandbox-component-contract-alignment.mjs` derives its rules from `COMPONENT_SPEC.md` prose instead of copying a spec list, and audits every `component.spec.json` here: a declared language must be backed by authored source, authored source must reference its language and code-style specs, and a `rust-api-assembly` must reference every spec its MUST sentence names and declare `component.surface: "api-assembly"`. `check-rust-manifest-standard.mjs`, `validate-api-assembly.mjs` and `verify-repo.mjs` cover none of these; a phantom `typescript` declaration, an assembly contract without `WEB_BACKEND_SPEC.md` and a repository adapter without `CODE_STYLE_SPEC.md` all passed every gate until 2026-09-22.

`check-sandbox-database-contract-reproducibility.mjs` re-runs the registered `db:materialize:contract` command against a throwaway copy of its inputs and fails when the committed `database/contract/` artifacts differ from what that command produces. `DATABASE_FRAMEWORK_SPEC.md` section 6.2 makes them generated artifacts, and regeneration deletes unknown fields rather than adding them, so a non-reproducible registry means an operator silently loses authored data; `table-registry.json` carried exactly such fields until 2026-09-22. `check-database-framework-standard.mjs` and `verify-database-initialization-state.mjs` validate the contract's shape against the baseline but never re-run the generator.

Commercial release preflight additionally requires `check-sandbox-commercial-readiness.mjs --require-go`, which stays unsuccessful until the repository decision stops being `NO-GO`; a green Gate 0 contract suite is not evidence of a runnable Sandbox product.
