# REVIEW-20261005: Sandbox Template Build Naming And Boundaries

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured session instruction repeated on 2026-10-04/05 ("检查是否支持镜像和snapshot能力，反复回归检查，确保该功能能实现，我要实现的是快速分配和快速启动资源实例，并支持资源池实例，回归检查是否实现按需分配、资源池化能力，打造专业灵活的sandbox能力"), continuing the structured instructions recorded in [REVIEW-20260929](REVIEW-20260929-sandbox-e2b-capability-alignment.md) under the single-owner convention of REVIEW-20260924, executing the Template Build readiness slice after the Runtime Pool, Template authority, Workspace transaction and Snapshot/Fork slices. Recorded by the executing agent on that instruction. Approval disposes the review, not the evidence: every Blocking Finding below remains a standing evidence obligation, the real-builder-execution and artifact-tuple gates stay release-blocking, and no E2B Template Build capability-parity claim may be made before a runtime slice lands.

Requirement: [REQ-2026-0032](../../product/requirements/REQ-2026-0032-sandbox-template-build.md)

Decision: [ADR-20261005](../../architecture/decisions/ADR-20261005-sandbox-template-build-authority.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-05

Risk: critical - public naming permanence, build-record data ownership against `REQ-2026-0029` (definitions/versions/build inputs/cache semantics) and `REQ-2026-0012` (artifact tuples/evidence), success-binding semantics (a failed build that claims an artifact), WarmMicroVmSlot gate interaction and premature E2B parity claims.

## Scope

本 Review 请求人工评审 Template Build 权威的公共命名（`SandboxTemplateBuild`、封闭生命周期状态集 `requested/building/succeeded/failed/quarantined`、封闭四操作集，全部 `sandbox_` 前缀）、数据所有权（构建权威拥有构建记录与生命周期；模板定义/版本/构建输入/缓存语义归 `REQ-2026-0029`；制品 Evidence 归 `REQ-2026-0012`）、结果绑定语义（成功必须绑定恰好一个制品 Tuple、失败与隔离不绑定任何制品、结果不确定进入隔离而不是静默成功或失败）、证据门（真实 Builder 执行、构建产物 Tuple）与 forbidden 面（Builder 运行时、流水线执行、产物/缓存存储、Registry、CLI、公共 API/SDK、部署 profile）。

本 Review 不批准 Builder 运行时、构建流水线执行、构建产物/缓存存储后端、Registry 服务、CLI、公共 API/SDK、部署 profile、`WarmMicroVmSlot` 启用（独立 Pool 证据门不变），也不批准任何"E2B Template Build 能力已对齐"的声明。

## Candidate Evidence

| Evidence | Result |
| --- | --- |
| REQ-2026-0032 | Draft capability carrier with goals, non-goals, candidate acceptance criteria and release boundary. |
| ADR-20261005 | Proposed build-record ownership, outcome-binding semantics, evidence gates and line layering. |
| `specs/sandbox-template-build.contract.json` | Machine-reviewable record shapes, closed lifecycle, failure semantics, evidence gates, forbidden block; implementation was unauthorized at capture. |
| `node --test tests/contract/sandbox-template-build.contract.test.mjs` | Focused static checks pin the carrier gate, field shapes, lifecycle, failure semantics, evidence gates, layering and the forbidden block. |
| Builder runtime/pipeline/storage/real-execution evidence | Absent; mandatory before any capability claim. |

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| BLD-01 | The authority model owns build records and their lifecycle; all wire fields carry the `sandbox_` prefix; references stay opaque; template definitions, versions, build inputs and cache semantics stay owned by `REQ-2026-0029`. | One reviewable naming plane, no duplicate template authority. | Two competing template authorities. |
| BLD-02 | The lifecycle is the closed five-state set with `requested` as the only initial state and `succeeded`/`failed`/`quarantined` terminal and immutable; timestamps never move backwards. | Build identity becomes a fixed provenance input. | Rewritable build history poisons the artifact chain. |
| BLD-03 | A successful build binds exactly one `REQ-2026-0012` artifact tuple; a failed or quarantined build binds none; an uncertain outcome quarantines instead of silently succeeding or failing. | The pool consumes only truthfully-bound artifacts. | Failed builds leak phantom artifacts into allocation. |
| BLD-04 | The build authority owns no artifact evidence and allows no second supply-chain authority; every tuple resolves to `REQ-2026-0012`. | Single supply-chain audit path. | Split-brain provenance. |
| BLD-05 | The Template Build line stays distinct from the Template authority's cache semantics (`REQ-2026-0029` cachePolicy: this line records expected cache layer semantics, never implements a backend or eviction) and from the Pool warm slot (`REQ-2026-0019` evidence gate untouched). | No duplicate cache or capacity authority. | Two competing cache/persistence semantics. |
| BLD-06 | Real-builder-execution and build-artifact-tuple evidence stay release-blocking; this registration binds no real artifact today. | No assurance claims without evidence. | The build path ships on design evidence. |
| BLD-07 | Builder runtime, pipeline execution, artifact/cache storage, registry service, CLI, public API/SDK and deployment profiles stay forbidden until their own requirement slices. | No unreviewed surface ships behind the authority. | Capability drift ahead of review. |

## Blocking Findings

1. No builder runtime, pipeline execution, artifact or cache storage backend exists; builds cannot execute until a future authorized slice delivers them.
2. No CLI or API/SDK surface exists; build invocation and record lookup are unresolved.
3. Real builder-execution evidence and `REQ-2026-0012` artifact-tuple evidence are absent.
4. No released `REQ-2026-0012` artifact tuple exists yet, so no successful build can bind a real immutable artifact set today.
5. `WarmMicroVmSlot` (REQ-2026-0019) consumption of a build artifact additionally requires the pool evidence gate; no bridge slice is approved yet.

## Required Evidence Before Builder Parity Claims

- Authorize and land the builder runtime slice with its own requirement, supply-chain evidence and real execution tests.
- Prove real builder execution and artifact-tuple binding on fixed matrices; bind at least one real `REQ-2026-0012` artifact tuple to a succeeded build.
- Prove the WarmMicroVmSlot bridge through the REQ-2026-0019 pool evidence gate.
- Pass uncertainty, timeout and quarantine fault-injection suites.
- Prove cache-layer semantics against the `REQ-2026-0029` cachePolicy without implementing a backend.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`. `Approved with follow-up` cannot defer the public naming, the data-ownership split, the outcome-binding semantics, the single supply-chain authority or the forbidden surfaces.

| Reviewer role | Reviewer | Outcome | Date | Decisions |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-10-05 | BLD-01, BLD-02, BLD-05, BLD-07 |
| Security/privacy owner | Repository Owner (structured approval) | Approved | 2026-10-05 | BLD-03, BLD-06, BLD-07 |
| Supply-chain owner | Repository Owner (structured approval) | Approved | 2026-10-05 | BLD-03, BLD-04, BLD-06 |
| Capacity/scheduler owner | Repository Owner (structured approval) | Approved | 2026-10-05 | BLD-05, BLD-06, BLD-07 |
| Workspace/storage owner | Repository Owner (structured approval) | Approved | 2026-10-05 | BLD-02, BLD-04, BLD-07 |

## Implementation Gate

Since 2026-10-05: REQ-2026-0032 is `ready`, ADR-20261005 is `accepted`, and this Review is `accepted` (single-owner structured approval, see the approval basis above). The approval authorizes the authority-model implementation slice only: typed build records, lifecycle validation, outcome-binding rules and evidence-gate documentation with machine-contract alignment tests, landed as `crates/sdkwork-intelligence-sandbox-build-authority`. `specs/sandbox-template-build.contract.json` flips to `implementationAuthorized: true` for that slice while remaining `draft`. Builder runtime, pipeline execution, artifact/cache storage, registry service, CLI, public API/SDK and deployment profiles stay forbidden until their own requirement slices, and no E2B Template Build capability-parity claim may be made before a runtime slice exists.
