# Repository Guidelines

## SDKWORK Soul

Read `../sdkwork-specs/SOUL.md` before executing tasks in this repository. Follow specs before memory, dictionary before context, stop on ambiguity, and evidence before completion.

## SDKWORK Standards

Canonical standards are rooted at `../sdkwork-specs/README.md`. This repository must reference global standards by relative path and must not copy `*_SPEC.md` bodies locally.

## Application Identity

The application code is `sandbox`, and the repository is `sdkwork-sandbox`. The repository is in Phase 0 and does not yet declare `sdkwork.app.config.json`; create that manifest only when registration, packaging, or deployment enters scope, then validate it against `../sdkwork-specs/APP_MANIFEST_SPEC.md`.

## Local Dictionary Structure

- `AGENTS.md`: repository agent entrypoint.
- `CLAUDE.md`, `GEMINI.md`, `CODEX.md`: compatibility shims pointing here.
- `.sdkwork/`: source-controlled repository skills, plugins, and local workspace metadata.
- `specs/`: repository-wide machine contracts; it must not copy global standards.
- `crates/*/specs/component.spec.json`: component-owned machine contracts.
- `docs/`: PRD, technical architecture, requirements, ADRs, guides, and evidence.
- `apis/`: future author-owned API contracts; generated SDK output does not belong here.
- `sdks/`: future SDK family workspaces and generated SDK ownership.
- `etc/`: future safe source runtime profiles; secrets and local values are forbidden.

## Documentation Canon

- [Documentation index](docs/README.md)
- [Product PRD](docs/product/prd/PRD.md)
- [Technical architecture](docs/architecture/tech/TECH_ARCHITECTURE.md)

## Spec Resolution Order

1. Read this file and any nearer component `AGENTS.md` if later introduced.
2. Read `sdkwork.app.config.json` only when application identity, runtime configuration, packaging, release, or deployment is touched.
3. Read the touched component's `specs/component.spec.json`.
4. Read repository `specs/` only for cross-component work.
5. Read `.sdkwork/README.md` and only the matching local skill or plugin when relevant.
6. Locate the task row in `../sdkwork-specs/README.md`, then read only the selected global spec sections.
7. Inspect implementation, edit narrowly, and run the narrowest applicable verification.

## Required Specs By Task Type

- Repository and agent workflow: `SOUL.md`, `AGENTS_SPEC.md`, `SDKWORK_WORKSPACE_SPEC.md`, `DOCUMENTATION_SPEC.md`, `TEST_SPEC.md`.
- Requirements and architecture: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `ENGINEERING_WORKFLOW_SPEC.md`, `QUALITY_GATE_SPEC.md`.
- Rust code: `CODE_STYLE_SPEC.md`, `NAMING_SPEC.md`, `RUST_CODE_SPEC.md`, `MODULE_SPEC.md`, `COMPONENT_SPEC.md`.
- Runtime, provider, security, and topology: `APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`, `CONFIG_SPEC.md`, `RUNTIME_DIRECTORY_SPEC.md`, `DEPLOYMENT_SPEC.md`, `SECURITY_SPEC.md`, `OBSERVABILITY_SPEC.md`, `EVENT_SPEC.md`, `PERFORMANCE_SPEC.md`.
- HTTP or SDK work: `API_SPEC.md`, `INTERNAL_API_SPEC.md`, `WEB_FRAMEWORK_SPEC.md`, `WEB_BACKEND_SPEC.md`, `SDK_SPEC.md`, `SDK_WORKSPACE_GENERATION_SPEC.md`, and `PAGINATION_SPEC.md` for list/search operations.
- Cache work: `CACHE_SPEC.md`; persistence work: `DATABASE_SPEC.md` and `DATABASE_FRAMEWORK_SPEC.md`.

## Code Style Rules

Rust components use responsibility-specific crate names, keep `lib.rs` and `main.rs` as assembly boundaries, forbid unsafe code unless an approved requirement and review authorize it, and expose only component-declared public entrypoints. Generic application-code crate suffixes such as `runtime`, `core`, `manager`, and `backend` are not allowed for new crates.

## Int64 Wire Contract (API_SPEC §13.6)

- OpenAPI `int64` fields and parameters `MUST` be `type: string`, `format: int64`,
  a decimal `pattern` such as `^-?[0-9]+$`, and `x-sdkwork-int64-string: true`.
  `type: integer, format: int64` is a contract violation: generated TypeScript
  SDKs then emit `number`, and browsers silently round ids past
  `Number.MAX_SAFE_INTEGER` (2^53), replaying wrong ids into lookups.
- Rust response DTOs `MUST` serialize `i64` wire fields with
  `#[serde(with = "sdkwork_utils_rust::serde_int64")]` (or `::option`); request
  boundaries parse inbound strings with the same helper.
- Generated TypeScript SDKs keep `int64` as `string`; frontend code `MUST NOT`
  convert ids/snowflake ids/sequence ids to `number` for storage, comparison,
  or submission.
- Verification: `node <sdkwork-specs>/tools/check-api-operation-patterns.mjs --workspace .`

## Build, Test, And Verification

Run commands from this repository root. Phase 0 verification is `cargo fmt --all -- --check`, `cargo check --workspace`, `cargo test --workspace`, the documentation checker, component port checker, packages-layout checker, and repository baseline audit listed in root `README.md`.

## Agent Execution Rules

Do not implement a Provider, API route, SDK, scheduler, isolation policy, secret injection mechanism, or deployable profile without a ready `REQ-*` and any required ADR. Do not hand-edit generated SDK output or replace SDK integration with raw HTTP. Record exact commands and important outputs before completion.

## Human Review Rules

Human review is required for public naming, provider isolation/security posture, generated SDK ownership, API authority, destructive workspace operations, cross-repository kernel integration, production deployment, or breaking contract changes.
