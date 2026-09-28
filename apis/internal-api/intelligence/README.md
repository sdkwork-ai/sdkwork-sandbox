# `apis/internal-api/intelligence/` — Sandbox Internal API Authority

Authoritative API contract for the SDKWork Sandbox internal-api control plane,
owned under `REQ-2026-0028` (E2B-Compatible API And SDK Family Authority,
`ready` since 2026-09-29) with `ADR-20260924` accepted.

- `sandbox-internal-api-authority.openapi.json` — the authority OpenAPI (slice
  v0: the owned sandbox-instance registry surface; every operation carries an
  `x-e2b-reference` mapping to the pinned reference baseline).
- `sandbox-e2b-parity-ledger.json` — the two-way parity ledger over all 71 E2B
  reference operations. Every operation is either `mapped` (authority route
  recorded) or `pending-gate` (capability without an owned implementation;
  never silent). Regenerate with:

  ```bash
  node tools/generate-sandbox-e2b-parity-ledger.mjs --check
  ```

Rules: the pinned E2B baseline is the compatibility reference, this directory
is the only authority; int64 wire fields are strings (`API_SPEC.md` §13.6);
the ingress-token security scheme is internal-api-only and never appears on a
data-plane surface; SDK output generated from this authority is generator-owned
(`SDK_SPEC.md`, `SDK_WORKSPACE_GENERATION_SPEC.md`) and must never be
hand-edited.
