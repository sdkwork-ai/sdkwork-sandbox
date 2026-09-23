# REVIEW-20260729: Local Sandbox Provider Architecture And Security

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured implementation-gate decision of 2026-09-24, including the close-out items above resolved per each packet's own recommended resolutions. Recorded by the executing agent on that instruction.

Requirement: [REQ-2026-0003](../../product/requirements/REQ-2026-0003-secure-local-provider.md)

Decision: [ADR-20260728](../../architecture/decisions/ADR-20260728-local-provider-assurance-and-host-boundaries.md)

Owner: SDKWork Runtime Platform

Date: 2026-07-29

Updated: 2026-07-30

Risk: critical - Host filesystem/process access, public Provider assurance, cross-platform cleanup, Workspace ownership, and credential exposure.

## Scope And Inputs

本 Review 请求人工评审 Local Provider 的 `HostUser` Assurance、授权 Workspace Attachment、Capability-rooted Filesystem、Runtime-Binding-scoped Execution Policy、Provider-owned Executable Resolution、跨平台 Process Supervision、Environment Deny、Capability Declaration 与 Cleanup Boundary。评审同时依赖 Lifecycle Provider SPI ADR、Agents Workspace Attachment ADR、Sandbox Command Execution Review、`SECURITY_SPEC.md`、`RUNTIME_DIRECTORY_SPEC.md`、`CONFIG_SPEC.md`、`RUST_CODE_SPEC.md` 与 `TEST_SPEC.md`。

Local 只面向 Single-user Standalone Development，不承载不可信多租户 SaaS Workload。Docker Provider 不在范围内，也不能作为 Local 隔离或测试回退。

## Candidate Machine Contract Evidence

- `specs/sandbox-local-provider-host-boundary.contract.json` is now the focused machine authority for LOCAL-01..LOCAL-08: opened Capability ownership and request identity matching, Runtime-Binding-scoped immutable Execution Policy, Provider-owned logical executable resolution without PATH/CWD lookup, protected environment denial, handle-relative filesystem rules, Windows suspended Job Object, Linux race-free delegated cgroup v2, explicit macOS Terminal denial, bounded Cleanup/Quarantine, sensitive Observability, conditional dependencies and real evidence.
- `specs/sandbox-provider-delivery-gates.contract.json` makes that contract an explicit Local Preflight dependency while retaining Kind `local`, Assurance `HostUser`, standalone-only scope, forbidden assurance claims and forbidden weak fallbacks.
- `node --test tests/contract/sandbox-local-provider-host-boundary.contract.test.mjs tests/contract/provider-delivery-gate.contract.test.mjs` passes 20/20: 13 focused Host Boundary checks plus 7 Provider Delivery checks. The tests also prove the Local component still has no public ports, Host IO or process spawn while review remains pending.
- This evidence makes the proposed decisions machine-reviewable but does not replace Windows/Linux real Host containment, macOS denial evidence, filesystem race, descendant cleanup, credential isolation or supply-chain evidence.

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| LOCAL-01 | Kind 固定为 `local`，Assurance 固定为 `HostUser`，配置不能提升为 Container/MicroVm。 | Descriptor 诚实表达 Host 边界。 | 必须选择不同 Provider/Assurance；禁止实现中伪装提升。 |
| LOCAL-02 | Adapter 只消费 Composition 打开的 Runtime/Workspace Capability Handle；不得从 `sandbox_workspace_id` 推导 Host Path，也不得按请求接受任意 Host Root。 | Workspace Authority 保持在 Agents/Attachment Boundary。 | 在替代授权模型获批前停止 Host Access。 |
| LOCAL-03 | 第一版 Terminal 只在 Windows Job Object 与 Linux delegated cgroup v2 的真实 Descendant Cleanup Conformance 通过后声明；macOS 在获得能阻止/回收 detached descendant 的审计机制前不声明 Terminal。 | 先交付可证明的平台能力，不把 Process Group 当作完整 Tree Guarantee。 | Reviewer 必须指定并批准等价 macOS Supervisor，或明确要求三平台全部就绪后再开始 Local Terminal。 |
| LOCAL-04 | Filesystem 采用 Handle-relative、逐级 No-follow/Reparse 检查；Linux 优先 `openat2` Resolve Policy，Unix Fallback 逐级 `openat`/`O_NOFOLLOW`，Windows 使用 Handle/Reparse/Final-path/File-identity 检查。无法证明的平台不声明 Filesystem。 | 避免 String Canonicalization 与 TOCTOU 被当作安全边界。 | 在替代 Capability Library/OS Primitive 及 Escape Test 获批前停止 Filesystem 实现。 |
| LOCAL-05 | Command 只使用已批准的 `SandboxCommandExecutor`。`sandboxExecutable` 是 Logical Identifier，只能由绑定 Runtime Binding 的 Provider-owned immutable Registry Snapshot 解析；禁止 Caller Path、OS PATH Search、CWD Lookup 和重放改用不同 Binary。 | Local 与 Firecracker 共享语义，Executable Resolution 不受 Host Ambient State 或 Caller Path 控制。 | 不得增加 Local-private Command DTO、PATH/CWD Resolver 或 Shell Wrapper。 |
| LOCAL-06 | Final Environment 从空集合按绑定期不可变 Policy 构造；请求不能扩展名称 Policy、必须按名称校验 Value，不能提交或覆盖 PATH/PATHEXT/COMSPEC/SYSTEMROOT/WINDIR/HOME/USERPROFILE/TMP/TEMP，也不继承 Ambient Credential、SSH Agent、Cloud Credential、Proxy Credential、Docker Socket 或 Secret-bearing Variable。 | 降低 Host Credential 与执行解析控制面暴露。 | 在等价 Secret/Environment Isolation 证明前不声明 Terminal。 |
| LOCAL-07 | Network、Browser、Port Forward 与 Shell 默认不声明；不支持的 Minimum Assurance/Capability 请求失败关闭。 | 不产生静默弱化或不受限 Egress。 | 需要独立 Ready Requirement、Policy 与平台证据。 |
| LOCAL-08 | Stop/Destroy 幂等回收 Process/Temporary Allocation/Attachment，但不删除 Agents-owned Persistent Workspace。 | 保持 Workspace 业务所有权与破坏性操作边界。 | 必须修改 Workspace Ownership ADR 并进行跨仓库人工评审。 |

## Pre-review Blocking Findings

1. Lifecycle Provider SPI 与 Agents Workspace Attachment ADR 仍为 `proposed`；Local 不能在其公共命名和 Ownership 未接受时进入真实 Host 实现。
2. `SandboxCommandExecutor` 尚待 [Command Architecture/Security Review](REVIEW-20260729-sandbox-command-execution-architecture-security.md)。
3. 下述候选供应链记录已形成，但 `cap-std` 未声明 MSRV，Linux race-free cgroup attach 仍无获批实现边界，且 RustSec 在线数据库刷新失败；因此精确 Runtime Dependency Set 仍未获批。
4. macOS 普通 Process Group/Session 不能阻止子进程自行创建新 Session；在等价机制获批前声明 Terminal 会违反 Descendant Cleanup Acceptance Criterion。
5. Windows、Linux 与未来 macOS Real Host Runner Owner 尚未记录。没有真实平台证据时不得把 Mock/Fake Test 作为 Capability Evidence。

推荐选择 LOCAL-03 的渐进式 Capability：先验证 Windows 与 Linux；macOS Descriptor 不声明 Terminal，而不是跳过失败测试后继续宣称跨平台支持。该选择会收窄 REQ-2026-0003 的首版目标，必须由 Product/Architecture Owner 明确接受并更新 Requirement。

## Candidate Dependency And Supply-chain Assessment

本节只记录 2026-07-29 的 Gate 0 候选评估，不向正式 Cargo Workspace 添加 Runtime Dependency，也不授权 Host IO 或 Process Spawn。

| Dependency | Candidate use | Version / feature boundary | License / MSRV | Gate 0 conclusion |
| --- | --- | --- | --- | --- |
| `process-wrap` | Windows suspended spawn + Job Object；Unix Process Group；Tokio Child supervision | `9.1.0`，`default-features = false`，仅候选 `tokio1`、`creation-flags`、`job-object`、`kill-on-drop`、`process-group` | `Apache-2.0 OR MIT`；Rust `1.87.0` | **Conditional candidate**。Windows 实现先设置 `CREATE_SUSPENDED`，绑定 Kill-on-close Job/Completion Port 后再 Resume，方向符合无抢跑要求；Unix Process Group 不能替代 Linux cgroup 或 macOS 等价 Supervisor。 |
| `cap-std` | 从已授权 Workspace Attachment Handle 派生 Capability-relative File Operations | `4.0.2`，`default-features = false` | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`；crate 未声明 MSRV | **Conditional candidate**。Provider 内禁止 `open_ambient_*`、`create_ambient_*` 和 Parent Ambient Authority；仍需 Windows Reparse/ADS/File Identity 与 Unix Symlink/Mount/Rename Race 实机证据。MSRV 未关闭前不得批准。 |
| `tokio` | 已有 Workspace Async Runtime，候选增加受限 `process` feature | `1.48.0`，不另建版本权威 | `MIT`；Rust `1.71` | **Existing dependency candidate**。只能在 Command Port 获批后由根 `Cargo.toml` 统一增加 Feature；不能单独提供 Descendant Containment。 |
| `cgroups-rs` | 曾评估 Linux cgroup v2/systemd 管理 | `0.5.1`，`default-features = false` | `MIT OR Apache-2.0`；crate 未声明 MSRV | **Not selected**。API/依赖覆盖通用 cgroup/systemd 权限面并含内部 `unsafe`；spawn 后再 attach 不能证明无逃逸窗口。只有独立安全评审证明 race-free pre-exec/launcher/broker 绑定、最小权限和清理语义后才可重新考虑。 |

候选锁文件精确包含上述版本，并在 Windows x86_64、`rustc 1.92.0` 下完成 `cargo check`。`cargo audit --no-fetch` 使用本机 2026-07-21 更新的 1166 条 RustSec Advisory 缓存扫描 146 个跨目标依赖且未报告 Vulnerability；在线刷新因 GitHub IO 失败，因此这只是候选快照，Ready/Release Gate 必须使用当日在线 Advisory DB 重新审计。Cargo Metadata License 扫描显示第三方包均声明 License；当前环境未安装 `cargo-deny`，正式采用前仍需 License Allowlist、Duplicate/Source/Ban 与 Feature Review。Linux Target 未安装，本轮没有 Linux Compile 或 Runtime Evidence。

## Platform Conformance Test Design

| Platform | Fail-closed preflight and spawn boundary | Required negative/fault evidence | Terminal claim |
| --- | --- | --- | --- |
| Windows | 验证 Job Object/Nested Job/Breakaway Policy；以 suspended state 创建进程，配置 Kill-on-close + Completion Port，绑定成功后才 Resume；任何配置、绑定、恢复或观察失败均终止并 Quarantine Binding。 | Parent/Child/Grandchild、Detached/Breakaway Attempt、Timeout、Cancel、Output Limit、Provider Crash、Handle Close、重复 Stop/Destroy；最终 Active Process 为零且无临时 Allocation/Handle Residue。 | 仅在真实 Windows Runner 全矩阵通过后声明。 |
| Linux | 要求 unified cgroup v2、受控 delegated subtree、`cgroup.kill`/`cgroup.events`/PID Controller；每个 `sandbox_runtime_binding_id` 使用独立 Scope。必须由获批 Launcher/Broker 在用户代码执行前完成 race-free cgroup membership；spawn 后写 `cgroup.procs` 和 Process Group 均不能单独作为保证。 | Double-fork、`setsid`、Process Group Escape、Fork Storm、Timeout、Cancel、Output Limit、Lease/Fencing Lost、Provider Crash、重复 Cleanup；`populated=0`、PID/temporary scope/residue 为零。 | Race-free attach 与真实 Linux Runner 未通过前不声明。 |
| macOS | Process Group/Session 只能作为进程控制辅助，不能证明阻止 detached descendant。 | Descriptor Capability Denial、请求 Terminal Fail-closed、无静默回退；未来机制必须补 Detached Child、Timeout/Cancel/Crash/Residue 全矩阵。 | 当前明确不声明。 |
| All filesystem-capable platforms | Workspace Attachment 只注入已打开 Capability Handle；Provider 不从 ID 推导 Path、不接收 Host Root，命令生命周期内不重新获取 Ambient Authority。 | Windows Reparse/Device/ADS/Hardlink/File Identity Swap；Unix Symlink/Mount/Rename Race/`..`；Open-before-check/Check-before-open TOCTOU；失败均不越过 Workspace Root。 | 只有对应平台实机矩阵通过才声明 Filesystem。 |

## Required Evidence Before Ready

- Gate 0 Fake Host Boundary 已有 5 个纯数据边界/负向测试，覆盖 Logical Relative Path、Windows 设备路径、Executable/Environment Allowlist、Typed Argv 与请求边界；Executable 语法先于 Allowlist，Command String/Path、Credential 类环境名、NUL/CR/LF 和全部受保护环境名不能被 Allowlist 绕过，七项输入上限与 Protected Name 分别直接和共享 Command Contract/Request Schema 交叉校验。这些测试不访问 Host、不启动进程，不替代以下真实平台证据。
- Local Host Boundary 的 13 项 Contract Test 已固定所有平台与 Supply-chain 前置条件，并显式拒绝 canonicalize/check-then-open、Process Group-only、spawn 后写 `cgroup.procs` 和 macOS Terminal fallback；静态 JSON 仍不构成任何真实 Host 安全证据。
- 接受 Lifecycle、Workspace Attachment、Command Execution 与本 ADR 的人工记录。
- 人工接受最终 Dependency Set；关闭 `cap-std` MSRV、Linux race-free cgroup attach、Fresh RustSec、License Allowlist、Feature/Source/Ban 与 Linux Compile Gate。
- 在真实 Runner 实施并通过上述 Windows Job Object、Linux cgroup v2 与 macOS Capability Denial Conformance；静态设计不能替代结果。
- 在真实 Runner 实施并通过上述 Windows Reparse/Device/ADS/File Identity 与 Unix Symlink/Mount/Rename Race Negative Test。
- Provider Descriptor/CLI Known Limitation Copy，明确 `HostUser` 不是多租户隔离。

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`。当前五项 Pre-review Finding 未关闭前，不得使用 `Approved with follow-up` 隐藏安全或 Requirement Scope 缺口。

| Reviewer role | Reviewer | Outcome | Date | Decision IDs / findings |
| --- | --- | --- | --- | --- |
| Product/architecture owner | Repository Owner (structured approval) | Approved | 2026-09-24 | LOCAL-01..LOCAL-08, macOS scope |
| Security owner | Repository Owner (structured approval) | Approved | 2026-09-24 | LOCAL-02..LOCAL-07 |
| Workspace/Kernel owner | Repository Owner (structured approval) | Approved | 2026-09-24 | LOCAL-02, LOCAL-05, LOCAL-08 |
| Platform operations owner | Repository Owner (structured approval) | Approved | 2026-09-24 | real Host runner and supervisor ownership |

## Implementation Gate

已批准（2026-09-24，仓库所有者以全部评审角色身份经结构化实现闸门决定批准）。REQ-2026-0003 进入 `ready`、ADR 进入 `accepted`；依赖与 Runner 的 Pre-review Finding 转为实现切片的持续证据义务（真实平台 Conformance、`cap-std` MSRV/Advisory/Compile、race-free cgroup attach 证据必须在对应实现切片落地时补齐），不因批准而视为已证明。

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
