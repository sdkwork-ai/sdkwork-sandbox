# ADR-20261005: Sandbox Template Build Authority

Status: accepted

Accepted: 2026-10-05 by the repository owner via the structured session instruction recorded in [REVIEW-20261005](../../engineering/reviews/REVIEW-20261005-sandbox-template-build-authority.md) (single-owner convention). BLD-01..BLD-07 approved; evidence obligations standing.

Requirement: [REQ-2026-0032](../../product/requirements/REQ-2026-0032-sandbox-template-build.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-05

## Context

E2B 的快速部署建立在 `Template.build()` 之上：模板定义预构建为镜像制品并版本化，`fromTemplate()` 复用构建缓存层，配合 start command 实现沙箱创建时进程已在运行。本仓 REQ-2026-0029 已承载模板权威（定义/版本/构建输入/缓存语义），REQ-2026-0032 已以 draft 登记构建能力类，但构建记录的公共命名、数据所有权与结果绑定语义未经人工评审，实现保持未授权。同时，Pool（REQ-2026-0019）快速分配消费的制品来源、REQ-2026-0012 的制品 Tuple 供应链、快照线（REQ-2026-0031）引用的制品集合都必须经同一条构建权威产出——构建记录是"快速分配 + 快速启动"链上制品侧的权威起点。

## Decision

1. Build 权威是 provider-neutral 的记录模型：`SandboxTemplateBuild`（构建身份、模板定义引用、模板版本引用、构建输入引用、封闭状态集、制品 Tuple 引用、时间戳）。所有 wire 字段 `sandbox_` 前缀，引用 opaque。
2. 生命周期是封闭五态：`requested` 唯一初始态，`building` 过渡态，`succeeded`/`failed`/`quarantined` 终态且终态不可变；时间戳单调不回退。
3. 结果绑定语义固定：成功必须绑定恰好一个满足 `REQ-2026-0012` 精确 Tuple 的制品集合；失败与隔离不绑定任何制品；结果不确定（超时、构建器丢失、结果不可验证）进入 `quarantined` 而不是静默成功或静默失败；超时单独不构成成功。
4. 分层：模板定义/版本/构建输入/缓存语义的权威归 `REQ-2026-0029`，本线只按 opaque 引用消费、不重定义；缓存语义只记录期待（Hot/Warm/Cold），不实现存储后端或淘汰执行；`WarmMicroVmSlot` 消费构建产物另需 `REQ-2026-0019` 的 Pool 证据门。
5. Build 权威不拥有制品 Evidence/签名/撤销，也不允许第二套供应链权威；每个 Tuple 解析到 `REQ-2026-0012`。
6. 证据门：真实 Builder 执行证据与构建产物 Tuple 证据为 release-blocking；本登记今天不绑定任何真实制品。
7. Builder 运行时、流水线执行、产物/缓存存储、Registry 服务、CLI、公共 API/SDK、部署 profile 保持 forbidden，直到各自的后续需求切片。

## Alternatives

- 直接实现 Builder 流水线与产物存储：拒绝——没有真实构建执行证据与供应链绑定之前，构建产物会成为第二套未审计的制品来源。
- 把构建记录并入 REQ-2026-0029 的模板权威：拒绝——0029 的 non-goals 明文排除构建执行，且模板记录（发布后不可变）与构建记录（生命周期跟踪、终态不可变）是两种不变量；混同会让发布不可变语义吞掉构建过程语义。
- 允许失败构建绑定部分产物以加速重试：拒绝——失败构建声明制品正是"幽灵制品进入分配"的来源；重试应开新构建记录。
- 允许超时构建按最后已知状态判定成功：拒绝——超时的结果不可验证，静默成功会让 Pool 消费未验证制品。

## Consequences

- 权威模型先行落地为带校验的记录类型与机器契约钉面；Builder 运行时/流水线/存储在后续切片授权前不得存在。
- 构建记录引用制品 Tuple 而不拥有 Evidence，供应链审计仍走 `REQ-2026-0012` 单一权威。
- 在 Builder 运行时切片落地前，本仓不得宣称 E2B Template Build 能力对齐。
- `WarmMicroVmSlot` 的构建产物衔接必须穿过本权威与 Pool 证据门两道，缺一不可。

## Verification

- 机器契约 `specs/sandbox-template-build.contract.json` 与权威模型 crate 逐字段对齐（契约测试钉授权门、字段形状、封闭状态集、生命周期操作、失败语义、证据门、分层、forbidden）。
- 引用 opaque 校验、终态不可变、结果绑定规则（成功有 Tuple/失败无 Tuple）、时间戳单调与不确定隔离语义以单元测试钉死。
- 后续切片（Builder 运行时/流水线/存储/Registry/CLI/API）各自带 REQ、真实构建执行证据与故障注入测试；在此之前禁止实现。

## Review

Required human owners: Architecture, Security/Privacy, Supply Chain, Capacity/Scheduler, Workspace/Storage。由仓库所有者以单一所有者结构化决策批准（见 REVIEW-20261005 的 approval basis）。

## Supersedes / Superseded By

None.
