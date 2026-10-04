# ADR-20261004-sandbox-app-api-console-face

Status: accepted

Requirement: REQ-2026-0030

Owner: SDKWork Runtime Platform

Date: 2026-10-04

Specs: ARCHITECTURE_DECISION_SPEC.md, API_SPEC.md, PAGINATION_SPEC.md, IAM_SPEC.md, WEB_FRAMEWORK_SPEC.md, SECURITY_SPEC.md

## Context

The Web Server console ships a Sandbox Instances page, and the module's only HTTP
face is the operator-trusted internal-api surface (`ADR-20260731`,
`ADR-20260924`): ingress-token gated, tenant-wide, cursor-paginated. A browser
principal can never hold an ingress token, so the console page has no servable
route — it calls `/app/v3/api/sandbox/*` paths no crate mounts. Deciding the
browser-facing surface means fixing, in one place:

- the credential model (browser dual token versus internal ingress token),
- the listing scope (self-scoped versus tenant-wide),
- the pagination vocabulary (console numbered tables versus keyset feeds),
- and the permission model (per-route IAM codes federated through the shared
  catalog versus operator trust).

## Decision

1. **One app-api face, one composed contribution.** A new
   `sdkwork-routes-sandbox-app-api` routes crate serves
   `/app/v3/api/sandbox/sandbox_instances{/{sandboxInstanceId}}` with dual-token
   auth (`HttpRoute::dual_token`) and per-route IAM permission codes. The
   assembly composes the app-api and internal-api routers, manifests, and a
   single surface-branching domain context injector into one `sdkwork-sandbox`
   `ApiAssemblyContribution` — the same one-contribution-per-owner shape every
   dependency module hands the Web Server edge (`API_ASSEMBLY_SPEC.md` section
   4.1.1). Standalone mounts that need a single face keep their own crates'
   routers and manifests; the composed path is the only one the edge takes.

2. **Self-scoped by construction, guarded per item.** The domain context
   injector derives `tenant_id` and `actor_id` from the verified principal's
   positive ids only; the collection listing always narrows to the calling
   owner, and no owner query parameter exists on the face. Item routes re-read
   the instance and answer the same `404` an absent row returns when the stored
   owner is not the caller. This guard is race-free without a database change
   because ownership is immutable after creation — no update path writes
   `sandbox_instance_owner_id`, and the optimistic-concurrency CAS covers the
   rest of the row.

3. **Offset pagination in the platform vocabulary.** The console table is a
   numbered, low-volume, total-counted listing (`PAGINATION_SPEC.md` section 3:
   offset mode). The face accepts `page` / `page_size` /
   `sandbox_instance_state` only, rejects forbidden aliases and out-of-range
   values instead of clamping, and answers `data.pageInfo` with exact
   `totalItems` / `totalPages` / `hasMore`. The service gains a bounded
   `list_offset` (page `1..=10000`, size `1..=200`, offset arithmetic checked)
   backed by a `LIMIT/OFFSET` + `COUNT` repository listing next to the existing
   keyset cursor listing, which the internal-api face keeps unchanged.

4. **IAM catalog federated, permissions declared per route.** The module ships
   `specs/iam.module.manifest.json` (`moduleId: "sandbox"`, four
   `sandbox.instances.*` codes, standard roles plus `app_user` / `org_admin` /
   `org_operations` grant extensions) so the Web Server gateway materializes it
   into the shared catalog the same way it federates skills and mcp. Route
   permission codes are observability-only on app-api surfaces (the platform's
   surface-authorization tiers decide access in the service layer); the catalog
   makes the codes grantable and the composed inventory auditable.

5. **Wire rules inherited, not re-invented.** Success and failure leave through
   the same envelope helpers as the internal-api face (`SdkWorkApiResponse`,
   `data.item` / `data.items` + `data.pageInfo`, `201` create, `204` delete,
   `application/problem+json` with numeric `code` and `traceId`); `sandboxVersion`
   stays an int64-as-string; query vocabulary is `lower_snake_case`
   (`PAGINATION_SPEC.md` section 0), so the browser client is corrected from the
   non-conformant `pageSize` alias it shipped with.

## Alternatives

- **Point the console at the internal-api face.** Rejected: a browser cannot
  hold an ingress token, and the internal face's tenant-wide inventory is
  exactly the scope a browser principal must never get.
- **Serve offset pages by walking the keyset cursor.** Rejected: O(page) store
  walks per numbered page and no exact total; the table contract needs both.
- **Hand the owner filter to the client on the app-api face.** Rejected: a
  client-supplied owner is a tenant-wide read primitive; identity is derived
  server-side per `API_SPEC.md` section 12.
- **Two contributions (app-api and internal-api) from two entrypoints.**
  Rejected: one owner must arrive as one contribution — a second entrypoint
  would trip composition or silently drop a surface at the edge.

## Consequences

Benefits: the console page serves real data under the principal's own scope;
the module's permission catalog becomes grantable and auditable; both faces
share one service, one repository, and one composed contribution, so the trust
boundary is a manifest fact rather than a convention.

Costs: the offset listing adds one bounded `COUNT` per console page; the IAM
manifest is one more catalog the edge must materialize; the two faces' separate
pagination vocabularies must each keep their own contract tests.

## Verification

- `cargo test -p sdkwork-routes-sandbox-app-api` — manifest auth shape, offset
  vocabulary, alias rejection, payload contracts.
- `cargo test -p sdkwork-intelligence-sandbox-service` — offset window, bounds,
  totals; cursor listing untouched.
- `cargo test -p sdkwork-api-sandbox-assembly` — composed manifest ↔ OpenAPI
  inventory ↔ permission catalog agreement.
- `check-api-operation-patterns` / `check-api-response-envelope` over the
  authority document.
- Web Server edge end-to-end: browser dual-token `200`, unauthenticated `401`,
  internal path still ingress-token-only.

## Supersedes / Superseded By

None. Extends `ADR-20260924` (authority ownership) with a second, independently
authenticated surface; the parity ledger scope is unchanged.
