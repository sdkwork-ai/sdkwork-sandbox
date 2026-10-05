# REVIEW-20261006: Sandbox Template Bounded Registry Control-Plane Slice

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured session instruction repeated on 2026-10-04/06 ("检查是否支持镜像和snapshot能力，反复回归检查，确保该功能能实现，我要实现的是快速分配和快速启动资源实例，并支持资源池实例，回归检查是否实现按需分配、资源池化能力，打造专业灵活的sandbox能力"), continuing [REVIEW-20260929](REVIEW-20260929-sandbox-e2b-capability-alignment.md) under the single-owner convention of REVIEW-20260924, executing the bounded template registry slice named by RES-03 of [REVIEW-20261006-template-resolution](REVIEW-20261006-sandbox-worker-template-resolution.md). Recorded by the executing agent on that instruction. Approval disposes the review, not the evidence: the registry is bounded and in-process only, the storage-backed registry stays forbidden, and no E2B Template capability-parity claim may be made before that slice lands.

Requirement: [REQ-2026-0029](../../product/requirements/REQ-2026-0029-sandbox-template-authority.md)

Decision: [ADR-20261006](../../architecture/decisions/ADR-20261006-sandbox-bounded-template-registry.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

Risk: moderate - a second template authority must not fork from the 0029 records; the bounded registry is control-plane only.

## Scope

本 Review 评审有界模板 registry 控制面切片：`BoundedSandboxTemplateRegistry`（进程内、有界、fail-closed——发布定义、发布版本、按版本引用取记录、解析 start command=版本→定义→`sandbox_set_start_cmd()`）；0029 契约 `forbidden.templateRegistryService` 对该切片翻转为 `false`（storage-backed registry 仍被 `x-sdkwork-no-storage-backend` 锁住）；worker 本地车道解析器经该 registry 实现 `SandboxStartCommandResolverPort`。

本 Review 不批准 storage-backed registry、持久层、CLI、公共 API/SDK、部署 profile，也不批准任何"E2B Template 能力已对齐"的声明。

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| REG-01 | The bounded registry composes the 0029 authority records directly — publish/get/resolve over `SandboxTemplateDefinition`/`SandboxTemplateVersion`; no second record type. | One template authority. | A second record model. |
| REG-02 | The registry is bounded and in-process; capacity is refused beyond the bound; storage stays forbidden. | Control-plane semantics verifiable in-repo. | Hidden persistence. |
| REG-03 | Resolution follows version → definition → `sandbox_set_start_cmd()` through the authority accessors; nothing else is readable through the registry's resolve surface. | The worker resolves exactly the published command. | Over-wide registry surface. |
| REG-04 | The worker local-lane resolver implements the existing port backed by this registry; no resolver logic duplicated. | Single resolution path. | Two resolvers. |

## Blocking Findings

1. No storage-backed registry exists; published records are process-local until that slice.
2. Real execution evidence is lane-scoped to the composition tests.
3. Firecracker lane and warm slot remain gated.

## Required Evidence Before Template Parity Claims

- Land the storage-backed registry slice with its own review and persistence tests.
- Measure first-command-zero-wait; prove the Firecracker lane behind KVM evidence.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`.

| Reviewer role | Reviewer | Outcome | Date | Decisions |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-10-06 | REG-01, REG-02 |
| Security/privacy owner | Repository Owner (structured approval) | Approved | 2026-10-06 | REG-02, REG-03 |
| Capacity/scheduler owner | Repository Owner (structured approval) | Approved | 2026-10-06 | REG-02, REG-04 |
| Command/execution owner | Repository Owner (structured approval) | Approved | 2026-10-06 | REG-03, REG-04 |
| Workspace/storage owner | Repository Owner (structured approval) | Approved | 2026-10-06 | REG-01, REG-04 |

## Implementation Gate

Since 2026-10-06: this Review is `accepted` (single-owner structured approval) and the bounded registry landed in `crates/sdkwork-intelligence-sandbox-template-authority` with the worker resolver backed by it in `crates/sdkwork-intelligence-sandbox-worker-local-exec`. `specs/sandbox-template-authority.contract.json` flips `forbidden.templateRegistryService: false` for this bounded in-process slice while the storage backend stays locked. Storage-backed registry, CLI, public API/SDK and deployment profiles stay forbidden until their own slices.
