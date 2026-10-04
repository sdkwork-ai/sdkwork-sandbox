---
id: REQ-2026-0031
title: 交付 Sandbox Snapshot 与 Fork 权威
owner: SDKWork Runtime Platform
status: draft
priority: high
source: customer
problem: E2B 的持久化与快速复制建立在 Snapshot/Fork 之上（createSnapshot 含内存与文件系统、从 snapshotId 创建新沙箱、fork 一次派生 N 个并行沙箱）；本仓该能力类此前零需求承载，是"快速启动"路径上 Template（REQ-2026-0029）之后第二个结构性空档：没有快照权威，WarmMicroVmSlot 的清洁不可变快照来源、派生式扩容与故障恢复都没有权威起点。
goals:
  - 建立 provider-neutral 的 Snapshot 权威记录：快照身份、来源沙箱绑定、内存/文件系统内容界定、制品 Tuple 绑定（引用 REQ-2026-0012，不拥有 Evidence）与只读元数据。
  - 固定快照生命周期语义的封闭操作集（创建/枚举/删除），以及创建后不可变、删除确定性失败的边界。
  - 固定 Fork 派生语义边界：一个快照派生 N 个新沙箱、来源快照不可变、派生体获得全新 Guest Identity 与租户零残留证据义务。
  - 与 Runtime Pool（REQ-2026-0019）的 `WarmMicroVmSlot` 和 Workspace Checkpoint 线（REQ-2026-0021）分层：快照是沙箱运行时全状态的权威物化，Checkpoint 是 Workspace 事务耐久性，两者不互替。
  - 把真实 KVM 恢复证据与跨租户残留证据登记为实现前的持续义务。
non_goals:
  - 不实现快照引擎、快照存储后端、恢复流水线、API/SDK/CLI 或部署 profile。
  - 不授权 Auto Pause / Auto Resume（Idle 收敛是独立能力线）。
  - 不解除 REQ-2026-0019 `WarmMicroVmSlot` 的独立 KVM 证据门；本需求登记不等于 Warm 槽启用。
  - 不涉及 Workspace 业务删除与保留策略（`sdkwork-agents` 权威）。
  - 不做商业恢复时延声明（E2B 的约 4 s/GiB 与约 1 s 恢复属实测后的测量纪律）。
users:
  - SDKWork SaaS runtime operators
  - Agent 平台集成方
affected_surfaces:
  - rust-components
  - composition
  - api
---

# REQ-2026-0031: 交付 Sandbox Snapshot 与 Fork 权威

## Readiness Blockers

- 人工接受本需求的公共命名、快照数据所有权与 Fork 一致性语义边界（本切片先落 draft 权威模型与机器契约；命名/评审包随后提交）。
- 与 `REQ-2026-0021` Workspace Checkpoint 的分层边界必须在评审包中定案：Checkpoint 是 Workspace 事务的耐久候选与 Compare-and-swap Revision，Snapshot 是沙箱运行时全状态的物化；两者共享的存储分层由后续切片定义。
- 真实 Linux KVM 快照恢复、跨租户残留套件与派生体身份轮换证据是实现授权前的持续义务（本机无 KVM，证据需真实环境）。

## Candidate Acceptance Criteria

- 候选权威类型为 `SandboxSnapshot`（快照身份、来源沙箱引用、内存包含标志、文件系统指纹、制品 Tuple 引用、封闭状态集、创建时间）；所有 wire 字段 `sandbox_` 前缀，引用 opaque。
- 快照生命周期操作为封闭集：创建、枚举、删除；快照创建后不可变；删除不确定时进入隔离而不是静默消失。
- Fork 派生规则：来源快照不可变；一个快照可派生 N 个新沙箱；派生体获得全新 Guest Identity 与独立租户授权；任何派生路径不得复用来源租户状态。
- 每个快照必须绑定一个满足 `REQ-2026-0012` 精确 Tuple 的制品集合；Snapshot 权威不拥有制品 Evidence。
- 真实 KVM 恢复证据、跨租户残留证据与 `WarmMicroVmSlot`（REQ-2026-0019）的清洁快照衔接是 Warm 槽启用的前置义务。

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `COMPONENT_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `SUPPLY_CHAIN_SECURITY_SPEC.md`, `TEST_SPEC.md`.

Decisions: 后续提交随命名评审包登记。

## Verification Plan

机器契约 `specs/sandbox-snapshot-fork.contract.json`（draft、`implementationAuthorized: false`）由契约测试钉住：字段形状、封闭状态集与操作集、Fork 派生语义、证据门、与 REQ-2026-0012/0019/0021 的分层引用。实现授权前不写实现用例。

## Release Boundary

在人工评审通过并将机器契约翻转为 `implementationAuthorized: true` 之前，不得实现快照引擎、存储后端、恢复流水线、API/SDK/CLI，也不得宣称 E2B Snapshot/Fork 能力对齐。
