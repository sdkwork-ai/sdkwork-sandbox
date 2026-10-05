# ADR-20261006: Sandbox Worker Start-Command Template Resolution

Status: accepted

Accepted: 2026-10-06 by the repository owner via the structured session instruction recorded in [REVIEW-20261006](../../engineering/reviews/REVIEW-20261006-sandbox-worker-template-resolution.md) (single-owner convention). RES-01..RES-04 approved; evidence obligations standing.

Requirement: [REQ-2026-0034](../../product/requirements/REQ-2026-0034-sandbox-worker-launch-execution.md)

Owner: SDKWork Runtime Platform

Date: 2026-10-06

## Context

WRE-04 将 start command 定为构造期注入，并把模板版本解析命名为下一切片。构造期注入让执行内容与计划携带的版本引用脱钩——不同版本的计划无法得到各自的命令。端口签名需要扩展，解析需要声明端口，解析失败的结局类别必须定案。

## Decision

1. 端口签名扩展：派发随计划携带模板版本引用；适配器从计划透传（RES-01）。
2. 解析是声明端口 `SandboxStartCommandResolverPort`（版本引用 → 已解析命令）；registry 支持的实现由 `REQ-2026-0029` 的 registry 切片拥有，本切片只落端口、测试脚本化（RES-03）。
3. 解析失败是确定性 `failed`，不是 `uncertain`——配置错误不隔离容量（RES-02）。
4. 真实进程证明：解析出的命令在本地车道端到端运行至 `started`（RES-04）。

## Alternatives

- 保持构造期注入：拒绝——执行内容与版本引用脱钩，不同版本计划的命令语义失去权威来源。
- 解析失败报 `uncertain`：拒绝——配置错误是确定性事实；隔离容量是对不确定性的处理，不是对配置错误的。
- 本切片直接实现 registry 查询：拒绝——0029 契约明锁 Registry 服务；端口先行让实现切片各守各的门。

## Consequences

- 端口签名演进（round-12 WRE-01 的"不改签名"由本切片评审修正）；组合引擎只透传版本引用，无新状态。
- 解析失败进入 `failed` 终态，容量审计与配置审计分离。
- 在 registry-backed 解析落地前，执行内容仍来自测试脚本与后续注入实现；不得宣称 E2B 快速创建能力对齐。

## Verification

- 真实进程测试：`version-1` 解析至 echo 调用并真实运行至 `started`；未知版本确定性 `failed`；白名单拒绝 `uncertain`。
- registry-backed 解析切片自带 REQ、0029 权威对齐与发布版本解析证明；在此之前禁止实现。

## Review

Required human owners: Architecture, Security/Privacy, Capacity/Scheduler, Command/Execution, Workspace/Storage。由仓库所有者以单一所有者结构化决策批准（见 REVIEW-20261006 的 approval basis）。

## Supersedes / Superseded By

None.
