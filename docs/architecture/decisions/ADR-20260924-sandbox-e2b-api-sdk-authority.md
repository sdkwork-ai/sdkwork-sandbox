# ADR-20260924-sandbox-e2b-api-sdk-authority

Status: proposed

Requirement: REQ-2026-0028

Owner: SDKWork Runtime Platform

Date: 2026-09-24

Specs: REQUIREMENTS_SPEC.md, ARCHITECTURE_DECISION_SPEC.md, API_SPEC.md, SDK_SPEC.md, SDK_WORKSPACE_GENERATION_SPEC.md, SECURITY_SPEC.md

## Context

The product mandates E2B-compatible sandbox capability coverage and three-language generated SDKs, but the repository owns no API authority: `apis/` holds no OpenAPI and `sdks/` holds no generated output. Source-level study of the vendored E2B trees established the facts any compatibility decision must respect:

- E2B's "public API" is two independently authenticated surfaces, not one: control-plane REST (74 operations, team API key) and an in-sandbox envd data plane (17 RPC methods plus a small REST face, per-sandbox access token and URL signatures). E2B's own aggregated public document disagrees with its source in both directions (28 source-only and 25 documented-only operations).
- The SDK-facing semantics that decide whether "the same capability behaves the same" live mostly outside OpenAPI: 20 error families (with two real inheritance divergences between the JS and Python SDKs), 9 envd version gates, 7 gRPC status-code mappings, 45 public method members (4 one-sided), and pagination via a `x-next-token` response header.
- E2B's default security posture differs from this repository's root rules in reviewed places (default-allow egress versus `REQ-2026-0014`'s default-deny; Docker as build-input-only versus E2B's runtime boundary; admin operations exposed on the public document).

## Decision

1. **Reference, never authority.** E2B's public contracts are the compatibility reference, pinned by captured sha256 in `specs/sandbox-e2b-capability-baseline.json`. The owned authority is an authored OpenAPI under `apis/`, derived from and traceable to the reference; E2B's files are never imported as the authority itself.
2. **Two-surface split preserved.** Control plane and data plane are separate contracts with separate security schemes and independent versioning, mirroring E2B's own trust boundary. The authority records each surface's credential model explicitly.
3. **Capability-level parity with a deviation ledger.** Compatibility is judged by a two-way operation ledger and an SDK five-face matrix (errors, version gates, code mapping, method members, pagination). Every point where root posture overrides E2B is a recorded, human-reviewed deviation naming the owning decision - default-deny egress, no Docker runtime boundary, tenant fencing ownership - never a silent divergence.
4. **Generator-owned SDK family.** TypeScript, Python, and Rust SDKs are generated from the authority through the standard generator; the audited cross-language facts are reproduced per language from one source, and hand-edited generated output is a gate failure.
5. **Wire rules carry over.** `int64` fields are string-encoded per `API_SPEC.md` section 13.6; pagination uses the reference's header-cursor shape with its failure mode (a dropped header silently truncates a listing) documented for client authors.

## Alternatives

- **Adopt E2B's public OpenAPI verbatim as the authority.** Rejected: the aggregated document mixes admin and internal operations into the public face, disagrees with E2B's own source in both directions, encodes a security posture contrary to root rules, and is not ours to evolve - compatibility claims would be unauditable.
- **Design a native API and treat E2B as inspiration.** Rejected: the product mandate is compatibility; a native surface makes "the same capability behaves the same" unverifiable and re-opens naming without an owner.
- **Hand-write SDKs per language.** Rejected: cross-language divergence is exactly what the five-face audit shows hand maintenance produces (4 one-sided members, 2 inheritance divergences); `SDK_WORKSPACE_GENERATION_SPEC.md` makes generation the mandated path.
- **Fold the two surfaces into one contract for simplicity.** Rejected: E2B's trust boundary (team credential versus per-sandbox token) is real; merging the surfaces would let one credential model leak into the other's operations.

## Consequences

Benefits: compatibility becomes a reviewable, re-derivable ledger rather than a claim; upstream drift is a build-time pinning failure; deviations carry their owning decisions with them; the SDK family has exactly one source of truth per language.

Costs: the reference must be re-pinned and the ledger re-derived whenever upstream moves; every deviation needs a named owning decision and human review; the authority is one more contract that must stay synchronized with the capability audit and the machine contracts.

## Verification

- The capability audit (`tools/check-sandbox-e2b-parity-matrix.mjs`, `tools/check-sandbox-e2b-field-parity.mjs`) and the upstream sampler keep the reference and its readings pinned.
- The future authority contract carries a two-way operation ledger check and the deviation-review gate; the API operation-pattern checker enforces the int64 wire rule on the authority.
- SDK generation checks forbid hand-edited output under `sdks/`.
- Requirement/decision citation gates keep this record linked from live documents.

## Supersedes / Superseded By

None. This record composes with `ADR-20260731-sandbox-internal-control-plane.md` (the internal port the control-plane surface will map) and leaves data-plane transport decisions to the readiness package.
