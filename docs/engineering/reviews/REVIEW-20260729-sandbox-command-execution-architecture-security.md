# REVIEW-20260729: Sandbox Command Execution Architecture And Security

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured implementation-gate decision of 2026-09-24, including the close-out items above resolved per each packet's own recommended resolutions. Recorded by the executing agent on that instruction.

Requirement: [REQ-2026-0007](../../product/requirements/REQ-2026-0007-sandbox-command-execution-contract.md)

Decision: [ADR-20260729](../../architecture/decisions/ADR-20260729-sandbox-command-execution-and-terminal-boundary.md)

Owner: SDKWork Runtime Platform

Date: 2026-07-29

Risk: critical - public Rust contract, execution security boundary, Provider composition, and cross-repository Kernel integration.

## Scope And Inputs

本 Review 请求人工评审 Provider-neutral `SandboxCommandExecutor`、公共 `SandboxCommandExecution*` 类型、Terminal Capability 语义、Executable Resolution/Environment Policy Authority、Fencing/Idempotency、Output Bound 与 Local/Firecracker Common Conformance。评审输入为 Product PRD、REQ-2026-0007、对应 ADR、[Provider Delivery Plan](../plans/PLAN-2026-0001-local-and-firecracker-provider-delivery.md)、`CODE_REVIEW_SPEC.md`、`SECURITY_SPEC.md`、`PERFORMANCE_SPEC.md`、`OBSERVABILITY_SPEC.md`、`RUST_CODE_SPEC.md` 与 `TEST_SPEC.md`。

当前实现只存在 Lifecycle Port；本 Review 不包含代码完成结论。Docker、Interactive PTY、Shell、Network、Browser、Port、Secret Injection、HTTP/RPC、SDK 与 Deployment Profile 不在本次批准范围。

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| CMD-01 | `SandboxProvider` 保持 Lifecycle Port；Command 使用独立 `SandboxCommandExecutor`，按同一 `SandboxProviderId` 组合。 | 保持接口隔离，进入公共 Contract 实现。 | 在修改 REQ/ADR 并重新评审前停止实现；不得创建 Provider-private `exec`。 |
| CMD-02 | 公共类型固定为 `SandboxCommandExecutionRequest`、`SandboxCommandCancellationRequest`、`SandboxCommandLimits`、`SandboxCommandExecutionResult`、`SandboxCommandExitStatus` 与 `SandboxCommandExecutionError`。 | 允许进入 Provider SPI Public Export；Sandbox-owned 字段继续使用 `sandbox_*`。 | 记录替代命名及迁移影响，更新 PRD/REQ/ADR/Component Spec 后重审。 |
| CMD-03 | `RuntimeCapability::Terminal` 第一版只表示有界、非交互 Executable + Argv；禁止 Command String、Implicit Shell 与自动回退。 | Local/Firecracker 使用同一 No-shell Contract。 | Terminal Capability 必须拆分或重命名后重审，不能保留含混语义。 |
| CMD-04 | Request 必须携带 Tenant、Sandbox Workspace/Session/Allocation/Runtime Binding、Fencing Token 与 Command Operation Identity。 | Provider 能在副作用前验证 Ownership、Readiness、Fencing 与 Idempotency。 | 在缺失的身份/所有权契约明确前停止实现。 |
| CMD-05 | stdout/stderr 使用分别有界的 Byte Buffer 与 Truncated 标志，不强制 UTF-8；Raw Command、Argument、Path、Environment、Output 与 Provider-private Identity 不进入普通 Log/Metric。 | 保留任意字节并满足 Redaction/Bound。 | 需要给出等价的数据完整性、内存上限与隐私证明后重审。 |
| CMD-06 | Timeout、Cancel、Output Limit、Lease Lost 与 Provider Shutdown 共用有界 Descendant Cleanup Contract；Stale Fencing 在启动副作用前失败。 | Common Conformance 固定安全收敛语义。 | 不得声明 Terminal Capability。 |
| CMD-07 | Descriptor 只有在 Lifecycle Provider、匹配 Identity 的 Executor 与 Common Conformance 同时存在时才声明 Terminal；不一致时 Provider Unavailable。 | Composition 关闭失败且不夸大 Capability。 | 必须给出同等级的机器可验证 Capability Authority 后重审。 |
| CMD-08 | 同一 Operation ID + 同一 Request Fingerprint 可重放有界结果；不同 Fingerprint 返回 Conflict，不启动第二个 Command。 | 固定重试与重复提交语义。 | 在替代幂等模型与保留边界获批前停止实现。 |
| CMD-09 | Execution/Cancel Fingerprint 由 Sandbox Service 使用版本化、域隔离、长度前缀 Canonical Encoding 派生，Executor 必须重算；`traceId` 不参与语义 Hash。 | 防止调用方伪造 Fingerprint 绕过 Operation Conflict，并允许同一语义重试保持稳定。 | 在替代的双边机器可验证 Fingerprint Authority 获批前停止实现。 |
| CMD-10 | Cancel 使用完整 Ownership/Fencing、目标 Command Operation、独立 Cancellation Operation 与 Fingerprint；启动后的 Timeout/Cancel/Output/Resource/Fencing 只返回终态 Result，Error 只用于启动前拒绝或权威 Result 不可得。 | 取消可审计、幂等且不跨 Session；不把可能产生副作用的终态伪装成可盲重试错误。 | 必须先提供同等级的取消幂等、未知完成恢复与副作用安全证明。 |
| CMD-11 | Executor 使用 durable first-terminal CAS 仲裁 Exit/Timeout/Cancel/Output/Resource/Fencing 竞争；Result 明确 Command Result Replay、Outcome/Exit/Truncation、Cleanup Status/Duration，Cleanup Failure 保留 Primary Outcome 并 Quarantine Binding。 | Local/Firecracker 终态竞争只有一个权威结果，Cleanup 不确定性不会被隐藏或允许资源复用。 | 不得声明 Terminal Capability，直到提供等价的终态唯一性、清理可见性和残留隔离证明。 |
| CMD-12 | `sandboxExecutable` 固定为 Provider-neutral Logical Identifier。Provider-owned Registry Snapshot 绑定 Runtime Binding 且生命周期内不可变；禁止 Caller Path、OS PATH Search 与 CWD Lookup，Resolved Identity 保持私有，重放不得改用不同 Binary。 | Executable Resolution 不受调用方或 Ambient Host State 控制，HA Replay 保持执行身份一致。 | 在同等级的可验证 Resolution Authority 获批前停止实现。 |
| CMD-13 | Final Environment 从空集合按同一 Binding Policy Snapshot 构造；请求不能扩展名称 Policy，Value 按名称校验，受保护名称不可提交或覆盖，Provider 固定值仅在请求验证后注入。 | Host Control/Resolution/Credential Environment 不进入调用方控制面。 | 在等价的 Environment Authority 与 Override Denial 获批前停止实现。 |

## Review Evidence

- 当前 Provider SPI、Lifecycle Service、PostgreSQL Repository 与 Kernel Adapter 已通过 Cargo Test/Clippy、Component Port、Layering、Naming 与依赖链检查。
- Gate 0 下 Local crate 的 `#[cfg(test)]` Fake Host Boundary 已通过 5 个纯数据测试：`.` Workspace Root、可移植 ASCII Logical Relative Path、任意反斜杠/Windows Device/Console Alias/非法字符拒绝、Typed Argv 无 Shell 解析、Logical Executable 语法先于 Allowlist、Credential/Protected Environment Name 和 NUL/CR/LF 拒绝、七项输入上限与 Protected Name 契约交叉校验；该 Harness 不访问 Host、不导出 Command Port，也不构成执行或隔离证据。
- `node --test tests/contract/provider-delivery-gate.contract.test.mjs` 通过 7/7 个跨组件 Contract Test，其中 4 个专门锁定 Gate 0 的 REQ/ADR 状态、Local 空 Port、延迟 Provider Crate 和公共 Command Port 禁止项。
- `cargo tree` 证明当前依赖方向为 `sdkwork-agents -> sdkwork-kernel -> sdkwork-sandbox`；本设计不向 Kernel 引入 Local/Firecracker 分支。
- REQ/ADR 已定义 No-shell、Provider-owned Logical Executable Resolution、无 PATH/CWD Search、Runtime Binding Policy Snapshot 不可变、Protected Environment Override Denial、Byte Output、Bounded Buffer、Stale Fencing、Canonical Fingerprint、Execution/Cancel 双幂等、Result-unavailable 同 Operation 重试、durable first-terminal CAS、Outcome/Exit/Truncation 一致性、Cleanup Status/Quarantine、Terminal Result/Error Partition、Redaction 与 Unsupported Capability Negative Cases。
- 真实 Local Host 与 Linux KVM Firecracker Evidence 尚不存在，不能作为本 Design Review 的完成证据。

## Candidate Contract Evidence

- `apis/commands/sandbox-command-execution-request.schema.json` fixes Sandbox identity, provider binding, fencing, request fingerprint, Provider-neutral logical executable identifier, bounded Argv, logical relative working directory, protected deny-by-default environment, and bounded limits.
- `apis/commands/sandbox-command-cancellation-request.schema.json` fixes a separately idempotent, tenant-scoped, fully bound and fenced cancellation request without arbitrary reason text.
- `apis/commands/sandbox-command-execution-result.schema.json` fixes binary-safe base64 output, command-result replay, Outcome/Exit/Truncation consistency, bounded timing/resource usage, Cleanup Status/Duration, and a closed execution outcome set.
- `apis/commands/sandbox-command-execution-error.schema.json` fixes the safe pre-start/result-unavailable error taxonomy, code-bound retryability and same-operation retry rules without host, allocation, or secret details.
- `apis/commands/sandbox-command-contract.json` fixes Provider-owned executable resolution without PATH/CWD lookup, Runtime-Binding-scoped immutable environment policy, canonical fingerprint authority, idempotency keys, bounded HA replay, durable first-terminal arbitration, cleanup/quarantine, terminal completion semantics, forbidden execution modes and common conformance scenario names; `implementationAuthorized` remains `false` while this Review is pending.
- `node --test tests/contract/sandbox-command-contract.contract.test.mjs` passes 9/9. This is static contract evidence only and does not authorize implementation or replace human review.

## Blocking Review Questions

1. Architecture Reviewer 是否接受 CMD-01 至 CMD-13 的 Port、命名、Ownership、Executable/Environment Policy Authority、Fingerprint、Cancel、Terminal Arbitration 与 Composition Boundary？
2. Security Reviewer 是否接受 `Terminal` 的首版非交互语义、无 PATH/CWD Resolution、Protected Environment Override Denial、Output/Environment 数据分类、Canonical Fingerprint、Result/Error Partition、Terminal Race 与 Cleanup/Quarantine Failure Model？
3. Kernel Owner 是否接受仅消费 Provider-neutral Port，并保持 `sdkwork-agents -> sdkwork-kernel -> sdkwork-sandbox`？
4. Reviewer 若拒绝任一项，必须记录 Decision ID、替代方案、受影响 REQ/ADR/Component 与重新评审条件。

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`。`Approved with follow-up` 不得用于推迟公共命名、Fencing、Cleanup、Redaction、Capability Authority 或真实 Provider 安全证据。

| Reviewer role | Reviewer | Outcome | Date | Decision IDs / findings |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-09-24 | CMD-01..CMD-13 |
| Security owner | Repository Owner (structured approval) | Approved | 2026-09-24 | CMD-03..CMD-13 |
| Kernel integration owner | Repository Owner (structured approval) | Approved | 2026-09-24 | CMD-01, CMD-02, CMD-04, CMD-07..CMD-13 |

## Implementation Gate

已批准（2026-09-24，仓库所有者以全部评审角色身份经结构化实现闸门决定批准）。REQ-2026-0007 进入 `ready`、ADR 进入 `accepted`；按既定顺序实施：Provider SPI -> Service/Registry -> Common Conformance -> Kernel/Agents Dependency Chain。

## Close-Out Checklist (Reviewer 执行项)

Review Approved 前必须逐项核验：

- [x] REQ-STATUS: 对应 REQ 处于 `ready` 或 `accepted`
- [x] ADR-STATUS: 对应 ADR 处于 `accepted`
- [x] ARCH-REVIEW: 接口契约、命名、Port 边界、L0-L6 分层符合 COMPONENT_SPEC
- [x] SEC-REVIEW: 数据分类、红字规则、零化清理、Secret 流、并发控制符合 SECURITY_SPEC
- [x] PERF-REVIEW: 有界 Page/Buffer、低 Cardinality Metric 符合 PERFORMANCE_SPEC
- [x] OBS-REVIEW: Trace/Audit/Event/Outbox/Meter 符合 OBSERVABILITY_SPEC
- [x] TEST-EVIDENCE: Unit Test 全量通过；Contract Test 通过
- [x] DEPENDENCY-DIRECTION: cargo tree 方向正确
- [x] EVIDENCE-SIGN-OFF: 对应 Verification Review 接受状态非 pending
- [x] HUMAN-DECISION: Decision Matrix 每条均 Approved 或 Changes + 替代方案

## Exit Gate

1. 全部 Checklist 勾选
2. 所有 Reviewer Role 表决 Approved
3. REQ 进入 `ready`，ADR 进入 `accepted`
4. Gate 0 `implementationAuthorized` 最后一个 Review 通过后可置 true

未经上述门禁，禁止进入 V1 实现阶段。
