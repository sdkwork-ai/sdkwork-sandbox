# ADR-20261006: Sandbox Worker Local-Lane Start-Command Executor Wiring

Status: accepted

Accepted: 2026-10-06 by the repository owner via the structured session instruction recorded in [REVIEW-20261006](../../engineering/reviews/REVIEW-20261006-sandbox-worker-exec-wiring.md) (single-owner convention). WRE-01..WRE-04 approved; evidence obligations standing.

Requirement: [REQ-2026-0034](../../product/requirements/REQ-2026-0034-sandbox-worker-launch-execution.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

## Context

EXE-06 命名了执行器接线切片：`SandboxLaunchStartCommandPort` 需要一个经 `REQ-2026-0007` 已就绪本地执行器（真实 tokio 进程）的实现。端口是同步单方法，执行器是 async_trait；两者之间需要桥。命令、作用域与限制必须显式注入——模板版本解析是独立切片。

## Decision

1. 接线实现既有的窄端口，不改端口签名（WRE-01）。
2. 桥接用适配器自有的专用多线程 tokio runtime（`block_on`），不假设外部 runtime 存在（WRE-02）。
3. 结果映射诚实：executor 无终态结果的错误 → 端口 `Uncertain`（执行记录 `quarantined`）；非零退出 → `Failed`；仅零退出 → `Started`（WRE-03）。
4. 命令、作用域与限制构造期注入；operation id 从计划引用派生；模板版本解析是命名中的下一切片（WRE-04）。

## Alternatives

- 把端口改为 async：拒绝——改签名会波及已落地的组合引擎与全部测试。
- 复用调用方 runtime（假定 async 上下文）：拒绝——组合宿主可能是纯同步进程，块上外部 runtime 会 panic。
- executor 错误一律报 `Failed`：拒绝——无终态结果的错误（超时、不可用）与确定性失败是两种事实，混同会让幽灵成功进入审计。

## Consequences

- 本地车道自此可真实执行 start command（真实进程、有界输出、硬超时——全部继承自 REQ-2026-0007 执行器）。
- start command 仍为构造期注入；模板解析切片落地前，执行内容与模板版本无权威绑定。
- 在模板解析切片落地前，不得宣称 E2B 快速创建能力对齐。

## Verification

- 真实进程测试：真实 `echo` 经 host boundary 白名单、真实 runner、端口、适配器端到端到达 `started`；白名单拒绝路径到达 `failed`/`quarantined`。
- 后续切片（模板解析/Firecracker 车道/CLI/API）各自带 REQ 与真实证据；在此之前禁止实现。

## Review

Required human owners: Architecture, Security/Privacy, Capacity/Scheduler, Command/Execution, Workspace/Storage。由仓库所有者以单一所有者结构化决策批准（见 REVIEW-20261006 的 approval basis）。

## Supersedes / Superseded By

None.
