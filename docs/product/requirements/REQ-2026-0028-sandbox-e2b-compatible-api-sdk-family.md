# REQ-2026-0028: E2B-Compatible API And SDK Family Authority

Status: draft

Owner: SDKWork Runtime Platform

Source: product

Priority: P1

Updated: 2026-09-24

Specs: REQUIREMENTS_SPEC.md, ARCHITECTURE_DECISION_SPEC.md, API_SPEC.md, INTERNAL_API_SPEC.md, SDK_SPEC.md, SDK_WORKSPACE_GENERATION_SPEC.md, PAGINATION_SPEC.md, APPLICATION_LAYERED_ARCHITECTURE_SPEC.md, SECURITY_SPEC.md, TEST_SPEC.md, QUALITY_GATE_SPEC.md

Related: REQ-2026-0002, REQ-2026-0007, REQ-2026-0009, REQ-2026-0014, REQ-2026-0023, REQ-2026-0027

## Problem

The product mandates E2B-compatible sandbox capability coverage and three-language generated SDKs with identical semantics, but the repository owns no API authority: `apis/` holds no OpenAPI document, `sdks/` holds no generated output, and the capability attribution ledger confirms the SDK family and the API surface are carried by no other requirement record. The E2B compatibility target is not one contract but two independently authenticated and versioned surfaces: the control-plane REST (74 operations, team API key) and the in-sandbox envd data plane (17 RPC methods plus a small REST face, per-sandbox access token and URL signatures). The two surfaces also expose 28 source-only and 25 documented-only operation discrepancies against E2B's own aggregated public document.

Without one owned authority, a future implementation could hand-write HTTP routes, fork E2B's contract as its own, generate SDKs from E2B's file instead of an owned derivation, silently diverge from root security posture while claiming compatibility, or let the two surfaces blur their trust boundary - each a compatibility claim that cannot be re-derived.

## Goals

- Define the compatibility reference set: E2B's public control-plane OpenAPI and envd data-plane contracts, pinned by captured sha256 with capture dates in `specs/sandbox-e2b-capability-baseline.json`; the reference is an input, never the authority.
- Establish `apis/` as the single authored API authority from which all SDKs are generated, with every authority operation carrying either an E2B reference mapping or a recorded, reviewed deviation entry.
- Keep the two-surface split: control plane and data plane remain separate contracts with separate authentication models and independent versioning; a deviation in one never implies a deviation in the other.
- Define compatibility acceptance at capability level with a two-way operation-parity ledger, an SDK five-face matrix (error families, envd version gates, gRPC code mapping, public method members, pagination semantics), and the int64-as-string wire rule of `API_SPEC.md` section 13.6.
- Make deviation a reviewed artifact: where root posture differs from E2B (default-deny egress versus E2B's default-allow, no Docker runtime boundary, tenant fencing ownership), the authority records the deviation and the decision that owns it; silent divergence is a gate failure.
- Constrain generated SDK output to the generator per `SDK_WORKSPACE_GENERATION_SPEC.md` for TypeScript, Python, and Rust, with the audited cross-language facts (20 error families, 9 envd version gates, 7 gRPC code mappings, pagination via `x-next-token`) reproduced per language from one source.

## Non-Goals

- Implementing any HTTP route, RPC server, envd equivalent, service host wiring, or deployment profile.
- Generating any SDK before the authority contract and its generator profile are reviewed and authorized.
- Adopting E2B internal, admin, dashboard, edge, or hyperloop contracts; they are not SDK-facing surfaces.
- Deciding public endpoint naming, domain ownership, or edge ingress placement; these remain open product questions reviewed with the API authority.
- Redefining network policy semantics, which `REQ-2026-0014` owns; the authority records the resulting deviation, not the policy.
- Covering MCP gateway server-directory semantics beyond what the capability matrix already scopes.

## Acceptance Criteria

1. `apis/` contains an authority OpenAPI whose every operation carries either an E2B reference mapping (`method`, `path`, and operation identity from the pinned reference) or a deviation entry naming the owning decision record.
2. A machine-checkable parity ledger maps the 71 E2B public operations two-way against the authority: every E2B operation is mapped or recorded as a deviation, and every authority operation traces to E2B or to a deviation.
3. Deviation entries name the overriding root rule or requirement (for example default-deny egress per `REQ-2026-0014`) and are human-reviewed; an unreviewed deviation fails the gate.
4. The authority declares the two surfaces separately with their distinct security schemes, and no control-plane credential schema appears on the data plane or vice versa.
5. The SDK face matrix covers error families, envd version gates, gRPC code mapping, public method members, and pagination semantics, and records for each face which source artifact generates it.
6. Every `int64` wire field in the authority is `type: string, format: int64` with the decimal pattern and `x-sdkwork-int64-string: true`, per `API_SPEC.md` section 13.6.
7. All SDK output is generator-owned; hand-edited generated files fail the workspace generation checks.
8. A compatibility regression command re-derives the operation ledger and the face matrix from the pinned reference and the authority in one run, and fails on drift in either direction.

## Non-Functional Requirements

| Area | Required outcome |
| --- | --- |
| Security | Compatibility never weakens root posture: E2B's default-allow egress, token-free internal endpoints, and admin operations are recorded deviations, never defaults. Surface trust boundaries are explicit in the authority. |
| Privacy | The authority carries no user-data fields beyond what the mapped capabilities require; telemetry-facing fields follow `OBSERVABILITY_SPEC.md` naming and bounds. |
| Performance | Compatibility imposes no synchronization calls on request paths; the ledger is static evidence. No latency commitments beyond root standards. |
| Reliability | Reference pinning and two-way ledger make drift a build failure, not a runtime discovery; regenerated SDKs are reproducible from the authority and generator profile. |

## Affected Surfaces

- `apis/` authored authority OpenAPI and the parity ledger artifact
- `sdks/` generated TypeScript, Python, and Rust SDK family workspaces
- documentation surfaces that cite operation or SDK identity, including the capability audit and PRD
- the SDK generator profile for the sandbox family (consumes, never replaces, `SDK_WORKSPACE_GENERATION_SPEC.md` tooling)

## Traceability

- [ADR-20260924](../../architecture/decisions/ADR-20260924-sandbox-e2b-api-sdk-authority.md)
- [Architecture and security review](../../engineering/reviews/REVIEW-20260924-sandbox-e2b-api-sdk-authority.md)
- Capability reference: [E2B capability baseline](../../../specs/sandbox-e2b-capability-baseline.json)
- Source-level audit: [E2B upstream source parity](../../engineering/reviews/REVIEW-20260923-sandbox-e2b-upstream-source-parity.md)
- [Internal Control Plane](REQ-2026-0023-sandbox-internal-control-plane.md), [Command Execution](REQ-2026-0007-sandbox-command-execution-contract.md)

## Implementation Gate

This requirement remains `draft`. It defines authority, mapping, and acceptance only. It does not authorize an HTTP route, RPC server, envd implementation, generated SDK, public endpoint name, domain, edge ingress, or any runtime behavior. Implementation begins only after this requirement is ready, the ADR and review are accepted, an `apis/` authority contract exists as a reviewed artifact, and the surfaces it maps receive API, security, and cross-repository human approval per `AGENTS.md`.
