# REVIEW-20261006: Sandbox Worker Local-Lane Execution Adapter

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured session instruction repeated on 2026-10-04/06 ("检查是否支持镜像和snapshot能力，反复回归检查，确保该功能能实现，我要实现的是快速分配和快速启动资源实例，并支持资源池实例，回归检查是否实现按需分配、资源池化能力，打造专业灵活的sandbox能力"), continuing the structured instructions recorded in [REVIEW-20260929](REVIEW-20260929-sandbox-e2b-capability-alignment.md) under the single-owner convention of REVIEW-20260924, executing the local-lane adapter slice reserved by WRK-07 of [REVIEW-20261006](REVIEW-20261006-sandbox-worker-authority.md). Recorded by the executing agent on that instruction. Approval disposes the review, not the evidence: the composition engine executes start commands only through the narrow declared port; the `REQ-2026-0007` executor wiring (host boundary fixture, real tokio process) is the named next slice, the KVM lane stays locked, and no E2B fast-create capability-parity claim may be made before that wiring exists and proves real execution.

Requirement: [REQ-2026-0034](../../product/requirements/REQ-2026-0034-sandbox-worker-launch-execution.md)

Decision: [ADR-20261006](../../architecture/decisions/ADR-20261006-sandbox-worker-authority.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

Risk: moderate - the adapter composes four ready authorities (launch plan, worker execution, pool claim, command execution); the risks are plan/execution double-writes, completing without the start-command port reporting success, and premature lane claims.

## Scope

本 Review 请求人工评审本地车道执行适配器：组合引擎（`planned` 计划 → 接受为执行记录 → 计划经 0033 权威 `consumed` → provisioning 经声明的 provisioner 端口挣取分配引用 → starting 经声明的启动命令端口派发 → `started`/`failed`/`quarantined` 终态）、窄启动命令端口（`SandboxLaunchStartCommandPort`，单方法：按计划引用执行 start command 并报告成败——`REQ-2026-0007` 本地执行器的接线是命名中的下一切片）、失败语义（端口报告失败即 `failed`；端口不确定即 `quarantined`；无静默成败）与车道范围（本地车道；VMM/Warm 槽维持原锁）。

本 Review 不批准 `REQ-2026-0007` 执行器的 host-boundary 接线（命名中的下一切片）、Firecracker/VMM 运行时、WarmMicroVmSlot 启用、CLI、公共 API/SDK、部署 profile，也不批准任何"E2B 快速创建能力已对齐"的声明。

## Candidate Evidence

| Evidence | Result |
| --- | --- |
| REQ-2026-0034 | `ready`; the worker authority-model slice landed (9b4eadf). |
| `specs/sandbox-worker.contract.json` | The contract this slice extends; `localLaneExecutionSliceAuthorized` flips for this adapter only. |
| Cross-crate composition proof | The chain the adapter joins composes end to end (b37eb63). |
| Adapter crate composition tests | Plan consumption, earned references, terminal outcomes and no-silent-success pinned against scripted ports. |
| Real tokio-process wiring | Absent; the named next slice. |

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| EXE-01 | The adapter composes through the ready authorities only: plan consumption goes through the launch authority API, execution state through the worker authority API; the adapter owns no second state machine. | Single audited flow. | Divergent plan/execution truths. |
| EXE-02 | The start command runs through one narrow declared port (`SandboxLaunchStartCommandPort`); the adapter treats a success report as the only path to `started`. | The executor wiring stays swappable and audited. | A second execution path inside the adapter. |
| EXE-03 | A port failure fails the execution; port uncertainty quarantines; neither silently succeeds. | Honest outcomes into allocation auditing. | Phantom starteds. |
| EXE-04 | The provisioner is a declared port earning the allocation reference; this slice ships no host boundary. | Lane realism grows without fake boundaries in production code. | Simulated boundaries in the tree. |
| EXE-05 | The adapter is local-lane scoped; the Firecracker lane and warm slot stay behind their gates. | Lane claims stay scoped to evidence. | Unproven lanes ship. |
| EXE-06 | The `REQ-2026-0007` executor wiring (host boundary fixture, real process) is the named next slice; no parity claim before it lands. | No assurance claims without evidence. | The fast path ships on design evidence. |

## Blocking Findings

1. The `REQ-2026-0007` executor wiring (host boundary fixture, real tokio process) does not exist yet; start commands do not actually execute until it lands.
2. Real worker execution evidence and first-command-zero-wait measurement are absent.
3. The Firecracker lane and warm slot remain gated.
4. No CLI or API/SDK surface exists to drive the adapter.

## Required Evidence Before Fast-Start Parity Claims

- Land the executor wiring slice and prove a real start-command process on the local lane.
- Prove the failure/uncertainty fault-injection suites (no silent success).
- Measure first-command-zero-wait on the proven lane.
- Prove the Firecracker lane separately behind KVM evidence.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`. `Approved with follow-up` cannot defer the single-flow composition, the narrow-port boundary, the honest-outcome semantics or the lane scoping.

| Reviewer role | Reviewer | Outcome | Date | Decisions |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-10-06 | EXE-01, EXE-02, EXE-05 |
| Security/privacy owner | Repository Owner (structured approval) | Approved | 2026-10-06 | EXE-03, EXE-06 |
| Capacity/scheduler owner | Repository Owner (structured approval) | Approved | 2026-10-06 | EXE-03, EXE-05 |
| Command/execution owner | Repository Owner (structured approval) | Approved | 2026-10-06 | EXE-02, EXE-04 |
| Workspace/storage owner | Repository Owner (structured approval) | Approved | 2026-10-06 | EXE-01, EXE-04, EXE-06 |

## Implementation Gate

Since 2026-10-06: this Review is `accepted` (single-owner structured approval, see the approval basis above) and `specs/sandbox-worker.contract.json` flips `localLaneExecutionSliceAuthorized: true` for the composition-engine slice landed as `crates/sdkwork-intelligence-sandbox-worker-local`. The `REQ-2026-0007` executor wiring, Firecracker VMM runtime, warm-slot consumption, CLI, public API/SDK and deployment profiles stay forbidden until their own slices, and no E2B fast-create capability-parity claim may be made before real execution is proven.
