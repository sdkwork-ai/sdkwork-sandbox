# E2B 能力对齐审计

Status: active

Owner: SDKWork Runtime Platform

Updated: 2026-09-22

Parent: [SDKWork Sandbox Technical Architecture](TECH_ARCHITECTURE.md)

Specs: `REQUIREMENTS_SPEC.md`, `DOCUMENTATION_SPEC.md`, `QUALITY_GATE_SPEC.md`, `TEST_SPEC.md`, `PERFORMANCE_SPEC.md`

本分片以 E2B 公开文档的**逐能力清单**为基准，对本仓实现做四级状态对照，给出缺口优先级与解锁路径。产品级 34 行概览见 [PRD-capabilities.md](../../product/prd/PRD-capabilities.md) 第 11 节；本分片是它的**字段级展开**（78 行），不复制其结论，也不替代其权属判定。运行模式、Template、Snapshot、Fork 的产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md)。

## 0. 基准快照与状态口径

### 0.1 基准

| 项 | 值 |
| --- | --- |
| 基准产品 | E2B（`https://docs.e2b.dev`） |
| 索引快照 | `https://docs.e2b.dev/llms.txt`（全站页面索引，2026-09-22 抓取） |
| 索引覆盖 | 约 260 个页面路径，含 sandbox / template / filesystem / volumes / network / secrets / iam / commands / cli / sdk-reference / code-interpreting / mcp-gateway / agents / api-reference / byoc / faq |
| 引用 SDK 版本 | JS/TS `v2.38.2`、Python `v2.37.1`、Code Interpreter JS `v2.7.0` / Python `v2.9.0`、Desktop JS `v2.3.1` / Python `v2.4.2`、CLI `v2.16.1` |
| 字段级抓取深度 | 索引全量 + 8 个子页：`sandbox`、`sandbox/persistence`、`sandbox/snapshots`、`template/quickstart`、`filesystem/read-write`、`commands`、`network/internet-access`、`sandbox/metrics` |
| 基准完整性 | **不全**。`volumes`、`secrets`、`iam/workload-identity`、`sandbox/pty`、`sandbox/fork`、`sandbox/connect`、`code-interpreting/*`、`mcp-gateway/*`、`cli/*`、`byoc` 等约 250 页未逐页抓取。这些分类的行只按索引标题口径判定，字段名未逐字核对，一律标注 `基准仅索引`。 |

### 0.2 四级状态口径

| 标记 | 含义 |
| --- | --- |
| ✅ 完整对齐 | 本仓存在等价能力、有测试覆盖，且可被调用方端到端消费 |
| 🟡 部分 / 形态不同 | 仅有机器契约或领域服务候选实现，或只在某一分层存在、产品面缺失；必须写出差异是什么 |
| ❌ 未实现 | 本仓无对应实现 |
| ⛔ 刻意不做 | 本仓 Canon 明文不做；理由必须可取证，须引 Canon 原话 |

**本仓 ✅ 行数为 0。** 这不是口径过严，而是本仓的真实形状：所有"看起来有"的条目都是机器契约或领域服务候选，没有一个可被 Agent 端到端消费。任何一行在没有可点的实现点或测试全名之前，不得对外声明为已具备。

## 1. 结论速览

### 1.1 直接回答

**没有对齐。差距是结构性的，不是补几个函数能闭合的。**

E2B 让 Agent 执行的两条核心路径，本仓**一条都不可用**：

| 路径 | E2B 的形态 | 本仓现状 |
| --- | --- | --- |
| 快速创建 | `Sandbox.create()` 一次调用返回一个可执行命令的 Linux VM；配合 Template 的 start command，沙箱创建时进程**已在运行**，首命令零等待 | 无 HTTP/RPC 入口、无 CLI、无真实 Provider。`SandboxSessionLifecyclePort` 只是领域服务方法，调用方无处可调 |
| 快速部署环境 | `Template.build()` 预构建镜像 + 构建缓存 + `fromTemplate()` 层复用 + tags 版本化；模板即部署单元 | `Template` 在本仓**零承载**——无 `REQ-*`、无 `ADR`、无契约、无组件、无缓存。这是"快速部署"的全部基础设施 |

具体到三个数字：

- E2B 的能力集合共 **78 项**（本分片逐行展开），本仓 ✅ **0**、🟡 **16**、❌ **60**、⛔ **2**。
- 27 份 `REQ-*` 中 **0 份 `ready`**（5 `accepted` / 22 `draft`）；27 份 `ADR` **全部 `proposed`**；16 份机器契约 **全部 `implementationAuthorized: false`**。
- 8 份契约声明的 **127 个证据 id** 中，只有 **2 个**有 host-precondition 半产出，**125 个**仍被真实 runner 或人工评审完全阻塞。

因此本仓对用户画像的承诺（`PRD.md` 第 2 节"AI Agent 应用开发者：用少量代码获得一个可执行代码、可读写文件、可访问网络、可持久化的独立运行环境"）**当前为零兑现**。

### 1.2 本仓当前真实形状（可点证据）

| 组件 | 路径 | 规模 | 真实状态 |
| --- | --- | --- | --- |
| Provider SPI | `crates/sdkwork-sandbox-provider-spi` | 4 模块 | `SandboxProvider` trait 只有 `descriptor`/`health`/`allocate`/`start`/`stop`/`destroy`（`provider.rs:136`）；**无 pause/resume/snapshot** |
| Lifecycle Service | `crates/sdkwork-intelligence-sandbox-service` | 9 模块 | `SandboxSessionState` 只有 `Created/Starting/Running/Stopping/Stopped/Failed/Destroying/Destroyed`（`model.rs:13`）；**无 `Pausing/Paused/Recovering`** |
| Memory Repository | `crates/sdkwork-intelligence-sandbox-repository-memory` | test-only | 仅在测试中可用 |
| PostgreSQL Repository | `crates/sdkwork-intelligence-sandbox-repository-sqlx` | candidate | 4 张表：`sandbox_session` / `sandbox_session_operation` / `sandbox_runtime_binding` / `sandbox_session_lease`；**无 template / snapshot / pool / quota / node / event 表** |
| Local Provider | `crates/sdkwork-sandbox-provider-local` | 8 行 `lib.rs` | 整个 crate 是 `#[cfg(test)] mod fake_host_boundary;`（`lib.rs:7`）——**零生产实现** |
| Service Host | `crates/sdkwork-sandbox-service-host` | 5 行 | 只有 doc comment，**无 composition、无 wiring** |
| CLI | `crates/sdkwork-sandbox-cli` | 3 行 | `fn main() {}`（`main.rs:3`）——**零命令** |
| API Assembly | `crates/sdkwork-api-sandbox-assembly` | 骨架 | `ROUTE_CRATE_COUNT: usize = 0`（`generated.rs:3`）+ `Router::new()`（`bootstrap.rs:19`）——**零路由** |
| Command Executor | — | 不存在 | 全仓无 `SandboxCommandExecutor`，`apis/commands/*.json` 只有契约 |
| Template / Snapshot / Fork / Pool | — | 不存在 | 全仓无对应实现，也无产品级 `REQ-*` |
| SDK | `sdks/` | 目录 + README | **零生成产物**，`apis/` 无权威 OpenAPI |

### 1.3 分类四态计数

| E2B 分类 | 行数 | ✅ | 🟡 | ❌ | ⛔ |
| --- | --- | --- | --- | --- | --- |
| Sandbox 生命周期 | 11 | 0 | 3 | 8 | 0 |
| 持久化（Pause / Resume） | 8 | 0 | 1 | 7 | 0 |
| Snapshot 与 Fork | 5 | 0 | 0 | 5 | 0 |
| Template | 9 | 0 | 1 | 7 | 1 |
| Filesystem | 7 | 0 | 0 | 7 | 0 |
| Volumes | 4 | 0 | 2 | 2 | 0 |
| Commands 与 Process | 5 | 0 | 1 | 4 | 0 |
| PTY | 1 | 0 | 1 | 0 | 0 |
| Code Interpreter | 3 | 0 | 0 | 3 | 0 |
| Network | 8 | 0 | 2 | 6 | 0 |
| Secrets 与 IAM | 3 | 0 | 1 | 2 | 0 |
| Metrics 与 Telemetry | 3 | 0 | 1 | 2 | 0 |
| CLI | 1 | 0 | 0 | 1 | 0 |
| SDK | 4 | 0 | 0 | 4 | 0 |
| MCP Gateway | 1 | 0 | 0 | 1 | 0 |
| 平台与部署 | 4 | 0 | 3 | 1 | 0 |
| Agent 框架集成 | 1 | 0 | 0 | 0 | 1 |
| **合计** | **78** | **0** | **16** | **60** | **2** |

## 2. 逐项对照

`基准仅索引` 表示该行只依据 `llms.txt` 的页面标题口径判定，未逐字核对字段名。

### 2.1 Sandbox 生命周期

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 1 | `Sandbox.create()`（template / `envs` / `metadata` / `timeoutMs` / `network` 参数） | `SandboxSessionLifecyclePort::create_sandbox_session`（`port.rs:10`）+ 4 张 PG 表 | 🟡 | 领域服务候选；无 Provider 实现、无入口。E2B 的 `metadata` 在本仓**无对应字段** |
| 2 | `Sandbox.connect()`（暂停自动恢复；TTL 只延长不缩短） | 无 | ❌ | — |
| 3 | `setTimeout()` / `keepAlive`（运行中改 TTL） | 无 | ❌ | 本仓只有 **Lease** 过期时间（`repository.rs:57`），语义是生命周期控制权租约，不是沙箱 TTL，别混为一谈 |
| 4 | `getInfo()`（`templateId`/`name`/`metadata`/`startedAt`/`endAt`） | 无 | ❌ | `get_sandbox_session` 只返回领域聚合，无查询 API，无 metadata |
| 5 | `kill()` | `destroy_sandbox_session` → `Destroyed` 终态 | 🟡 | 领域候选；无入口 |
| 6 | `Sandbox.list()`（`state`/filter + paginator） | 无 | ❌ | — |
| 7 | Lifecycle events API（事件流） | `apis/async/sandbox-events.asyncapi.json` + `sandbox-event-catalog.json` 契约 | 🟡 | 仅契约，无 runtime exporter/worker |
| 8 | Lifecycle webhooks | 无 | ❌ | 基准仅索引 |
| 9 | Auto-resume on request | 无 | ❌ | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 9 节；无 `REQ-*` |
| 10 | SSH access（WebSocket 代理） | 无 | ❌ | 基准仅索引 |
| 11 | Secured access / 访问令牌门控 | 无 | ❌ | 本仓有 `SandboxFencingToken`，对象是控制权竞争而非访问面，形态不同 |

### 2.2 持久化（Pause / Resume）

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 12 | `pause()`（同时保存文件系统**与内存**） | `REQ-2026-0008`/`REQ-2026-0021` 门禁（draft） | ❌ | PRD 状态机含 `Pausing/Paused`，实现枚举**没有**；`SandboxProvider` trait 无 `pause` |
| 13 | Filesystem-only pause（`keepMemory: false`） | 无 | ❌ | — |
| 14 | `connect()` 恢复（热恢复） | 无 | ❌ | — |
| 15 | Reboot-on-resume（`resume-without-memory`） | 无 | ❌ | — |
| 16 | Paused 无限期保留、无 TTL、无自动删除 | 无 | ❌ | **形态相反**：`REQ-2026-0020` 定义的是**有界**热状态投影与终态保留窗口，不是无限期保留 |
| 17 | Auto-pause on timeout（`onTimeout: 'pause'`） | 无 | ❌ | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 9 节；无独立 `REQ-*` |
| 18 | Pause 被拒语义（HTTP 503 `ServiceBusyError`，沙箱保持运行可重试） | `SandboxLifecycleError::LeaseUnavailable` / `LeaseLost` | 🟡 | 形态不同：本仓的拒绝对象是**生命周期控制权竞争**，不是快照拥塞；E2B 那套快照背压语义本仓无对应 |
| 19 | 暂停/恢复性能承诺（约 4 s/GiB RAM；恢复约 1 s） | 无 | ❌ | [TECH-performance-and-capacity.md](TECH-performance-and-capacity.md) 有恢复时延目标，但无参考硬件与测量 |

### 2.3 Snapshot 与 Fork

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 20 | `createSnapshot()`（含内存与文件系统；原沙箱短暂暂停后继续，ID 不变） | 无产品级能力 | ❌ | 仅有 Workspace Checkpoint 与 Firecracker Snapshot 的 Gate 0 候选（`REQ-2026-0021`、`REQ-2026-0008`） |
| 21 | `Sandbox.create(snapshotId)`（从快照派生沙箱） | 无 | ❌ | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 5 节 |
| 22 | `listSnapshots()` / `deleteSnapshot()` | 无 | ❌ | — |
| 23 | `fork`（一次调用在原地快照并派生 N 个沙箱） | 无 | ❌ | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 6 节；无 `REQ-*`、无 `ADR`、一致性语义未定 |
| 24 | Snapshot 与原沙箱并行运行、一个快照派生多个 | 无 | ❌ | — |

### 2.4 Template

Template 是 E2B"快速创建 + 快速部署"的**唯一基础设施**：预构建镜像 + start command 常驻 + 构建缓存 + 层复用。本仓整类零承载。

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 25 | 声明式 Template 定义（`Template().fromBaseImage()` / `fromTemplate()` / `copy()` / `setEnvs()` / `setStartCmd()`） | **无任何承载** | ❌ | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 4 节；`REQ-*` 为零 |
| 26 | `e2b template init` / `build` / `deploy` | 无 CLI | ❌ | `crates/sdkwork-sandbox-cli/src/main.rs:3` = `fn main() {}` |
| 27 | Start / Ready command（沙箱创建时长驻进程**已在运行**，首命令零等待） | 无 | ❌ | — |
| 28 | 构建缓存与层级复用（`fromTemplate()` 复用已缓存基础层） | 无 | ❌ | PRD 第 4 节要求 Template 缓存 Hot/Warm/Cold + 淘汰策略；无 `REQ-*` |
| 29 | Template tags / versioning / names | 无 | ❌ | — |
| 30 | Base image / 私有 registry 接入 | 无 | ❌ | 基准仅索引 |
| 31 | 构建限额（1 h / 8 vCPU / 8 GiB / 10 GiB / 20 并发） | 无 | ❌ | — |
| 32 | 以 Dockerfile 或构建脚本作为**构建输入** | 无（产品要求已写） | 🟡 | [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 4 节已写"构建输入允许使用 Dockerfile 或构建脚本"；无 `REQ-*` |
| 33 | 以 Docker 作为运行时依赖或隔离边界 | 明确不做 | ⛔ | `PRD.md` 非目标原话："不把 Docker 作为运行时依赖或隔离边界；Docker 只允许作为 Template 的构建输入格式" |

### 2.5 Filesystem

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 34 | `files.read()` 单文件读取 | 无 | ❌ | `REQ-2026-0007` 只有命令契约，无文件系统端口 |
| 35 | `files.write()` / `writeFiles()` 批量写入 | 无 | ❌ | — |
| 36 | `files.getInfo()` / stat / 存在性 | 无 | ❌ | — |
| 37 | 文件自定义 metadata（上传时 `X-Metadata-<key>` → xattr） | 无 | ❌ | 基准仅索引 |
| 38 | `files.watch()` / `WatchDir` 变更流 | 无 | ❌ | 基准仅索引 |
| 39 | upload / download（含目录、`X-Metadata-` 头） | 无 | ❌ | 基准仅索引 |
| 40 | `listDir` / `makeDir` / `move` / `remove` | 无 | ❌ | 基准仅索引 |

### 2.6 Volumes

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 41 | Volume 创建 / 列举 / 检视 / 销毁（独立于沙箱生命周期的持久存储） | `REQ-2026-0013` Workspace Block Device（draft） | 🟡 | 形态不同：本仓的字节权威在 Drive / 批准的 Volume Authority，语义权威在 `sdkwork-agents`，Sandbox 不拥有 Volume |
| 42 | 创建沙箱时把 Volume 挂载到自定义路径 | `REQ-2026-0004` Agents Workspace Attachment（accepted） | 🟡 | 只有 Attachment 边界；无挂载参数面、无入口 |
| 43 | Volume 内读写 / 上传 / 下载 | 无 | ❌ | 基准仅索引 |
| 44 | Volume 间迁移（挂载两个 Volume + rsync） | 无 | ❌ | 基准仅索引 |

### 2.7 Commands 与 Process

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 45 | `commands.run()`（`envs` / `cwd` / `user` / `timeoutMs`） | `apis/commands/sandbox-command-contract.json`（`implementationAuthorized: false`） | 🟡 | 仅契约：全仓无 `SandboxCommandExecutor`。契约的 `executionModes` 只有 `executable-argv`，`forbiddenExecutionModes` 显式禁止 `shell-string` |
| 46 | 流式 stdout/stderr（`onStdout`/`onStderr`） | 无 | ❌ | — |
| 47 | 后台进程（`background: true` + `commands.list` + `commands.kill`） | 无 | ❌ | — |
| 48 | stdin 输入 / `CloseStdin` | 无 | ❌ | 基准仅索引 |
| 49 | `SendSignal` / 进程 `Update` | 无 | ❌ | 基准仅索引 |

### 2.8 PTY

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 50 | Interactive terminal（PTY） | `specs/sandbox-interactive-terminal-session.contract.json` | 🟡 | 仅契约，且 `materialization` 显式禁止物化：`ptyOrConptyAllowed: false`、`processSpawnAllowed: false`、`rustPortOrTypesAllowed: false`（`REQ-2026-0024`） |

### 2.9 Code Interpreter

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 51 | `runCode()` / 代码上下文（`contexts`） | 无 | ❌ | 基准仅索引 |
| 52 | 多语言执行（python / js / ts / r / java / bash） | 无 | ❌ | 基准仅索引 |
| 53 | 图表与可视化预置库 | 无 | ❌ | 基准仅索引 |

### 2.10 Network

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 54 | 出网开/关（`allowInternetAccess`，默认开启） | `REQ-2026-0014` 的 `DenyAll` 门禁（draft） | 🟡 | 契约方向**相反**：本仓默认拒绝，E2B 默认允许。仅契约，无 network runtime |
| 55 | allow / deny 列表（IP / CIDR / 域名 / 通配） | `specs/sandbox-firecracker-network-isolation.contract.json`（draft） | 🟡 | 仅契约 |
| 56 | per-host rules / header 注入（`network.rules`，public beta） | 无 | ❌ | 基准仅索引 |
| 57 | 运行中 `updateNetwork`（替换式，不合并） | 无 | ❌ | — |
| 58 | 端口暴露（public URL / `getHost`） | 无 | ❌ | 产品要求见 [PRD-sandbox-surfaces.md](../../product/prd/PRD-sandbox-surfaces.md) 第 8 节；无 `REQ-*` |
| 59 | 限制公开访问（`allowPublicTraffic` / `maskRequestHost` / `httpsPorts`） | 无 | ❌ | 基准仅索引 |
| 60 | 自定义域名 | 无 | ❌ | 基准仅索引 |
| 61 | 出网代理隧道 / BYOP SOCKS5 | 无 | ❌ | 基准仅索引 |

### 2.11 Secrets 与 IAM

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 62 | Secret 存储（create / update / delete / list / rotate；**无读值面**） | `REQ-2026-0025` value-free opaque grant（draft） | 🟡 | 仅契约，无 Secret Authority |
| 63 | 出网代理注入 Secret（值不进沙箱） | 无 | ❌ | 基准仅索引 |
| 64 | Workload identity（JWT-SVID 短期令牌） | 无 | ❌ | 基准仅索引 |

### 2.12 Metrics 与 Telemetry

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 65 | `getMetrics()`（`cpuUsedPct` / `cpuCount` / `memUsed` / `memTotal` / `diskUsed` / `diskTotal`，5 s 采样） | `apis/async/sandbox-observability-catalog.json`（32 个指标契约） | 🟡 | 仅契约，无 runtime。且该契约与 [PRD-sandbox-surfaces.md](../../product/prd/PRD-sandbox-surfaces.md) 第 13 节的指标族名**三向不相交**（3 直接对应 / 3 部分对应 / 9 无对应），两个清单之间无门禁比对 |
| 66 | Team 级 metrics | 无 | ❌ | 基准仅索引 |
| 67 | OTel telemetry export | 无 | ❌ | 基准仅索引 |

### 2.13 CLI

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 68 | `e2b` CLI（auth / sandbox list·create·connect·fork·exec·shutdown / template init·build / snapshot / metrics） | `crates/sdkwork-sandbox-cli` | ❌ | `main.rs:3` = `fn main() {}`；零命令、零参数解析 |

### 2.14 SDK

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 69 | 官方 JS/TS + Python SDK（同步/异步） | `sdks/` 目录 | ❌ | 只有 README，零生成产物；`apis/` 无权威 OpenAPI，`ROUTE_CRATE_COUNT: 0` |
| 70 | Code Interpreter SDK / Desktop SDK | 无 | ❌ | 基准仅索引 |
| 71 | 分页器与错误类型族（`ServiceBusyError` 等） | 无 | ❌ | — |
| 72 | 多语言同语义 SDK 的生成链 | `PRD.md` 目标已写 | ❌ | `PRD.md` 目标列出"Rust、TypeScript、Python 三语言同语义 SDK 消费面，由权威契约生成，不手写方言"；但 `apis/` 无权威 OpenAPI，`sdks/` 零生成产物 |

### 2.15 MCP Gateway

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 73 | MCP Gateway（200+ servers / custom templates / custom servers） | 仅 Transport 级 | ❌ | [PRD-capabilities.md](../../product/prd/PRD-capabilities.md) 第 5 节只有传输级描述；无独立 `REQ-*` |

### 2.16 平台与部署

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 74 | BYOC（AWS / GCP，托管部署） | 无 | ❌ | `deployments/` 有 8 个 profile 配置，`sandbox.cloud-data-residency.contract.json` 为 draft，无部署产物 |
| 75 | 多区域（EU cluster） | `REQ-2026-0026` region tuple（draft） | 🟡 | 仅契约：`regionCode` / `providerRegion` / `storageRegion` / `availabilityZone` |
| 76 | 计划与限额（并发沙箱、vCPU、内存、磁盘、连续运行时长） | `REQ-2026-0015` / `0016` / `0018`（draft） | 🟡 | 仅契约，无 quota runtime |
| 77 | 合规（SOC 2 Type II、静态加密、DPA） | `REQ-2026-0006` 分配元数据加密有**候选实现 + 测试** | 🟡 | 加密机制存在（`repository-sqlx/src/encryption.rs`），但平台合规证据本仓不拥有 |

### 2.17 Agent 框架集成

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 78 | 第三方 Agent 框架官方适配（25+：Claude Code / Codex / CrewAI / Mastra / …） | 定位差异 | ⛔ | `PRD.md` 非目标原话："不负责 Prompt、Model、Agent 推理、对话语义、Agent Provider SDK 或 Provider 特有 Agent 行为"、"不替代 `sdkwork-kernel` 的工具编排或 MCP 协议语义"。依赖方向固定为 `sdkwork-agents -> sdkwork-kernel -> sdkwork-sandbox`，本仓不直连框架 |

## 3. 测试覆盖矩阵

### 3.1 已实现面的覆盖（本仓全部真实实现）

| 实现面 | 实现点 | 测试全名 |
| --- | --- | --- |
| Provider descriptor 能力/隔离等级 fail-closed | `crates/sdkwork-sandbox-provider-spi/src/provider.rs:12` | `provider::tests::sandbox_provider_descriptor_fails_closed_on_capability_and_assurance` |
| Readiness 要求 Workspace Attachment + Policy 生效 | `crates/sdkwork-sandbox-provider-spi/src/provider.rs:101` | `provider::tests::sandbox_provider_readiness_requires_workspace_attachment_and_policy_enforcement` |
| Tenant 标识拒绝 path-like 取值 | `crates/sdkwork-sandbox-provider-spi/src/identity.rs:43` | `identity::tests::rejects_path_like_tenant_identifiers` |
| Provider Allocation 引用 Debug 脱敏 | `crates/sdkwork-sandbox-provider-spi/src/identity.rs:110` | `identity::tests::sandbox_provider_reference_debug_output_is_redacted` |
| Fencing Token 拒绝 0 与有符号上溢 | `crates/sdkwork-sandbox-provider-spi/src/identity.rs:88` | `identity::tests::sandbox_fencing_token_rejects_zero_and_signed_maximum_overflow` |
| Lifecycle 幂等/Lease/Fencing/Readiness | `crates/sdkwork-intelligence-sandbox-service/src/service.rs` | `cargo test -p sdkwork-intelligence-sandbox-service` |
| Local Fake Host Boundary | `crates/sdkwork-sandbox-provider-local/src/fake_host_boundary/mod.rs` | `cargo test -p sdkwork-sandbox-provider-local` |

计数（2026-09-22 实测）：

```bash
cargo test --workspace
```

`63 passed / 1 ignored`（1 ignored 是声明需要外部 PostgreSQL 的测试）。契约测试：

```bash
node --test tests/contract/*.test.mjs
```

`406 pass / 0 fail`（含本轮新增的 22 个）。

### 3.2 覆盖空档

按"先高价值后低价值"的优先级：

| 优先级 | 空档 | 说明 |
| --- | --- | --- |
| 1 | **已实现面没有任何消费点测试** | 5 个 SPI 测试全部落在谓词/构造器上。`allocate`/`start`/`stop`/`destroy` 的**真实 Provider 调用序列**没有任何实现可测——因为 Provider 不存在。这是本仓最深的空档 |
| 2 | **PRD 状态机 ⊋ 实现状态机** | [PRD-capabilities.md](../../product/prd/PRD-capabilities.md) 第 3 节的规范状态机含 `Pausing / Paused / Recovering`，实现枚举（`model.rs:13`）没有这三个，且 PRD 该处**没有任何"未实现/目标态"标记**。REQ-2026-0002 的 scope 其实只管 create/start/stop/destroy，所以是 PRD 图缺标记，不是实现缺状态 |
| 3 | **指标契约与指标族名不相交** | `apis/async/sandbox-observability-catalog.json`（32 指标）与 [PRD-sandbox-surfaces.md](../../product/prd/PRD-sandbox-surfaces.md) 第 13 节（15 指标族）名字集完全不相交，9 个族无任何对应。且 PRD 的 `*_latency` 命名违反 `OBSERVABILITY_SPEC.md` 第 57 节"Duration 指标名必须含单位，通常 `_duration_seconds`"。**无门禁比对两份清单** |
| 4 | **`PRD-capabilities.md` 第 11 节与 E2B 基准之间仍无门禁** | 本分片的 78 行已由 `tools/check-sandbox-e2b-parity-matrix.mjs` 自我校验（词表 / 编号 / 分类对应 / 普查算术 / 引用解析 / 登记），但第 11 节那 34 行到 E2B 的映射仍只存在于散文，会随基准演进而静默腐化 |
| 5 | **"快速创建/快速部署"的性能断言全为零测试** | [TECH-performance-and-capacity.md](TECH-performance-and-capacity.md) 与 `PRD.md` 第 6 节的 500 ms 热分配、恢复时延、Template 缓存命中率等指标，既无参考硬件也无 Benchmark 套件（`REQ-*` 为零） |

**本轮没有新增实现用例**，因为没有获批的实现可测：16 份机器契约全部 `implementationAuthorized: false`，8 类未授权能力被 `PRD.md` 第 8 节明文列入"尚无任何 `REQ-*` 承载"。在实现授权到位前写"用例"只能写成断言契约文本，属于假门禁。

### 3.3 本轮新增的门禁与用例（含变异结果）

矩阵本身是一个会被反复引用的**数字**，而本仓此前没有任何东西读它。本轮补上：

| 新增物 | 内容 | 用例数 | 变异自证 |
| --- | --- | --- | --- |
| `tools/check-sandbox-e2b-parity-matrix.mjs` | 6 条规则族：词表 / 编号与形状 / 分类对应 / 普查算术 / 引用解析 / 登记 | — | 见下 |
| `tests/contract/e2b-parity-matrix-tool.contract.test.mjs` | 22 个用例，每个规则族各有一条能变红的反面用例 | 22 | 22/22 pass |

变异自证（2026-09-22 实测，落盘改后跑，跑完还原并核验）：

```bash
node tools/check-sandbox-e2b-parity-matrix.mjs
```

把合计行 `| **合计** | **78** | **0** | **16** | **60** | **2** |` 的第二个数字改成 `**1**`（只脏这一个点）后，门禁 **exit 1** 并给出三条各自独立的诊断：与分类行求和不符、与矩阵逐行重算不符、合计行不再是 `N` 的划分。还原后 **exit 0**，文档 SHA256 前缀 `d654a2095335acaa398f46c7b687eb2a8477266ae76c45fc553dcdd77392c7ba` 与改前逐字节一致。

**门禁自身踩到并修掉的静默漏洞**：合计行写作 `| **合计** | **78** | … |`，初版解析器用 `^\d+$` 判行，星号使其不被识别为合计行 ⇒ 全部合计断言被跳过，门禁把**重算值**当作**已核对值**打印。这正是本仓最在意的那类缺陷（"看着通过、实际什么都没查"），已改为先剥离 Markdown 强调再解析，并加"没有合计行即失败"与"计数非数字即报告而非静默丢弃"两条规则锁住。

## 4. 缺口清单

每条标注改动性质：**纯增量**（新增能力，不动既有模型）或**设计级**（需要新的权属模型、共享类型变更或跨仓契约，成本差一个数量级）。

### P0 — 阻塞"能创建任何沙箱"

| 缺口 | 性质 | 说明 |
| --- | --- | --- |
| 零运行入口（无 HTTP/RPC、无 CLI、无 Service Host wiring） | 设计级 | `ROUTE_CRATE_COUNT: 0`、`fn main() {}`、service-host 5 行。需要 `REQ-2026-0023`（internal control plane）与 `REQ-2026-0009`（service host）进入 `ready` |
| 零真实 Provider（Local 只有 fake host boundary） | 设计级 | `REQ-2026-0003` 的 5 条 Readiness Blocker 全是人工评审/接受 |
| 无 Template（含定义、构建、缓存、tags、start command） | **设计级** | E2B 快速创建与快速部署的**全部**依赖它。本仓零 `REQ-*`；与 Firecracker 制品元组的权威边界未定（见 `PRD.md` 第 9 节待决问题） |
| 无 Command / Terminal / Filesystem 执行面 | 设计级 | `REQ-2026-0007`、`REQ-2026-0024` 仅契约且显式禁止物化 |

### P1 — 阻塞"创建得快"

| 缺口 | 性质 | 说明 |
| --- | --- | --- |
| 无 Runtime Pool（`PreparedSlot` / `WarmMicroVmSlot` / fenced Claim） | 设计级 | `REQ-2026-0019`（draft）。无 Pool 则每次都是冷启动，"快"无从谈起 |
| 无 Snapshot / Fork（含 `Sandbox.create(snapshotId)`） | 设计级 | 产品要求已写，`REQ-*` 为零；Fork 一致性语义未定 |
| 无 Pause / Resume（含 fs-only 与 reboot-on-resume） | 设计级 | Provider trait 无 `pause`/`resume`；实现状态枚举无 `Paused` |
| 无 Auto Pause / Auto Resume（Idle 收敛） | 纯增量 | 依赖可观测事实组合，而非单一定时器 |
| 无构建缓存与层复用（`fromTemplate` 等价物） | 设计级 | 需要 Template + 三级存储分层 + 对等缓存协调 |

### P2 — 阻塞"Agent 真能在里面干活"

| 缺口 | 性质 | 说明 |
| --- | --- | --- |
| 无 Network Policy / Egress / Port 暴露运行时 | 设计级 | `REQ-2026-0014` 仅契约；`shared` 模式与端口暴露归属未定 |
| 无 Secret 注入与 Workload Identity | 设计级 | `REQ-2026-0025` 仅契约；值通道与 process projection 未批准 |
| 无 Metrics / OTel / 事件 runtime | 纯增量 | 契约已有，缺 exporter/worker/migration |
| 无 SDK 家族（Rust / TS / Python 同语义） | 设计级 | `apis/` 无权威 OpenAPI；需先有 internal-api 契约 |
| 无 MCP 执行面与 Skills | 设计级 | 仅传输级描述，无 `REQ-*` |

### P3 — 平台与集成

| 缺口 | 性质 | 说明 |
| --- | --- | --- |
| 无 BYOC / 多区域 / 限额执行 | 设计级 | `REQ-2026-0026` 与 `0015`/`0016`/`0018` 均为 draft |
| 无 Code Interpreter / Desktop / Browser 能力面 | 设计级 | PRD 非目标明确 Browser 与 Computer Use 不在第一阶段 |
| 无性能基准套件与参考硬件基线 | 纯增量 | 所有性能数字都是工程目标，无测量则不能写入 Release Evidence |

### 解锁路径（唯一路径，且是人工决策）

本仓不是"有些功能没做完"，而是**治理门禁未打开**。四条硬门禁互相依赖：

1. 27 份 `REQ-*` 中 0 份 `ready`（5 `accepted` / 22 `draft`）→ 需逐份人工评审进 `ready`。
2. 27 份 `ADR` 全部 `proposed` → 需 `accepted`。
3. 16 份机器契约全部 `implementationAuthorized: false` → 需人工评审签字后翻转。
4. 8 份契约声明的 127 个证据 id 中 125 个无产出者 → 需真实 runner 与人工评审闭合。

当前签字积压（机器读数）：28 份评审记录中 22 份为 `pending-human-review`，其中 **14 份被机器契约点名门控**。完整清单与 5 步签字程序见 [human-review-signoff-backlog.md](../../engineering/human-review-signoff-backlog.md)。

`node tools/check-sandbox-commercial-readiness.mjs` 输出 `NO-GO`：6 个交付切片 blocked、5 个缺 `ready` 契约、4 个跨仓权威 blocked。这是**预期行为**，不是缺陷；任何把它读成"就差一点"的解释都是错的。

## 5. 我们比 E2B 强的地方

这一节对决策同样重要：以下能力是本仓**已有**、E2B 公开文档中**没有等价承诺**的，属于应当保留而不是在追赶中丢掉的差异。

| 本仓能力 | 证据 | 为什么保留 |
| --- | --- | --- |
| 单写者 Lease + 单调 Fencing Token 防止双重活动所有权 | `crates/sdkwork-sandbox-provider-spi/src/identity.rs:88`、`service.rs`、`sandbox_session_lease` 表 | E2B 未公开等价机制。多控制器竞争下的 Provider 副作用去重是自建平台必须自证的 |
| 稳定 `sandbox_operation_sequence` + 恢复重放校验 + 幂等 ledger | `crates/sdkwork-intelligence-sandbox-service/src/model.rs:24`、`REQ-2026-0020` | 恢复时先重放 Create/Start/Stop/Destroy 并校验组合，非法组合关闭失败。E2B 不对外承诺这一层 |
| Tenant-scoped 加密的 Provider 恢复元数据 + 有界密钥轮换/重加密 | `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/encryption.rs`、`REQ-2026-0006` | 已有候选实现**与测试**，是本仓少数可点的实现面 |
| Provider 无关 SPI + fail-closed Capability/IsolationAssurance 协商 | `provider.rs:50`、`capability.rs:2` | 禁止静默降级到更弱隔离；E2B 是单一 microVM 层，不存在这层协商 |
| 显式运行模式分层（Shared / Namespace / MicroVM）+ 禁止回退 | [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 2 节 | 成本分层能力；E2B 只有一种隔离强度 |
| 数据驻留与恢复 Gate（Local `device-local-persistence` / Cloud region tuple） | `REQ-2026-0022`、`REQ-2026-0026` | 企业私有化与合规场景的硬要求 |
| 工作区业务权威在 `sdkwork-agents`，Sandbox 只拥有运行投影 | `REQ-2026-0004`、`ADR-20260728-agents-workspace-and-sandbox-attachment-ownership` | `Workspace ≠ Sandbox` 不等式：销毁执行环境不连带销毁用户数据 |
| 跨仓不可变 Release Set 与多维兼容矩阵 | `REQ-2026-0027` | 四仓联合发布的可追溯性 |
| 127 个证据 id 的机器可读注册表 + 双向漂移门禁 | `specs/sandbox-real-evidence-registry.json`、`tools/check-sandbox-evidence-traceability.mjs` | 把"声称完成"与"有证据"分开，是本仓最重要的自证机制 |

在这些维度上，本仓的设计**比 E2B 更严**。问题不在设计，在于**没有一行运行时代码把它们跑起来**。

## 6. 复核方式

```bash
node tools/check-sandbox-requirement-traceability.mjs
node tools/check-sandbox-evidence-traceability.mjs
node tools/check-sandbox-human-review-signoff.mjs
node tools/check-sandbox-commercial-readiness.mjs
cargo test --workspace
node --test tests/contract/*.test.mjs
```

全局标准在 `../sdkwork-specs/` 下保持权威，本分片只引用不复制。
