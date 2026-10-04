# REQ-2026-0030: Sandbox App API Console Face

Status: ready

Owner: SDKWork Runtime Platform

Source: product

Priority: P1

Updated: 2026-10-04

Specs: REQUIREMENTS_SPEC.md, ARCHITECTURE_DECISION_SPEC.md, API_SPEC.md, PAGINATION_SPEC.md, IAM_SPEC.md, WEB_FRAMEWORK_SPEC.md, APPLICATION_LAYERED_ARCHITECTURE_SPEC.md, SECURITY_SPEC.md, TEST_SPEC.md

Related: REQ-2026-0023, REQ-2026-0028

## Problem

The per-user sandbox instance registry is served today by exactly one HTTP face: the
operator-trusted internal-api surface (`REQ-2026-0028`, ingress-token gated,
`/internal/v3/api/intelligence/sandbox/*`). The Web Server console ships a "Sandbox
Instances" page (`sdkwork-webserver-pc-console-sandbox`) that must be reachable by a
signed-in browser principal, but a browser can never hold an ingress token, so the
console page cannot be served by the existing face — it 404s against the
`/app/v3/api/sandbox/*` paths it calls. The module therefore owns no browser-facing
surface at all: a capability the product ships in the console has no authorized API
route backing it.

## Goals

- Authorize and serve a dual-token **app-api** face for the sandbox instance registry:
  list, create, retrieve, update, delete, under the canonical
  `/app/v3/api/sandbox/sandbox_instances` collection paths and the platform
  `SdkWorkApiResponse` envelope.
- Make the face self-scoped by construction: tenant and owner are derived server-side
  from the verified principal (`API_SPEC.md` section 12); the collection listing is
  always narrowed to the calling owner, and item-level reads and writes carry an
  ownership guard so one account cannot address another account's instance even by
  guessing ids inside the same tenant.
- Declare the per-route IAM permission codes and ship the module's IAM permission
  catalog (`specs/iam.module.manifest.json`) so the Web Server edge federates it and
  console roles can be granted the codes.
- Serve the console's numbered-table pagination in the platform offset vocabulary:
  `page` / `page_size` query parameters, `data.items` + `data.pageInfo` with exact
  `totalItems` (`PAGINATION_SPEC.md` section 3), backed by a bounded
  `LIMIT/OFFSET + COUNT` repository listing alongside the existing keyset cursor.
- Keep the two-surface trust boundary: the internal-api face keeps its
  ingress-token gate, its tenant-wide inventory semantics, and its cursor pagination;
  the app-api face never widens either.

## Non-Goals

- No E2B compatibility surface changes: the app-api face is an SDKWork-native
  console contract and carries no `x-e2b-reference` mapping; the
  `REQ-2026-0028` parity ledger is unchanged.
- No generated SDK family for the app-api face; the Web Server console's hand-written
  transport remains the single call site until the family is activated under
  `REQ-2026-0028`.
- No tenant-wide (cross-owner) listing on the app-api face; that inventory stays the
  internal-api face's job behind the ingress token.
- No lifecycle operations (start, stop, exec, snapshot); the face exposes the same
  instance-registry CRUD the internal-api face already owns.
- No new database tables; the offset listing reads the existing `sandbox_instance`
  baseline.

## Users

- SDKWork console operators (browser principals) managing their own VM instances.
- Platform integrators composing the sandbox assembly into the Web Server edge.

## Affected Surfaces

- rust-components (`sdkwork-routes-sandbox-app-api`, service/repository listing,
  assembly composition)
- composition (`sdkwork-webserver` standalone gateway IAM federation)
- api (`apis/app-api/sandbox/sandbox-app-api-authority.openapi.json`)

## Acceptance Criteria

- `apis/app-api/sandbox/sandbox-app-api-authority.openapi.json` is the authored
  authority; every operation declares the dual-token security scheme, the
  `x-sdkwork-permission` code, and int64-as-string wire fields
  (`API_SPEC.md` sections 4.5, 13.6, 14-16).
- The assembly composes both faces as one `sdkwork-sandbox` contribution; the
  composed route manifest, the OpenAPI inventory generated from it, and the
  permission catalog agree (assembly bootstrap test).
- The listing rejects — never clamps — `page_size` outside `1..=200`, `page`
  outside `1..=10000`, and every forbidden pagination alias
  (`pageSize`, `limit`, `cursor`, `offset`, `sandboxInstanceOwnerId`, ...).
- A principal cannot read, update, or delete another owner's instance: the response
  is the same `404` the absent-row case returns, and ownership can never flip after
  creation.
- The Web Server standalone gateway federates the sandbox IAM module manifest and
  serves the app-api routes next to the internal-api routes on the same origin.

## Verification Plan

- `cargo test -p sdkwork-routes-sandbox-app-api` pins the route manifest auth shape,
  the offset query vocabulary, and the payload contracts.
- `cargo test -p sdkwork-intelligence-sandbox-service` pins the offset listing
  window, bounds, and total counts against the in-memory adapter.
- `node ../sdkwork-specs/tools/check-api-operation-patterns.mjs --workspace .` and
  `check-api-response-envelope.mjs --workspace .` over the authority.
- `node ../sdkwork-specs/tools/check-rust-crate-naming-standard.mjs --root .`,
  `check-rust-manifest-standard.mjs --root .`, and the repository's own
  component-contract and requirement-traceability gates.
- End-to-end on the Web Server development edge: the app-api collection answers
  `401` without a browser token and `200` with one, while the internal-api path keeps
  answering `401` `missing-ingress-token` to the browser.

## Release Boundary

The face ships only when the Web Server edge composes the updated assembly and the
console page reads and writes through it without a 404; the internal-api contract,
its parity ledger, and its generated SDK are frozen by this requirement.
