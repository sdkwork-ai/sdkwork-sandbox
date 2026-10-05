# ADR-20261006: Sandbox Worker Launch Execution Authority

Status: accepted

Accepted: 2026-10-06 by the repository owner via the structured session instruction recorded in [REVIEW-20261006](../../engineering/reviews/REVIEW-20261006-sandbox-worker-authority.md) (single-owner convention). WRK-01..WRK-07 approved; evidence obligations standing.

Requirement: [REQ-2026-0034](../../product/requirements/REQ-2026-0034-sandbox-worker-launch-execution.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

## Context

快启动链的治理面已完整并经组合证明（0029 定义 → 0032 构建 → 0012 制品 → 0019 池槽/fenced claim → 0033 启动计划），REQ-2026-0034 已以 draft 登记 Worker 能力类，但执行记录的公共命名、数据所有权与绑定语义未经人工评审，实现保持未授权。没有 Worker，启动计划只能停在 `consumed`，模板的 start command 无处执行。REQ-2026-0033 契约明文把执行运行时与 Worker 锁在其各自需求切片之后——本 ADR 就是那个切片的治理锚：先落执行记录权威模型，本地车道执行适配器作为独立后续切片。

## Decision

1. Worker 权威是 provider-neutral 的记录模型：`SandboxWorkerExecution`（执行身份、启动计划引用、Provider 分配引用、启动命令执行引用、封闭状态集、时间戳）。所有 wire 字段 `sandbox_` 前缀，引用 opaque。
2. 生命周期是封闭六态：`accepted` 唯一初始态，`provisioning`/`starting` 过渡态，`started`/`failed`/`quarantined` 终态且终态不可变；时间戳单调不回退。
3. 绑定语义固定：一执行最多消费一个启动计划；acceptance 是唯一计划交接——计划状态由 0033 权威转移（`planned -> consumed`），Worker 永不直写计划状态；已被消费或已失效的计划不得再被接受；`accepted`/`provisioning`/`starting` 单独不构成完成声明，`started` 只在执行器报告成功后可达；结果不确定进入 `quarantined` 而不是静默成败。
4. 执行器复用边界：start command 经 `REQ-2026-0007` 命令执行器端口执行，Provider 分配/启动经 Provider SPI——Worker 不重实现任何执行器，全仓只有一条经审计的执行路径。
5. 车道范围显式化：本地车道优先（经已就绪的本地命令执行器）；Firecracker 车道锁在 `REQ-2026-0008` KVM 证据之后；Warm 槽消费锁在 `REQ-2026-0019` 之后；快照派生实例经同一 Worker 线执行。
6. 证据门：真实 Worker 执行证据（车道范围）、KVM 车道证据与首命令零等待测量为 release-blocking；本切片不执行任何东西。
7. 本地车道执行适配器、Firecracker VMM 运行时、Warm 槽消费、CLI、公共 API/SDK、部署 profile 保持 forbidden，直到各自的后续需求切片。

## Alternatives

- Worker 直接写启动计划状态（跳过 0033 权威）：拒绝——两个写者共用一个状态机必然竞态；acceptance 作为唯一交接点让计划侧与执行侧的审计各自封闭。
- Worker 内嵌自己的命令执行逻辑：拒绝——REQ-2026-0007 已交付真实执行器（有界输出、硬超时、取消注册表）并经 Common Conformance 套件验证；第二套执行器就是第二套未审计路径。
- 接受计划时立即声明 `started`（乐观完成）：拒绝——`started` 必须由执行器成功报告挣取；乐观完成会把幽灵启动注入分配审计。
- 本地车道与 Firecracker 车道共用一条无差别执行路径：拒绝——车道能力差一个 KVM 证据等级；混同会让未证明的车道借已证明车道的名义出货。

## Consequences

- 权威模型可先行落地为带校验的记录类型与机器契约钉面；执行适配器/VMM 在后续切片授权前不得存在。
- 执行记录引用计划与分配而不拥有其语义，三条线各守各的权威。
- 在本地车道适配器落地并证明真实执行前，本仓不得宣称 E2B 快速创建（首命令零等待）能力对齐。
- `WarmMicroVmSlot` 的 Worker 衔接必须穿过本权威与 Pool 证据门两道，缺一不可。

## Verification

- 机器契约 `specs/sandbox-worker.contract.json` 与权威模型 crate 逐字段对齐（契约测试钉授权门、字段形状、封闭状态集、生命周期操作、绑定语义、证据门、分层、forbidden）。
- 引用 opaque 校验、终态不可变、时间戳单调、不确定隔离语义以单元测试钉死。
- 后续切片（本地车道适配器/Firecracker 车道/CLI/API）各自带 REQ、真实执行证据与故障注入测试；在此之前禁止实现。

## Review

Required human owners: Architecture, Security/Privacy, Capacity/Scheduler, Command/Execution, Workspace/Storage。由仓库所有者以单一所有者结构化决策批准（见 REVIEW-20261006 的 approval basis）。

## Supersedes / Superseded By

None.
