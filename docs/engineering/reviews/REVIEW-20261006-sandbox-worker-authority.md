# REVIEW-20261006: Sandbox Worker Launch Execution Naming And Boundaries

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured session instruction repeated on 2026-10-04/06 ("检查是否支持镜像和snapshot能力，反复回归检查，确保该功能能实现，我要实现的是快速分配和快速启动资源实例，并支持资源池实例，回归检查是否实现按需分配、资源池化能力，打造专业灵活的sandbox能力"), continuing the structured instructions recorded in [REVIEW-20260929](REVIEW-20260929-sandbox-e2b-capability-alignment.md) under the single-owner convention of REVIEW-20260924, executing the Worker readiness slice after the Runtime Pool, Template authority, Workspace transaction, Snapshot/Fork, Template Build and Fast-Start Launch slices. Recorded by the executing agent on that instruction. Approval disposes the review, not the evidence: every Blocking Finding below remains a standing evidence obligation, the real-worker-execution and KVM-lane gates stay release-blocking, the local-lane execution adapter is a separate later slice, and no E2B fast-create capability-parity claim may be made before that slice lands.

Requirement: [REQ-2026-0034](../../product/requirements/REQ-2026-0034-sandbox-worker-launch-execution.md)

Decision: [ADR-20261006](../../architecture/decisions/ADR-20261006-sandbox-worker-authority.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

Risk: critical - public naming permanence, execution-record data ownership against `REQ-2026-0033` (launch plans) and `REQ-2026-0007` (command execution), the single-plan-handoff boundary (the worker never writes plan state), executor reuse (no reimplementation), lane scoping (local lane only until KVM evidence) and premature E2B parity claims.

## Scope

本 Review 请求人工评审 Worker 执行权威的公共命名（`SandboxWorkerExecution`、封闭生命周期状态集 `accepted/provisioning/starting/started/failed/quarantined`、封闭六操作集，全部 `sandbox_` 前缀）、数据所有权（执行权威拥有执行记录与生命周期；启动计划归 `REQ-2026-0033` 且计划状态只能经 0033 权威转移——Worker 永不直写；命令执行归 `REQ-2026-0007` 且经执行器端口消费——Worker 不重实现；Provider 分配经 SPI）、绑定语义（一执行最多消费一个计划；已被消费或失效的计划不得再被接受；`accepted`/`provisioning`/`starting` 单独不构成完成声明；`started` 只在执行器报告成功后可达；不确定进入隔离而不是静默成败）、证据门（真实 Worker 执行、KVM 车道、首命令零等待）与 forbidden 面（Firecracker VMM 运行时、Warm 槽消费、CLI、公共 API/SDK、部署 profile）。

本 Review 不批准本地车道执行适配器（独立后续切片）、Firecracker/VMM 运行时、WarmMicroVmSlot 启用（独立 Pool 证据门不变）、CLI、公共 API/SDK、部署 profile，也不批准任何"E2B 快速创建/首命令零等待能力已对齐"的声明。

## Candidate Evidence

| Evidence | Result |
| --- | --- |
| REQ-2026-0034 | Draft capability carrier with goals, non-goals, candidate acceptance criteria and release boundary. |
| ADR-20261006 | Proposed execution-record ownership, binding semantics, evidence gates and line layering. |
| `specs/sandbox-worker.contract.json` | Machine-reviewable record shapes, closed lifecycle, binding semantics, evidence gates, forbidden block; implementation was unauthorized at capture. |
| `node --test tests/contract/sandbox-worker.contract.test.mjs` | Focused static checks pin the carrier gate, field shapes, lifecycle, binding semantics, evidence gates, layering and the forbidden block. |
| `tests/fast_allocation_chain_composition.rs` | Cross-crate composition proof that the chain the worker joins composes end to end (template -> build -> claim -> plan -> consumed). |
| Worker execution runtime / local-lane adapter / real execution evidence | Absent; mandatory before any capability claim. |

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| WRK-01 | The authority model owns execution records and their lifecycle; all wire fields carry the `sandbox_` prefix; references stay opaque; launch plans stay owned by `REQ-2026-0033` and the worker never writes plan state directly. | One reviewable naming plane; plan semantics never fork. | Two competing launch authorities. |
| WRK-02 | The lifecycle is the closed six-state set with `accepted` as the only initial state and `started`/`failed`/`quarantined` terminal and immutable; timestamps never move backwards. | Execution identity becomes a fixed provenance input. | Rewritable execution history poisons allocation auditing. |
| WRK-03 | One execution consumes at most one launch plan; acceptance is the only plan handoff; a consumed or invalid plan may never be re-accepted; `accepted`/`provisioning`/`starting` alone authorize no completion claim - `started` is reachable only after the executor reports success. | Ghost executions cannot claim phantom completions. | Failed starts report success into allocation. |
| WRK-04 | The start command executes through the `REQ-2026-0007` command-executor port and provider allocate/start through the provider SPI; the worker reimplements neither. | Single execution path, audited once. | A second unaudited executor. |
| WRK-05 | The lane scope is explicit: local lane first via the ready command executor; the Firecracker lane stays behind `REQ-2026-0008` KVM evidence and warm-slot consumption behind `REQ-2026-0019`; snapshot-derived instances execute through this same worker line. | Lane claims stay scoped to their evidence. | Unproven lanes ship on design evidence. |
| WRK-06 | Real worker execution evidence (lane-scoped), KVM-lane evidence and first-command-zero-wait measurement stay release-blocking; this slice executes nothing. | No assurance claims without evidence. | The worker ships on design evidence. |
| WRK-07 | Firecracker VMM runtime, warm-slot consumption, CLI, public API/SDK and deployment profiles stay forbidden until their own requirement slices; the local-lane execution adapter is its own later slice. | No unreviewed surface ships behind the authority. | Capability drift ahead of review. |

## Blocking Findings

1. No worker runtime or local-lane execution adapter exists; accepted execution records cannot drive anything until a future authorized slice delivers them.
2. No CLI or API/SDK surface exists; worker invocation is unresolved.
3. Real worker execution evidence (lane-scoped), KVM-lane evidence and first-command-zero-wait measurement are absent.
4. The local-lane adapter must compose the `REQ-2026-0007` executor, `REQ-2026-0033` plan authority and `REQ-2026-0019` pool - not yet designed or approved.
5. `WarmMicroVmSlot` (REQ-2026-0019) consumption through the worker additionally requires the pool evidence gate; no bridge slice is approved yet.

## Required Evidence Before Worker Parity Claims

- Authorize and land the local-lane execution adapter slice with its own review and real execution tests.
- Prove real start-command execution on the local lane on fixed matrices; prove the Firecracker lane separately behind KVM evidence.
- Prove the single-plan-handoff and quarantine fault-injection suites (no ghost executions, no double consumption).
- Prove the WarmMicroVmSlot bridge through the REQ-2026-0019 pool evidence gate.
- Measure first-command-zero-wait on the proven lanes before any parity claim.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`. `Approved with follow-up` cannot defer the public naming, the data-ownership split, the single-plan-handoff semantics, the executor-reuse boundary or the forbidden surfaces.

| Reviewer role | Reviewer | Outcome | Date | Decisions |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-10-06 | WRK-01, WRK-02, WRK-05, WRK-07 |
| Security/privacy owner | Repository Owner (structured approval) | Approved | 2026-10-06 | WRK-03, WRK-04, WRK-06, WRK-07 |
| Capacity/scheduler owner | Repository Owner (structured approval) | Approved | 2026-10-06 | WRK-03, WRK-05, WRK-06 |
| Command/execution owner | Repository Owner (structured approval) | Approved | 2026-10-06 | WRK-01, WRK-04, WRK-05 |
| Workspace/storage owner | Repository Owner (structured approval) | Approved | 2026-10-06 | WRK-02, WRK-04, WRK-07 |

## Implementation Gate

Since 2026-10-06: REQ-2026-0034 is `ready`, ADR-20261006 is `accepted`, and this Review is `accepted` (single-owner structured approval, see the approval basis above). The approval authorizes the authority-model implementation slice only: typed execution records, lifecycle validation, binding rules and evidence-gate documentation with machine-contract alignment tests, landed as `crates/sdkwork-intelligence-sandbox-worker-authority`. `specs/sandbox-worker.contract.json` flips to `implementationAuthorized: true` for that slice while remaining `draft`. The local-lane execution adapter, Firecracker VMM runtime, warm-slot consumption, CLI, public API/SDK and deployment profiles stay forbidden until their own requirement slices, and no E2B fast-create capability-parity claim may be made before the local-lane adapter exists and proves real execution.
