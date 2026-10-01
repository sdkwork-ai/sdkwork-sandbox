# REQ-2026-0019: Sandbox Runtime Pool And Fast Allocation

id: REQ-2026-0019

title: 交付可清理、可审计的 Sandbox Runtime Pool 与快速分配

owner: SDKWork Runtime Platform

status: ready

source: customer

## Problem

云端 Agent 按需启动时，逐次执行节点选择、制品物化、Host 资源准备和 microVM 冷启动会放大首命令时延。直接复用已承载租户 Workspace、Secret、Network Policy 或 Guest Identity 的 microVM 又会产生跨租户残留、旧 Fencing Token 复活、容量超卖和计量归属错误。当前 PRD 声明 Pool 产品目标，而 Firecracker 第一版明确不包含 Warm Pool；本 Requirement 与对应 ADR/机器合同将该能力拆为可独立评审的后续切片，当前仍未授权实现。

## Goals

- 建立 provider-neutral `SandboxRuntimePool`、`SandboxPoolSlot` 和 `SandboxPoolClaim` 语义，使 Kernel 只提交 Capability、Minimum Assurance 和已授权 Workspace/Session Identity，不选择具体 Node、Pool 或 Provider。
- 第一阶段交付不含租户状态的 `PreparedSlot`：可信节点、不可变制品、运行目录身份和可预备的 Host 资源已验证，但 Provider Allocate/Start 仍在租户 Claim 后执行。
- 第二阶段只在真实 Linux KVM Snapshot、身份轮换、设备重绑定、网络/资源重验证和跨租户残留证据通过后交付 `WarmMicroVmSlot`。
- 将 Admission Reservation、Verified Node Inventory、Capacity Reservation、Pool Claim、Provider Allocation 和 Workspace/Network/Resource Grant 串成可恢复且有 Fencing 的单一分配链。
- 在公开参考环境和固定工作负载中证明 Pool Claim 到 Sandbox Running Ready 的 p50/p95/p99；产品目标为 p95 小于 500 ms，未达标时不得把目标写成已实现 SLO。

## Non-goals

- Pool 不拥有 IAM、价格、账单、支付、Agent 业务 Session 或 Workspace 业务生命周期。
- Pool 不缓存租户 Workspace 数据、Secret 值、租户 Network Grant、Guest Credential、命令结果或 Provider-private Metadata。
- Pool 不允许 Local、Docker 或低 Assurance Provider 作为 Firecracker 容量不足时的回退。
- 本 Requirement 不批准 Public API、SDK、Deployment Profile、Kubernetes、多区域调度、GPU 或 Browser Sandbox。
- 本 Requirement 不批准实现；必须先完成人工评审并将依赖 Requirement/ADR 和机器门禁升级到可实施状态。

## Acceptance Criteria

1. `SandboxPoolSlotState` 至少区分 `Preparing`、`Ready`、`Claiming`、`Claimed`、`Sanitizing`、`Quarantined` 和 `Retired`；非法转换确定性失败。
2. 每个 Claim 绑定 `tenant_id`、`sandbox_session_id`、`sandbox_runtime_binding_id`、`sandbox_operation_id`、单调 `sandbox_fencing_token`、Capacity Reservation Revision 和不可变 Request Fingerprint。
3. 同 Operation + Fingerprint 重放返回同一 Claim；不同 Fingerprint 冲突；旧 Fencing Token 在任何 Host 或 Provider Side Effect 前失败。
4. Scheduler 只从新鲜、Active、Attested 且 Artifact/Policy/Capacity Revision 匹配的 Verified Node Inventory 选择候选；Confirmed Capacity Reservation 必须先于 Pool Claim 和 Provider Allocate。
5. `Ready` Slot 不包含任何租户数据或授权。Claim 后必须应用新的 Workspace、Network、Resource 和 Guest Identity 绑定，并回读全部 Effective Evidence 后才允许 Session 进入 Running。
6. Release 必须停止 Guest、撤销短期 Grant/Identity、分离并清理 Workspace/Network/Resource、清除 Ephemeral Layer、完成 Residue Scan；任一步骤不确定时进入 `Quarantined`，不得回到 `Ready` 或释放容量计数。
7. Pool Reconciler 使用有界 Batch、Lease/Fencing、数据库时间、At-least-once Idempotency 和 Tenant-aware Fairness；禁止 Process-local Memory 作为 Cloud Authority、无界扫描或 TTL 猜测释放 Claimed Capacity。
8. `PreparedSlot` 和 `WarmMicroVmSlot` 分别报告命中率、Claim/Ready 延时、补池延时、Quarantine、容量空闲和冷启动回退原因；Metric 标签低基数且不含 Tenant/Session/Node Raw ID。
9. Pool Usage Fact 与 Sandbox Resource Usage Fact 可关联但不成为 Billing Truth；Commerce 只消费经持久化验证的计量事实。
10. 压测和故障测试覆盖并发 Claim、双控制器竞争、Node Drain、VMM Crash、控制面重启、Grant 过期、清理失败、残留检测、池耗尽和回补风暴。
11. `sdkwork-kernel` 的 `SandboxSessionLifecycleAdapter` 继续只传递 Agents Identity、所需 Capability 和 Minimum Assurance；legacy one-shot Kernel `SandboxProvider` 不得成为 Pool 或 Sandbox 生命周期权威。
12. Standalone Local Provider 不宣称多租户 Pool；如需本地预备能力，只能作为同一 Contract 的单节点非多租户 Adapter 并单独评审。

## Non-functional Requirements

| Area | Requirement |
| --- | --- |
| Security | Slot 入池前必须证明无租户 Workspace、Secret、Credential、Network Grant 和 Provider-private 残留；不确定即 Quarantine。 |
| Privacy | Pool 状态、日志、指标和事件不得暴露 Raw Tenant/Session/Node Identity 或 Host Path。 |
| Performance | 记录固定硬件、Artifact Tuple、池配置、样本量和工作负载；分别统计 Prepared、Warm 和 Cold 路径。 |
| Reliability | Claim/Release/Scale/Reconcile 全部有界、幂等、可恢复；容量不确定时宁可少卖不可超卖。 |

## Affected Surfaces

- backend
- composition
- database
- observability
- deployment (future, not authorized by this draft)
- cross-repository Kernel integration (human review required)

## Dependencies

- REQ-2026-0005 durable lifecycle persistence and reconciliation
- REQ-2026-0007 provider-neutral command execution
- REQ-2026-0008 Firecracker Provider
- REQ-2026-0009 Service Host composition
- REQ-2026-0010 observability, audit and outbox
- REQ-2026-0011 Host Isolation Broker
- REQ-2026-0012 Firecracker artifact compatibility
- REQ-2026-0013 Workspace block-device attachment and sanitization
- REQ-2026-0014 network isolation
- REQ-2026-0015 resource isolation and usage facts
- REQ-2026-0016 admission, scheduling and capacity
- REQ-2026-0017 node trust and verified inventory
- REQ-2026-0018 PostgreSQL quota/capacity persistence

## Decision

[ADR-20260730: Sandbox Runtime Pool Claim And Sanitization](../../architecture/decisions/ADR-20260730-sandbox-runtime-pool-claim-and-sanitization.md).

## Verification

- Static contract tests for state, identity, ordering, fencing, fail-closed cleanup and forbidden tenant residue after a machine contract is approved.
- PostgreSQL multi-controller Claim/Release/Recovery tests with real constraints, lock order, query plans and PITR evidence.
- Real Linux KVM x86_64/aarch64 conformance for Prepared and, when separately approved, Warm slots.
- Cross-repository Kernel adapter conformance proving no Provider/Node/Pool branching and no legacy Sandbox lifecycle expansion.
- Performance report with cold/Prepared/Warm distributions and saturation behavior.

## Implementation Gate

This Requirement is `ready` since 2026-09-29: the REVIEW-20260730 packet was approved for every listed owner role via the single-owner structured decision recorded in REVIEW-20260929 (ALIGN-01 slice 2). The authorization covers the control-plane implementation slice only (provider-neutral pool state machine, fenced idempotent claims, bounded registries); `specs/sandbox-runtime-pool.contract.json` stays implementation-gated (`implementationAuthorized: false`) until that slice lands with code and tests, and Pool runtime against a real VMM, PostgreSQL claim authority, Snapshot reuse, Node operation, API, SDK, config key or deployment profile stay gated on the dependency gates (REQ-2026-0011..0018) and the standing evidence obligations.

## Implementation Authorization

`ready` since 2026-09-29: the repository owner approved the REVIEW-20260730 architecture/security packet for every listed reviewer role via the structured session instruction recorded in REVIEW-20260929 (single-owner convention), executing ALIGN-01 slice 2. The approval authorizes the control-plane implementation slice (provider-neutral pool state machine, fenced idempotent claims, bounded registries) while `specs/sandbox-runtime-pool.contract.json` remains `implementationAuthorized: false` until that slice lands. The packet's evidence obligations (real KVM Prepared-Slot conformance, PostgreSQL multi-controller proof, residue/fencing/saturation suites) remain standing; `WarmMicroVmSlot` stays behind its separate KVM evidence gate and no fast-allocation SLO is claimed.
