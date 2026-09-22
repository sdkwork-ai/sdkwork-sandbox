# SDKWork Sandbox 能力面与访问路径需求

Status: draft

Owner: SDKWork Runtime Platform

Updated: 2026-09-22

Parent: [SDKWork Sandbox PRD](PRD.md)

Specs: `REQUIREMENTS_SPEC.md`, `API_SPEC.md`, `INTERNAL_API_SPEC.md`, `SDK_SPEC.md`, `SDK_WORKSPACE_GENERATION_SPEC.md`, `SECURITY_SPEC.md`, `PRIVACY_SPEC.md`, `PAGINATION_SPEC.md`, `OBSERVABILITY_SPEC.md`, `EVENT_SPEC.md`, `DRIVE_SPEC.md`

本分片定义 Agent 在 Sandbox 内部与外部可用的能力面：Workspace 目录契约、文件系统、进程、终端、网络、出网策略、端口暴露、Agent Runtime、MCP、Skills、SDK 消费面与可观测性。传输形态、监听器与进程拓扑见 [TECH-runtime-backends-and-pools.md](../../architecture/tech/TECH-runtime-backends-and-pools.md)；能力所有权与 Provider 契约见 [PRD-capabilities.md](PRD-capabilities.md)。

## 1. 能力面对齐范围

本分片覆盖的产品能力面如下。每一项都必须拥有独立 Capability 声明、独立 `REQ-*`、独立验证；不得因为没有 `REQ-*` 就在实现中默认开启。

| 能力面 | 最小产品行为 | 当前承载 |
| --- | --- | --- |
| Workspace 目录契约 | 固定的 Workspace 内目录布局与用途分离 | 无；本分片定义 |
| Filesystem | 受限的目录、列举、属性、读写、追加、删除、重命名、复制、移动、权限、监听、上传、下载 | [REQ-2026-0007](../requirements/REQ-2026-0007-sandbox-command-execution-contract.md)（`draft`）部分覆盖 |
| Process | 执行、派生、终止、信号、等待、列举、标准输入输出错误流 | `REQ-2026-0007`（`draft`） |
| PTY / Interactive Terminal | 交互式终端、窗口尺寸调整、会话流式传输与重连 | [REQ-2026-0024](../requirements/REQ-2026-0024-sandbox-interactive-terminal-session.md)（`draft`） |
| Network | shared / proxy / isolated 三模式与出网策略 | [REQ-2026-0014](../requirements/REQ-2026-0014-sandbox-firecracker-network-isolation.md)（`draft`） |
| Port Exposure | 端口注册、暴露、转发、撤销与外部访问端点 | 无独立 `REQ-*`；本分片定义 |
| Agent Runtime | Sandbox 内负责进程、文件、终端、端口、环境、MCP、Skills 的托管组件 | 无；`REQ-2026-0024` 只把 Guest Agent Stream 列为未授权项 |
| MCP | 为 Kernel 拥有的 MCP 语义提供受治理的进程与网络执行 | `PRD-capabilities.md` 第 5 节（Transport 级） |
| Skills | Workspace 内技能目录的声明、读取、执行与外部调用 | 无；本分片定义 |
| SDK 消费面 | Rust / TypeScript / Python 客户端对上述能力的稳定访问 | 无；仓库当前无任何 SDK |
| Observability | 指标、日志、Trace、事件与审计 | [REQ-2026-0010](../requirements/REQ-2026-0010-sandbox-observability-event-audit-outbox.md)（`draft`） |

## 2. Workspace 目录契约

Workspace 内部目录布局属于产品契约：Agent、Skills 与用户脚本都会依赖它，因此必须固定并文档化。

| 目录 | 用途 | 生命周期要求 |
| --- | --- | --- |
| `/home` | Agent 与进程的 Home 与配置 | 随 Workspace 持久 |
| `/workspace` | 用户工作集与源码 | 随 Workspace 持久；与 Agents Workspace Revision 语义绑定 |
| `/tmp` | 临时数据 | 可清理；不得作为持久事实源 |
| `/cache` | 派生缓存（包管理器、构建缓存等） | 派生数据；必须声明命名空间与失效策略 |
| `/data` | 结构化数据 | 随 Workspace 持久；敏感级别需单独声明 |
| `/runtime` | Sandbox 运行目录与进程运行时状态 | 由 Sandbox 拥有；Cleanup/Reset 时回收，不得承载用户数据 |

要求：

- `/runtime` 与 `/workspace` 必须是不同的 Capability，且不得指向同一物理路径。
- Cleanup、默认 Reset 与 Uninstall 保留用户拥有的 Workspace；只有显式删除 Workspace 才删除持久工作集。
- 目录布局一经发布即为契约：变更需要新的 `REQ-*` 与迁移计划，不得静默调整。

## 3. Filesystem 能力面

必须支持：目录创建、列举、属性查询、读取、写入、追加、删除、重命名、复制、移动、权限变更、目录监听、上传与下载。

要求：

- 路径解析必须阻止目录遍历、符号链接/重解析点越界、挂载逃逸与 Alternate Data Stream 等适用攻击。
- 破坏性操作必须显式化并进入审计；默认不提供跨 Workspace 的访问能力。
- 大文件的上传下载必须有界、可续、可取消，且不得在内存中整体物化。
- 高频路径必须避免“Agent → HTTP API → 文件系统”的绕行；HTTP 只作为管理与低频面，数据传输走 Sandbox 内通道。
- 文件读写能力不得依赖宿主文件系统直接暴露；宿主路径不得进入公开契约。
- 文件系统能力必须同时受 Template RootFS 只读层与 Sandbox 可写层约束，写入目标是可写层而非 Template。

## 4. Process 能力面

必须支持：执行、派生、终止、信号、等待、列举，以及标准输入/输出/错误流。

要求：

- 命令必须以结构化形式提交（可执行对象 + 参数数组 + 工作目录 + 环境声明），不接受由字符串拼接后交给 Shell 解释的语义。
- 每个命令必须带单调 Fencing、幂等标识与有界超时；重放必须返回同一结果或明确的冲突。
- 进程必须受 CPU、Memory、PID、IO、Wall Time 限制约束，且限制不可由被约束进程自行放宽。
- 终止必须保证进程组与后代进程被回收，不遗留孤儿进程。
- 执行结果必须区分已启动后的终态结果与启动前/结果不可得的错误，二者语义不得混用。
- 命令输出必须有界并可按游标或分页回放；禁止把无界历史读入内存后切片。

## 5. PTY 与交互式终端

必须支持：交互式终端会话、窗口尺寸调整、输入流、流式输出与会话重连。

要求：

- 交互终端与非交互命令是不同 Capability，必须分别声明，不得用一个 Capability 同时暗示两者。
- 终端会话必须有单一控制者租约、至多一次输入语义与幂等尺寸调整。
- 输出必须有序且有界，断连后允许有限窗口内重放；超出窗口必须显式失败而不是静默截断。
- 传输使用 WebSocket 或等价的流式通道；内部到 Sandbox 走 Sandbox 内通道。
- macOS 上的终端能力保持**显式拒绝**，直到 detached-descendant containment 获得独立证据批准。
- 终端流、运行日志与审计事件是不同数据类别，拥有不同脱敏与保留策略；终端流不得包含 Raw Token、Credential、Private Key 或完整 Secret 环境。

## 6. 网络三模式

网络必须支持三种显式模式，由调用方声明，且默认不得放宽：

| 模式 | 拓扑 | 资源开销 | 适用 | 强制要求 |
| --- | --- | --- | --- | --- |
| shared | 直接使用宿主网络 | 最低 | 仅可信任务 | 必须显式选择；禁止作为默认；禁止用于多租户不可信负载 |
| proxy | 经 SDKWork Network Proxy 出口 | 低 | 推荐默认 | 所有出网必须经过策略判定 |
| isolated | 独立 Network Namespace + 虚拟网卡 + Bridge + NAT | 最高 | 高隔离要求 | 每 Sandbox 独立 IP、路由、DNS 与端口空间 |

要求：

- 默认模式为 proxy 或 isolated；`shared` 必须有独立安全评审与显式启用条件。
- 无论何种模式，永久拒绝：Cloud Metadata 访问、宿主机控制面访问、租户间横向流量。
- 每 Binding 必须独立网络身份与策略版本；策略带 Fencing，应用后必须回读并探测生效。
- 网络策略应用、回读、探测、拆除、残留扫描与隔离任一不确定时关闭失败。

## 7. 出网策略 (Egress Policy)

所有外网请求必须经过策略判定。策略至少支持：允许域名、拒绝域名、允许 IP、拒绝 IP、DNS 策略、带宽上限、连接数上限、请求数上限。

要求：

- 域名级控制必须基于可信的 SNI/Host 判定，不得依赖可被篡改的客户端声明。
- 策略为默认拒绝：未明确允许的出网一律阻断，并产生可审计的阻断事实。
- 策略变更必须版本化、可回读、可审计；变更期间不得出现策略失效窗口。
- 出网策略与其他资源限制（CPU、Memory、Disk）独立执行与独立报告。
- 策略不得成为绕过端口暴露认证的通道。

## 8. 端口暴露

Agent 在 Sandbox 内监听的端口可以按策略对外提供访问。

要求：

- 暴露必须由显式策略授权，包含端口、协议、生命周期与撤销语义；默认不暴露任何端口。
- 外部访问端点必须由服务端生成，格式稳定、不可猜测，且**不得**直接暴露 Provider 私有地址、Node 地址或宿主端口。
- 外部访问必须经过统一边缘入口与认证，禁止绕过控制面直接访问 Node。
- 端口注册、暴露、转发与撤销必须幂等、可审计，并在 Sandbox 终止时确定性回收。
- 端口数量受租户配额约束。
- 端口暴露的公开端点命名属于公共契约，变更需要新的 `REQ-*`。

## 9. Agent Runtime（Sandbox 内托管组件）

Sandbox 内需要一个受控的托管组件，负责进程、文件系统、终端、端口、环境、MCP 与 Skills 的执行代理，并向 Sandbox 外部暴露受治理的能力面。

要求：

- 该组件属于 Sandbox 内部实现，不得成为公共 API 权威；外部调用者只能经 Sandbox 的 Capability 面访问。
- 组件不得直接访问 Control Plane 数据库，也不得持有超出其 Binding 的授权。
- 组件与 Sandbox 外部的通信必须使用 Sandbox 内通道（Unix Domain Socket / Vsock 或等价机制）；高频数据流不得经 Control Plane。
- 组件必须随 Sandbox 生命周期启停，并在 Sandbox 销毁时移除。
- 组件自身的供应链完整性必须纳入 [REQ-2026-0012](../requirements/REQ-2026-0012-sandbox-firecracker-artifact-compatibility-and-supply-chain.md) 的不可变元组。

当前状态：仓库无该组件的任何授权；`REQ-2026-0024` 明确把 Guest Agent Stream 列为未批准项。

## 10. MCP

Sandbox 必须能为 Kernel 拥有的 MCP 语义提供受治理的执行环境，支持 MCP Client、MCP Server 与 MCP Proxy 三种角色。

要求：

- MCP 语义、协议版本与工具编排由 `sdkwork-kernel` 拥有；Sandbox 只提供进程与网络执行能力。
- MCP 的所有出网必须经过本分片第 7 节的出网策略判定，禁止直接使用宿主网络。
- 不可信 MCP 必须运行在 MicroVM 运行模式，且必须显式声明最低隔离等级。
- MCP 的凭据与 Secret 遵循 [REQ-2026-0025](../requirements/REQ-2026-0025-sandbox-runtime-secret-projection.md) 的 value-free 授权模型，Sandbox 不持有 Secret 明文。

## 11. Skills

Sandbox 必须支持 Workspace 内的技能声明目录，使技能可被读取、执行，并可调用 MCP 与 Agent。

要求：

- 技能目录必须位于 Workspace 内固定位置并纳入 Workspace 持久语义。
- 技能执行等同于进程执行：受相同资源、网络、文件系统与审计约束；技能不得获得超出其 Binding 的能力。
- 技能可以声明所需 Capability 与出网策略，但**声明不等于授权**；授权由策略方与 Sandbox 准入共同决定。
- 技能的加载与执行必须可审计，包括来源、版本与执行结果。
- 技能内容的供应链与完整性属于独立议题，需在实现前确定 Owner。

当前状态：仓库无 Skills 定义；技能目录命名、加载规则与授权模型需独立 `REQ-*`。

## 12. SDK 消费面

第一阶段目标语言为 Rust、TypeScript 与 Python，三者必须共享同一契约与同一语义，不得各自定义方言。

要求：

- SDK 必须由生成器从权威契约产出，禁止手写与手工维护生成物；生成与目录规则遵循 `SDK_SPEC.md` 与 `SDK_WORKSPACE_GENERATION_SPEC.md`。
- SDK 的调用形态必须覆盖：创建 Sandbox、写文件、执行命令、读取结果、暂停/恢复、快照与清理。
- 每个调用必须携带幂等标识与超时；取消不得留下不确定的服务端状态。
- `int64` 标识在 TypeScript SDK 中必须保持字符串，禁止转为 `number`。
- SDK 不得暴露 Provider 私有身份、宿主路径或内部 Node 拓扑。
- 第一套 HTTP 控制面若获批必须是 `internal-api`，不得使用 `backend-api` 或自定义 `/api/*` 前缀；因此第一阶段 SDK 是内部 SDK 家族，公开商业 SDK 需独立评审。

## 13. 可观测性能力面

必须支持指标、日志、Trace、事件与审计，且四类数据的脱敏与保留策略相互独立。

必须提供的指标族至少包括：

```text
sandbox_create_latency
sandbox_resume_latency
sandbox_pause_latency
sandbox_fork_latency

cpu_usage
memory_usage
disk_usage
network_rx
network_tx

runtime_pool_size
runtime_pool_hit
runtime_pool_miss

snapshot_restore_latency
template_cache_hit
template_cache_miss
```

要求：

- 每个命令与生命周期操作必须携带服务端权威 `traceId`，并在身份存在时关联 `sandboxSessionId`、`sandboxWorkspaceId`、`sandboxId` 与 `sandboxRuntimeBindingId`；跨域关联可额外携带已授权的 `agentSessionId`/`agentWorkspaceId`。
- 指标标签必须低基数，不得包含原始租户、会话或节点标识。
- 日志与事件不得包含 Raw Token、Credential、Private Key、完整 Secret 环境、Provider 私有 Allocation 引用或宿主路径。
- 所有 Sandbox 操作必须产生审计事件，包括被拒绝的操作。
- 事件精确名称与 Schema 必须以 `apis/async/` 机器契约为权威，本分片不重复定义。

## 14. 未决门禁

- 端口暴露的公开端点命名与边缘入口归属由哪个仓拥有？
- Agent Runtime 组件的命名、打包形态与升级策略是什么？
- Skills 目录的权威命名与供应链 Owner 是谁？
- 公开商业 SDK 是否存在，还是长期只提供内部 SDK？
- `shared` 网络模式的启用条件与允许工作负载清单是什么？
- 交互终端的重放窗口与断连宽限期取值由谁批准？
