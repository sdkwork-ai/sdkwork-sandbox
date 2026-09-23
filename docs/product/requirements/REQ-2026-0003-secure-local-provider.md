---
id: REQ-2026-0003
title: Deliver a constrained local sandbox provider
owner: SDKWork Runtime Platform
status: ready
source: platform
problem: Local development needs executable Sandbox capabilities, but an unrestricted host process adapter would bypass workspace containment, cleanup, resource, credential, and network policy.
goals:
  - Provide an honest HostUser-assurance provider for single-user standalone development.
  - Enforce capability-rooted workspace access on each claimed platform and bounded process execution only where platform supervision has real-runner evidence.
  - Make unsupported isolation and network guarantees machine-discoverable and fail closed.
non_goals:
  - Claim hardened multi-tenant, container, user-space-kernel, microVM, or dedicated-VM isolation.
  - Provide Docker, Firecracker, gVisor, Kubernetes, Remote VM, Browser, or unrestricted network execution.
  - Delete persistent workspaces when a Session or Sandbox allocation is destroyed.
users:
  - SDKWork developers using standalone local mode
  - Sandbox provider maintainers
  - SDKWork Kernel integrators
affected_surfaces:
  - rust-components
  - composition
  - tooling
---

# REQ-2026-0003: 交付受约束的 Local Sandbox Provider

## Readiness Blockers

本需求尚未 Ready，必须先完成以下评审和契约：

- 人工接受 `ADR-20260728-sandbox-lifecycle-provider-spi-and-memory-store` 的 `0.1` 公共命名。
- 人工安全评审并接受 [Local Provider Assurance ADR](../../architecture/decisions/ADR-20260728-local-provider-assurance-and-host-boundaries.md)。
- 接受 Agents Workspace 与 Sandbox Attachment ADR；Local Provider 不得根据 `sandbox_workspace_id` 猜测 Host Path。
- 人工接受首版平台切片：Windows 只有在 suspended Job Object 全矩阵通过后声明 Terminal；Linux 只有在用户代码执行前完成 delegated cgroup v2 membership 且全矩阵通过后声明 Terminal；macOS 在 detached descendant containment 获批前明确拒绝 Terminal。
- 人工接受精确 Runtime Dependency Set，并关闭 `cap-std` MSRV、Fresh Online RustSec、License/Source/Feature Review、Windows/Linux Build、任何 macOS 声明能力的 macOS Build 与真实平台安全测试。

## Candidate Acceptance Criteria

- Provider Descriptor 固定报告 Kind `local` 和 Assurance `HostUser`，拒绝 `Container` 或更高 Minimum Assurance。
- L5 Composition 只向 Adapter 注入已经打开的 Runtime/Workspace Capability Handle；Adapter 不读取全局环境变量，也不接收任意 Host Root String 作为每次请求输入。
- Workspace Attachment 来自经授权的 Sandbox Attachment Port/Capability，关联 `sandbox_workspace_id` 并包含 Provider-private Reference；`SandboxSession`/Allocation Destroy 不删除 Agents-owned Workspace。
- Filesystem API 只接受 Logical Relative Path，并使用 handle-relative no-follow 与 file-identity verification；拒绝 Absolute、Parent Traversal、Symlink/Reparse Escape、Mount Escape、Device Path、Alternate Data Stream、Hardlink/File Identity Swap 和不支持的 Normalization。String canonicalization、check-then-open 或 open 后不验证不能作为安全边界。
- Terminal/Process 默认使用 Provider-neutral Logical Executable Identifier + Argv，不经 Shell 解析。Logical Identifier 只能由绑定 `sandbox_runtime_binding_id` 的 Provider-owned Executable Registry Snapshot 解析；调用方不能提交 Path，禁止 OS `PATH` Search 和 Working Directory Lookup，Binding 生命周期内 Registry 不可变，重放不得解析为不同 Binary Identity。Shell 必须是显式 Capability 和 Policy Grant。
- Windows 进程必须 suspended spawn，在 Kill-on-close Job Object 与 Completion Port 绑定成功后才 Resume；Nested Job/Breakaway 无法安全约束时关闭失败并 Quarantine。
- Linux 进程必须在用户代码执行前进入每 Runtime Binding 独立的 delegated unified cgroup v2 Scope，并使用 `cgroup.kill`、`cgroup.events` 与 PID Controller；Process Group 或 spawn 后再写 `cgroup.procs` 不能单独满足保证。
- macOS Process Group/Session 不能阻止 detached descendant；首版 Descriptor 不声明 Terminal，Terminal 请求关闭失败且不得回退平台或 Provider。
- Process 执行限制 Working Directory、Environment、Timeout、Output Bytes、PID/Descendant、CPU/Memory（平台支持时）和 Cancellation；Stop/Destroy 幂等且有界终止全部 Descendant，清理不确定时 Quarantine Binding。
- 未实现可验证 Egress Enforcement 前不声明 Network、Browser 或 Port Forward Capability；DNS 可解析不视为 Network Grant。
- Environment 从空集合按绑定期不可变的 Execution Policy Snapshot 构造；请求不能扩展名称 Policy，Value 必须按名称校验，且不能提交或覆盖 `PATH`、`PATHEXT`、`COMSPEC`、`SYSTEMROOT`、`WINDIR`、`HOME`、`USERPROFILE`、`TMP`、`TEMP`。Provider 固定值只能在请求验证后注入。不继承 SSH Agent、Cloud/Proxy Credential、Docker Socket、Host Runtime Control 或 Secret-bearing Environment。Secret 注入不在当前实现授权内；后续只允许经独立批准的短期 Reference，Value 不进入 Command Debug、Environment Dump、Log、Event、Metric、Workspace Metadata 或 Durable Terminal Replay。
- Allocation/Start/Stop/Destroy、Filesystem 和 Terminal 通过共同 Provider Conformance；Windows 与 Unix 分别覆盖适用的 Link/Reparse 和 Process-tree Test。
- CLI 的破坏性操作要求显式目标和确认策略，输出不显示 Private Host Path 或 Secret。

## Candidate Non-functional Requirements

| 领域 | 要求 |
| --- | --- |
| Security | Local 只承诺当前 OS User 边界；能力未实现或策略无法证明时拒绝请求。 |
| Privacy | Host Path 和 Terminal 内容保持私有、访问受控、有界并使用独立 Retention。 |
| Performance | Output/Filesystem/List 均有上限；不能用全量目录或无界 Buffer 实现交互操作。 |
| Reliability | Stop/Destroy 幂等；Process Tree Cleanup 和残留 Allocation 扫描有故障注入证据。 |

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `SECURITY_SPEC.md`, `RUNTIME_DIRECTORY_SPEC.md`, `CONFIG_SPEC.md`, `OBSERVABILITY_SPEC.md`, `PERFORMANCE_SPEC.md`, `SUPPLY_CHAIN_SECURITY_SPEC.md`, `RUST_CODE_SPEC.md`, `TEST_SPEC.md`.

Components: `crates/sdkwork-sandbox-provider-local`, `crates/sdkwork-sandbox-service-host`, `crates/sdkwork-sandbox-cli`, plus a future reviewed Sandbox Workspace Attachment adapter; `sdkwork-agents` remains the `AgentWorkspace` owner.

Decision: [ADR-20260728: Local Provider Assurance And Host Boundaries](../../architecture/decisions/ADR-20260728-local-provider-assurance-and-host-boundaries.md).

## Gate 0 Progress

2026-07-29 已在 `sdkwork-sandbox-provider-local` 增加仅 `#[cfg(test)]` 编译的 Fake Host Boundary，5 个测试覆盖 Logical Relative Path 逃逸/Windows 设备路径、Typed Argv 无 Shell 解析、Executable/Environment Allowlist、参数与环境边界。Executable 语法校验先于 Allowlist，显式 Allowlist 不能放行 Command String 或 Path；Credential 类名称、NUL/CR/LF 和 Request Schema 中全部受保护环境名同样不能被 Allowlist 绕过。Executable、Argument、Working Directory 与 Environment 的七项上限由 Rust Test 直接读取 `apis/commands/sandbox-command-contract.json` 交叉校验，受保护名称直接读取 Execution Request Schema，防止 Provider Harness 与共享契约漂移。

2026-07-30 新增 `specs/sandbox-local-provider-host-boundary.contract.json` 与 13 项 Contract Test，并由 `sandbox-provider-delivery-gates.contract.json` 将其设为 Local Preflight 依赖。该机器权威固定 opened Capability Handle、请求/Capability/Runtime Binding Identity 一致性、绑定期不可变 Execution Policy Snapshot、Provider-owned Executable Registry、无 PATH/CWD Search、受保护环境拒绝、Windows suspended Job Object、Linux race-free delegated cgroup v2、macOS Terminal denial、Filesystem Race/Identity、Cleanup/Quarantine、敏感 Observability 与 Supply-chain Gate；同时明确 Process Group、spawn 后写 `cgroup.procs`、String canonicalization 和 check-then-open 均不是安全保证。全部契约仍为 `draft` 且 `implementationAuthorized: false`；它们不访问 Host Filesystem、不启动进程、不导出 Provider/Command Port、不注入 Secret，也不改变 Runtime Dependency。本需求继续等待人工架构/安全/平台/Workspace 评审和真实 Runner 证据。

同日 Gate 0 供应链评估将 `process-wrap 9.1.0` 作为 Windows suspended Job Object/POSIX Process Group 条件候选、`cap-std 4.0.2` 作为 Capability Directory 条件候选，并拒绝直接采用 `cgroups-rs 0.5.1` 作为现成安全保证。候选依赖图在 Windows/Rust 1.92 完成锁定、编译、License Metadata 和离线 RustSec 扫描，但 `cap-std` MSRV、Fresh Online Advisory、Linux Compile、race-free cgroup attach、真实 Runner 与人工 Dependency/Security Review 均未关闭；具体证据与平台测试矩阵见 [Local Provider Architecture And Security Review](../../engineering/reviews/REVIEW-20260729-local-provider-architecture-security.md)。

## Verification Plan

Implementation verification will include focused provider tests, cross-platform path/process tests, Provider Conformance, Cargo/Clippy, strict component binding, security review evidence, and a manual limitation review. Exact commands will be frozen when the Requirement becomes `ready`.

## Implementation Authorization

`ready` since 2026-09-24: the repository owner approved the corresponding architecture/security packet for every listed reviewer role via the structured implementation-gate decision of that date, promoting this requirement to `ready` and its decision record to `accepted`. The packet's evidence obligations (real-platform conformance, dependency review items, and runner ownership where named) remain standing evidence requirements for the implementation slices; approval disposes the review, not the evidence.
