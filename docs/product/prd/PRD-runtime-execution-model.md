# SDKWork Sandbox 运行模式、Runtime Pool 与状态物化需求

Status: draft

Owner: SDKWork Runtime Platform

Updated: 2026-09-22

Parent: [SDKWork Sandbox PRD](PRD.md)

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `COMPONENT_SPEC.md`, `SECURITY_SPEC.md`, `PERFORMANCE_SPEC.md`, `CACHE_SPEC.md`, `DATABASE_SPEC.md`, `RUNTIME_DIRECTORY_SPEC.md`, `EVENT_SPEC.md`, `DOCUMENTATION_SPEC.md`

本分片定义“Sandbox 如何被承载”的产品要求：共享 Runtime 与隔离边界的关系、运行模式分层、Runtime Pool、Template、Snapshot/Fork 与状态物化、存储分层与缓存、以及 Idle 收敛。具体机制选型、目录布局与数据模型见 [TECH-runtime-backends-and-pools.md](../../architecture/tech/TECH-runtime-backends-and-pools.md)。

本分片的每一条能力都必须由 `ready` 状态的 `REQ-*` 承载后才进入实施范围。当前除已有 REQ 覆盖项外，下列内容均为候选产品要求，**未授权实现**。

## 1. 运行解耦原则

产品层固定四条不等式，它们是本分片所有能力的设计前提，不得为实现便利而合并：

```text
Agent ≠ Sandbox
Sandbox ≠ Runtime
Runtime ≠ Node
Workspace ≠ Sandbox
```

| 原则 | 产品含义 | 违反后果 |
| --- | --- | --- |
| Runtime 与 Sandbox 分离 | Runtime 是共享计算资源，Sandbox 是隔离边界。禁止 `1 Agent = 1 Runtime`。 | 每 Agent 独占运行时，闲置成本线性放大，无法支撑海量租户。 |
| Agent Session 与 Sandbox 分离 | `AgentSession` 是可迁移的业务会话；`Sandbox` 是可销毁的执行分配。同一 `AgentSession` 允许跨不同 `Sandbox` 续跑。 | 会话与执行绑定后，Pause/Resume、迁移与故障恢复都变成不可解。 |
| Workspace 与 Sandbox 分离 | Workspace 是持久业务状态权威；Sandbox 只提供受控运行访问。 | 删除执行环境会连带删除用户数据。 |
| Sandbox 与 Node 分离 | Node 是容量与调度单位；Sandbox 不是 Node。禁止 `Kubernetes Pod = Agent Sandbox` 的等价化。 | 编排单位与隔离单位混淆，容量与隔离强度都不可控。 |

上述原则与 [PRD.md](PRD.md) 第 4 节术语所有权表、[PRD-capabilities.md](PRD-capabilities.md) 第 1 节职责边界共同生效。`AgentSession`/`AgentWorkspace` 属于 `sdkwork-agents`，Sandbox 只拥有 `SandboxSession` 运行生命周期投影。

## 2. 运行模式分层

不同工作负载对隔离强度的要求差异极大。让所有 Agent 都进入 MicroVM 会浪费容量，让所有 Agent 都共享运行时会使不可信负载越界。因此产品定义**运行模式 (Sandbox Mode)** 分层，并要求调用方显式声明所需最低隔离等级，而不是由实现静默选择。

| 运行模式 | 目标隔离机制 | 隔离边界 | 适用工作负载 | 当前状态 |
| --- | --- | --- | --- | --- |
| Mode 0：Shared Execution Runtime | 无独立内核边界；按租户、文件系统、网络、进程、CPU、Memory 做逻辑隔离；共享 Runtime、库与基础文件系统 | 弱于 Container | LLM Agent、Workflow、MCP、文档处理、数据处理、非危险计算 | 候选；**未批准**，需独立 `REQ-*` 与 ADR |
| Mode 1：Namespace Sandbox | Mount/PID/Network Namespace、Cgroup v2、Seccomp、Capabilities、OverlayFS | Container 级 | 普通 Coding Agent、构建、测试 | 候选；仓库当前无 Container 级 Provider |
| Mode 2：MicroVM Sandbox | Firecracker + KVM + unikernel-style Guest | MicroVm 级 | 任意代码、Coding Agent、Browser Agent、不可信 MCP、Docker、系统级操作 | 已有候选边界（`REQ-2026-0008` 等，均为 `draft`） |

### 2.1 与 `IsolationAssurance` 的映射

隔离等级判定必须复用 SDKWork 共享类型 `IsolationAssurance`，不得为运行模式新建平行枚举语义。当前枚举值为 `HostUser`、`Container`、`UserSpaceKernel`、`MicroVm`、`DedicatedVm`。

| 运行模式 | 目标 `IsolationAssurance` | 差异说明 |
| --- | --- | --- |
| Mode 0 Shared Execution Runtime | **当前枚举中不存在可比项** | 需要一个明确的弱于 `Container` 的候选值。新增枚举值属于 SDKWork 共享类型变更，必须走独立 ADR 与人工评审。 |
| Mode 1 Namespace Sandbox | `Container` | 现有值可直接承载。 |
| Mode 2 MicroVM Sandbox | `MicroVm` | 现有值可直接承载。 |
| （未来）User-space Kernel | `UserSpaceKernel` | 对应 gVisor，仓库当前列为后续 Provider。 |
| （未来）专用虚拟机 | `DedicatedVm` | 对应 Remote VM / 强隔离单租户。 |

### 2.2 分层门禁

- 调用方必须显式声明所需最低 `IsolationAssurance`。不满足时**失败关闭**，禁止静默降级到更弱运行模式。
- Mode 0 只在调用方显式声明其可接受弱隔离、且工作负载被归类为非危险时才允许；禁止把 Mode 0 作为 Mode 1/Mode 2 容量不足时的回退。
- 每个运行模式必须独立声明 Capability 矩阵与公开隔离限制说明；不允许用一个模式的 Capability 声明推断另一个模式。
- Mode 0 的多租户共享（共享 Runtime、库与基础文件系统）必须先通过跨租户残留、信息泄露与逃逸 Threat Model 评审。

## 3. Runtime Pool

Runtime Pool 是本产品面向海量租户的核心容量能力：把“每次请求都从零创建执行环境”改为“维持可分配的已预备容量，按需 Claim”。

- Pool 状态至少区分 Warm（已预备未分配）、Idle（曾分配已释放待复用或回收）、Busy（已分配）、Draining（停止接纳新分配）。
- Pool 必须可按 Template 与用途分级，例如按语言运行时（Node、Python、Rust、Java）、按 Agent 类型（Coding、Browser、General）、按租户专用池。
- 每个池配置最小与最大预备量；节点必须在配置区间内维持可分配容量。
- Pool 不足时必须回退到**同等级合规**冷启动，不能回退到更弱隔离等级或更弱 Provider。
- Pool 必须支持背压：资源不足时排队并返回安全的重试信息，而不是无限创建。

安全与归属要求：

- 第一阶段的池只保存 tenant-neutral 的已预备槽位，不保存任何租户 Workspace 数据、Process、Port、Secret、Network Grant、Guest Identity、Terminal Buffer、Command Output 或 Provider 私有 Allocation。
- 槽位入池前必须证明无租户残留；复用前必须应用新的 Workspace/Network/Resource/Guest Identity 绑定并回读 Effective Evidence。
- 残留检测不确定、清理失败或容量计数不确定一律隔离，宁可少卖不可超卖。
- Pool 的分配链必须串入 Admission Reservation、Verified Node Inventory、Capacity Reservation、Provider Allocation 与各 Grant，并带单调 Fencing；旧 Fencing Token 在任何 Host 或 Provider 副作用前失败。

已有载体：[REQ-2026-0019](../requirements/REQ-2026-0019-sandbox-runtime-pool-and-fast-allocation.md)（`draft`）、[REQ-2026-0016](../requirements/REQ-2026-0016-sandbox-multi-tenant-admission-scheduling-and-capacity.md)（`draft`）、[REQ-2026-0018](../requirements/REQ-2026-0018-sandbox-postgresql-quota-and-capacity-reservation-persistence.md)（`draft`）、[REQ-2026-0017](../requirements/REQ-2026-0017-sandbox-node-trust-enrollment-attestation-and-inventory.md)（`draft`）。

## 4. Template

Template 是 Sandbox 的基础镜像与运行环境声明，需要同时覆盖内核、根文件系统、运行时、库、工具、Agent 组件与默认环境。

产品要求：

- Template 必须可版本化、可寻址、不可变；同一 Template Version 在多次分配中必须给出相同内容。
- Template 至少包含：Guest Kernel、RootFS、Runtime/库集合、工具链、Agent 组件、环境变量声明、启动命令、健康检查声明。
- Template 构建输入允许使用 Dockerfile 或构建脚本，但 **Docker 只作为构建输入格式，不得成为运行时依赖**，也不得成为 Sandbox 运行时的隔离边界。
- Template 与 Firecracker 制品元组的关系必须分层：制品元组固定二进制与镜像完整性，Template 在其上声明运行时与应用层内容。二者不得互相替代。
- Template 的命名、签名、供应链与吊销必须继承 [REQ-2026-0012](../requirements/REQ-2026-0012-sandbox-firecracker-artifact-compatibility-and-supply-chain.md) 的不可变元组、Evidence、Revocation 与 Rollback 门禁。
- Template 缓存必须声明 Hot/Warm/Cold 分级、淘汰策略、容量上限与失效规则；缓存不得成为 Template 或执行策略的唯一事实源。

当前状态：仓库 Canon 尚未定义 Template 产品能力，需独立 `REQ-*` 与 ADR。

## 5. Snapshot

Snapshot 必须区分三类不同权威的产物，禁止混为一个概念：

| 产物 | 内容 | 权威归属 | 可移植性 |
| --- | --- | --- | --- |
| Workspace Revision / Checkpoint Candidate | 用户可见的工作集与持久业务状态 | Agents 晋级 Revision；Drive 或批准的 Volume Authority 拥有字节 | 跨 Provider 可移植 |
| Provider Snapshot | Guest Memory、VM/Hardware State、RootFS Diff、Runtime State、Metadata | Provider 私有，用于加速恢复 | 不承诺跨 Provider 可移植 |
| Session Checkpoint | 运行会话的可移植产品状态 | Sandbox 组合，用于重建执行上下文 | 可移植，且必须是恢复的唯一持久事实源 |

产品要求：

- Provider Snapshot 只能是恢复加速器，不能成为唯一持久事实。
- Snapshot 必须绑定租户、Template、架构、内核、运行时版本与安全版本；任一维度不兼容时**拒绝恢复**。
- 禁止跨租户恢复：任何情况下不得把租户 A 的 Snapshot 恢复到租户 B 的执行环境。
- 恢复前必须校验完整性校验值、签名、版本与兼容性，并确认独占所有权与 Secret 新鲜度。
- Snapshot 的持久化位置、保留策略、加密与访问控制必须继承 Cloud/Local 数据驻留要求（[REQ-2026-0026](../requirements/REQ-2026-0026-sandbox-cloud-data-residency-and-recovery.md)、[REQ-2026-0022](../requirements/REQ-2026-0022-sandbox-standalone-data-residency-and-recovery.md)，均为 `draft`）。

当前状态：仓库 Canon 已有 Workspace Checkpoint 与 Firecracker Snapshot 的 Gate 0 候选（`REQ-2026-0021`、`REQ-2026-0008`），但**没有** Product-level `Snapshot` 创建/恢复/删除能力定义，需独立 `REQ-*`。

## 6. Fork

Fork 从一个既有 Sandbox 派生多个共享 Base、写入独立的执行环境，用于强化学习、评测、并行 Agent、代码测试与 A/B 对比。

产品要求：

- Fork 的派生实例必须共享不可变 Base（Memory 与 RootFS 的只读部分），并各自拥有独立可写层。
- Fork 必须继承源沙箱的租户、Template 与隔离等级约束；不得因 Fork 而降级隔离。
- Fork 必须显式声明一致性语义：来源 Write 操作在 Fork 点之后是否可见，必须有确定答案，不允许依赖实现巧合。
- Fork 数量必须受租户配额与容量准入约束。
- Fork 与 Snapshot 是不同操作：Snapshot 是持久化，Fork 是派生；Fork 可以基于 Snapshot 实现，但不得要求调用方理解实现细节。

当前状态：仓库 Canon 完全没有 Fork 能力定义，需独立 `REQ-*`、ADR 与安全评审。

## 7. 状态物化：Lazy Memory 与 COW RootFS

这两项能力决定 Pause/Resume 与冷启动的真实成本，因此属于产品级要求而非纯实现细节：调用方可依赖的行为承诺必须写清。

### 7.1 Lazy Memory

- 目标行为：恢复时内存按需加载，仅加载真正被访问的页；未访问页不占用物理内存。
- 承诺边界：调用方可依赖的是“恢复不需要等待全部内存传输完成”；具体页加载时序属实现细节。
- 必须建立热点页画像，用于恢复时后台预取以降低首屏页缺失；预取不得改变语义正确性。

### 7.2 COW RootFS

- 目标行为：Template RootFS 只读共享，Sandbox 只把实际修改的数据写入独立可写层。
- 承诺边界：调用方可依赖“未修改数据不复制”，不得依赖具体的写时复制实现。
- 暂停时允许把脏块导出为差异数据，该差异必须与 Workspace Revision 语义区分开。
- 可写层生命周期必须与 Sandbox 生命周期绑定；Sandbox 销毁必须确保可写层被清理或显式保留，且不得隐式影响 Template。

当前状态：仓库无对应能力定义与证据，需独立 `REQ-*` 与真实 Linux KVM 证据。

## 8. 存储分层与缓存

存储按三级分层，读取优先级由近到远：

| 级别 | 位置 | 内容 | 说明 |
| --- | --- | --- | --- |
| L1 | 本地 NVMe | 活跃 Template、可写层、热数据 | 低延迟路径 |
| L2 | 节点共享缓存 | 跨节点可复用的 Template 与制品 | 允许节点间对等获取 |
| L3 | 对象存储 | Template 权威、Snapshot、冷数据 | 兼容 S3 协议；可对接主流对象存储服务 |

产品要求：

- 同一 Template 在相邻节点已被缓存时，优先从对等节点获取，而不是回源对象存储，以减少带宽与首命令时延。
- 缓存必须声明命名空间、敏感级别、TTL/保留、容量上限与失效策略。
- 缓存是派生数据，不得成为持久 Workspace 或执行策略的唯一事实源。
- 存储选择与数据驻留区域必须遵循 `REQ-2026-0026` 的 `regionCode`/`providerRegion`/`storageRegion` 分层，禁止把数据写到未声明区域。

## 9. Idle 收敛与自动暂停/恢复

- 空闲判定必须基于可观测事实的组合（无活动进程、无网络活动、无外部请求），而不是单一定时器。
- 达到空闲阈值后允许自动暂停并将状态物化，释放 Node 资源。
- 收到流量时允许自动恢复并继续服务；恢复必须满足第 5 节的兼容性与所有权校验。
- Pause/Resume 不得丢失持久 Workspace 状态；恢复失败必须回到可恢复的显式状态，不得静默丢弃或回退到更弱隔离。
- Auto Resume 不得绕过 Admission、Quota、Node Health 与兼容性检查。

## 10. 未决门禁

- Mode 0 的隔离弱于 `Container`，是否引入新的 `IsolationAssurance` 值、其命名与取值范围是什么？
- Mode 0 是否允许跨租户共享基础文件系统与库，哪些数据类必须物理隔离？
- Fork 的一致性语义取哪种口径，是否需要限制在同一 Node 内？
- Provider Snapshot 的跨 Provider 可移植契约是否存在，最小可移植子集是什么？
- 热点页画像的采集与使用是否需要独立的隐私评审？
- Auto Pause 阈值、恢复预算与 SLO 由哪套参考硬件与工作负载定义？

每项未决问题都必须在其 `REQ-*` 进入 `ready` 前得到人审结论。
