---
id: REQ-2026-0034
title: 交付 Sandbox Worker 与启动执行权威（本地车道优先）
owner: SDKWork Runtime Platform
status: draft
priority: high
source: customer
problem: 快启动链的治理面已完整（REQ-2026-0029 模板定义 → 0032 构建记录 → 0012 制品 Tuple → 0019 池槽/fenced claim → 0033 启动计划），且跨 crate 组合已被测试机械证明——但没有任何东西执行：REQ-2026-0033 契约明文把启动执行运行时与 Worker 锁在其各自需求切片之后，启动计划只能停在 `consumed`，模板的 start command 无处执行，"沙箱创建时进程已在运行"在所有车道上都未发生。这是快启动链上最后一个空环。
goals:
  - 建立 provider-neutral 的执行记录权威与控制面：执行身份、启动计划引用（一对一：一个启动计划最多被一个执行消费）、Provider 分配引用、启动命令执行引用（经 REQ-2026-0007 的命令执行器端口，不重实现）、封闭状态集与时间戳。
  - 固定执行生命周期：封闭状态集与封闭操作集；终态不可变；结果不确定进入隔离而不是静默成败；一个执行只消费一个计划，计划被接受即从 `planned` 转入 `consumed`。
  - 本地车道实现切片：经 Provider SPI 的 allocate/start 与 REQ-2026-0007 已就绪的本地命令执行器（真实进程、有界输出、硬超时）执行 start command 并记录结果；Firecracker 车道维持 KVM 证据锁。
  - 把真实 Worker 执行证据（车道范围）与 KVM 车道证据登记为实现前的持续义务。
non_goals:
  - 不实现 Firecracker/VMM 运行时（REQ-2026-0008 KVM 证据锁）；不启用 WarmMicroVmSlot（REQ-2026-0019 证据门不动）。
  - 不重实现命令执行器（REQ-2026-0007 权威）；不做进程监督守护（supervisor 语义属后续切片）。
  - 不实现 CLI、公共 API/SDK 或部署 profile。
  - 不做启动时延声明（首命令零等待属实测后的测量纪律）。
users:
  - SDKWork SaaS runtime operators
  - Agent 平台集成方
affected_surfaces:
  - rust-components
  - composition
  - api
---

# REQ-2026-0034: 交付 Sandbox Worker 与启动执行权威（本地车道优先）

## Readiness Blockers

- 人工接受本需求的公共命名、执行记录数据所有权与执行绑定语义边界（本切片先落 draft 载体与机器契约；命名/评审包随后提交）。
- 与 `REQ-2026-0033` 的分层边界必须在评审包中定案：0033 权威拥有启动计划与 `planned -> consumed` 的转移；本权威拥有执行记录与执行生命周期——消费计划是两者之间唯一的交接点。
- 真实 Worker 执行证据（本地车道范围）与 Firecracker 车道 KVM 证据是实现授权前的持续义务。

## Candidate Acceptance Criteria

- 候选权威类型为 `SandboxWorkerExecution`（执行身份、启动计划引用、Provider 分配引用、启动命令执行引用、封闭状态集、时间戳）；所有 wire 字段 `sandbox_` 前缀，引用 opaque。
- 执行生命周期为封闭状态集：`accepted`/`provisioning`/`starting`/`started`/`failed`/`quarantined`；`accepted` 是唯一初始态；`started`/`failed`/`quarantined` 是终态且终态不可变；结果不确定进入 `quarantined` 而不是静默成败。
- 绑定语义：一个执行最多消费一个启动计划；计划被接受时由 0033 权威置为 `consumed`（本权威不写计划状态）；已被消费或已失效的计划不得再被接受；start command 经 REQ-2026-0007 命令执行器端口执行，本权威不重实现执行器。
- 生命周期操作为封闭集：`sandbox_accept_launch`、`sandbox_record_provisioning`、`sandbox_record_starting`、`sandbox_record_started`、`sandbox_record_execution_failed`、`sandbox_quarantine_execution`；固定操作之外无副作用操作。
- `accepted`/`provisioning`/`starting` 单独不构成完成声明；`started` 只在启动命令执行器报告成功后可达。

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `COMPONENT_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `TEST_SPEC.md`.

Decisions: 后续提交随命名评审包登记。

## Verification Plan

机器契约 `specs/sandbox-worker.contract.json`（draft、`implementationAuthorized: false`）由契约测试钉住：字段形状、封闭状态集与操作集、绑定语义、证据门、与 REQ-2026-0007/0008/0019/0033 的分层引用。实现授权前不写实现用例。

## Release Boundary

在 Worker 权威/控制面切片单独评审并授权之前不写实现；在 Firecracker 车道切片单独授权并落地之前，Worker 不得驱动 VMM，也不得宣称 E2B 快速创建（首命令零等待）能力对齐；本地车道实现切片的行为范围以本需求明文为限。
