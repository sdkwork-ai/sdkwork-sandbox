# ADR-20261004: Sandbox Template Authority

Status: accepted

Accepted: 2026-10-04 by the repository owner via the structured session instruction recorded in [REVIEW-20261004](../../engineering/reviews/REVIEW-20261004-sandbox-template-authority-naming-and-boundaries.md) (single-owner convention; see that packet's approval basis). TMPL-01..TMPL-08 approved; evidence obligations standing.

Requirement: [REQ-2026-0029](../../product/requirements/REQ-2026-0029-sandbox-template-authority.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-04

## Context

E2B 的"快速创建 + 快速部署"全部建立在 Template 之上：预构建环境定义、tags/aliases 版本化命名、start command 常驻、构建缓存与层复用。本仓在登记 REQ-2026-0029 之前对该能力类零承载；draft 权威模型与机器契约已于 2026-09-29 落树（`specs/sandbox-template-authority.contract.json`），但其公共命名、数据所有权与构建输入边界尚未经人工评审，实现保持未授权。

## Decision

1. Template 权威是 provider-neutral 的记录模型：`SandboxTemplateDefinition`（声明式定义：基础环境引用、文件层、`sandbox_set_envs`、`sandbox_set_start_cmd`）、`SandboxTemplateVersion`（tags/aliases 版本化命名，绑定一个制品 Tuple 引用）与 `SandboxTemplateBuildInput`（Dockerfile/构建脚本引用，仅作构建输入）。所有 wire 字段 `sandbox_` 前缀，引用 opaque。
2. 定义发布后不可变。tags 与 aliases 是版本化命名，发布后同样不可变；变更 = 新版本。
3. Docker 与构建脚本只是**构建输入格式**，永远不是运行时依赖或隔离边界（PRD 非目标原话）。构建输入是 opaque 引用：不允许 Host Path、下载 URL 或嵌入式签名/密钥材料。
4. 构建缓存的权威语义固定为 Hot/Warm/Cold 三层 + 显式淘汰策略 + 跨 Template 层复用必须精确 digest 匹配；存储后端、淘汰引擎与跨节点协调全部属于后续切片，不在本需求内。
5. 制品分层：每个 Template 版本必须引用一个满足 `REQ-2026-0012` 精确 Tuple 的制品集合；Template 权威不拥有制品 Evidence/签名/撤销，也不允许出现第二套供应链权威。
6. Builder 运行时、Registry 服务、构建流水线/构建产物存储、CLI（template init/build/deploy）、公共 API/SDK 面与部署 profile 全部保持 forbidden，直到各自的后续需求切片单独授权。
7. Start/Ready command 语义与 Runtime Pool `PreparedSlot`（REQ-2026-0019）及 Guest Boot Contract 的衔接由后续切片定义；本需求只固定定义字段。
8. 快速分配/快速启动的商业延迟声明仍受 REQ-2026-0019 的测量纪律约束：没有发布的实测 profile，不得把目标写成 SLO。

## Alternatives

- 直接实现 E2B 式 Builder/Registry：拒绝——没有构建产物供应链边界与存储权威之前，构建流水线会成为第二套未审计的制品来源。
- 把 Template 作为 Pool `PreparedSlot` 的一个字段：拒绝——Template 是部署单元（定义/版本/缓存），Pool 是运行时容量；耦合会让两者都无法独立评审。
- 用 Docker 镜像作为 Template 运行时形态：拒绝——PRD 明文非目标；Docker 仅允许作为构建输入格式。
- 把构建缓存做成透明缓存（无显式淘汰策略）：拒绝——不可审计的缓存失效会把"环境一致性"变成概率声明。

## Consequences

- 权威模型可先行落地为带校验的记录类型与机器契约钉面；Builder/Registry/缓存后端在后续切片授权前不得存在。
- Template 引用制品 Tuple 而不拥有 Evidence，供应链审计仍走 `REQ-2026-0012` 单一权威。
- 在 Builder 切片落地前，本仓不得宣称 E2B Template 能力对齐；快速部署路径的"最后一公里"仍是缺失的。
- `sandbox_set_start_cmd` 的语义连接（PreparedSlot/Guest Boot Contract）必须后续定义，避免出现两套启动语义。

## Verification

- 机器契约 `specs/sandbox-template-authority.contract.json` 与权威模型 crate 逐字段对齐（契约测试钉 states/字段/构建输入/缓存/分层/forbidden）。
- 构建输入 fail-closed 校验：Host Path、下载 URL、嵌入式密钥材料、越界长度全部拒绝。
- 缓存语义只允许封闭的三层词汇与显式淘汰策略；跨 Template 复用强制精确 digest 匹配。
- 后续切片（Builder/Registry/缓存后端/CLI/API）各自带 REQ、证据义务与真实构建流水线测试；在此之前禁止实现。

## Review

Required human owners: Architecture, Security/Privacy, Supply Chain, API/SDK, Capacity/Performance, Workspace/Agents。由仓库所有者以单一所有者结构化决策批准（见 REVIEW-20261004 的 approval basis）。

## Supersedes / Superseded By

None.
