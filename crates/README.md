# Rust Crates

Purpose: authored Rust components for Sandbox domain services, provider ports/adapters, composition, and CLI entrypoints.

Owner: SDKWork Runtime Platform maintainers.

Allowed: Cargo crates with component-local specs, focused source, unit tests, and public root exports. Forbidden: generated SDK transports, generic catch-all crates, undeclared sibling source paths, deployment values, and secrets.

Phase 0 components:

- `sdkwork-sandbox-provider-spi`: L3 provider port boundary.
- `sdkwork-intelligence-sandbox-service`: L2 sandbox use-case boundary.
- `sdkwork-intelligence-sandbox-pool-control`: L2 runtime-pool control-plane boundary (`REQ-2026-0019` control-plane slice: slot/claim state machine, fenced idempotent claims, bounded registries).
- `sdkwork-intelligence-sandbox-template-authority`: L2 template-authority model boundary (`REQ-2026-0029` authority-model slice: immutable published definitions/versions, build-input boundary, cache-semantics authority).
- `sdkwork-intelligence-sandbox-transaction-control`: L2 workspace-transaction control-plane boundary (`REQ-2026-0021` control-plane slice: transaction state machine, 21-stage orchestration order, checkpoint CAS, compensation windows, bounded registry).
- `sdkwork-intelligence-sandbox-snapshot-authority`: L2 snapshot/fork authority-model boundary (`REQ-2026-0031` authority-model slice: immutable snapshot records, closed lifecycle, fork derivation semantics, evidence gates).
- `sdkwork-intelligence-sandbox-build-authority`: L2 template-build authority-model boundary (`REQ-2026-0032` authority-model slice: terminal-immutable build records, closed five-state lifecycle, outcome binding, evidence gates).
- `sdkwork-intelligence-sandbox-launch-authority`: L2 fast-start launch authority-model boundary (`REQ-2026-0033` authority-model slice: immutable launch plans, closed four-state lifecycle, fenced-claim binding, fresh-identity enforcement).
- `sdkwork-sandbox-provider-local`: L4 local-provider adapter boundary.
- `sdkwork-sandbox-provider-firecracker`: L4 firecracker-provider adapter boundary (`REQ-2026-0008` Gate 0 slice: exact-tuple artifact manifest, fail-closed preflight, durable fencing, broker/guest seams).
- `sdkwork-sandbox-service-host`: L5 composition host boundary.
- `sdkwork-sandbox-cli`: L6 local command entrypoint boundary.

Related specs: `../../sdkwork-specs/RUST_CODE_SPEC.md`, `../../sdkwork-specs/NAMING_SPEC.md`, `../../sdkwork-specs/COMPONENT_SPEC.md`, `../../sdkwork-specs/APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`.

Verification: `cargo check --workspace` and `cargo test --workspace`.
