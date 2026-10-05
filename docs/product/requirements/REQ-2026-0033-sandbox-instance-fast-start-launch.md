---
id: REQ-2026-0033
title: 交付 Sandbox Instance Fast-Start Launch 权威
owner: SDKWork Runtime Platform
status: ready
priority: high
source: customer
problem: E2B 的快速创建语义是 `Sandbox.create()` 一次调用返回一个可执行命令的 Linux VM，配合 Template 的 start command，沙箱创建时进程已在运行、首命令零等待。本仓"快速分配 + 快速启动"链的治理面已齐：模板权威（REQ-2026-0029）、构建权威（REQ-2026-0032）、制品 Tuple（REQ-2026-0012）、池槽位与 fenced claim（REQ-2026-0019）、快照/派生（REQ-2026-0031）、命令执行（REQ-2026-0007）——但"模板 start command × 已认领池槽位 × 全新 Guest Identity"三者的绑定没有权威承载：没有 launch 权威，start-command 语义连不上 PreparedSlot（REQ-2026-0029 Blocking Findings 明文登记），Worker 执行时没有可审计的启动计划，首命令零等待声明也没有权威起点。这是快启动链上最后一个结构性空档。
goals:
  - 建立 provider-neutral 的启动计划权威：计划身份、模板版本引用（start command 的权威在 0029，按 opaque 引用消费、绝不复制）、池 claim 引用（fenced 单所有者）、全新 Guest Identity 证据引用、封闭状态集与时间戳。
  - 固定启动计划的封闭状态集与封闭操作集；创建后不可变；绑定不确定进入隔离而不是静默下发执行。
  - 固定绑定语义：计划必须绑定一个活跃 fenced claim；claim 过期或释放使计划失效（不静默执行）；全新身份是每次启动的强制前置。
  - 与命令执行（REQ-2026-0007）分层：本权威只产出与跟踪启动计划，不执行命令；与快照/派生（REQ-2026-0031）分层：从快照派生的实例同样经本权威产出计划。
  - 把真实 fast-start 运行时证据与"首命令零等待"测量证据登记为实现前的持续义务。
non_goals:
  - 不实现启动执行运行时、Worker、CLI、API/SDK 或部署 profile。
  - 不复制、不重定义模板 start command 语义（REQ-2026-0029 权威）；不实现命令执行（REQ-2026-0007 权威）。
  - 不解除 REQ-2026-0019 的 WarmMicroVmSlot 证据门；本需求登记不等于 Warm 路径启用。
  - 不做首命令零等待的时延声明（E2B 的对应时延属实测后的测量纪律）。
users:
  - SDKWork SaaS runtime operators
  - Agent 平台集成方
affected_surfaces:
  - rust-components
  - composition
  - api
---

# REQ-2026-0033: 交付 Sandbox Instance Fast-Start Launch 权威

## Readiness Blockers

- 人工接受本需求的公共命名、启动计划数据所有权与绑定语义边界（本切片先落 draft 载体与机器契约；命名/评审包随后提交）。
- 与 `REQ-2026-0007`（命令执行）和 `REQ-2026-0019`（池 claim）的分层边界必须在评审包中定案：本权威只产出与跟踪计划；执行归 Worker 切片，claim 语义归池权威。
- 真实 fast-start 运行时证据与首命令零等待测量证据是实现授权前的持续义务（本机无真实运行时）。

## Candidate Acceptance Criteria

- 候选权威类型为 `SandboxInstanceLaunchPlan`（计划身份、模板版本引用、池 claim 引用、全新身份证据引用、封闭状态集、时间戳）；所有 wire 字段 `sandbox_` 前缀，引用 opaque。
- 启动计划生命周期为封闭状态集：`planned`/`consumed`/`expired`/`quarantined`；`planned` 是唯一初始态且创建后字段不可变；`consumed`/`expired`/`quarantined` 是终态；绑定不确定进入 `quarantined` 而不是静默下发执行。
- 绑定语义：每个计划必须绑定一个活跃的 fenced pool claim；claim 到期或释放后计划只能 `expired`，永不下发执行；每次启动必须携带全新 Guest Identity 证据。
- 生命周期操作为封闭集：`sandbox_plan_launch`、`sandbox_mark_launch_consumed`、`sandbox_expire_launch`、`sandbox_quarantine_launch`；固定操作之外无副作用操作。
- `planned` 状态单独不构成执行授权；执行必须由后续 Worker 切片单独授权。

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `COMPONENT_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `TEST_SPEC.md`.

Decisions: 后续提交随命名评审包登记。

## Verification Plan

机器契约 `specs/sandbox-instance-fast-start.contract.json`（draft、`implementationAuthorized: false`）由契约测试钉住：字段形状、封闭状态集与操作集、绑定语义、证据门、与 REQ-2026-0007/0019/0029/0031/0032 的分层引用。实现授权前不写实现用例。

## Release Boundary

在 Worker/执行运行时切片单独授权并落地之前，不得实现启动执行、CLI、API/SDK，也不得宣称 E2B 快速创建（首命令零等待）能力对齐。权威模型切片（记录类型、生命周期校验、绑定规则、证据门文档）已于 2026-10-06 授权并落地。

## Implementation Gate

`ready` since 2026-10-06: REVIEW-20261006（命名、数据所有权、绑定语义、证据门、forbidden 面）由仓库所有者以单一所有者结构化决策接受，ADR-20261006 同日 `accepted`。授权的实现切片仅为权威模型：`crates/sdkwork-intelligence-sandbox-launch-authority` 承载 `SandboxInstanceLaunchPlan` 记录、生命周期校验、绑定规则与机器契约对齐测试；`specs/sandbox-instance-fast-start.contract.json` 对该切片翻转为 `implementationAuthorized: true` 并保持 `draft`。启动执行运行时、Worker、CLI、公共 API/SDK 与部署 profile 仍被契约 `forbidden` 块锁住，直到各自的后续需求切片落地；Worker 切片存在之前不得宣称 E2B 快速创建（首命令零等待）能力对齐。

## Implementation Authorization

`ready` since 2026-10-06：批准记录见 REVIEW-20261006 的 approval basis（本会话重复的镜像/snapshot 能力指令 + REVIEW-20260929 结构化指令延续，单一所有者惯例）。权威模型切片已随码与测试落地；Blocking Findings（执行运行时/Worker 缺失、CLI/API 未定、真实 fast-start 运行时与首命令零等待证据缺失、start-command↔PreparedSlot 运行时接线未批、Warm 槽桥未批）全部保持为后续切片的持续证据义务。
