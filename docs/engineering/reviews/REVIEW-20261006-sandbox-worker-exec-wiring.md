# REVIEW-20261006: Sandbox Worker Local-Lane Start-Command Executor Wiring

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured session instruction repeated on 2026-10-04/06 ("检查是否支持镜像和snapshot能力，反复回归检查，确保该功能能实现，我要实现的是快速分配和快速启动资源实例，并支持资源池实例，回归检查是否实现按需分配、资源池化能力，打造专业灵活的sandbox能力"), continuing [REVIEW-20260929](REVIEW-20260929-sandbox-e2b-capability-alignment.md) under the single-owner convention of REVIEW-20260924, executing the executor wiring named by EXE-06 of [REVIEW-20261006-local-lane](REVIEW-20261006-sandbox-worker-local-lane.md). Recorded by the executing agent on that instruction. Approval disposes the review, not the evidence: the start command source (template-version resolution) is the named next slice, the KVM lane stays locked, and no E2B fast-create capability-parity claim may be made before that source slice lands.

Requirement: [REQ-2026-0034](../../product/requirements/REQ-2026-0034-sandbox-worker-launch-execution.md)

Decision: [ADR-20261006](../../architecture/decisions/ADR-20261006-sandbox-worker-local-lane.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

Risk: moderate - sync-over-async bridging, request scoping, and outcome honesty (a no-terminal-outcome executor error must never report `started`).

## Scope

本 Review 评审执行器接线：新适配器 crate 实现 `SandboxLaunchStartCommandPort`，桥接至 `REQ-2026-0007` 已就绪的 `SandboxLocalCommandExecutor`（真实 tokio 进程）；请求按构造期注入的命令与作用域构造，operation id 从计划引用派生；executor 无终态结果的错误映射为端口 `Uncertain`（quarantined），非零退出映射为 `Failed`，零退出映射为 `Started`；tokio 桥为适配器自有的专用多线程 runtime。

本 Review 不批准 start command 的模板版本解析（命名中的下一切片）、Firecracker/VMM、WarmMicroVmSlot、CLI、公共 API/SDK、部署 profile，也不批准任何"E2B 快速创建能力已对齐"的声明。

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| WRE-01 | The wiring implements the existing narrow port; no port signature changes. | The landed composition engine stays untouched. | Signature churn ripples into landed slices. |
| WRE-02 | The async executor is bridged through the adapter's own dedicated multi-thread tokio runtime. | A sync port with a real process behind it. | Sync-over-async panics inside foreign runtimes. |
| WRE-03 | Executor errors without a terminal outcome map to port `Uncertain` (quarantined); non-zero exits map to `Failed`; only zero exits report `Started`. | Outcome honesty end to end. | Ghost starteds. |
| WRE-04 | The command, scope and limits are injected at construction; template-version resolution is the named next slice. | Auditable, bounded wiring. | The adapter re-reads template state. |

## Blocking Findings

1. The start command source (template-version resolution through `REQ-2026-0029`) does not exist; the executed command is construction-injected.
2. Real worker execution evidence and first-command-zero-wait measurement are lane-scoped to this wiring's tests only.
3. Firecracker lane and warm slot remain gated.

## Required Evidence Before Fast-Start Parity Claims

- Land the template-resolution slice; prove the executed command comes from the published template version.
- Measure first-command-zero-wait on the proven lane; prove the Firecracker lane behind KVM evidence.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`.

| Reviewer role | Reviewer | Outcome | Date | Decisions |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-10-06 | WRE-01, WRE-02 |
| Security/privacy owner | Repository Owner (structured approval) | Approved | 2026-10-06 | WRE-03, WRE-04 |
| Capacity/scheduler owner | Repository Owner (structured approval) | Approved | 2026-10-06 | WRE-03 |
| Command/execution owner | Repository Owner (structured approval) | Approved | 2026-10-06 | WRE-02, WRE-04 |
| Workspace/storage owner | Repository Owner (structured approval) | Approved | 2026-10-06 | WRE-01, WRE-04 |

## Implementation Gate

Since 2026-10-06: this Review is `accepted` (single-owner structured approval) and the wiring landed as `crates/sdkwork-intelligence-sandbox-worker-local-exec`. The template-resolution slice, Firecracker VMM runtime, warm-slot consumption, CLI, public API/SDK and deployment profiles stay forbidden until their own slices.
