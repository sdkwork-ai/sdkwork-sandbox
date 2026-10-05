# ADR-20261006: Sandbox Bounded Template Registry

Status: accepted

Accepted: 2026-10-06 by the repository owner via the structured session instruction recorded in [REVIEW-20261006](../../engineering/reviews/REVIEW-20261006-sandbox-bounded-template-registry.md) (single-owner convention). REG-01..REG-04 approved; evidence obligations standing.

Requirement: [REQ-2026-0029](../../product/requirements/REQ-2026-0029-sandbox-template-authority.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

## Context

RES-03 为模板解析预留了 registry-backed 解析器，但 0029 契约把 Registry 服务锁在其各自切片之后。本地车道的模板解析需要一个进程内有界 registry 才能闭合（发布版本 → 解析 → 真实执行）——这是 0029 线的控制面切片，与 pool-control/transaction-control 同型。

## Decision

1. `BoundedSandboxTemplateRegistry` 直接组合 0029 权威记录（`SandboxTemplateDefinition`/`SandboxTemplateVersion`），不引入第二记录类型（REG-01）。
2. 有界、进程内：超出容量拒绝；存储后端仍被 `x-sdkwork-no-storage-backend` 锁住（REG-02）。
3. 解析面只暴露 版本引用 → 定义 start command（经权威访问器 版本→定义→`sandbox_set_start_cmd()`），不扩大 registry 表面（REG-03）。
4. Worker 本地车道解析器经该 registry 实现既有端口，不重复解析逻辑（REG-04）。

## Alternatives

- 第二记录类型（轻量 registry 条目）：拒绝——两套记录必然漂移；0029 记录已是发布不可变。
- 直接上存储后端：拒绝——存储切片另有评审；进程内有界切片先钉控制面语义。
- 解析面暴露完整版本/定义：拒绝——worker 只需要 start command；过宽表面扩大审计面。

## Consequences

- 本地车道解析闭合（发布版本 → 解析 → 真实执行）。
- 发布记录仍是进程内事实；storage 切片落地前不跨进程、不重启存活。
- 在 storage 切片落地前，不得宣称 E2B Template 能力对齐。

## Verification

- 契约 `forbidden.templateRegistryService` 翻转由契约测试钉住（bounded in-process only）。
- 有界拒绝、解析路径（版本→定义→start cmd）与重复发布拒绝以单元测试钉死。
- storage 切片自带 REQ、持久层评审与测试；在此之前禁止实现。

## Review

Required human owners: Architecture, Security/Privacy, Capacity/Scheduler, Command/Execution, Workspace/Storage。由仓库所有者以单一所有者结构化决策批准（见 REVIEW-20261006 的 approval basis）。

## Supersedes / Superseded By

None.
