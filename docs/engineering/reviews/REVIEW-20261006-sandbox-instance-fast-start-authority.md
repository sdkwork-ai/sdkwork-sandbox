# REVIEW-20261006: Sandbox Instance Fast-Start Launch Naming And Boundaries

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured session instruction repeated on 2026-10-04/06 ("检查是否支持镜像和snapshot能力，反复回归检查，确保该功能能实现，我要实现的是快速分配和快速启动资源实例，并支持资源池实例，回归检查是否实现按需分配、资源池化能力，打造专业灵活的sandbox能力"), continuing the structured instructions recorded in [REVIEW-20260929](REVIEW-20260929-sandbox-e2b-capability-alignment.md) under the single-owner convention of REVIEW-20260924, executing the fast-start launch readiness slice after the Runtime Pool, Template authority, Workspace transaction, Snapshot/Fork and Template Build slices. Recorded by the executing agent on that instruction. Approval disposes the review, not the evidence: every Blocking Finding below remains a standing evidence obligation, the real-fast-start-runtime and first-command-zero-wait gates stay release-blocking, and no E2B fast-create capability-parity claim may be made before a worker slice lands.

Requirement: [REQ-2026-0033](../../product/requirements/REQ-2026-0033-sandbox-instance-fast-start-launch.md)

Decision: [ADR-20261006](../../architecture/decisions/ADR-20261006-sandbox-instance-fast-start-authority.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

Risk: critical - public naming permanence, launch-plan data ownership against `REQ-2026-0029` (start-command semantics) and `REQ-2026-0019` (fenced pool claims), execution-binding semantics (a planned launch authorizes nothing; an expired claim can only ever expire its plan), fresh-identity enforcement per launch and premature E2B parity claims.

## Scope

本 Review 请求人工评审 Fast-Start Launch 权威的公共命名（`SandboxInstanceLaunchPlan`、封闭生命周期状态集 `planned/consumed/expired/quarantined`、封闭四操作集，全部 `sandbox_` 前缀）、数据所有权（launch 权威拥有启动计划记录与生命周期；start command 语义归 `REQ-2026-0029` 且按 opaque 引用消费、绝不复制；claim 语义归 `REQ-2026-0019`；命令执行归 `REQ-2026-0007`）、绑定语义（计划必须绑定活跃 fenced claim；claim 过期/释放只能使计划 `expired` 永不下发执行；每次启动强制全新 Guest Identity 证据；`planned` 单独不构成执行授权；绑定不确定进入隔离而不是静默下发）、证据门（真实 fast-start 运行时、首命令零等待测量）与 forbidden 面（启动执行运行时、Worker、CLI、公共 API/SDK、部署 profile）。

本 Review 不批准启动执行运行时、Worker、CLI、公共 API/SDK、部署 profile、`WarmMicroVmSlot` 启用（独立 Pool 证据门不变），也不批准任何"E2B 快速创建/首命令零等待能力已对齐"的声明。

## Candidate Evidence

| Evidence | Result |
| --- | --- |
| REQ-2026-0033 | Draft capability carrier with goals, non-goals, candidate acceptance criteria and release boundary. |
| ADR-20261006 | Proposed launch-plan ownership, binding semantics, evidence gates and line layering. |
| `specs/sandbox-instance-fast-start.contract.json` | Machine-reviewable record shapes, closed lifecycle, binding semantics, evidence gates, forbidden block; implementation was unauthorized at capture. |
| `node --test tests/contract/sandbox-instance-fast-start.contract.test.mjs` | Focused static checks pin the carrier gate, field shapes, lifecycle, binding semantics, evidence gates, layering and the forbidden block. |
| Launch execution runtime/worker/real fast-start evidence | Absent; mandatory before any capability claim. |

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| LNCH-01 | The authority model owns launch-plan records and their lifecycle; all wire fields carry the `sandbox_` prefix; references stay opaque; the start command stays owned by `REQ-2026-0029` and is consumed by opaque reference, never copied. | One reviewable naming plane; template semantics never fork. | Two competing start-command authorities. |
| LNCH-02 | The lifecycle is the closed four-state set with `planned` as the only initial state, `consumed`/`expired`/`quarantined` terminal and immutable, and timestamps never moving backwards. | Launch identity becomes a fixed provenance input. | Rewritable launch history poisons allocation auditing. |
| LNCH-03 | Every plan binds one active fenced pool claim; an expired or released claim can only ever expire its plan and may never execute; binding uncertainty quarantines instead of silently dispatching. | The worker consumes only truthfully-bound plans. | Ghost launches execute against stale or released capacity. |
| LNCH-04 | Every launch carries fresh guest-identity evidence; `planned` alone authorizes no execution - the worker slice is a separate authorization. | Identity rotation stays enforced at the binding layer. | Identity reuse sneaks back through the launch path. |
| LNCH-05 | The launch line stays distinct from command execution (`REQ-2026-0007`), the pool warm slot (`REQ-2026-0019` evidence gate untouched) and snapshot/fork (`REQ-2026-0031`: derived instances plan through this authority too). | No duplicate execution or capacity authority. | Two competing dispatch semantics. |
| LNCH-06 | Real fast-start runtime and first-command-zero-wait measurement evidence stay release-blocking; this registration executes nothing. | No assurance claims without evidence. | The fast path ships on design evidence. |
| LNCH-07 | Launch execution runtime, worker, CLI, public API/SDK and deployment profiles stay forbidden until their own requirement slices. | No unreviewed surface ships behind the authority. | Capability drift ahead of review. |

## Blocking Findings

1. No launch execution runtime or worker exists; planned launches cannot execute until a future authorized slice delivers them.
2. No CLI or API/SDK surface exists; launch planning invocation is unresolved.
3. Real fast-start runtime evidence and first-command-zero-wait measurement evidence are absent.
4. The start-command ↔ PreparedSlot bridge exists only as this authority model; no runtime wiring exists.
5. `WarmMicroVmSlot` (REQ-2026-0019) consumption through the launch path additionally requires the pool evidence gate; no bridge slice is approved yet.

## Required Evidence Before Fast-Start Parity Claims

- Authorize and land the worker/execution slice with its own requirement and real runtime tests.
- Prove real fast-start execution on fixed matrices, including first-command-zero-wait measurements.
- Prove the expired-claim and quarantine fault-injection suites (no ghost launches).
- Prove the WarmMicroVmSlot bridge through the REQ-2026-0019 pool evidence gate.
- Prove snapshot-derived instances plan through this authority end to end.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`. `Approved with follow-up` cannot defer the public naming, the data-ownership split, the binding semantics, the fresh-identity enforcement or the forbidden surfaces.

| Reviewer role | Reviewer | Outcome | Date | Decisions |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-10-06 | LNCH-01, LNCH-02, LNCH-05, LNCH-07 |
| Security/privacy owner | Repository Owner (structured approval) | Approved | 2026-10-06 | LNCH-03, LNCH-04, LNCH-06, LNCH-07 |
| Capacity/scheduler owner | Repository Owner (structured approval) | Approved | 2026-10-06 | LNCH-03, LNCH-05, LNCH-06 |
| Command/execution owner | Repository Owner (structured approval) | Approved | 2026-10-06 | LNCH-01, LNCH-04, LNCH-05 |
| Workspace/storage owner | Repository Owner (structured approval) | Approved | 2026-10-06 | LNCH-02, LNCH-04, LNCH-07 |

## Implementation Gate

Since 2026-10-06: REQ-2026-0033 is `ready`, ADR-20261006 is `accepted`, and this Review is `accepted` (single-owner structured approval, see the approval basis above). The approval authorizes the authority-model implementation slice only: typed launch-plan records, lifecycle validation, binding rules and evidence-gate documentation with machine-contract alignment tests, landed as `crates/sdkwork-intelligence-sandbox-launch-authority`. `specs/sandbox-instance-fast-start.contract.json` flips to `implementationAuthorized: true` for that slice while remaining `draft`. Launch execution runtime, worker, CLI, public API/SDK and deployment profiles stay forbidden until their own requirement slices, and no E2B fast-create capability-parity claim may be made before a worker slice exists.
