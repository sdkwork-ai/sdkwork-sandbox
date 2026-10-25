# Application Surfaces

Purpose: index runnable SDKWork Sandbox application roots and surfaces.

Owner: SDKWork Runtime Platform maintainers.

## Primary App Surface

The repository root is the primary application surface and will own the CLI and service-host composition. Phase 0 contains only a non-operational CLI scaffold and does not yet declare `agent.app.config.json`.

## Directory Index

| Directory | Surface role | Runnable | Purpose | Entry |
| --- | --- | --- | --- | --- |
| Repository root | Primary CLI and service composition | No, Phase 0 scaffold only | Future standalone Sandbox application surface | [Root README](../README.md) |

There are no direct child application roots under `apps/` in Phase 0. Future independently built client or operator surfaces must be added as architecture-qualified `apps/sdkwork-sandbox-<client-arch>/` roots and indexed here.

Allowed: independently runnable application roots and application-shell documentation. Forbidden: generic package families, Rust service crates, generated SDK output, and runtime data.

Related specs: `../../sdkwork-specs/APPLICATION_SPEC.md`, `../../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md`, `../../sdkwork-specs/DOCUMENTATION_SPEC.md`.

Verification: `node ../sdkwork-specs/tools/check-apps-directory-index.mjs --root .`.
