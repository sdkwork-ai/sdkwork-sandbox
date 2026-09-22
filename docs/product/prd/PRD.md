# SDKWork Sandbox PRD

Status: active

Owner: SDKWork Runtime Platform

Application: sandbox

Updated: 2026-09-22

Specs: `REQUIREMENTS_SPEC.md`, `DOCUMENTATION_SPEC.md`, `SECURITY_SPEC.md`, `PRIVACY_SPEC.md`, `API_SPEC.md`, `SDK_SPEC.md`, `DEPLOYMENT_SPEC.md`, `PERFORMANCE_SPEC.md`

## 文档地图 (Document Map)

- [能力、生命周期与产品规则](PRD-capabilities.md)
- [运行模式、Runtime Pool 与状态物化需求](PRD-runtime-execution-model.md)
- [能力面与访问路径需求](PRD-sandbox-surfaces.md)
- [交付路线图、阶段门禁与验收标准](PRD-roadmap.md)
- [REQ-2026-0001: 初始化 Sandbox 工程基础](../requirements/REQ-2026-0001-sandbox-foundation.md)
- [REQ-2026-0002: 交付 Provider-neutral Sandbox 生命周期核心](../requirements/REQ-2026-0002-sandbox-lifecycle-core.md)
- [REQ-2026-0003: 交付受约束的 Local Sandbox Provider](../requirements/REQ-2026-0003-secure-local-provider.md)
- [REQ-2026-0004: Agents Workspace 与 Sandbox Attachment](../requirements/REQ-2026-0004-agents-workspace-attachment.md)
- [REQ-2026-0005: 持久化 Sandbox Session Repository 与崩溃恢复](../requirements/REQ-2026-0005-durable-sandbox-session-repository-and-reconciliation.md)
- [REQ-2026-0006: Sandbox Provider Allocation 密钥轮换与有界重加密](../requirements/REQ-2026-0006-sandbox-provider-allocation-key-rotation.md)
- [REQ-2026-0007: Provider-neutral Sandbox Command Execution Contract](../requirements/REQ-2026-0007-sandbox-command-execution-contract.md)
- [REQ-2026-0008: Firecracker Sandbox Provider](../requirements/REQ-2026-0008-firecracker-sandbox-provider.md)
- [REQ-2026-0009: Sandbox Service Host Composition And Readiness](../requirements/REQ-2026-0009-sandbox-service-host-composition-and-readiness.md)
- [REQ-2026-0010: Sandbox Observability, Event, Audit And Outbox Contract](../requirements/REQ-2026-0010-sandbox-observability-event-audit-outbox.md)
- [REQ-2026-0011: Sandbox Host Isolation Broker Boundary](../requirements/REQ-2026-0011-sandbox-host-isolation-broker.md)
- [REQ-2026-0012: Sandbox Firecracker Artifact Compatibility And Supply Chain](../requirements/REQ-2026-0012-sandbox-firecracker-artifact-compatibility-and-supply-chain.md)
- [REQ-2026-0013: Sandbox Workspace Block Device Attachment And Sanitization](../requirements/REQ-2026-0013-sandbox-workspace-block-device-attachment-and-sanitization.md)
- [REQ-2026-0014: Sandbox Firecracker Network Isolation And Egress Policy](../requirements/REQ-2026-0014-sandbox-firecracker-network-isolation.md)
- [REQ-2026-0015: Sandbox Firecracker Resource Isolation And Usage Facts](../requirements/REQ-2026-0015-sandbox-firecracker-resource-isolation-and-usage.md)
- [REQ-2026-0016: Sandbox Multi-tenant Admission, Scheduling And Capacity](../requirements/REQ-2026-0016-sandbox-multi-tenant-admission-scheduling-and-capacity.md)
- [REQ-2026-0017: Sandbox Node Trust, Enrollment, Attestation And Verified Inventory](../requirements/REQ-2026-0017-sandbox-node-trust-enrollment-attestation-and-inventory.md)
- [REQ-2026-0018: Sandbox PostgreSQL Quota And Capacity Reservation Persistence](../requirements/REQ-2026-0018-sandbox-postgresql-quota-and-capacity-reservation-persistence.md)
- [REQ-2026-0019: Sandbox Runtime Pool And Fast Allocation](../requirements/REQ-2026-0019-sandbox-runtime-pool-and-fast-allocation.md)
- [REQ-2026-0020: Sandbox Lifecycle Hot State And Idempotency Retention](../requirements/REQ-2026-0020-sandbox-lifecycle-hot-state-and-idempotency-retention.md)
- [REQ-2026-0021: Sandbox Workspace Runtime Transaction And Checkpoint](../requirements/REQ-2026-0021-sandbox-workspace-runtime-transaction-and-checkpoint.md)
- [REQ-2026-0022: Sandbox Standalone Data Residency And Recovery](../requirements/REQ-2026-0022-sandbox-standalone-data-residency-and-recovery.md)
- [REQ-2026-0023: Sandbox Internal Control Plane](../requirements/REQ-2026-0023-sandbox-internal-control-plane.md)
- [REQ-2026-0024: Sandbox Interactive Terminal Session](../requirements/REQ-2026-0024-sandbox-interactive-terminal-session.md)
- [REQ-2026-0025: Sandbox Runtime Secret Projection](../requirements/REQ-2026-0025-sandbox-runtime-secret-projection.md)
- [REQ-2026-0026: Sandbox Cloud Data Residency And Recovery](../requirements/REQ-2026-0026-sandbox-cloud-data-residency-and-recovery.md)
- [REQ-2026-0027: Sandbox Cross-Repository Version Compatibility And Release Set](../requirements/REQ-2026-0027-sandbox-cross-repository-version-compatibility.md)
- [技术架构](../../architecture/tech/TECH_ARCHITECTURE.md)

## 1. 背景与问题 (Background And Problem)

Codex、Claude Code、OpenCode、Gemini CLI、Qwen Code 等 AI Coding Agent 在执行任务时，需要运行命令、读写工作区、构建代码、启动浏览器、开放端口，并保存可恢复的会话状态。`sdkwork-kernel` 负责 Agent Provider、Prompt、模型交互、工具编排和 Agent 行为，但不应同时内置 Windows、macOS、Linux、Docker、Firecracker、gVisor、Kubernetes 与 Remote VM 的所有执行机制。

本地开发、企业私有化、SaaS、Web IDE、Remote Coding、Browser Coding、CI/CD 和 Serverless Agent 对隔离、调度、存储和可观测性的要求不同。如果缺少独立运行时边界，Kernel 消费方只能直接获得不安全的宿主机权限，或者在 Kernel 内累积大量 Provider 分支，最终无法维持一致的生命周期、安全、配额和恢复语义。

SDKWork Sandbox 是面向 SDKWork Agent 的 Provider 无关执行环境。它将“在哪里执行、如何隔离”收敛为基础设施选择，同时保持 Kernel 面向的 Runtime 契约稳定。

### 产品定位与边界

`SDKWork Sandbox` 不是 Docker 的 Rust 版本，也不是对成熟开源 Agent Runtime 的简单移植，而是面向 SDKWork 海量租户场景自主设计的高性能 Agent 运行环境。它以公开成熟的 microVM Agent Runtime 能力（microVM 编排、Snapshot、COW RootFS、按需内存加载、Template、边缘路由、VM 内 Agent 进程）作为**架构参考基线**，目标是**能力兼容**而不是 API 拷贝：对齐其已被验证的运行时能力与关键性能思想，同时针对本平台的多租户规模重新设计调度、资源池与轻量运行模式。

因此产品的核心问题不是“如何创建更多 Sandbox”，而是：

> **如何让 Sandbox 只在需要的时候消耗资源。**

产品最终要交付的体验是：Agent 感觉自己拥有一台完整的独立计算机，而基础设施尽可能共享所有可以共享的计算、内存、文件系统、网络与 Runtime 资源。

### 规模目标

产品的容量设计目标是分层共享而不是逐实例独占：

```text
100 万级注册用户
        ↓
10 万级活跃 Sandbox
        ↓
共享 Runtime
        ↓
动态实例池
        ↓
按需分配 CPU / Memory / Disk / Network
```

这些数字是**工程目标，不是未经验证的承诺**。任何对外容量声明都必须由参考硬件上的真实 Benchmark 修正；未完成测量前不得把它们写成已实现 SLO。

### 解耦原则

产品必须在长期演进中坚持下列边界不等式，它们约束全部后续能力设计：

```text
Control Plane ≠ Data Plane
Agent ≠ Sandbox
Sandbox ≠ Runtime
Runtime ≠ Node
Workspace ≠ Sandbox
Template ≠ Snapshot
Snapshot ≠ Storage
Network ≠ API
Storage ≠ Runtime
```

每个模块必须高内聚、低耦合、可替换、可测试、可观测、可水平扩展。控制面与数据面彻底分离，控制面不执行 Agent 任务，数据面不拥有跨租户业务权威；Sandbox 流量不经过控制面。详细运行边界见 [PRD-runtime-execution-model.md](PRD-runtime-execution-model.md)。

## 2. 目标用户 (Target Users)

| 用户 | 核心诉求 |
| --- | --- |
| 本地开发者 | 在 Windows、macOS 或 Linux 上绑定本地 Workspace，无需部署服务器即可运行 Agent。 |
| 企业运维人员 | 在私有基础设施中执行 Agent，并控制网络、文件系统、Secret 与资源策略。 |
| SaaS 平台运维人员 | 在多租户集群中调度 Session，使用 Pool、Quota、Metering、Snapshot 和故障恢复。 |
| SDKWork Kernel 集成人员 | 使用同一 Runtime 契约，不按 Local、Docker、Firecracker、gVisor、Kubernetes 或 Remote VM 编写行为分支。 |
| Sandbox Provider 开发者 | 实现并验证新的 Provider，而无需修改 Kernel 或产品生命周期策略。 |
| IDE 与自动化开发者 | 通过稳定 SDK 获取终端、日志、事件和状态流，而不是适配各 Provider 私有协议。 |
| AI Agent 应用开发者 | 用少量代码获得一个可执行代码、可读写文件、可访问网络、可持久化的独立运行环境，并按需暂停与恢复。 |
| Coding Agent 集成方 | 在受控环境中运行任意构建、测试与工具链命令，并拿到结构化结果与终端。 |
| Browser Agent 集成方 | 在强隔离环境中运行浏览器与自动化协议，且出网受策略约束。 |
| MCP 工具与技能提供方 | 让自有的 MCP Server 与 Skills 在受治理的进程与网络边界内运行，不依赖宿主网络。 |
| Runtime 平台工程团队 | 以节点容量、Pool 命中率、启动时延与隔离等级为可度量目标运营集群，而不是逐实例独占资源。 |

## 3. 目标与非目标 (Goals And Non-Goals)

### 目标

- 为本地、私有化与 SaaS 组合提供一套经过评审的 Runtime 契约。
- 将 Agents-owned 持久 Workspace 与可销毁 Sandbox 分离。
- 让 BirdCoder Local 与 Cloud 复用同一 Workspace Revision、运行分配、Command、耐久 Checkpoint 和补偿语义；Local Workspace 默认不隐式上传，全部数据留在设备上的声明必须通过独立驻留/恢复 Gate，Cloud 通过隔离 Attachment 挂载持久数据。
- 为每个 `SandboxSession` 提供独立运行生命周期、`SandboxRuntimeBinding`、Quota、日志/事件流与恢复状态，同时不复制 `AgentSession` 业务聚合。
- 通过 SPI 与一致性测试扩展 Provider，不修改 Kernel。
- 先覆盖 Windows、macOS、Linux 的 Local Provider 平台发现与精确 Capability Matrix：Windows/Linux 只有真实 containment 通过后声明 Terminal，macOS 在 detached-descendant containment 获批前明确拒绝 Terminal；再以同一 Provider-neutral Command Contract 跑通 Linux KVM Firecracker Provider。Docker 明确延期到 Local 与 Firecracker 验证完成之后，后续再分阶段评审 gVisor、Kubernetes 与 Remote VM。
- 按 Provider 声明的隔离等级执行默认拒绝的文件系统、进程、网络、Capability、Secret 与资源策略。
- 让启动时延、容量、失败、配额、安全事件和 Provider 健康状态可观测。
- 对齐成熟 microVM Agent Runtime 的完整能力集合：创建、删除、暂停、恢复、重启、Fork、Snapshot、Template、Workspace、文件系统、Shell、进程、PTY、端口转发、网络隔离、出网策略、资源配额、指标、日志、Runtime Pool、Placement、多租户、自动暂停/恢复、COW 存储、按需内存、Template 缓存、对象存储、本地缓存、边缘路由、MCP、Skills 与 Agent Runtime。每项能力都必须有承载的 `REQ-*`、独立验证与门禁状态，不允许用推断代替证据。
- 提供可显式声明的**运行模式分层**，覆盖从轻量共享运行时到强隔离 microVM 的完整区间，并让调用方按工作负载风险选择，而不是由实现沉默决定。
- 提供 Runtime Pool 与可配置预热容量，把热分配时延与冷启动时延分开度量与承诺。
- 提供 Template、Snapshot、Fork 与状态物化能力，使执行环境可复制、可加速恢复、可并行派生。
- 提供三级网络模式与统一出网策略：默认拒绝，永久阻断云 Metadata、宿主控制面与租户间横向流量。
- 提供端口暴露能力，使 Sandbox 内服务经统一边缘入口受控对外访问，而不暴露 Provider 私有地址。
- 提供 Sandbox 内受控 Agent 运行时，使 MCP、Skills、进程、文件、终端与端口能力可被受治理地消费。
- 提供 Rust、TypeScript、Python 三语言同语义 SDK 消费面，由权威契约生成，不手写方言。

### 非目标

- 不负责 Prompt、Model、Agent 推理、对话语义、Agent Provider SDK 或 Provider 特有 Agent 行为。
- 不替代 `sdkwork-kernel` 的工具编排或 MCP 协议语义；Sandbox 只提供受控的执行能力。
- 不负责 IAM 登录与租户身份权威，也不负责价格、账单和支付；Sandbox 只消费已验证身份/配额策略并输出计量事实。
- 不宣称 Local Provider 具备与容器、gVisor 或 microVM 相同的隔离强度。
- 不要求 V1 一次性实现全部 Provider；当目标隔离等级不可用时，禁止静默降级到更弱 Provider。
- 不自己实现 KVM Hypervisor 或自研 microVM 监视器；第一阶段以成熟开源 VMM 作为 Backend 承载，自研不构成产品价值。
- 不把 Docker 作为运行时依赖或隔离边界；Docker 只允许作为 Template 的构建输入格式。
- 第一阶段不实现 Windows 内核级 Sandbox；Windows Agent 属于未来 VM Backend 议题。
- GPU Sandbox、Computer Use 与多区域 SaaS 不属于第一阶段核心范围。
- 不以“增加线程数”解决并发问题，也不以“增加容器数量”解决隔离问题。
- 不承诺任何未经参考硬件压测的时延、并发或资源占用数字。

## 4. 范围 (Scope)

产品范围包括 Runtime、Session、Workspace Runtime Transaction、Sandbox Provider SPI、运行模式分层、资源与配额执行、Scheduler、Placement、Pool 与预热容量、Template 与构建链、状态物化（按需内存与写时复制）、面向执行的 Filesystem/Terminal/Browser/PTY/Port 能力、Checkpoint/Snapshot/Fork、Cache 与存储分层、边缘路由与端口暴露、Network Policy 与出网策略、Secret Projection、Sandbox 内 Agent Runtime、MCP 与 Skills 执行面、SDK 消费面、Node 信任与 Drain、单机与集群双形态、日志、指标、Trace、审计事件与恢复编排。

范围拆分为三个层次，各自的详细要求位于对应分片：

| 层次 | 内容 | 详细要求 |
| --- | --- | --- |
| 运行与容量 | 运行解耦原则、运行模式分层、Runtime Pool、Template、Snapshot/Fork、状态物化、存储分层与缓存、Idle 收敛 | [PRD-runtime-execution-model.md](PRD-runtime-execution-model.md) |
| 能力面 | Workspace 目录契约、Filesystem、Process、PTY、网络三模式、出网策略、端口暴露、Agent Runtime、MCP、Skills、SDK、可观测性 | [PRD-sandbox-surfaces.md](PRD-sandbox-surfaces.md) |
| 生命周期与所有权 | 身份、状态机、Workspace Attachment、Provider 契约、Scheduler/Pool/Quota、日志事件 | [PRD-capabilities.md](PRD-capabilities.md) |

### 术语与所有权

`Runtime`、`Session`、`Workspace`、`Sandbox` 与 `Provider` 是本产品固定术语，不得用 `RuntimeLocation`、`SandboxSpec`、`SandboxInstance` 等替代词重命名领域概念。跨域实现按所有权限定名称：

| 固定术语 | 权威领域对象 | Sandbox Rust 字段/变量 | 预留 Wire 字段 | 所有权与映射 |
| --- | --- | --- | --- | --- |
| Workspace | `AgentWorkspace` / `SandboxWorkspaceId` | `sandbox_workspace_id` | `sandboxWorkspaceId` | `sdkwork-agents` 拥有 Identity、业务生命周期、授权与持久化；Kernel 只把已授权 Identity 映射为 Opaque Sandbox Context。 |
| Session | `AgentSession` / `SandboxSession` / `SandboxSessionId` | `sandbox_session_id`、`sandbox_session_state` | `sandboxSessionId`、`sandboxSessionState` | Agents 拥有业务聚合；Sandbox 只拥有 Provider-neutral 运行生命周期投影。 |
| Runtime | `SandboxRuntimeBinding` / `SandboxRuntimeBindingId` | `sandbox_runtime_binding`、`sandbox_runtime_binding_id` | `sandboxRuntimeBindingId` | Sandbox 拥有当前 Sandbox Provider Allocation 的运行绑定；Kernel 只向 Agents 暴露 Opaque `runtimeLocationId`。 |
| Sandbox | `SandboxId` | `sandbox_id` | `sandboxId` | Sandbox 生成并拥有可销毁的执行分配身份。 |
| Provider | `SandboxProvider` / `SandboxProviderId` / `SandboxProviderKind` / `SandboxProviderDescriptor` | `sandbox_provider`、`sandbox_provider_id`、`sandbox_provider_kind`、`sandbox_provider_descriptor` | `sandboxProviderId`、`sandboxProviderKind` | Sandbox Provider SPI 拥有执行环境契约；Kernel 不按具体 Sandbox Provider Kind 编写业务分支。 |
| Lifecycle Operation | `OperationId` / `SandboxSessionOperation` | `sandbox_operation_id`、`sandbox_session_operation` | `sandboxOperationId` | `OperationId` 是 SDKWork 共享类型；Sandbox 字段和变量用 `sandbox_` 表达所属生命周期。 |
| Lifecycle Ownership | `SandboxLeaseOwnerId` / `SandboxFencingToken` / `SandboxSessionLease` | `sandbox_lease_owner_id`、`sandbox_fencing_token`、`sandbox_session_lease` | 内部控制面契约，暂不承诺 Public Wire | Sandbox 使用 Tenant-scoped Lease 与单调 Fencing Token 防止多个控制器同时拥有 Provider Side Effect。 |

SDKWork 共享类型 `TenantId`、`OperationId`、`RuntimeCapability` 与 `IsolationAssurance` 保持标准名称，不创建 `SandboxTenantId`、`SandboxOperationId` 等重复别名。共享类型进入 Sandbox Command、Projection、Log 或 Event 后，存在领域歧义的字段和变量仍使用 `sandbox_` 前缀，例如 `sandbox_operation_id`、`sandbox_required_capabilities`、`sandbox_runtime_capabilities`、`sandbox_minimum_assurance` 与 `sandbox_isolation_assurance`；对应预留 Wire 字段为 `sandboxOperationId`、`sandboxRequiredCapabilities`、`sandboxRuntimeCapabilities`、`sandboxMinimumAssurance` 与 `sandboxIsolationAssurance`。

产品叙述继续使用 `Runtime`、`Session`、`Workspace`、`Sandbox` 与 `Provider`，不得为了实现命名而改写产品术语。Rust 类型、字段、局部变量、测试夹具和未来 API/事件示例必须使用上表的所有权限定名称；Sandbox-owned 上下文禁止使用无前缀的 `workspace_id`、`session_id`、`runtime_binding_id`、`operation_id`、`provider_id`、`lease_owner_id` 或 `fencing_token`。`SandboxProviderAllocationRef` 及变量 `sandbox_allocation_reference` 是 Provider 私有持久/恢复上下文，不得成为普通 Projection、Log、Event 或 Wire 字段。

公开错误和 Result 也属于领域契约：Identifier Boundary 使用 `SandboxIdentifierError`，生命周期使用 `SandboxLifecycleError`/`SandboxLifecycleResult`，Repository Port 使用 `SandboxSessionRepositoryError`/`SandboxSessionRepositoryResult`。应用尚未发布，不保留 `IdentifierError`、`LifecycleError`、`LifecycleResult`、`RepositoryError` 或 `RepositoryResult` 兼容别名。

生命周期控制权竞争使用 `SandboxLifecycleError::LeaseUnavailable`，已取得控制权后续租、令牌校验、持久化 Lease 校验或成功业务后的释放失败使用 `SandboxLifecycleError::LeaseLost`；已有 Provider/Readiness 错误不被并发释放错误覆盖。Reconciler 必须在取得 Lease 后重新读取权威 `SandboxSession`，不得依据 Lease 前陈旧状态触发 Provider Side Effect。跨 Kernel 边界时 `LeaseUnavailable`/`LeaseLost` 映射为来源为 Runtime 的可重试 `KernelErrorKind::Conflict`，不得误分类为参数校验或 Sandbox Provider 故障。

依赖方向固定为 `sdkwork-agents -> sdkwork-kernel -> sdkwork-sandbox`。Sandbox 不依赖 Agents，不建立第二套 Workspace Registry，也不从 `sandbox_workspace_id` 猜测物理路径。

详细能力边界、状态机和产品验收规则见 [PRD-capabilities.md](PRD-capabilities.md)。阶段承诺与延期能力见 [PRD-roadmap.md](PRD-roadmap.md)。

## 5. 用户场景 (User Scenarios)

1. BirdCoder Desktop 通过本地 Agents 组合创建 Session，Kernel 把已授权的 Workspace/Revision 映射到 Local Sandbox 已打开的 Workspace Capability；当 REQ-2026-0022 的 Local-only 驻留/恢复证据通过后，Workspace、业务状态、Runtime State 和派生副本才能声明留在本机，停止运行环境、默认重置或卸载均不删除 Workspace。
2. SaaS 运维人员将同一 Runtime 请求提交到 Firecracker Provider，并附带由 `SandboxNetworkPolicyPort` 授权的显式 DNS/Egress Policy 请求、Secret 引用、CPU/Memory/Disk 限制和可审计策略；不存在有效 `SandboxNetworkPolicyGrant` 或合规 MicroVm Provider 时关闭失败，不回退 Local 或延期的 Docker Provider。
3. BirdCoder Cloud 通过 Agents/Kernel 请求 Firecracker Runtime；Sandbox 在 Capacity Reservation 后选择 Cold 或干净 Pool Slot，挂载不可变 Workspace Revision，执行命令，生成耐久 Checkpoint Candidate 并交由 Agents CAS 晋级 Revision，然后完成 Detach、Sanitization、Residue Scan 和资源归还。
4. Provider 开发者增加 Firecracker 适配器，通过生命周期、文件系统约束、网络、资源、事件与清理一致性测试，不改变 Kernel-facing 契约。
5. Kernel 仅请求 Capability 与隔离等级，不选择具体 Provider；不存在合规 Provider 时返回类型化错误，而不是降低隔离等级。
6. Coding Agent 在强隔离运行模式下创建执行环境，挂载不可变 Workspace Revision，运行构建与测试命令，经受控出网拉取依赖，产出耐久 Checkpoint 后释放执行环境；Workspace 数据不随执行环境销毁而丢失。
7. Browser Agent 在 MicroVM 运行模式中启动浏览器与自动化协议；其出网受与普通进程相同的策略约束，其对外端口经统一边缘入口受控暴露，运行时不接触宿主网络与云 Metadata。
8. 并行评测与 A/B 场景从一个已就绪执行环境派生多个实例：只读 Base 共享，可写层独立；派生数量受租户配额约束，且不得因派生而降低隔离等级。
9. 海量轻量 Agent 请求共享运行时：工作负载被归类为非危险且调用方显式接受弱隔离时使用最轻运行模式；一旦风险分类升级，必须显式切换到强隔离模式并重新准入，禁止静默降级或静默升级掩盖配置错误。
10. Agent Session 跨执行环境迁移：会话在环境 A 暂停并物化状态，在环境 B 恢复继续；Workspace 与业务状态权威始终由 Agents 保持，Sandbox 只拥有运行生命周期投影与清理事实。

## 6. 成功指标 (Success Metrics)

| 指标 | 目标与证据 |
| --- | --- |
| Runtime 可移植性 | Firecracker Provider 候选完成前，同一 Command/Lifecycle Conformance 场景至少通过 Local 与真实 Linux KVM Firecracker；Kernel 不出现按 Provider 类型分支的业务行为。 |
| 本地可用性 | 当前 Windows、macOS、Linux CI Runner 上通过受支持的本地 Smoke 场景。 |
| 热分配速度 | 在公开参考环境中，Pool 到 Workspace 绑定 p95 小于 500 ms；冷启动单独统计。 |
| 生命周期正确性 | 可重试操作具备幂等证据；非法状态转换确定性失败；孤儿回收有恢复测试。 |
| 隔离安全 | 发布安全套件中不存在已知 Workspace 越界、禁止宿主机 Socket 挂载、云 Metadata 访问或日志 Secret 泄露。 |
| 恢复能力 | 可恢复 Session 能重新绑定 Workspace 与最后有效 Snapshot，且不会产生双重活动所有权。 |
| 可观测性 | 每个命令与生命周期操作携带 `traceId`，并在对应身份存在时关联 `sandboxSessionId`、`sandboxWorkspaceId`、`sandboxId` 与 `sandboxRuntimeBindingId`；Agents 关联另行使用 `agentSessionId`/`agentWorkspaceId`。 |
| 容量安全 | 并发 Session 准入不超过租户与节点配额；拒绝结果包含安全的重试信息。 |
| Workspace 持久性 | ReadWrite Runtime 释放前有耐久 Checkpoint/Handoff；并发 Writer 不覆盖新 Revision，断连恢复测试不存在静默丢写。 |
| 数据驻留与恢复 | Local 的 `device-local-persistence`/`strict-device-local-processing` 声明分别通过完整数据清单、无隐式远程持久化/内容外传、角色正确的本地数据库、备份恢复、导出清除和真实 OS/网络证据；Cloud Workspace 只通过 Drive 或批准的 Block-volume Authority 投影。 |
| 能力对齐完整性 | 能力对齐矩阵（见 [PRD-capabilities.md](PRD-capabilities.md)）中每一项都拥有承载的 `REQ-*`、门禁状态与验证证据；不存在“已声明但零证据”的能力。 |
| 运行模式正确性 | 不满足最低隔离等级的请求全部失败关闭；负向测试能复现“无静默降级路径”，包括容量不足、Provider 不可用与策略拒绝三种情形。 |
| 热分配时延 | 在参考硬件、固定 Template 与固定工作负载下分别记录热分配与冷启动的 P50/P95/P99，热路径显著优于冷启动；未达标不得写成已实现 SLO。 |
| 恢复时延 | 暂停后恢复的 P50/P95/P99 在参考环境下记录；恢复不得丢失持久 Workspace 状态，也不得产生双重活动所有权。 |
| 资源效率 | 单节点空闲执行环境占用、单节点可承载的轻量执行环境数量与活跃强隔离实例数量分别测量并记录，测量方法与模板一致。 |
| 网络策略有效性 | 默认拒绝、云 Metadata 阻断、宿主控制面阻断与租户间横向阻断在真实内核与真实网络栈上被验证，而不是仅在单测中模拟。 |
| 状态物化安全 | 跨租户 Snapshot 恢复被拒绝、不兼容版本恢复被拒绝、Fork 派生不产生跨租户可见性，三类负向用例均有可复现证据。 |
| 水平扩展性 | 创建、暂停、恢复、删除的吞吐与错误率在控制面水平扩展下被记录，且不随副本数增加而劣化。 |
| 缓存有效性 | Template 缓存命中率、对等缓存命中率与首命令时延改善被记录；缓存始终可被绕过且不构成唯一事实源。 |

性能目标只有在参考硬件、工作负载、Provider 和统计方法被记录后才能作为发布门禁；它们不是对 Phase 0 空骨架的性能声明。

## 7. 阶段 (Phases)

- **Phase 0, Foundation：** 仓库基线、产品/架构 Canon、组件边界，无执行能力。
- **V1, Local Runtime：** Local Provider、Runtime/Session/Workspace 生命周期、Provider-neutral Command/Terminal 与核心工具能力、资源限制、结构化事件与日志。
- **V2, Isolated Cloud Runtime：** Firecracker、Scheduler、Pool、Snapshot、恢复、Cluster Placement，以及控制面/数据面分离。
- **V3, Elastic Platform：** 延期的 Docker Provider 重新评审，以及 Kubernetes、gVisor、Remote VM、GPU 策略、Browser Sandbox 研究与 Serverless 执行。
- **V4, Runtime Platform：** 多区域 SaaS、受治理 Provider 生态、工作负载感知调度，以及 SDKWork IDE/Browser/Workflow/DevOps 的统一接入。

详细阶段门禁见 [PRD-roadmap.md](PRD-roadmap.md)。Roadmap 条目只有在形成 `ready` 状态的 `REQ-*` 后才进入实施范围。

### 能力建设顺序与交付阶段的映射

产品能力存在一个自然的建设顺序（先有单机原生执行，再有池化与状态物化，再有强隔离，最后是集群与 Agent 集成）。它与本仓库的交付阶段不是一对一关系：本仓库的阶段是**交付门禁**，只有对应 `REQ-*` 进入 `ready` 才会推进。映射如下：

| 能力建设顺序 | 内容 | 本仓阶段 | 状态 |
| --- | --- | --- | --- |
| 1. 原生执行底座 | 生命周期核心、文件系统、进程、资源限制、安全约束、网络策略、API 面 | V1 | 部分候选实现（`REQ-2026-0002`~`0007`） |
| 2. 池化与状态物化 | Runtime Pool、Scheduler、Template、Workspace、Snapshot | V1/V2 交界 | 仅 Gate 0 候选（`REQ-2026-0016`~`0021`） |
| 3. 强隔离 | microVM 监视器后端、Snapshot 恢复、按需内存、写时复制根文件系统 | V2 | 仅 Gate 0 候选（`REQ-2026-0008`、`0012`~`0015`） |
| 4. 集群能力 | 集群 Placement、自动扩缩、高可用、故障转移、迁移 | V2/V3 | 仅 Gate 0 候选（`REQ-2026-0016`、`0017`） |
| 5. Agent 集成 | Sandbox 内 Agent 运行时、MCP、Skills、Browser、Computer Use | V3/V4 | 未授权（`REQ-2026-0024`、`0025` 仅门禁） |

本表只表达**建设先后**，不构成任何实施授权。所列能力均需独立 `REQ-*`、必要 ADR、Verification、Release Evidence 与 Rollback Plan；阶段标签只表达产品顺序。具体组件拆分与命名必须遵守本仓架构决策与 `NAMING_SPEC.md`，不得按能力清单直接推导 Crate 结构（见 [TECH_ARCHITECTURE.md](../../architecture/tech/TECH_ARCHITECTURE.md) 第 3 节）。

## 8. 关联需求 (Linked Requirements)

- [REQ-2026-0001: 初始化 SDKWork Sandbox 工程基础](../requirements/REQ-2026-0001-sandbox-foundation.md) - Phase 0 仓库、文档和组件边界。
- [REQ-2026-0002: 交付 Provider-neutral Sandbox 生命周期核心](../requirements/REQ-2026-0002-sandbox-lifecycle-core.md) - `SandboxSession`、Provider SPI、幂等生命周期与 Memory Repository 候选实现。
- [REQ-2026-0003: 交付受约束的 Local Sandbox Provider](../requirements/REQ-2026-0003-secure-local-provider.md) - HostUser Assurance、路径/进程约束与 Local Provider 安全门禁。
- [REQ-2026-0004: Agents Workspace 与 Sandbox Attachment](../requirements/REQ-2026-0004-agents-workspace-attachment.md) - Agents Workspace 权威、Kernel ID 映射与 Sandbox Attachment 边界。
- [REQ-2026-0005: 持久化 Sandbox Session Repository 与崩溃恢复](../requirements/REQ-2026-0005-durable-sandbox-session-repository-and-reconciliation.md) - PostgreSQL 权威、加密 Runtime Binding 恢复元数据、Lease/Fencing 与瞬态 Session 恢复。
- [REQ-2026-0006: Sandbox Provider Allocation 密钥轮换与有界重加密](../requirements/REQ-2026-0006-sandbox-provider-allocation-key-rotation.md) - 注入式版本化 Key Source、Tenant-scoped 重加密、页目标 Protection Version 稳定性、Session-bound 密文 CAS 与旧密钥撤销门禁。
- [REQ-2026-0007: Provider-neutral Sandbox Command Execution Contract](../requirements/REQ-2026-0007-sandbox-command-execution-contract.md) - Local 与 Firecracker 共用的 Executable/Argv、Limit、Fencing、Result、Error 与 Conformance。
- [REQ-2026-0008: Firecracker Sandbox Provider](../requirements/REQ-2026-0008-firecracker-sandbox-provider.md) - Linux KVM、Jailer、Artifact Integrity、cgroup、Network/Workspace Boundary 与 MicroVm Assurance。
- [REQ-2026-0009: Sandbox Service Host Composition And Readiness](../requirements/REQ-2026-0009-sandbox-service-host-composition-and-readiness.md) - L5 typed Composition、依赖注入、fail-closed Readiness、安全 Shutdown 与 Standalone/Cloud parity；保持 `draft`，不批准真实 Provider/API/Deployment。
- [REQ-2026-0010: Sandbox Observability, Event, Audit And Outbox Contract](../requirements/REQ-2026-0010-sandbox-observability-event-audit-outbox.md) - versioned event envelope、event catalog、structured telemetry、audit-fact 与 transactional Outbox 边界；保持 `draft`，不批准 Runtime exporter、worker、migration、API、SDK 或 deployment。
- [REQ-2026-0011: Sandbox Host Isolation Broker Boundary](../requirements/REQ-2026-0011-sandbox-host-isolation-broker.md) - Firecracker Host 特权固定操作、Local IPC、短期 Grant、Fencing/Idempotency、Audit、Cleanup 与 Supply-chain 边界；保持 `draft`，不批准 Broker runtime 或 privileged implementation。
- [REQ-2026-0012: Sandbox Firecracker Artifact Compatibility And Supply Chain](../requirements/REQ-2026-0012-sandbox-firecracker-artifact-compatibility-and-supply-chain.md) - Firecracker/Jailer/Guest Kernel/RootFS/Guest Agent 的不可变 Architecture Tuple、Evidence、Materialization、Revocation 与 Rollback 边界；保持 `draft`，不批准 Artifact 发布、下载、构建或 Provider runtime。
- [REQ-2026-0013: Sandbox Workspace Block Device Attachment And Sanitization](../requirements/REQ-2026-0013-sandbox-workspace-block-device-attachment-and-sanitization.md) - Agents/Drive Ownership、授权 Guest Block Device、At-rest Encryption、Fencing、Sanitization、Residue Scan 与 Quarantine 边界；保持 `draft`，不批准 Storage/KMS/Device/Provider runtime。
- [REQ-2026-0014: Sandbox Firecracker Network Isolation And Egress Policy](../requirements/REQ-2026-0014-sandbox-firecracker-network-isolation.md) - provider-neutral Policy Authority、`DenyAll`、显式 DNS/Egress Grant、永久 Metadata/Host/Tenant Lateral Denial、per-binding netns/Tap、Atomic Apply/Verify、Cleanup/Quarantine 与 Durable Audit 边界；保持 `draft`，不批准 Network Runtime。
- [REQ-2026-0015: Sandbox Firecracker Resource Isolation And Usage Facts](../requirements/REQ-2026-0015-sandbox-firecracker-resource-isolation-and-usage.md) - provider-neutral Resource Policy、Firecracker Machine Config/cgroup v2 CPU/Memory/PID/IO、Effective Readback、immutable Usage Fact、Commerce Ownership、Cleanup/Quarantine 边界；保持 `draft`，不批准 Resource/Quota/Billing Runtime。
- [REQ-2026-0016: Sandbox Multi-tenant Admission, Scheduling And Capacity](../requirements/REQ-2026-0016-sandbox-multi-tenant-admission-scheduling-and-capacity.md) - provider-neutral Admission/Node Inventory/Scheduler/Capacity Reservation、Hard Placement Filter、Tenant-aware Fairness、PostgreSQL Atomic Reservation、Fencing/Recovery 与 Resource Grant Binding 边界；保持 `draft`，不批准 Scheduler/Database/Node Agent/Pool Runtime。
- [REQ-2026-0017: Sandbox Node Trust, Enrollment, Attestation And Verified Inventory](../requirements/REQ-2026-0017-sandbox-node-trust-enrollment-attestation-and-inventory.md) - provider-neutral Enrollment/Attestation Verification/Inventory Publication/Lifecycle Control、短期 Machine Identity、Verified Inventory、Rotation/Revocation、Drain/Quarantine 与 Scheduler Binding 边界；保持 `draft`，不批准 Node Agent/PKI/Verifier/Database/Deployment Runtime。
- [REQ-2026-0018: Sandbox PostgreSQL Quota And Capacity Reservation Persistence](../requirements/REQ-2026-0018-sandbox-postgresql-quota-and-capacity-reservation-persistence.md) - `SandboxTenantQuotaState`、`SandboxAdmissionReservation`、`SandboxNodeCapacityState` 与 `SandboxCapacityReservation` 候选 PostgreSQL Authority、全局 Lock Order、CAS/Fencing、TTL/Quarantine、RLS/Role、PITR/RPO/RTO 及现有 `tenant_id TEXT` 到标准 `BIGINT` 的预发布迁移门禁；保持 `draft`，不批准 Table/Migration/Repository/Scheduler Runtime。
- [REQ-2026-0019: Sandbox Runtime Pool And Fast Allocation](../requirements/REQ-2026-0019-sandbox-runtime-pool-and-fast-allocation.md) - tenant-neutral `PreparedSlot`/`WarmMicroVmSlot`、fenced Claim、Sanitization/Residue/Quarantine、bounded scaling 与 fast-allocation evidence；保持 `draft`，不批准 Pool、Snapshot、Table、Worker、API、SDK 或 Deployment。
- [REQ-2026-0020: Sandbox Lifecycle Hot State And Idempotency Retention](../requirements/REQ-2026-0020-sandbox-lifecycle-hot-state-and-idempotency-retention.md) - bounded current-state projection、Tenant-scoped point-lookup idempotency ledger、current-operation-only hydration、Session limits、terminal retention、late retry 与 expand/backfill/cutover migration gate；保持 `draft`，不批准 Rust/Database/API/SDK/Kernel 实现。
- [REQ-2026-0021: Sandbox Workspace Runtime Transaction And Checkpoint](../requirements/REQ-2026-0021-sandbox-workspace-runtime-transaction-and-checkpoint.md) - Local/Firecracker lane parity、Workspace Revision Writer Lease、allocation/attachment/command/checkpoint/cleanup 顺序、耐久 Handoff、失败补偿与 bounded SaaS backpressure；保持 `draft`，不批准 Runtime/Storage/API/SDK/Kernel/BirdCoder 实现。
- [REQ-2026-0022: Sandbox Standalone Data Residency And Recovery](../requirements/REQ-2026-0022-sandbox-standalone-data-residency-and-recovery.md) - Local-only 四仓数据清单、设备本地持久化/严格本地处理声明、数据库角色、独立 Runtime Capability、无隐式传输、备份恢复、导出清除和真实 OS 证据；保持 `draft`，不批准数据库、配置、打包、遥测、同步或跨仓库实现。
- [REQ-2026-0023: Sandbox Internal Control Plane](../requirements/REQ-2026-0023-sandbox-internal-control-plane.md) - Sandbox-owned application port、in-process standalone/generated internal-RPC cloud adapter parity、service identity、durable operation/idempotency、independent Kernel/Sandbox fencing、bounded operation event、version/discovery/drain/rollback gate；保持 `draft`，不批准 Rust Port、Proto、SDK、server/client、route、config、deployment 或 Kernel 实现。
- [REQ-2026-0024: Sandbox Interactive Terminal Session](../requirements/REQ-2026-0024-sandbox-interactive-terminal-session.md) - Command/Interactive Terminal Capability 拆分、subordinate Terminal Session、single-controller lease、at-most-once input、idempotent resize、bounded output replay/reconnect、first-terminal CAS、Checkpoint ordering、platform containment 与 privacy gate；保持 `draft`，不批准 PTY/ConPTY、process、guest stream、Proto/SDK/API、persistence、Provider、Service Host 或跨仓库实现。
- [REQ-2026-0025: Sandbox Runtime Secret Projection](../requirements/REQ-2026-0025-sandbox-runtime-secret-projection.md) - value-free opaque grant、Agents/IAM/Secret Authority/Kernel/Sandbox 职责拆分、Local/Cloud lane 与 region binding、显式 process target、rotation/revocation/outage、Checkpoint/Pool exclusion 和 scoped security claim；保持 `draft`，不批准 Secret Authority、value transport、process projection、Proto/SDK/API、persistence、Provider、Service Host 或跨仓库实现。
- [REQ-2026-0026: Sandbox Cloud Data Residency And Recovery](../requirements/REQ-2026-0026-sandbox-cloud-data-residency-and-recovery.md) - Cloud-only data inventory、`regionCode/providerRegion/storageRegion/availabilityZone` tuple、Drive/Agents/Sandbox authority split、explicit replication、backup/PITR、ordered recovery、tenant isolation、export/delete、Secret exclusion 与 RPO/RTO gate；保持 `draft`，不批准 storage/replication/backup/restore/purge implementation、API/SDK、Provider、Service Host 或跨仓库实现。
- [REQ-2026-0027: Sandbox Cross-Repository Version Compatibility And Release Set](../requirements/REQ-2026-0027-sandbox-cross-repository-version-compatibility.md) - immutable BirdCoder/Agents/Kernel/Sandbox revision set, Workspace/storage/RPC/SDK/config/artifact/evidence provenance, explicit multi-dimensional compatibility matrix, peer preflight, drain/rollout/rollback/downgrade and bounded support window；保持 `draft`，不批准 release registry、SDK/proto/artifact publication、migration、deployment 或跨仓库实现。

后续 Runtime API、生命周期、Provider、Scheduler、安全、Snapshot、Cache 与 SaaS 工作必须在实施前拆分为可评审的需求记录。

### 尚未拆分的能力

下列能力的**需求拆分尚未闭合**，因此处于未授权状态。缺口列逐行写明仍缺什么：凡该列仍标注**尚无任何 `REQ-*` 承载**的能力，必须在实施前各自拆分为独立、可评审的需求记录，且不得在实现中默认开启。

本表按**产品需求分组**列举，与 [PRD-capabilities.md](PRD-capabilities.md) 第 11 节的**逐能力**矩阵不是同一粒度，也不是彼此的完整换算：第 11 节还标出本表未列出的能力（Restart、Auto Pause、Auto Resume、Template Cache、Object Storage、Local Cache、Edge Router），本表则包含第 11 节没有对应行的产品要求（运行模式分层、网络 `shared` 模式、SDK 家族、Node Drain 与迁移、Benchmark 套件与容量基线）。两份清单互补，判断某个能力是否已有承载必须以第 11 节逐行状态为准，而不是以本表是否列出为准；`node tools/check-sandbox-requirement-traceability.mjs` 每次运行都会打印第 11 节矩阵的逐行分类普查，可作为该判断的机器读数。

| 能力 | 产品要求位置 | 缺口 |
| --- | --- | --- |
| 运行模式分层（Mode 0 / Mode 1） | [PRD-runtime-execution-model.md](PRD-runtime-execution-model.md) 第 2 节 | 无 `REQ-*`；Mode 0 还需新的 `IsolationAssurance` 值决策 |
| Template 与构建链 | [PRD-runtime-execution-model.md](PRD-runtime-execution-model.md) 第 4 节 | 无 `REQ-*`；与 Firecracker 制品元组的分层关系未定 |
| Snapshot 产品能力（创建/恢复/删除） | [PRD-runtime-execution-model.md](PRD-runtime-execution-model.md) 第 5 节 | 仅有 Checkpoint 与 Firecracker Snapshot 的 Gate 0，无产品级能力 |
| Fork | [PRD-runtime-execution-model.md](PRD-runtime-execution-model.md) 第 6 节 | 无 `REQ-*`；一致性语义未定 |
| 按需内存与写时复制根文件系统 | [PRD-runtime-execution-model.md](PRD-runtime-execution-model.md) 第 7 节 | 无 `REQ-*`；无真实 KVM 证据 |
| 端口暴露 | [PRD-sandbox-surfaces.md](PRD-sandbox-surfaces.md) 第 8 节 | 无 `REQ-*`；公开端点命名与边缘入口归属未定 |
| 网络 `shared` 模式 | [PRD-sandbox-surfaces.md](PRD-sandbox-surfaces.md) 第 6 节 | 无 `REQ-*`；无安全评审 |
| Sandbox 内 Agent 运行时 | [PRD-sandbox-surfaces.md](PRD-sandbox-surfaces.md) 第 9 节 | 无 `REQ-*`；`REQ-2026-0024` 明确将 Guest Agent Stream 列为未批准 |
| MCP 执行面 | [PRD-sandbox-surfaces.md](PRD-sandbox-surfaces.md) 第 10 节 | 仅 Transport 级描述，无独立 `REQ-*` |
| Skills | [PRD-sandbox-surfaces.md](PRD-sandbox-surfaces.md) 第 11 节 | 无 `REQ-*`；目录命名与供应链 Owner 未定 |
| SDK 家族 | [PRD-sandbox-surfaces.md](PRD-sandbox-surfaces.md) 第 12 节 | 无 `REQ-*`；无 `apis/` 权威契约 |
| Node Drain 与迁移 | [PRD-runtime-execution-model.md](PRD-runtime-execution-model.md) 第 9 节 | `REQ-2026-0017` 仅覆盖 Drain 的信任侧，迁移无 `REQ-*` |
| Benchmark 套件与容量基线 | [PRD-roadmap.md](PRD-roadmap.md) 验收标准 | 已有承载：`REQ-2026-0019` 的 Goals 与 Performance 行要求记录固定硬件、工作负载与统计方法并出具 p50/p95/p99，`tools/bench-sandbox-lifecycle.mjs` 与 `docs/architecture/tech/TECH-performance-baseline.md`（两平台实测）也已在树中，但该基线自述发布门禁资格不合格。仍缺：可写入发布门禁的参考硬件与容量基线定义 |

## 9. 待决问题 (Open Questions)

- 各操作系统上的 Local Provider 最低隔离保证是什么，哪些 Workload 必须升级到 Firecracker 或更强 Provider？
- V1 与 V2 分别需要哪些 Workspace 持久化后端和保留等级？
- 第一版远程控制权威选择 HTTP internal-api、internal RPC，还是二者并存？
- Quota 策略、用量聚合和向 Commerce Billing 交接分别由哪个团队拥有？
- 哪一套参考机器与工作负载定义 500 ms 热分配目标？
- 哪些 Provider 能提供 Snapshot/Restore，跨 Provider 的最小可移植 Snapshot 契约是什么？
- 最大 Lifecycle Operation 数、最大活动 Session 生命周期、终态幂等保留窗口及窗口结束后的安全 Late Retry Outcome 分别是什么？
- 首个 Local 商业版本采用哪一种公开驻留声明，哪些数据类进入本地备份，以及其 RPO/RTO、保留和验证恢复预算分别是多少？
- 最轻的共享运行时运行模式是否需要引入新的 `IsolationAssurance` 取值？若是，其命名、取值范围、允许工作负载清单与禁止回退规则是什么？
- 端口暴露的公开端点格式、认证方式与边缘入口由哪个仓库拥有？
- Fork 的一致性语义取“快照点一致”还是“写后可见”，是否限制在同一节点内？
- Template 与 Firecracker 制品元组的权威边界如何划分，单一 Template 是否允许跨架构复用？
- 参考硬件、Template、工作负载与统计方法由哪套基线定义，才能把热分配、恢复与资源目标写成发布门禁？
- 公开商业 SDK 是否存在，还是长期仅提供内部 SDK 家族？
- 能力对齐的验收口径是“接口可用”还是“带真实证据的端到端闭环”，覆盖多少项才算对齐完成？
