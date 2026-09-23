# ADR-20260729: Sandbox Command Execution And Terminal Boundary

Status: accepted

Accepted: 2026-09-24 by the repository owner (structured implementation-gate decision, all listed reviewer roles).

Requirement: REQ-2026-0007

Owner: SDKWork Runtime Platform

Date: 2026-07-29

Updated: 2026-07-30

Specs: `ARCHITECTURE_DECISION_SPEC.md`, `APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`, `MODULE_SPEC.md`, `COMPONENT_SPEC.md`, `SECURITY_SPEC.md`, `OBSERVABILITY_SPEC.md`, `PERFORMANCE_SPEC.md`, `RUST_CODE_SPEC.md`, `TEST_SPEC.md`

## Context

当前 `SandboxProvider` 只定义 Allocate、Start、Stop 与 Destroy。它足以证明 `SandboxSession` Lifecycle，但不能执行 Agent 工具、Build 或 Terminal Command。把 `exec` 分别添加到 Local 和 Firecracker 私有接口会产生两套输入、输出、错误、Fencing、Limit 与 Redaction 语义，并迫使 Kernel 或 Service 按 Sandbox Provider 类型分支。

Command Execution 也不应无限扩张生命周期端口。Filesystem、Terminal、Browser、Port、Snapshot 与未来 GPU 的演进速度和支持矩阵不同；把所有可选能力塞进一个 Trait 会迫使 Provider 实现无意义的方法，并削弱 Descriptor 的可验证性。

## Decision

1. 保持 `SandboxProvider` 为 Provider Lifecycle Port；新增候选独立端口 `SandboxCommandExecutor`，通过同一 `SandboxProviderId` 在 Composition/Registry 绑定。
2. `RuntimeCapability::Terminal` 代表第一版有界非交互 Command Execution。只有 Lifecycle Provider、Command Executor 和对应 Common Conformance 全部存在时，Descriptor 才能声明该 Capability。
3. 公共候选类型使用 `SandboxCommandExecutionRequest`、`SandboxCommandCancellationRequest`、`SandboxCommandLimits`、`SandboxCommandExecutionResult`、`SandboxCommandExitStatus` 与类型化 `SandboxCommandExecutionError`。存在歧义的字段和变量全部使用 `sandbox_*` 前缀。
4. Execution/Cancel Request 保留服务器拥有的 `traceId`、`TenantId`、`SandboxWorkspaceId`、`SandboxSessionId`、`SandboxId`、`SandboxRuntimeBindingId`、`SandboxFencingToken` 与共享 `OperationId`。Command/Cancel Operation 字段分别使用 `sandbox_command_operation_id`/`sandbox_cancellation_operation_id`，不创建重复的 `SandboxOperationId`。
5. 第一版只接受 Provider-neutral Logical Executable Identifier + Bounded Argv + Portable ASCII Logical Relative Working Directory。Identifier 不是 Path，只能由绑定 Runtime Binding 的 Provider-owned Registry Snapshot 解析；禁止 Caller Path、OS `PATH` Search 与 Working-directory Lookup，Snapshot 在 Binding 生命周期内不可变，Resolved Binary Identity 保持 Provider-private，重放不得解析为不同 Binary。`.` 是唯一 Workspace Root，子路径只使用 `/`；允许多个点与内部空格的常见安全目录名，绝对路径、任意反斜杠、Traversal、空 Segment、首空格、尾点/空格、控制符、Windows 非法字符、Device/Console Alias/ADS、Shell String、Implicit Shell、PTY、Network、Browser、Port、Secret Value 和任意 Host Root 不进入本契约。
6. stdout/stderr 以有界 Byte Buffer 表达，并分别报告 Truncated；Result 不把非 UTF-8 强制转换为有损 String，Captured Byte Count 与解码结果一致。Result 使用 `sandbox_command_result_replayed` 明确表达 Command Result 重放，并返回 Cleanup Status/Duration；Succeeded 必须是 Exit Code 0，Failed 必须是非零 Exit 或 Signal，Output-limit 与 Truncated 双向一致，已开始 Command 的 Process Count 至少为 1。Terminal Streaming/Replay 在后续独立 Requirement 中定义，不能用无界 Channel 代替。
7. Command Executor 在启动副作用前验证 Workspace Attachment、Provider Readiness、Policy、当前 Lease/Fencing 与幂等 Request Fingerprint。Fingerprint 由 Service 使用域隔离、版本化长度前缀 Encoding 派生，Executor 独立重算，`traceId` 不参与语义 Hash；Stale Token、Fingerprint Mismatch、Operation Conflict 或 Capability 缺失均在启动前关闭失败。
8. Cancel 使用完整 Ownership/Fencing Context、目标 Command Operation、独立 Cancellation Operation 与派生指纹；取消本身幂等并返回目标 Command 的终态 Result。Timeout、Cancellation、Output Hard Limit、Lease Lost 和 Provider Shutdown 使用同一个有界 Descendant Cleanup Contract。Provider-specific Cleanup Mechanism 由 Adapter 实现并通过 Common + Platform-specific Conformance 证明。
9. Command、Argument、Environment Value、Logical/Physical Path、Output、Host PID、API Socket、microVM Identity 与 `SandboxProviderAllocationRef` 不进入 Metric Label 或普通 Operational Log。Trace 只记录安全 Operation/Provider/Outcome/Duration 属性。
10. 已接受并启动的 Command 始终收敛为终态 Result；Executor 使用持久化 first-terminal Compare-and-swap 仲裁 Exit、Timeout、Cancel、Output、Resource 与 Fencing 竞争，首个持久化 Primary Terminal Fact 胜出，后到信号只能取得同一结果。Terminal Result 在有界 Cleanup 完成或明确失败后持久化；Cleanup Failure 不重写 Primary Outcome，必须显式返回并触发 Binding Quarantine 与 Provider Unavailable。Timeout、Cancelled、Output Limit、Resource Exhausted 与 Fencing Lost 不作为可盲重试 Error。Error 只表达启动前拒绝或权威 Result 不可获得；`result-unavailable`、`operation-in-progress` 与 `provider-unavailable` 只能用同一 Operation+Fingerprint 查询/重放，禁止自动创建新 Operation。
11. Service/Registry 通过端口存在性与 Descriptor Capability 交叉校验：声明 Terminal 但没有 Executor、或注册 Executor 但 Provider Identity 不匹配时，Provider 保持 Unavailable。
12. Executable、Argv、Working Directory、Environment Name/Value、Canonical Request、stdout/stderr、Cleanup 与 Durable HA Replay 都有独立硬上限；字符限制不能替代 UTF-8 Byte Limit。Final Environment 从空集合按同一 Binding Policy Snapshot 构造；请求不能扩展名称 Policy，Value 必须按名称校验，受保护 Host Control/Resolution 名称不能由调用方提交或覆盖，Provider 固定值仅在请求验证后注入。
13. 本 ADR 不批准 Interactive PTY、Shell Capability、Secret Injection、Network、Browser、Port Forward、Docker Provider、HTTP/RPC、SDK 或 Deployment Profile。

## Alternatives

### 直接扩展 `SandboxProvider` 加入所有能力方法

拒绝。它违反接口隔离，并让每个 Provider 为不支持能力维护占位实现；新增能力会持续扩大公共 Trait 的破坏面。

### Local 与 Firecracker 各自公开 `exec`

拒绝。它会让 Kernel、Service、Error Mapping 和 Conformance 感知具体 Sandbox Provider，破坏 `sdkwork-agents -> sdkwork-kernel -> sdkwork-sandbox` 的稳定依赖语义。

### 接受单个 Shell Command String

拒绝。Shell Parsing、Quoting、Expansion 与 Injection 语义跨平台不一致。第一版使用 Executable + Argv；Shell 需要独立 Capability 与 Policy Grant。

### 把 stdout/stderr 只作为 UTF-8 String

拒绝。Build Tool 和子进程可能输出任意字节；强制解码会丢失数据或制造不一致的截断边界。

## Consequences

收益：Local 与 Firecracker 共享同一 Command、Limit、Fencing、Error 和 Conformance；生命周期端口保持高内聚；未来 PTY、Filesystem、Network 等能力可按独立端口扩展，不修改 Kernel Provider 分支。

成本：Composition 必须验证成对端口和 Provider Identity；Command 幂等结果需要有界保留；每个 Provider 仍需实现并证明平台特定 Descendant Cleanup。

## Verification

- Contract Test 验证 Execution/Cancel 字段、Canonical Fingerprint、Idempotency、跨平台 Path/Console Alias、UTF-8 Byte Bound、Byte Output、Outcome/Exit/Truncation、Terminal Race、Cleanup/Quarantine、Terminal Result/Error Partition、Error Retry Taxonomy、Debug Redaction 与 `sandbox_*` Naming。
- Registry/Service Test 验证 Terminal Descriptor 与 Executor 端口的一致性并关闭失败。
- Common Conformance 运行在 Local 和 Firecracker，覆盖 Argv、No-shell、Provider-owned Executable Resolution 且无 PATH/CWD Search、Runtime Binding Policy Snapshot 不可变、Protected Environment Override Denial、Timeout、Cancellation、Output Limit、Working-directory Escape、Environment Deny、Stale Fencing、Idempotency 与 Cleanup。
- 实际 Local Host 与 Linux KVM Firecracker Smoke 必须执行同一命令场景；Fake Executor 只用于 Service Unit Test。
- 公共命名、Error Contract、Provider Security Posture 与跨仓库 Kernel Integration 在实现前完成人工评审。

## Supersedes / Superseded By

Supersedes: none.

Superseded by: none.
