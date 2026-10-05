---
id: REQ-2026-0032
title: 交付 Sandbox Template Build 权威
owner: SDKWork Runtime Platform
status: ready
priority: high
source: customer
problem: E2B 的快速部署建立在 `Template.build()` 之上（预构建镜像 + 构建缓存 + `fromTemplate()` 层复用 + tags 版本化）；本仓 Template 线（REQ-2026-0029）已承载权威模型（定义/版本/构建输入/缓存语义），但"构建输入 → 可启动制品"的转换没有权威承载：没有构建权威，构建记录、成功产物与 `REQ-2026-0012` 制品 Tuple 的绑定、以及 Pool 快速分配消费的制品来源都没有权威起点——这是"快速分配 + 快速启动"链上 Template 权威（0029）与 Snapshot 权威（0031）之后的第三个结构性空档。
goals:
  - 建立 provider-neutral 的构建记录权威：构建身份、模板定义/版本绑定（引用 REQ-2026-0029，不重定义其语义）、构建输入引用（消费 `SandboxTemplateBuildInput`，不拥有其语义）、成功产物的制品 Tuple 引用（引用 REQ-2026-0012，不拥有 Evidence）与只读元数据。
  - 固定构建生命周期的封闭状态集与封闭操作集：终态后不可变；结果不确定进入隔离而不是静默成功或静默失败。
  - 与缓存语义（REQ-2026-0029 cachePolicy）分层：构建权威记录产物与缓存层的期待语义，不实现缓存存储后端，不定义淘汰执行。
  - 把真实 Builder 执行证据与构建产物 Tuple 证据登记为实现前的持续义务。
non_goals:
  - 不实现 Builder 运行时、构建流水线执行、构建产物/缓存存储后端、Registry 服务、API/SDK/CLI 或部署 profile。
  - 不变更 REQ-2026-0029 的权威模型与缓存语义；模板定义/版本/构建输入的权威仍在 0029。
  - 不解除 REQ-2026-0012 供应链权威；本需求登记不等于任何制品可绑定。
  - 不授权 Pool 消费构建产物的 Warm 路径；REQ-2026-0019 的 WarmMicroVmSlot 证据门不动。
users:
  - SDKWork SaaS runtime operators
  - Agent 平台集成方
affected_surfaces:
  - rust-components
  - composition
  - api
---

# REQ-2026-0032: 交付 Sandbox Template Build 权威

## Readiness Blockers

- 人工接受本需求的公共命名、构建记录数据所有权与制品绑定语义边界（本切片先落 draft 载体与机器契约；命名/评审包随后提交）。
- 与 `REQ-2026-0029` 的分层边界必须在评审包中定案：0029 拥有模板定义/版本/构建输入/缓存语义的权威；本需求只承载"构建记录"这一转换的权威，构建输入的语义仍在 0029。
- 真实 Builder 执行证据与满足 `REQ-2026-0012` 精确 Tuple 的构建产物证据是实现授权前的持续义务（本机无构建运行时与制品存储）。

## Candidate Acceptance Criteria

- 候选权威类型为 `SandboxTemplateBuild`（构建身份、模板定义引用、模板版本引用、构建输入引用、封闭状态集、制品 Tuple 引用、时间戳）；所有 wire 字段 `sandbox_` 前缀，引用 opaque。
- 构建生命周期为封闭状态集：`requested`/`building`/`succeeded`/`failed`/`quarantined`；`requested` 是唯一初始态；`succeeded`/`failed`/`quarantined` 是终态且终态后不可变；结果不确定时进入 `quarantined` 而不是静默成功或静默失败。
- 生命周期操作为封闭集：`sandbox_request_build`、`sandbox_record_building`、`sandbox_record_build_outcome`、`sandbox_quarantine_build`；固定操作之外无副作用操作。
- 成功的构建必须绑定一个满足 `REQ-2026-0012` 精确 Tuple 的制品集合；构建权威不拥有制品 Evidence，也不允许出现第二个供应链权威。
- 时间语义使用注入时钟的整秒读数；超时与结果不确定等价，一律隔离。

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `COMPONENT_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `SUPPLY_CHAIN_SECURITY_SPEC.md`, `TEST_SPEC.md`.

Decisions: 后续提交随命名评审包登记。

## Verification Plan

机器契约 `specs/sandbox-template-build.contract.json`（draft、`implementationAuthorized: false`）由契约测试钉住：字段形状、封闭状态集与操作集、证据门、与 REQ-2026-0029/0012/0019 的分层引用。实现授权前不写实现用例。

## Release Boundary

在 Builder 运行时切片单独授权并落地之前，不得实现构建执行、构建产物/缓存存储、Registry 服务、API/SDK/CLI，也不得宣称 E2B Template Build 能力对齐。权威模型切片（记录类型、生命周期校验、结果绑定规则、证据门文档）已于 2026-10-05 授权并落地。

## Implementation Gate

`ready` since 2026-10-05: REVIEW-20261005（命名、数据所有权、结果绑定语义、证据门、forbidden 面）由仓库所有者以单一所有者结构化决策接受，ADR-20261005 同日 `accepted`。授权的实现切片仅为权威模型：`crates/sdkwork-intelligence-sandbox-build-authority` 承载 `SandboxTemplateBuild` 记录、生命周期校验、结果绑定规则与机器契约对齐测试；`specs/sandbox-template-build.contract.json` 对该切片翻转为 `implementationAuthorized: true` 并保持 `draft`。Builder 运行时、流水线执行、构建产物/缓存存储、Registry 服务、CLI、公共 API/SDK 与部署 profile 仍被契约 `forbidden` 块锁住，直到各自的后续需求切片落地；Builder 运行时切片存在之前不得宣称 E2B Template Build 能力对齐。

## Implementation Authorization

`ready` since 2026-10-05：批准记录见 REVIEW-20261005 的 approval basis（本会话重复的镜像/snapshot 能力指令 + REVIEW-20260929 结构化指令延续，单一所有者惯例）。权威模型切片已随码与测试落地；Blocking Findings（Builder 运行时/流水线/存储缺失、CLI/API 未定、真实构建执行与制品 Tuple 证据缺失、无可绑定的真实制品 Tuple、Warm 槽桥未批）全部保持为后续切片的持续证据义务。
