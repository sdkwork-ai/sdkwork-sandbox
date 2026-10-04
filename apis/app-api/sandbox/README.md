# `apis/app-api/sandbox/` — Sandbox App API Authority

Authoritative API contract for the SDKWork Sandbox app-api console face, owned
under `REQ-2026-0030` with `ADR-20261004` accepted.

- `sandbox-app-api-authority.openapi.json` — the authority OpenAPI (slice v0:
  the owned per-user sandbox-instance registry surface for the Web Server
  console; IAM dual-token authenticated, listing always narrowed to the
  verified principal's owner).

Rules: this directory is the only authority for the app-api face; the IAM
dual-token security scheme is app-api-only and never appears on the
internal-api face (`apis/internal-api/intelligence/`), whose ingress-token
scheme never appears here; the tenant and the owner are derived server-side
from the verified principal (`API_SPEC.md` §12) and no operation accepts
either from the wire; query vocabulary is `lower_snake_case`
(`PAGINATION_SPEC.md` §0) and int64 wire fields are strings
(`API_SPEC.md` §13.6); SDK output generated from this authority is
generator-owned (`SDK_SPEC.md`, `SDK_WORKSPACE_GENERATION_SPEC.md`) and must
never be hand-edited.

Verification:

```bash
node ../sdkwork-specs/tools/check-api-operation-patterns.mjs --workspace .
node ../sdkwork-specs/tools/check-api-response-envelope.mjs --workspace .
cargo test -p sdkwork-routes-sandbox-app-api
cargo test -p sdkwork-api-sandbox-assembly
```
