# ADR-20261005: Sandbox Snapshot And Fork Authority

Status: accepted

Accepted: 2026-10-05 by the repository owner via the structured session instruction recorded in [REVIEW-20261005](../../engineering/reviews/REVIEW-20261005-sandbox-snapshot-fork-authority.md) (single-owner convention). SNAP-01..SNAP-07 approved; evidence obligations standing.

Requirement: [REQ-2026-0031](../../product/requirements/REQ-2026-0031-sandbox-snapshot-and-fork.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-05

## Context

E2B 的持久化与快速复制建立在 Snapshot/Fork 之上：`createSnapshot()` 物化沙箱运行时全状态（内存+文件系统，原沙箱身份不变），从快照创建或派生新沙箱，`fork` 一次派生 N 个并行沙箱。本仓 REQ-2026-0031 已以 draft 登记该能力类，但其公共命名、数据所有权与 Fork 一致性语义未经人工评审，实现保持未授权。同时，Pool（REQ-2026-0019）的 `WarmMicroVmSlot` 需要一份清洁不可变快照作为来源，Workspace Checkpoint（REQ-2026-0021）是另一条持久化线——三条线的边界必须一次定清。

## Decision

1. Snapshot 权威是 provider-neutral 的记录模型：`SandboxSnapshot`（快照身份、来源沙箱引用、内存包含标志、文件系统指纹、制品 Tuple 引用、封闭状态集、创建时间）。所有 wire 字段 `sandbox_` 前缀，引用 opaque。
2. 快照创建后不可变；生命周期是封闭操作集（创建/枚举/删除）；删除确定性失败，删除不确定进入隔离而不是静默消失。
3. Fork 派生语义固定：来源快照不可变；一个快照可派生 N 个新沙箱且与来源并行运行；派生体获得全新 Guest Identity 与独立租户授权；任何派生路径不得复用来源租户状态。
4. 每个快照绑定一个满足 `REQ-2026-0012` 精确 Tuple 的制品集合；Snapshot 权威不拥有制品 Evidence/签名/撤销，也不允许第二套供应链权威。
5. 分层：本线是沙箱运行时全状态的物化；Workspace Checkpoint（REQ-2026-0021）是 Workspace 事务的耐久候选与 CAS Revision；Workspace 业务删除归 `sdkwork-agents`。三者不互替、不共享持久化权威。
6. 证据门：真实 Linux KVM 恢复、跨租户残留、派生体身份轮换证据为 release-blocking；`WarmMicroVmSlot` 复用快照另需 REQ-2026-0019 的 Pool 证据门。
7. 快照引擎、存储后端、恢复流水线、CLI、公共 API/SDK、部署 profile 保持 forbidden，直到各自的后续需求切片。
8. 不做商业恢复时延声明（E2B 的约 4 s/GiB 与约 1 s 恢复属于实测后的测量纪律）。

## Alternatives

- 直接实现快照引擎/存储后端：拒绝——没有真实 KVM 证据与供应链绑定之前，快照会成为第二套未审计的制品来源。
- 把 Snapshot 并入 Workspace Checkpoint（REQ-2026-0021）：拒绝——Checkpoint 是 Workspace 事务耐久性（Revision CAS），快照是沙箱运行时全状态；混同会产生两套语义共用一个持久化权威。
- 允许派生体复用来源 Guest Identity 以加速启动：拒绝——跨沙箱身份复用正是 Pool 线已否决的隔离风险。
- Fork 无界派生：拒绝——派生数必须有界且授权，否则容量核算与残留扫描都不可收敛。

## Consequences

- 权威模型可先行落地为带校验的记录类型与机器契约钉面；引擎/存储/恢复在后续切片授权前不得存在。
- 快照引用制品 Tuple 而不拥有 Evidence，供应链审计仍走 `REQ-2026-0012` 单一权威。
- 在引擎切片落地前，本仓不得宣称 E2B Snapshot/Fork 能力对齐。
- `WarmMicroVmSlot` 的快照衔接必须穿过本权威与 Pool 证据门两道，缺一不可。

## Verification

- 机器契约 `specs/sandbox-snapshot-fork.contract.json` 与权威模型 crate 逐字段对齐（契约测试钉 draft 状态、字段形状、封闭状态集、生命周期操作、Fork 语义、证据门、分层、forbidden）。
- 构建输入 opaque 校验、Fork 派生校验、删除确定性/隔离语义以单元测试钉死。
- 后续切片（引擎/存储/恢复/CLI/API）各自带 REQ、真实 KVM 证据与故障注入测试；在此之前禁止实现。

## Review

Required human owners: Architecture, Security/Privacy, Supply Chain, Firecracker/KVM Operations, Capacity/Scheduler, Workspace/Storage。由仓库所有者以单一所有者结构化决策批准（见 REVIEW-20261005 的 approval basis）。

## Supersedes / Superseded By

None.
