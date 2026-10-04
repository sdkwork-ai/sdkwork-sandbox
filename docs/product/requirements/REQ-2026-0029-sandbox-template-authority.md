---
id: REQ-2026-0029
title: Deliver the Sandbox Template Authority
owner: SDKWork Runtime Platform
status: ready
priority: critical
source: customer
problem: E2B 的快速创建与快速部署全部依赖 Template（预构建环境定义、tags 版本化、start command 常驻、构建缓存与层复用）；本仓该能力类此前零需求承载，是能力对齐的最大结构性空档。
goals:
  - 建立 provider-neutral 的 Template 权威模型：声明式定义（基础环境引用、文件层、环境变量、start command）、tags/aliases 版本化命名、构建输入引用与只读元数据。
  - 固定构建输入边界：Dockerfile 或构建脚本只允许作为构建输入格式（PRD 非目标原话），运行时隔离边界永远不是 Docker。
  - 定义构建缓存与层复用的语义边界（Hot/Warm/Cold 分层、淘汰策略、跨 Template 层复用条件），作为后续缓存切片的权威。
  - 与 Firecracker 制品元组（REQ-2026-0012）分层：Template 权威引用制品 Tuple，不复制其 Evidence/签名/撤销边界。
non_goals:
  - 不实现 Template Builder、Registry、构建流水线、构建产物存储或缓存存储后端。
  - 不授权 CLI（template init/build/deploy）、公共 API/SDK 面、部署 profile 或多区域分发。
  - 不把 Docker 作为运行时依赖或隔离边界。
  - 不授权 Snapshot/Fork（REQ-2026-0021 线）或 Runtime Pool（REQ-2026-0019 线）。
users:
  - SDKWork SaaS runtime operators
  - Agent 平台集成方
  - Sandbox Template maintainers
affected_surfaces:
  - rust-components
  - composition
  - api
---

# REQ-2026-0029: 交付 Sandbox Template 权威

## Readiness Blockers

- ~~人工接受本需求与对应 ADR 的公共命名、数据所有权和构建输入边界~~ — 已解除：REVIEW-20261004 于 2026-10-04 以单一所有者结构化决策接受（公共命名、数据所有权、构建输入边界、缓存语义权威、forbidden 面全部批准），ADR-20261004 同日 `accepted`。
- 构建产物的供应链证据边界必须复用 `REQ-2026-0012` 的 Artifact Evidence 体系，不得另立第二套签名/撤销权威。（持续义务）
- `e2b template init/build/deploy` 的 CLI 面与 REST 构建流水线在后续切片单独评审；本需求只承载权威模型与缓存语义。（持续义务）

## Candidate Acceptance Criteria

- 候选权威类型为 `SandboxTemplateDefinition`（声明式：基础环境引用、文件层、`sandbox_set_envs`、`sandbox_set_start_cmd`）、`SandboxTemplateVersion`（tags/aliases 版本化命名）与 `SandboxTemplateBuildInput`（Dockerfile/构建脚本引用，仅作构建输入）。
- 所有 wire 字段 `sandbox_` 前缀；引用 opaque，不携带 Host Path、下载 URL 或嵌入签名材料。
- 构建缓存语义固定为 Hot/Warm/Cold 三层与显式淘汰策略的**权威定义**；实现、存储后端与跨节点协调全部推迟到后续切片。
- 每个 Template 版本必须引用一个满足 `REQ-2026-0012` 精确 Tuple 的制品集合；Template 权威不拥有制品 Evidence。
- Start/Ready command 语义与 Runtime Pool 的 `PreparedSlot`（REQ-2026-0019）及 Guest Boot Contract 的衔接在后续切片定义；本需求只固定定义字段。

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `COMPONENT_SPEC.md`, `API_SPEC.md`, `SDK_WORKSPACE_GENERATION_SPEC.md`, `SUPPLY_CHAIN_SECURITY_SPEC.md`, `TEST_SPEC.md`.

Decisions: 后续提交随命名评审包登记。

## Verification Plan

机器契约 `specs/sandbox-template-authority.contract.json`（draft、`implementationAuthorized: true`，仅权威模型切片）由契约测试钉住：字段形状、opaque 引用、Docker 仅作构建输入、缓存语义边界、与 REQ-2026-0012 的分层引用，以及权威模型 crate 与契约的逐项对齐。Builder/Registry/缓存后端/CLI/API 切片在各自授权前不写实现用例。

## Release Boundary

在 Builder/Registry 切片单独授权并落地之前，不得实现 Builder/Registry/缓存后端/CLI，也不得宣称 E2B Template 能力对齐。权威模型切片（记录类型、校验、缓存语义词汇）已于 2026-10-04 授权并落地。

## Implementation Gate

`ready` since 2026-10-04: REVIEW-20261004（公共命名、数据所有权、构建输入边界、缓存语义权威、forbidden 面）由仓库所有者以单一所有者结构化决策接受，ADR-20261004 同日 `accepted`。授权的实现切片仅为权威模型：`crates/sdkwork-intelligence-sandbox-template-authority` 承载三个记录类型、fail-closed 校验、缓存语义词汇与机器契约对齐测试；`specs/sandbox-template-authority.contract.json` 对该切片翻转为 `implementationAuthorized: true` 并保持 `draft`。Builder 运行时、Registry 服务、构建流水线/构建产物存储、缓存后端、CLI、公共 API/SDK 与部署 profile 仍被契约 `forbidden` 块锁住，直到各自的需求切片落地；Builder 切片存在之前不得宣称 E2B Template 能力对齐。

## Implementation Authorization

`ready` since 2026-10-04：批准记录见 REVIEW-20261004 的 approval basis（本会话三次重复的镜像能力指令 + REVIEW-20260929 结构化指令延续，单一所有者惯例）。权威模型切片已随码与测试落地；Blocking Findings（Builder/Registry/缓存后端缺失、start-command 语义未连接 PreparedSlot、base-environment 目录权威未定、无真实制品 Tuple 可绑定）全部保持为后续切片的持续证据义务。
