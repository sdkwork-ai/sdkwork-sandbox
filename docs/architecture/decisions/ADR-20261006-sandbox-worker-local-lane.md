# ADR-20261006: Sandbox Worker Local-Lane Execution Adapter

Status: accepted

Accepted: 2026-10-06 by the repository owner via the structured session instruction recorded in [REVIEW-20261006](../../engineering/reviews/REVIEW-20261006-sandbox-worker-local-lane.md) (single-owner convention). EXE-01..EXE-06 approved; evidence obligations standing.

Requirement: [REQ-2026-0034](../../product/requirements/REQ-2026-0034-sandbox-worker-launch-execution.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

## Context

WRK-07 为本地车道执行适配器预留了独立切片。链上四个组件已就绪：0033 启动计划权威、0034 执行记录权威、0019 池 claim、0007 命令执行器端口（本地执行器为真实 tokio 进程实现）。缺的是把计划变成执行的组合引擎——以及执行器与 host boundary 的最终接线。

## Decision

1. 适配器只经就绪权威组合：计划消费走 0033 权威 API，执行状态走 0034 权威 API；适配器不自持第二套状态机。
2. start command 经单一窄端口 `SandboxLaunchStartCommandPort`（按计划引用执行并报告成败）派发；端口报告成功是 `started` 的唯一路径。
3. 端口失败 → `failed`；端口不确定 → `quarantined`；两者都不静默成功。
4. provisioner 是声明端口（挣取分配引用）；本切片不携带 host boundary——`REQ-2026-0007` 执行器接线（host boundary fixture + 真实 tokio 进程）是命名中的下一切片。
5. 车道范围：本地车道；Firecracker 车道锁在 KVM 证据之后，Warm 槽锁在 0019 之后。

## Alternatives

- 适配器内嵌计划/执行的第二套状态副本：拒绝——两套真相必然漂移；权威 API 是唯一写路径。
- 适配器直调 `SandboxCommandExecutor` 并自带 host boundary：拒绝——host boundary fixture 属执行器接线切片；本切片先把组合语义钉死，端口实现可热插。
- 端口超时即成功（乐观完成）：拒绝——不确定与成功是两种终态，混同会把幽灵启动注入审计。

## Consequences

- 组合语义可先行钉死；执行器接线在下一切片落地前，start command 不会真实执行。
- 适配器对计划/执行/端口三方都只有引用，无第二权威。
- 在执行器接线落地并证明真实执行前，不得宣称 E2B 快速创建能力对齐。

## Verification

- 契约 `localLaneExecutionSliceAuthorized` 翻转由契约测试钉住；组合引擎的plan 消费、挣取引用、终态可达性与无静默成败以脚本化端口测试钉死。
- 执行器接线切片自带 host boundary fixture、真实进程测试与故障注入。

## Review

Required human owners: Architecture, Security/Privacy, Capacity/Scheduler, Command/Execution, Workspace/Storage。由仓库所有者以单一所有者结构化决策批准（见 REVIEW-20261006-local-lane 的 approval basis）。

## Supersedes / Superseded By

None.
