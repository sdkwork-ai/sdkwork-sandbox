---
id: REQ-2026-0007
title: Deliver the provider-neutral Sandbox command execution contract
owner: SDKWork Runtime Platform
status: draft
priority: critical
source: platform
problem: Local and Firecracker execution cannot be completed through the lifecycle-only SandboxProvider port, and provider-specific command APIs would split Runtime semantics and force Kernel behavior branches.
goals:
  - Define one bounded, typed Sandbox command execution contract shared by Local and Firecracker Providers.
  - Preserve Sandbox Session, Runtime Binding, Workspace, Provider, and Fencing ownership on every command.
  - Prove timeout, cancellation, output bounds, environment minimization, cleanup, and unsupported-capability behavior through common conformance tests.
non_goals:
  - Implement an implicit shell, interactive PTY, Browser, Port Forward, Network, Secret resolver, Scheduler, HTTP route, SDK, or Docker Provider.
  - Expose physical Workspace paths, Provider allocation references, host process ids, API sockets, or microVM identities.
  - Change Agent tool selection, MCP semantics, AgentSession, or AgentWorkspace ownership.
users:
  - SDKWork Kernel integrators
  - Local and Firecracker Sandbox Provider maintainers
  - SDKWork developers executing bounded build and tool commands
affected_surfaces:
  - rust-components
  - composition
  - security
  - observability
---

# REQ-2026-0007: 交付 Provider-neutral Sandbox Command Execution Contract

## Readiness Blockers

本需求在以下决策完成人工评审前保持 `draft`：

- 接受 `SandboxCommandExecutor` 独立端口以及候选 `SandboxCommandExecution*`、`SandboxCommandCancellationRequest` 公共类型命名。
- 接受 `SandboxProvider` 继续只拥有生命周期，Command Execution 通过同一 `sandbox_provider_id` 组合，而不是向生命周期 Trait 填入所有能力方法。
- 接受 Local Provider 的 HostUser 边界与 Firecracker Provider 的 MicroVm 边界；共同契约不能把较弱 Provider 的限制静默提升为较强保证。
- 明确第一版只交付非交互 Executable + Argv；PTY、Shell、Network、Browser、Port 和 Secret Injection 分别使用后续 Ready Requirement。

## Candidate Acceptance Criteria

- 新端口候选名为 `SandboxCommandExecutor`；实现必须绑定一个已注册的 `SandboxProviderId`。Provider Descriptor 只有在同一 Provider 已注册并通过执行端口 Conformance 时才能声明 `RuntimeCapability::Terminal`。
- 请求候选类型为 `SandboxCommandExecutionRequest`，至少携带服务器拥有的 `trace_id`、`tenant_id`、`sandbox_workspace_id`、`sandbox_session_id`、`sandbox_id`、`sandbox_runtime_binding_id`、`sandbox_fencing_token`、`sandbox_command_operation_id`、`sandbox_request_fingerprint`、`sandbox_executable`、`sandbox_arguments`、`sandbox_working_directory`、`sandbox_environment` 与 `sandbox_command_limits`。
- 所有存在领域歧义的字段和局部变量使用 `sandbox_*` 前缀；共享 `TenantId`、`OperationId` 与 `RuntimeCapability` 不创建重复 `Sandbox*` 类型别名。
- `sandbox_executable` 是 Provider-neutral Logical Identifier，不是 Path。Executor 只能使用绑定 `sandbox_runtime_binding_id` 的 Provider-owned Registry Snapshot 解析它，禁止调用方 Path、OS `PATH` Search 和 Working-directory Executable Lookup；Registry 在 Binding 生命周期内不可变，Resolved Binary Identity 保持 Provider-private，重放时发生变化或不可用必须返回原结果不可得且不得执行不同 Binary。`sandbox_arguments` 是有界 Argv；第一版不经过 Shell 解析，不接受命令行字符串，也不提供自动 Shell 回退。
- `sandbox_working_directory` 只接受 Workspace Attachment 内使用 `/` 分隔的可移植 ASCII Logical Relative Path，`.` 是唯一 Workspace Root 表示；安全目录名可包含多个 `.` 与内部空格，但绝对路径、任意反斜杠、`.`/`..` Segment、空 Segment、首空格、尾点/空格、控制符、Windows 非法字符、Device/Console Alias/ADS 与空 Segment 关闭失败。公共请求、结果、错误、Debug、Log、Event 与 Metric 不包含物理 Host Path 或 `SandboxProviderAllocationRef`。
- `sandbox_environment` 只接受数量、名称与 UTF-8 Value Byte Length 均有界的非 Secret 项。最终环境从空集合按绑定期不可变的 Execution Policy Snapshot 构造；请求不能扩展名称 Policy，Value 必须按名称校验，且不能提交或覆盖 `PATH`、`PATHEXT`、`COMSPEC`、`SYSTEMROOT`、`WINDIR`、`HOME`、`USERPROFILE`、`TMP`、`TEMP`。Provider 固定值只能在请求通过验证后注入。默认不继承 Ambient Credential、SSH Agent、Cloud Credential、Proxy Credential、Docker Socket 或其他 Secret-bearing 变量。Argv、Environment 和整个 Canonical Request 分别具有独立硬字节上限。
- `sandbox_command_limits` 明确包含非零 Timeout、stdout/stderr Byte Limit 与 Provider 可执行的资源边界。达到 Timeout、Cancellation 或 Output Hard Limit 后，Provider 必须有界终止整个 Sandbox Command Descendant Tree。
- 结果候选类型 `SandboxCommandExecutionResult` 返回结构化且与 Outcome 一致的 Exit Status、请求指纹、`sandbox_command_result_replayed`、stdout/stderr 有界字节、截断标志、开始/结束时间、`sandbox_cleanup_status`/Cleanup Duration 与安全的 Captured Byte/Process 用量；不得假定输出是 UTF-8，也不得包含 Host PID 或 Provider-private Identity。Captured Byte Count 必须等于 Base64 解码后的实际字节数；`output-limit` 与至少一个 Truncated Stream 双向一致，已开始的 Command Process Count 不得为零。
- Command 开始前验证当前 `sandbox_fencing_token`；执行期间检测到 Lease/Fencing 失效时停止接收新输入并触发有界 Cleanup。低于 Provider 已观察 Token 的请求确定性关闭失败。
- `sandbox_request_fingerprint` 由 Sandbox Service 从版本化、域隔离、长度前缀的 Canonical Request 派生，`trace_id` 不参与语义指纹；Executor 必须独立重算并拒绝不匹配值，调用方不能覆盖。重复 `sandbox_command_operation_id` 使用 Tenant+Provider-scoped 幂等语义：同一指纹的已完成请求重放原终态结果并标记 Replay，运行中请求返回 `operation-in-progress`，不同指纹返回 `idempotency-conflict`；不得重复启动第二个进程或 microVM Guest Command。
- 取消使用独立 `SandboxCommandCancellationRequest`，携带完整 Tenant/Workspace/Session/Sandbox/Binding/Provider Identity、当前 Fencing Token、目标 `sandbox_command_operation_id`、独立 `sandbox_cancellation_operation_id` 与派生指纹。取消本身幂等且不接受任意自由文本 Reason；它返回目标 Command 的终态 Execution Result，目标已完成或重复取消时返回既有终态结果，目标不存在时安全返回 `command-not-found`。
- 一旦 Command 已被接受并开始执行，Succeeded、Failed、Timed-out、Cancelled、Output-limit、Resource-exhausted 与 Fencing-lost 统一由 `SandboxCommandExecutionResult` 表达；Executor 使用持久化 first-terminal CAS 仲裁 Exit/Timeout/Cancel/Output/Resource/Fencing 竞争，后到信号不得重写 Primary Outcome。Terminal Result 只在有界 Cleanup 完成或明确失败后持久化；Cleanup Failure 不隐藏 Primary Outcome，必须在 Result 中显式返回并令 Binding Quarantine、Provider Unavailable，禁止新 Operation 盲重试。`SandboxCommandExecutionError` 只表达启动前拒绝或权威结果不可获得，包括 Invalid Request、Unsupported Capability、Policy Denied、Stale Fencing、Idempotency Conflict、Operation In Progress、Command Not Found、Provider Unavailable、Result Unavailable 与 Internal Failure；`result-unavailable`、`operation-in-progress` 和 `provider-unavailable` 只允许同一 Operation+Fingerprint 查询/重放，外部安全消息不泄露 Host、Repository、API Socket、Jailer、KVM 或 Secret 细节。
- Common Conformance 同一套运行在 Local 与 Firecracker Adapter，至少覆盖无 Shell、Argv 保真、Workspace Root/Working-directory Escape/Windows Device/Console Alias、Provider-owned Executable Resolution 且无 PATH/CWD Search、Runtime Binding Policy Snapshot 不可变、Protected Environment Override Denial、Environment Deny/UTF-8 Byte Bound、Canonical Fingerprint 重算、Timeout、Fenced Idempotent Cancellation、Output Bound、Result/Error Partition、Terminal Race 单赢家重放、Cleanup Failure/Binding Quarantine、Stale Fencing、Idempotency 与 Private Metadata Redaction。

## Candidate Non-functional Requirements

| 领域 | 要求 |
| --- | --- |
| Security | 所有执行必须先通过 Workspace Attachment、Policy、Capability、Lease/Fencing 与 Provider Readiness；任一保证不可证明时关闭失败。 |
| Privacy | Command Output 与 Terminal Stream 是独立敏感数据类别；结果有界、访问受控，并禁止写入普通 Operational Log。 |
| Performance | 请求和输出内存与配置的 Byte Limit 成正比；Executable、Argv、Environment、Canonical Request、stdout/stderr 和 Replay 均具有独立硬上限，不得建立无界 Pipe、Buffer、Replay 或全量目录预扫描。 |
| Reliability | Timeout、Cancel、Lease Lost 与 Provider Failure 都必须收敛到有界 Descendant Cleanup；同一 Operation 的不确定完成只能查询/重放既有权威结果，不能自动生成新 Operation 再执行。 |
| Observability | Metric 只使用稳定、低基数 Provider/Outcome/Capability Label；Trace/Log 关联 Sandbox Identity，但不记录 Raw Command、Argument、Path、Output 或 Environment Value。 |

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`, `SECURITY_SPEC.md`, `OBSERVABILITY_SPEC.md`, `PERFORMANCE_SPEC.md`, `RUST_CODE_SPEC.md`, `TEST_SPEC.md`.

Components: `crates/sdkwork-sandbox-provider-spi`, `crates/sdkwork-intelligence-sandbox-service`, `crates/sdkwork-sandbox-provider-local`, and the proposed Firecracker Sandbox Provider component.

Decisions: [ADR-20260729: Sandbox Command Execution And Terminal Boundary](../../architecture/decisions/ADR-20260729-sandbox-command-execution-and-terminal-boundary.md), [ADR-20260728: Local Provider Assurance And Host Boundaries](../../architecture/decisions/ADR-20260728-local-provider-assurance-and-host-boundaries.md), and [ADR-20260729: Firecracker Provider Isolation And Node Boundaries](../../architecture/decisions/ADR-20260729-firecracker-provider-isolation-and-node-boundaries.md).

## Verification Plan

```bash
cargo fmt --check
cargo test -p sdkwork-sandbox-provider-spi
cargo test -p sdkwork-intelligence-sandbox-service
cargo test -p sdkwork-sandbox-provider-local
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node --test tests/contract/sandbox-command-contract.contract.test.mjs
node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root . --strict
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-identity-naming.mjs --root .
```

Provider-specific real Host/KVM commands and security suites are additional mandatory evidence; a Fake Executor is not release evidence.

## Current Gate 0 Evidence

2026-07-30 已对齐 Execution/Cancel Schema、服务器 Trace Authority、Canonical Fingerprint、Tenant+Provider Idempotency Key、Workspace Root 与跨平台 Path/Console Alias 拒绝、Logical Executable Resolution、无 PATH/CWD Search、Runtime Binding Policy Snapshot 不可变、Protected Environment Override Denial、UTF-8 Byte Bound、Outcome/Exit/Truncation 一致性、Command Result Replay、Result-unavailable 同 Operation 重试、durable first-terminal CAS、Cleanup Status/Quarantine、Terminal Result/Error Partition 与 Common Conformance 场景。机器契约现显式设置 `implementationAuthorized: false`，供 Service Host 与 Provider Gate 统一关闭失败。Local crate 的 `#[cfg(test)]` Fake Host Boundary 只验证相同 Executable/Path/Argv/Environment 纯数据规则，包括 Allowlist 不得覆盖 Command String/Path/Credential/Protected Name 和 NUL/CR/LF 拒绝；它不导出 Port、不访问 Host。

完整静态证据见对应 Architecture/Security Review；本节不授权真实 Host Process 或 Provider 实现。
