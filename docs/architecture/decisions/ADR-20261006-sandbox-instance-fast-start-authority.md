# ADR-20261006: Sandbox Instance Fast-Start Launch Authority

Status: accepted

Accepted: 2026-10-06 by the repository owner via the structured session instruction recorded in [REVIEW-20261006](../../engineering/reviews/REVIEW-20261006-sandbox-instance-fast-start-authority.md) (single-owner convention). LNCH-01..LNCH-07 approved; evidence obligations standing.

Requirement: [REQ-2026-0033](../../product/requirements/REQ-2026-0033-sandbox-instance-fast-start-launch.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

## Context

E2B 的快速创建语义是 `Sandbox.create()` 一次调用返回一个可执行命令的 Linux VM：模板的 start command 在沙箱创建时执行，进程已在运行、首命令零等待。本仓快分配链的治理面已齐（0029 模板权威、0032 构建权威、0012 制品 Tuple、0019 池槽位与 fenced claim、0031 快照/派生、0007 命令执行），REQ-2026-0033 已以 draft 登记启动绑定能力类，但"模板版本 × 已认领槽位 × 全新身份"的启动计划公共命名、数据所有权与绑定语义未经人工评审，实现保持未授权。没有这一环，start-command 语义连不上 PreparedSlot，Worker 没有可审计的启动计划，快分配链在制品侧之后仍然断开。

## Decision

1. Launch 权威是 provider-neutral 的记录模型：`SandboxInstanceLaunchPlan`（计划身份、模板版本引用、池 claim 引用、全新身份证据引用、封闭状态集、时间戳）。所有 wire 字段 `sandbox_` 前缀，引用 opaque。
2. 生命周期是封闭四态：`planned` 唯一初始态且创建后不可变，`consumed`/`expired`/`quarantined` 终态且终态不可变；时间戳单调不回退。
3. 绑定语义固定：每个计划绑定一个活跃 fenced pool claim；claim 过期或释放只能使计划 `expired`，永不下发执行；绑定不确定进入 `quarantined` 而不是静默下发；`planned` 单独不构成执行授权——Worker 切片是独立授权。
4. start command 的权威归 `REQ-2026-0029`（版本发布后不可变），计划按 opaque 引用消费、绝不复制——复制会让模板语义出现第二个权威。
5. 每次启动强制携带全新 Guest Identity 证据；快照派生的实例同样经本权威产出计划（`REQ-2026-0031` 分层）。
6. 证据门：真实 fast-start 运行时证据与首命令零等待测量证据为 release-blocking；本登记不执行任何命令。
7. 启动执行运行时、Worker、CLI、公共 API/SDK、部署 profile 保持 forbidden，直到各自的后续需求切片。

## Alternatives

- 把 start command 复制进启动计划以解除对模板权威的运行时依赖：拒绝——复制让模板语义出现第二个权威，版本不可变性形同虚设。
- 计划绑定槽位而非 claim：拒绝——槽位由 fenced claim 单所有者持有（REQ-2026-0019），绑定槽位会绕过 fencing；claim 过期后计划必须随之失效，否则幽灵启动会执行在已释放的容量上。
- `planned` 即执行授权（Worker 看到计划就跑）：拒绝——执行是独立授权面（Worker 切片）；绑定与执行混同会让"计划了什么"和"执行了什么"失去可审计边界。
- 快照派生实例绕过 launch 权威直接启动：拒绝——两条启动路径一套绑定语义；绕行会让派生实例跳过全新身份证据与 claim 绑定审计。

## Consequences

- 权威模型可先行落地为带校验的记录类型与机器契约钉面；执行运行时/Worker 在后续切片授权前不得存在。
- 启动计划引用模板版本与 pool claim 而不拥有其语义，三条线各守各的权威。
- 在 Worker 切片落地前，本仓不得宣称 E2B 快速创建（首命令零等待）能力对齐。
- `WarmMicroVmSlot` 的启动衔接必须穿过本权威与 Pool 证据门两道，缺一不可。

## Verification

- 机器契约 `specs/sandbox-instance-fast-start.contract.json` 与权威模型 crate 逐字段对齐（契约测试钉授权门、字段形状、封闭状态集、生命周期操作、绑定语义、证据门、分层、forbidden）。
- 引用 opaque 校验、终态不可变、claim 过期即计划失效、时间戳单调与不确定隔离语义以单元测试钉死。
- 后续切片（Worker/执行运行时/CLI/API）各自带 REQ、真实运行时证据与故障注入测试；在此之前禁止实现。

## Review

Required human owners: Architecture, Security/Privacy, Capacity/Scheduler, Command/Execution, Workspace/Storage。由仓库所有者以单一所有者结构化决策批准（见 REVIEW-20261006 的 approval basis）。

## Supersedes / Superseded By

None.
