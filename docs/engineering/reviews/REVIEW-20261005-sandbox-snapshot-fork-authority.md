# REVIEW-20261005: Sandbox Snapshot And Fork Naming And Boundaries

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured session instruction repeated on 2026-10-04/05 ("检查是否支持镜像和snapshot能力，反复回归检查，确保该功能能实现，我要实现的是快速分配和快速启动资源实例，并支持资源池实例，回归检查是否实现按需分配、资源池化能力，打造专业灵活的sandbox能力"), continuing the structured instructions recorded in [REVIEW-20260929](REVIEW-20260929-sandbox-e2b-capability-alignment.md) under the single-owner convention of REVIEW-20260924, executing the Snapshot/Fork readiness slice after the Runtime Pool, Template authority and Workspace transaction slices. Recorded by the executing agent on that instruction. Approval disposes the review, not the evidence: every Blocking Finding below remains a standing evidence obligation, the real-KVM restore/residue/identity-rotation gates stay release-blocking, and no E2B Snapshot/Fork capability-parity claim may be made before an engine slice lands.

Requirement: [REQ-2026-0031](../../product/requirements/REQ-2026-0031-sandbox-snapshot-and-fork.md)

Decision: [ADR-20261005](../../architecture/decisions/ADR-20261005-sandbox-snapshot-fork-authority.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-05

Risk: critical - public naming permanence, snapshot data ownership against `REQ-2026-0012` and `REQ-2026-0021`, fork consistency semantics (parallel derived sandboxes with fresh identity), WarmMicroVmSlot gate interaction and premature E2B parity claims.

## Scope

本 Review 请求人工评审 Snapshot/Fork 权威的公共命名（`SandboxSnapshot`、封闭生命周期状态集、`SandboxForkDerivation` 语义旗标，全部 `sandbox_` 前缀）、数据所有权（快照权威拥有快照记录与生命周期；制品 Evidence 归 `REQ-2026-0012`；Workspace Checkpoint 权威归 `REQ-2026-0021`；不拥有租户数据）、Fork 一致性语义边界（来源不可变、N 派生、派生体全新 Guest Identity 与独立租户授权、并行运行）、证据门（真实 KVM 恢复、跨租户残留、身份轮换）与 forbidden 面（快照引擎、存储后端、恢复流水线、CLI、公共 API/SDK、部署 profile）。

本 Review 不批准快照引擎运行时、存储后端、恢复流水线、CLI、公共 API/SDK、部署 profile、`WarmMicroVmSlot` 启用（独立 KVM 证据门不变），也不批准任何"E2B Snapshot/Fork 能力已对齐"的声明。

## Candidate Evidence

| Evidence | Result |
| --- | --- |
| REQ-2026-0031 | Draft capability carrier with goals, non-goals, candidate acceptance criteria and release boundary. |
| ADR-20261005 | Proposed snapshot-record ownership, fork consistency semantics, evidence gates and line layering. |
| `specs/sandbox-snapshot-fork.contract.json` | Machine-reviewable record shapes, closed lifecycle, fork semantics flags, evidence gates, forbidden block; implementation was unauthorized at capture. |
| `specs/sandbox-e2b-capability-baseline.json` rows 20-24 | E2B snapshot-and-fork surface captured at field level. |
| `node --test tests/contract/sandbox-snapshot-fork.contract.test.mjs` | Focused static checks pin the draft gate, field shapes, lifecycle, fork semantics, evidence gates, layering and the forbidden block. |
| Snapshot engine/storage/restore/KVM evidence | Absent; mandatory before any capability claim. |

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| SNAP-01 | The authority model owns snapshot records and their lifecycle; all wire fields carry the `sandbox_` prefix; references stay opaque. | One reviewable naming plane for later snapshot slices. | Rework naming before any implementation. |
| SNAP-02 | Snapshots are immutable after creation; deletion is deterministic and uncertain deletion quarantines instead of silently disappearing. | Snapshot identity becomes a fixed derivation input. | Snapshots cannot back reproducible forks. |
| SNAP-03 | Every snapshot binds exactly one `REQ-2026-0012` artifact tuple; the authority owns no evidence and allows no second supply-chain authority. | Single supply-chain audit path. | Split-brain provenance. |
| SNAP-04 | Fork derivation keeps the source immutable, allows N derived sandboxes running in parallel, and every derived sandbox gets fresh Guest Identity and independent tenant grants. | Parallel capacity scaling with a reviewable consistency boundary. | Fork stays a manual, unaudited operation. |
| SNAP-05 | The runtime full-state snapshot (this line) stays distinct from the Workspace transaction checkpoint (`REQ-2026-0021`) and from Workspace business deletion (`sdkwork-agents`). | No duplicate persistence authority. | Two competing durability semantics. |
| SNAP-06 | Real-KVM restore, cross-tenant residue and derived-identity rotation evidence stay release-blocking; `WarmMicroVmSlot` reuse additionally requires the REQ-2026-0019 pool evidence gate. | No assurance claims without evidence. | Warm path ships on design evidence. |
| SNAP-07 | Engine runtime, storage backend, restore pipeline, CLI, public API/SDK and deployment profiles stay forbidden until their own requirement slices. | No unreviewed surface ships behind the authority. | Capability drift ahead of review. |

## Blocking Findings

1. No snapshot engine, storage backend or restore pipeline exists; snapshots cannot be created or restored until a future authorized slice delivers them.
2. No CLI or API/SDK surface exists; snapshot lookup and lifecycle invocation are unresolved.
3. Real Linux KVM x86_64/aarch64 restore, cross-tenant residue and derived-identity rotation evidence are absent.
4. `WarmMicroVmSlot` (REQ-2026-0019) reuse of a snapshot additionally requires the pool evidence gate; no bridge slice is approved yet.
5. No released `REQ-2026-0012` artifact tuple exists yet, so no snapshot can bind a real immutable artifact set today.

## Required Evidence Before Engine Parity Claims

- Authorize and land the engine/storage slice with its own requirement, supply-chain evidence and real restore tests.
- Prove real Linux KVM restore, cross-tenant residue absence and derived-identity rotation on fixed matrices.
- Prove the WarmMicroVmSlot bridge through the REQ-2026-0019 pool evidence gate.
- Bind at least one real `REQ-2026-0012` artifact tuple to a created snapshot.
- Pass fork consistency, parallel-derivation and quarantine fault-injection suites.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`. `Approved with follow-up` cannot defer the public naming, the data-ownership split, the fork consistency semantics, the single supply-chain authority or the forbidden surfaces.

| Reviewer role | Reviewer | Outcome | Date | Decisions |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-10-05 | SNAP-01, SNAP-02, SNAP-05, SNAP-07 |
| Security/privacy owner | Repository Owner (structured approval) | Approved | 2026-10-05 | SNAP-04, SNAP-06, SNAP-07 |
| Supply-chain owner | Repository Owner (structured approval) | Approved | 2026-10-05 | SNAP-03, SNAP-06 |
| Firecracker/KVM operations owner | Repository Owner (structured approval) | Approved | 2026-10-05 | SNAP-04, SNAP-06 |
| Capacity/scheduler owner | Repository Owner (structured approval) | Approved | 2026-10-05 | SNAP-04, SNAP-07 |
| Workspace/storage owner | Repository Owner (structured approval) | Approved | 2026-10-05 | SNAP-02, SNAP-05, SNAP-07 |

## Implementation Gate

Since 2026-10-05: REQ-2026-0031 is `ready`, ADR-20261005 is `accepted`, and this Review is `accepted` (single-owner structured approval, see the approval basis above). The approval authorizes the authority-model implementation slice only: typed snapshot records, lifecycle validation, fork derivation semantics and evidence-gate documentation with machine-contract alignment tests. `specs/sandbox-snapshot-fork.contract.json` flips to `implementationAuthorized: true` for that slice while remaining `draft`. Engine runtime, storage backend, restore pipeline, CLI, public API/SDK and deployment profiles stay forbidden until their own requirement slices, and no E2B Snapshot/Fork capability-parity claim may be made before an engine slice exists.
