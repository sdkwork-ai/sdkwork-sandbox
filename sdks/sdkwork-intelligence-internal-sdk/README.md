# sdkwork-intelligence-internal-sdk

Generator-owned three-language SDK family (TypeScript, Python, Rust) for the
SDKWork Sandbox internal-api control plane, generated from the REQ-2026-0028
authority (`apis/internal-api/intelligence/`, E2B-referenced, `ready`
2026-09-29).

- Authority + parity ledger: `../../apis/internal-api/intelligence/`
- Materializer: `../materialize-intelligence-v0-openapi-boundaries.mjs`
- Generation input: `openapi/sdkwork-intelligence-internal-api.sdkgen.yaml`
- Generator: `../../../sdkwork-sdk-generator/bin/sdkgen.js` with
  `--standard-profile sdkwork-v3 -t custom --api-prefix /internal/v3/api/intelligence/sandbox`
- Generated output: `sdkwork-intelligence-internal-sdk-{typescript,python,rust}/generated/server-openapi/`
  — never hand-edited; regenerate through the canonical command instead.

Supported language subset: TypeScript, Python, Rust (REQ-2026-0028 AUTH-04
three-language parity; narrower than the full SDK_WORKSPACE_GENERATION_SPEC
baseline and declared here per that spec's language-subset rule).

Generator mode: sdkwork-v3 custom open-api profile with the explicit internal
api-prefix (`INTERNAL_API_SPEC.md` section 5). Runtime hosts must accept
`X-API-Key` as an ingress-token alias (`INTERNAL_API_SPEC.md` section 4); the
alias acceptance in the web framework is a pending cross-repository
obligation and does not block the generated contract.

Build artifacts (`dist/`, `target/`, `node_modules/`) are ignored; generated
sources are committed.
