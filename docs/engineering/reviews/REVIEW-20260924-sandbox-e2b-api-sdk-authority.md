# REVIEW-20260924: E2B-Compatible API And SDK Family Authority

Status: pending-human-review

Risk: high - the authority decides what E2B compatibility means for the public API and SDK contracts; a wrong authority source or an unreviewed deviation becomes a compatibility claim that cannot be re-derived, and SDK generation would amplify it into every language at once.

Owner: SDKWork Runtime Platform

Date: 2026-09-24

Requirement: [REQ-2026-0028](../../product/requirements/REQ-2026-0028-sandbox-e2b-compatible-api-sdk-family.md)

Decision: [ADR-20260924](../../architecture/decisions/ADR-20260924-sandbox-e2b-api-sdk-authority.md)

Review purpose: decide whether the repository adopts an owned, E2B-referenced API/SDK authority (reference-versus-authority split, two-surface preservation, capability-level parity with a reviewed deviation ledger, generator-owned SDK family) as the compatibility contract for the sandbox family. This packet authorizes at most a `draft` authority contract and its ledger; it does not authorize any route, server, SDK generation, or public naming.

## Scope And Inputs

- Capability baseline `specs/sandbox-e2b-capability-baseline.json` (101 pinned sources; control-plane reference 74 operations; the audit's five SDK faces).
- Source-level audit: [REVIEW-20260923 section 8/9](REVIEW-20260923-sandbox-e2b-upstream-source-parity.md) (error families, envd version gates, gRPC code mapping, pagination, method members, two-surface split, 28/25 operation divergence).
- Root rules that override the reference where noted: `API_SPEC.md` section 13.6 (int64 as string), `REQ-2026-0014` (default-deny egress), `PRD.md` non-goals (no Docker runtime boundary), `SDK_WORKSPACE_GENERATION_SPEC.md` (generator-owned output).
- Capability matrix rows 68-72 and the attribution ledger rows for the SDK family (confirmed unowned before this packet).

## Decision Matrix

| ID | Decision proposed | Rationale | Reviewer action required |
| --- | --- | --- | --- |
| AUTH-01 | E2B contracts are the pinned compatibility reference; `apis/` is the only authority. | Reference drift becomes a build-time failure instead of a runtime incompatibility; the authority is evolvable and owned. | Approve the reference-versus-authority split, or name the alternative authority source. |
| AUTH-02 | Control plane and data plane stay two contracts with separate security schemes. | Mirrors E2B's real trust boundary (team credential vs per-sandbox token); prevents credential-model leakage between surfaces. | Approve the split and the rule that neither scheme may appear on the other surface. |
| AUTH-03 | Parity is capability-level with a reviewed deviation ledger; root posture overrides the reference (default-deny egress, no Docker runtime boundary, fencing ownership). | Compatibility must not weaken root security; silent divergence is worse than recorded incompatibility. | Approve the initial deviation set or amend it; every amendment needs a named owning decision. |
| AUTH-04 | SDKs are generated (TypeScript, Python, Rust) from the authority; the audited five faces are reproduced per language from one source. | The cross-language divergences measured in upstream SDKs are what generation exists to prevent. | Approve the face matrix as the SDK acceptance surface, or narrow it. |
| AUTH-05 | E2B admin, dashboard, edge, and hyperloop contracts are out of scope and never mapped into the authority. | They are not SDK-facing; including them would import unaudited internal operations. | Confirm the exclusion list. |

## Pre-review Blocking Findings

- None open. The reference is pinned and byte-verified by the upstream sampler as of 2026-09-23 (network refresh currently unavailable in the working environment; recorded readings reproduce byte-identically).

## Platform Conformance Test Design

- Two-way operation ledger check over the authority and the pinned reference (machine-checkable; drift fails the build).
- SDK face matrix checks derived from the audit's recorded readings (error family table, version-gate equality, code mapping, member inventory, pagination shape).
- `API_SPEC.md` section 13.6 int64 pattern check over the authority.
- Generation-reproducibility check over `sdks/` once generation is separately authorized.

## Required Evidence Before Ready

1. The authority OpenAPI exists in `apis/` as a reviewed artifact with its two surfaces and security schemes.
2. The parity ledger and deviation entries are materialized and machine-checked; every deviation names its owning decision.
3. The compatibility regression command runs in one step and is wired into the verification block.
4. Public endpoint naming and edge ingress ownership answers land in `PRD.md` (open questions) or a superseding decision.

## Human Outcome

| Reviewer role | Reviewer | Outcome | Date | Decision IDs / findings |
| --- | --- | --- | --- | --- |
| Platform Owner | — | — | — | AUTH-01, AUTH-03, AUTH-05 |
| API/SDK Owner | — | — | — | AUTH-01, AUTH-02, AUTH-04 |
| Security Owner | — | — | — | AUTH-03, AUTH-05 |
| Product Owner | — | — | — | AUTH-03, AUTH-04 |

## Implementation Gate

Current recommended human outcome is `Changes requested` until the authority contract, its ledger, and the open naming questions return with evidence. REQ-2026-0028 stays `draft` and the ADR stays `proposed`; no HTTP route, RPC server, envd implementation, generated SDK, public endpoint name, domain, or edge ingress is authorized by this packet, with or without approval.

## Close-Out Checklist (Reviewer 执行项)

1. 每条 AUTH 决策逐项给出 Approved / Changes requested / Rejected，并写明 Decision ID。
2. AUTH-03 的初始偏离集逐条确认；新增偏离必须点名承载决策。
3. 确认五个 SDK 面（错误族 / 版本闸 / code 映射 / 方法成员 / 分页）作为生成 SDK 的验收面，或明确收窄。
4. 确认 E2B 内部契约（admin / dashboard / edge / hyperloop）的排除清单。
5. 决定 `apis/` 权威契约与其 ledger 的落地切片顺序，并在 `PRD.md` 开放问题中处置命名与边缘归属。

## Exit Gate

1. 本 packet 每个 Reviewer Role 表决 Approved。
2. Close-Out 全部完成，无 open finding。
3. REQ-2026-0028 → `ready`，ADR-20260924 → `accepted`。
4. 其后任何实现仍需各自机器契约翻转 `implementationAuthorized`，并满足 `AGENTS.md` 的人工评审清单（公共命名、API 权威、生成 SDK 所有权）。
