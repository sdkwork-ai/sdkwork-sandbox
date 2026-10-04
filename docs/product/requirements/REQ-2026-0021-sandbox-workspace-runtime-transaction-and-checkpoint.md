# REQ-2026-0021: Sandbox Workspace Runtime Transaction And Checkpoint

id: REQ-2026-0021

title: 交付 Local 与 Cloud 一致的 Workspace Runtime Transaction、Checkpoint 与资源归还语义

owner: SDKWork Runtime Platform

status: ready

source: customer

## Problem

BirdCoder 需要同时支持两种产品路径：Standalone Desktop 在本机保存 Workspace 与相关业务数据并使用 Local Sandbox 执行；Cloud Client 通过 `sdkwork-birdcoder -> sdkwork-agents -> sdkwork-kernel -> sdkwork-sandbox` 分配 Firecracker 环境，在隔离运行环境中挂载持久 Workspace 数据。当前 Sandbox 已分别定义 Lifecycle、Command、Workspace Block Device、Admission/Scheduler/Capacity 和 Runtime Pool Gate 0，但没有一个权威事务把 Workspace Revision 授权、资源预留、Pool/Cold 分配、Attachment、Command、耐久 Checkpoint、Revision 冲突、Detach、Sanitization 和 Capacity Release 串成可恢复顺序。

缺少该闭环时，多副本控制面和 IDE 长连接可能出现重复执行、双写挂载、断连丢写、旧 Revision 覆盖新 Revision、Checkpoint 未持久化即归还 Slot、清理不确定仍释放容量，以及 Local/Cloud 产生两套行为协议。单独通过 Pool、Attachment 或 Command 静态测试都不能证明端到端安全。

## Goals

- 固定一个 Provider-neutral `SandboxWorkspaceRuntimeTransaction`，组合现有 Gate 而不复制 Agents Workspace、Scheduler、Pool、Storage、Command 或 Provider 权威。
- 让 `standalone/local`、`standalone/firecracker` 和 `cloud/firecracker` 共享身份、Revision、Command、Checkpoint、错误和补偿语义，只在 Composition Adapter 与 Assurance 上变化。
- 保证 Local Lane 默认 Device-local Data Residency；Workspace 和 Runtime Root 使用不同的预打开 Capability，未经用户和上游产品明确授权不发生隐式 Cloud Transfer。
- 保证 Cloud Lane 使用 Immutable Workspace Revision 和加密 Guest Block-device Projection；运行环境、Workspace Device、Cache、Temp 和 Log 分离，Pool Ready Slot 不含任何 Tenant State。
- 对 ReadWrite Workspace 建立单 Writer Lease、Fencing、耐久 Checkpoint Candidate、Durable Handoff 和 Agents-owned Revision Compare-and-swap，禁止静默丢写和覆盖新 Revision。
- 对排队、断连、重连、取消、超时、Checkpoint、清理和 Reconciliation 建立有界行为、Tenant-aware Fairness 和确定性补偿。
- 明确 Kernel 只传 Opaque Identity、Workspace Revision Authorization、Capability、Minimum Assurance、Workload Class 和操作控制；Provider、Node、Pool、Slot 和 Physical Storage 由 Sandbox 选择。
- 明确 Kernel Execution Placement 与 Sandbox Capacity Placement/Runtime Allocation 是不同权威：记录 ID、Lease、Fencing Token、Operation/Idempotency Scope 和恢复状态都不能复用或互相推进。

## Non-goals

- 不在 Sandbox 创建 Agent Workspace、Project、Session、Revision、Git Branch 或源码业务模型。
- 不批准 Block-volume Provider、Drive 扩展、KMS、Scheduler、Pool、Firecracker、Local Host I/O、数据库迁移或 Service Host Runtime 实现。
- 不批准 BirdCoder 直接依赖 Kernel/Sandbox、Raw HTTP、手写 Auth Header 或手工生成 SDK。
- 不批准 Public API、Internal API、Generated SDK、Cloud Transport、Deployment Profile Materialization、Release Artifact 或 Production Deployment。
- 不把 Interactive PTY、Shell String、Secret Injection、Browser、Port Forward、Snapshot Restore 或 Billing 纳入本 Requirement。
- 不猜测最大 IDE Session 生命周期、Reconnect Grace 或 Checkpoint Candidate Retention；精确值必须由 Product、Agents、Storage、Reliability 和 Operations Owner 批准。

## Acceptance Criteria

1. Machine Contract 必须列出三个 Execution Lane，并证明 Local 只声明 `HostUser`、Cloud Firecracker 要求 `MicroVm`，Cloud 不得回退到 Local、Docker 或更弱 Assurance。
2. Request 必须显式携带 Workspace Revision、短期 Authorization Grant Ref、Session、Operation、Workload Class、Mount Mode、Checkpoint Policy、Fingerprint、Fencing、Deadline 和 Trace；不得携带 Host Path、Device Path、Object Key、Credential 或 Provider Identity。
3. End-to-end 顺序必须固定为 Authorization/Revision -> Admission -> Capacity -> Runtime Binding -> Pool/Cold/Local Selection -> Fresh Grants -> Provider Start -> Attachment Ack -> Effective Readiness -> Command -> Freeze/Drain -> Flush -> Durable Checkpoint/Handoff -> Stop -> Detach -> Sanitize/Residue -> Release。
4. Local 不适用的 Cloud 阶段必须写入 Typed Durable No-op Evidence，不能通过省略字段或推断后续状态绕过同一事务。
5. Workspace Revision 在一次 Attachment 内不可变；同一 Revision Target 只能有一个 ReadWrite Writer Lease，多个并行 Writer 必须使用 Agents 批准的不同 Revision Target。
6. ReadWrite Transaction 在释放 Runtime 前必须产生 Sealed Durable Checkpoint Candidate 并持久化 Handoff；ReadOnly 必须记录明确的 No-checkpoint Outcome。
7. Sandbox 只能返回 Opaque Checkpoint Candidate；只有 Agents 能以 Expected Source Revision 执行 Compare-and-swap Revision Promotion。冲突必须非破坏性，不得覆盖新 Revision。
8. Client Disconnect 不等于立即销毁 Runtime。Reconnect Lease/Grace 必须有界；Grace 到期后以新 Fencing Token Freeze、Checkpoint 和 Cleanup，不能以 TTL 直接归还 Slot。
9. Command 只能使用共享 `SandboxCommandExecutor` 的 Logical Executable + Argv 契约。Transaction Freeze 后拒绝新 Command，并在 Checkpoint 前有界 Drain 或 Cancel 活动 Command。
10. 每个失败窗口必须有确定性 Compensation。Checkpoint、Storage、Host 或 Cleanup Side Effect 不确定时 Quarantine 受影响 Binding/Attachment/Slot/Node/Capacity，且不确定容量继续计占用。
11. Cloud Queue、每 Tenant Active/Queued Transaction、Command Concurrency、Retry、Reconciliation Batch、Disconnect Buffer 和 Output 必须有界；Priority 不能由 Caller 提升。
12. Runtime Location 只有在完整 Effective Readiness 后才能作为 Opaque Binding Mapping 返回；不得泄露 Node、Slot、Allocation、Attachment、Path、Credential 或 Guest Identity。
13. Local Data Residency 测试证明默认不向 Cloud Workspace/Storage 传输；Cloud 数据面测试证明 Workspace Byte Authority 属于 Drive 或经独立 Ready Requirement 批准的 Block-volume Authority。
14. Cross-repository Contract Test 必须证明 BirdCoder 不直连 Kernel/Sandbox，Kernel 不选择 Provider/Node/Pool/Storage，且 legacy one-shot Kernel Sandbox/Host Process Path 不能执行生产命令。
15. Kernel Execution Placement Ref/Generation 必须在 Sandbox Admission 前验证；Kernel Placement 与 Sandbox Capacity Placement/Runtime Binding 必须拥有独立 ID、Lease/Fencing 和 Idempotency Scope，任一方都不能把自身记录当作另一方记录或直接推进另一方状态。
16. 实现前必须完成 Architecture/Security/Privacy/Workspace/Drive/Storage/Database/Reliability/Capacity/Local Platform/KVM/BirdCoder/Agents/Kernel 人工评审。

## Non-functional Requirements

| Area | Requirement |
| --- | --- |
| Security | 所有外部副作用前校验 Authorization、Revision、Fencing 和当前 Grant；不确定清理关闭失败并 Quarantine。 |
| Privacy | Local 默认 Device-local；Cloud Workspace、日志、指标和事件不暴露源码内容、Raw Identity、Path、Storage Metadata 或 Credential。 |
| Performance | Queue、Environment Ready、Command、Checkpoint 和 Release 分阶段记录 p50/p95/p99；Local/Cold/Prepared/Warm 分开报告。 |
| Reliability | Operation/Compensation Idempotent，Checkpoint Handoff Durable，Control-plane Restart/Node Loss 可恢复，不以 TTL 猜测成功。 |
| Scalability | Admission 前置 Backpressure、Tenant Fairness、单 Writer Lease、Bounded Commands/Reconciliation，禁止无界扫描和 Retry Storm。 |
| Coupling | 只组合稳定 Port、Opaque Ref 和 Machine Contract；Sandbox 不反向依赖 BirdCoder/Agents/Kernel。 |

## Affected Surfaces

- product architecture
- runtime composition
- workspace data plane
- command execution
- checkpoint and recovery
- admission, capacity and pool composition
- cross-repository BirdCoder/Agents/Kernel integration
- observability and operations

## Dependencies

- REQ-2026-0003 Local Provider
- REQ-2026-0004 Agents Workspace Attachment Ownership
- REQ-2026-0005 Durable Lifecycle Persistence
- REQ-2026-0007 Command Execution
- REQ-2026-0008 Firecracker Provider
- REQ-2026-0009 Service Host Composition
- REQ-2026-0010 Event/Audit/Outbox
- REQ-2026-0013 Workspace Block Device
- REQ-2026-0014 Network Isolation
- REQ-2026-0015 Resource Isolation
- REQ-2026-0016 Admission/Scheduler/Capacity
- REQ-2026-0017 Node Trust
- REQ-2026-0018 Quota/Capacity Persistence
- REQ-2026-0019 Runtime Pool
- REQ-2026-0020 Bounded Lifecycle/Idempotency

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`, `CONFIG_SPEC.md`, `DEPLOYMENT_SPEC.md`, `APP_RUNTIME_TOPOLOGY_SPEC.md`, `DRIVE_SPEC.md`, `SECURITY_SPEC.md`, `EVENT_SPEC.md`, `OBSERVABILITY_SPEC.md`, `PERFORMANCE_SPEC.md`, `QUALITY_GATE_SPEC.md`, `TEST_SPEC.md`.

Machine contract: `specs/sandbox-workspace-runtime-transaction.contract.json`.

Decision: [ADR-20260730: Sandbox Workspace Runtime Transaction And Checkpoint](../../architecture/decisions/ADR-20260730-sandbox-workspace-runtime-transaction-and-checkpoint.md).

Plan: [PLAN-2026-0002: Commercial Cloud Agent Runtime Delivery](../../engineering/plans/PLAN-2026-0002-commercial-cloud-agent-runtime-delivery.md).

## Verification

```text
node --test tests/contract/sandbox-workspace-runtime-transaction.contract.test.mjs
node --test tests/contract/sandbox-service-host-composition.contract.test.mjs
node --test tests/contract/*.test.mjs
node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root .
node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root . --strict
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
```

Future implementation evidence must additionally include real Local runners, real Linux KVM, live PostgreSQL multi-controller contention, Drive/approved volume integration, disconnect/reconnect fault injection, Checkpoint conflict, cross-tenant residue, saturation, soak, PITR and rollback.

## Implementation Gate

`ready` since 2026-10-05: REVIEW-20260730（事务架构与安全，10 角色）由仓库所有者以单一所有者结构化决策接受，ADR-20260730 同日 `accepted`。授权的实现切片仅为控制面：provider-neutral 事务状态机、21 阶段固定编排序（含类型化 N/A 证据）、Checkpoint 候选（耐久 handoff 与 Agents 独占 CAS 晋升）、Writer Lease/Fencing/幂等语义与有界注册表，以领域 crate 形式带机器契约对齐测试落地；`specs/sandbox-workspace-runtime-transaction.contract.json` 对该切片翻转为 `implementationAuthorized: true` 并保持 `draft`。真实 Provider 运行时、PostgreSQL 事务权威、storage/KMS 适配器、worker、API/SDK/transport、config 与部署面仍被契约 `x-sdkwork-no-*` 门锁住；真实 Local/KVM/PostgreSQL 证据、故障注入与饱和测试保持 release-blocking。
