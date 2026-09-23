# ADR-20260728: Local Provider Assurance And Host Boundaries

Status: accepted

Accepted: 2026-09-24 by the repository owner (structured implementation-gate decision, all listed reviewer roles).

Requirement: REQ-2026-0003

Owner: SDKWork Runtime Platform

Date: 2026-07-28

Updated: 2026-07-30

Specs: `ARCHITECTURE_DECISION_SPEC.md`, `SECURITY_SPEC.md`, `RUNTIME_DIRECTORY_SPEC.md`, `CONFIG_SPEC.md`, `OBSERVABILITY_SPEC.md`, `PERFORMANCE_SPEC.md`, `SUPPLY_CHAIN_SECURITY_SPEC.md`, `APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`, `RUST_CODE_SPEC.md`, `TEST_SPEC.md`

## Context

Standalone developers need low-latency execution without requiring a remote control plane. A Local Provider is useful for that workflow but cannot isolate untrusted workloads more strongly than the current OS account and the host capabilities deliberately granted to the process. Treating it as equivalent to Container、gVisor、microVM 或 Dedicated VM 会形成错误安全承诺。

Local execution also exposes platform-specific hazards：Windows Reparse Point/Device Path/Alternate Data Stream、Unix Symlink/Mount、Shell Parsing、Ambient Credential、Descendant Process、Unbounded Output 和不可验证 Network Egress。Provider Capability 必须反映实际可执行保证，而不是产品愿望。

## Decision

1. Local Provider 固定声明 Kind `local`、Assurance `HostUser` 和 Target `standalone`。它不得通过配置提升为更高 Assurance，也不得被 Scheduler 用于不可信多租户 Workload。
2. Local Adapter 只实现已验证的 Capability。首个版本在 Network/Egress、Browser 和 Port Isolation 可证明前不声明这些 Capability；Service 的 Minimum Assurance/Capability 选择继续故障关闭。
3. Runtime Data Root 由 L5 Composition 按 `RUNTIME_DIRECTORY_SPEC.md` 解析，并以 Capability Directory Handle 注入。Adapter 不在业务方法中读取 Environment，也不把 Source Checkout `.sdkwork/` 当 Runtime Root。
4. `AgentWorkspace` Creation/Persistence 由 `sdkwork-agents` 拥有。Local Provider 只消费经授权的 Sandbox Workspace Attachment Capability；Allocation Destroy、`SandboxSession` Destroy 和 Process Cleanup 不删除 Persistent Workspace。
5. 公共操作只携带 Logical Relative Path 和 Opaque ID。Physical Root/Host Path/Provider Reference 是 Private Metadata，不进入 Error、Event、Metric、CLI 默认输出或 SDK Type。
6. Filesystem 实现只消费已经打开的 Capability Handle，使用 handle-relative no-follow 与 file-identity verification。String canonicalization、check-then-open、open 后不验证都不能作为安全边界。Windows 拒绝 Drive/UNC/Device Prefix、Reserved Device Name、Alternate Data Stream、Reparse/Hardlink/File Identity Swap；Linux 优先 `openat2` 的 beneath/no-magiclink/no-symlink/no-xdev 约束，Fallback 必须提供逐级 `openat`/no-follow 与等价 Race Resistance；无法证明的文件类型或平台默认拒绝。
7. Process API 使用 Provider-neutral Logical Executable Identifier、Argv、Working Directory、Environment、Timeout、Output Limit 和 Cancellation。Provider-owned Executable Registry Snapshot 绑定 Runtime Binding 且生命周期内不可变；禁止 Caller Path、OS `PATH` Search 与 Working-directory Lookup，Resolved Binary Identity 保持私有且重放不能改为不同 Binary。默认不调用 Shell；需要 Shell 语义时必须声明独立 Capability 与 Policy Grant。
8. Process Supervision 必须对整个 Descendant Tree 生效并按平台声明。Windows 必须 suspended spawn，配置 Kill-on-close Job Object 与 Completion Port、成功绑定后才 Resume；Nested Job/Breakaway 不可约束时关闭失败。Linux 必须使用 unified delegated cgroup v2，每个 Runtime Binding 独立 Scope，在用户代码执行前 race-free 加入，并通过 `cgroup.kill`、`cgroup.events` 与 PID Controller 收敛；Process Group 或 spawn 后再写 `cgroup.procs` 不能单独作为保证。macOS Process Group/Session 不能证明 detached descendant containment，因此首版不声明 Terminal，请求关闭失败且不回退。
9. Output Buffer、Terminal Replay、Directory List/Search 和 File IO 都必须有界。达到 Hard Limit 返回结构化 Outcome，并触发 Cleanup；不能继续在后台产生无界数据。
10. Host Environment 从空集合按绑定期不可变的 Execution Policy Snapshot 构造；请求不能扩展名称 Policy、必须按名称校验 Value，并拒绝调用方提交或覆盖 `PATH`、`PATHEXT`、`COMSPEC`、`SYSTEMROOT`、`WINDIR`、`HOME`、`USERPROFILE`、`TMP`、`TEMP`。Provider 固定值仅在请求验证后注入。不继承 Ambient Credential、SSH Agent、Cloud/Proxy Credential、Docker Socket、Host Runtime Control 或 Secret-bearing Variables。Secret 注入不由本 ADR 当前授权；未来只有独立 Requirement/Review 批准的短期 Reference Resolver 才可注入并执行 Redaction/Revocation。
11. Local Provider 的限制必须进入 Component README、Provider Descriptor、CLI Inspection 和 Release Evidence；功能测试通过不等于隔离认证。
12. `specs/sandbox-local-provider-host-boundary.contract.json` 是本 ADR 的候选机器权威，并由 Provider Delivery Gate 作为 Local Preflight 依赖。它保持 `draft`、`implementationAuthorized: false` 和 no-runtime/no-host-io/no-process-spawn/no-secret-injection/no-dependency-change 标记，直到所有人工评审与真实平台证据完成。

## Alternatives

### 直接包装 `std::process::Command` 和绝对 Workspace Path

拒绝。它缺少 Capability Root、Process Tree Cleanup、Output Bound 和 Private Path Boundary，且容易继承宿主机全部 Environment。

### 把 Local Provider 标记为 Container Assurance

拒绝。当前 OS User 下的普通 Host Process 不形成 Container Namespace、Syscall 或 Kernel 隔离。

### 自动回退到不受限 Shell/Network

拒绝。Unsupported Capability 必须显式失败；便利性不能改变请求的安全语义。

### Destroy Session 时递归删除 Workspace

拒绝。Workspace 是独立持久身份，删除需要单独授权、Retention、Snapshot 和 Audit 决策。

## Consequences

收益：Local 模式的保证可理解、可机器发现，并与 Lifecycle Service 的 Fail-closed Selection 一致；后续 Docker/microVM Provider 不需要兼容 Local 的隐式 Host 权限。

成本：跨平台 Process Tree、Filesystem Link/Reparse 和 Resource Limit 需要平台专项实现与测试；在这些保证完成前，Local Provider 只能逐项开放 Capability，不能一次性声称完整 Terminal/Filesystem/Network 产品能力。

## Verification

- Requirement Readiness Checklist 必须先关闭 Workspace Attachment 和 Process Supervisor 选型。
- Provider Conformance 覆盖 Lifecycle Idempotency、Unsupported Capability、Path Escape、Provider-owned Executable Resolution 且无 PATH/CWD Search、Binding Policy Snapshot 不可变、Protected Environment Override Denial、Process Cleanup、Timeout、Cancellation、Output Bound、Environment Deny 和 Private Path Redaction。
- Windows 真实 Runner 证明 suspended Job Object、Completion Port、Nested Job/Breakaway、Crash/Timeout/Cancel 与零残留；Linux 真实 Runner 证明 race-free delegated cgroup v2、double-fork/setsid/fork-storm、Crash/Timeout/Cancel 与 `populated=0`；macOS 证明 Terminal Capability Denial 与无回退。平台不支持的 Enforcement 必须反映在 Descriptor，不能跳过失败测试后继续声明 Capability。
- Filesystem 平台套件覆盖 Windows Reparse/Device/ADS/Hardlink/File Identity Swap 与 Unix Symlink/Mount/Rename Race，不把 Fake 或 String Path Test 作为 Host containment 证据。
- Runtime Dependency Selection 前完成 Fresh Online RustSec、License/Source/Feature/MSRV、Windows/Linux Build、任何 macOS 声明能力的 macOS Build 与人工 Dependency/Security Review。
- 人工安全评审必须确认 Threat Boundary、Known Limitation 和 Release Copy，之后才能将 ADR 改为 `accepted` 并开始真实 Host Access Implementation。

## Supersedes / Superseded By

Supersedes: none.

Superseded by: none.
