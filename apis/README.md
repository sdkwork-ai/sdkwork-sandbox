# APIs

Purpose: author-owned Runtime API, RPC, and event contract sources.

Owner: SDKWork Sandbox API maintainers.

Allowed: reviewed OpenAPI, proto, AsyncAPI, schemas, examples, changelogs, and contract tests. Forbidden: handler implementation, generated SDK transports, credentials, and runtime state.

Related specs: `../../sdkwork-specs/API_SPEC.md`, `../../sdkwork-specs/INTERNAL_API_SPEC.md`, `../../sdkwork-specs/EVENT_SPEC.md`.

Authorities: [`internal-api/intelligence/`](internal-api/intelligence/) is the approved internal-api authority (REQ-2026-0028, ingress-token), and [`app-api/sandbox/`](app-api/sandbox/) is the approved app-api console-face authority (REQ-2026-0030, IAM dual-token).

Current candidates: [`async/`](async/) contains the draft Sandbox event contract for REQ-2026-0010, and [`commands/`](commands/) contains the draft provider-neutral Command/Terminal schemas for REQ-2026-0007. These are authoring surfaces only and do not authorize Event/Command runtime implementations, providers, host I/O, API routes, SDKs, or deployment profiles.

Verification: run the applicable contract tests, then API envelope, operation-pattern, route-collision, and pagination checks when an HTTP/RPC surface is approved.
