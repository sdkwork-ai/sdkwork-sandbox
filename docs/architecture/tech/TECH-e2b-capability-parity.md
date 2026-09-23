# E2B 能力对齐审计

Status: active

Owner: SDKWork Runtime Platform

Updated: 2026-09-23

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
| 字段级抓取深度 | 索引全量 + 99 个文档页 + 公开 OpenAPI 文档（`openapi-public.yaml`，57 paths / 71 operations）。最初只抓 8 个子页（`sandbox`、`sandbox/persistence`、`sandbox/snapshots`、`template/quickstart`、`filesystem/read-write`、`commands`、`network/internet-access`、`sandbox/metrics`）——那是最初的抽样，不是当前深度 |
| 基准完整性 | **全量逐页取证**。78 行由 `specs/sandbox-e2b-capability-baseline.json` 承载：101 个来源（openapi 1 · 索引 1 · 文档页 99）各自记录 url、抓取时间、字节数与 sha256；逐行给出 operationId 与 schema 字段名、SDK 方法名、CLI 命令式或页面小节标题。字段名不再只按索引标题口径判定。快照时间 `2026-09-22T09:36:31Z`；重抓命令见该清单的 `recaptureCommand`。 |
| 操作面覆盖 | 71 个 operation 中 **70 个**被某条矩阵行以 operationId 引用，仅 **1 个**（`getHealth`，控制面存活探针）登记为"本审计未枚举"并给出理由。逐操作记账在该清单的 `operationCoverage`，**双向互斥**：既未引用也未登记 = 红，既引用又登记为未判定 = 也红。见第 4 节。 |

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
| 快速部署环境 | `Template.build()` 预构建镜像 + 构建缓存 + `fromTemplate()` 层复用 + tags 版本化；模板即部署单元 | `Template` 在本仓**零承载**——无 `REQ-*`、无 `ADR`、无契约、无组件、无缓存。这是"快速部署"的全部基础设施〔§3.4/1〕 |

具体到三个数字：

- E2B 的能力集合共 **78 项**（本分片逐行展开），本仓 ✅ **0**、🟡 **16**、❌ **60**、⛔ **2**。
- 28 份 `REQ-*` 中 **3 份 `ready`**（5 `accepted` / 20 `draft`）；28 份 `ADR` 中 25 份 `proposed`（3 份 `accepted`）；机器契约授权状态：23 份 `*.contract.json` 中 21 份显式声明 `implementationAuthorized: false`、**1 份已授权实现**——`specs/sandbox-local-provider-host-boundary.contract.json`（其人审 packet 已于 2026-09-24 由仓库所有者全部评审角色签署），第 23 份 `specs/sandbox-commercial-readiness.contract.json` 是发布决定记录而非能力契约，它没有该字段、但独立声明 `runtimeImplementationAuthorizationGranted: false` 且 `releaseDecision.status: "no-go"`（缺字段在 `check-sandbox-human-review-signoff.mjs` 里按未授权处理，该处用 `value.implementationAuthorized === true` 判定）；另有两份不以 `.contract.json` 命名的机器契约：`apis/commands/sandbox-command-contract.json` 已于 2026-09-24 随 `REQ-2026-0007` 进入 `ready` 授权实现，`apis/async/sandbox-observability-catalog.json` 仍为 `false`。
- 8 份契约声明的 **127 个证据 id** 中，只有 **2 个**有 host-precondition 半产出，**125 个**仍被真实 runner 或人工评审完全阻塞。

因此本仓对用户画像的承诺（`PRD.md` 第 2 节"AI Agent 应用开发者：用少量代码获得一个可执行代码、可读写文件、可访问网络、可持久化的独立运行环境"）**当前为零兑现**。

### 1.2 本仓当前真实形状（可点证据）

| 组件 | 路径 | 规模 | 真实状态 |
| --- | --- | --- | --- |
| Provider SPI | `crates/sdkwork-sandbox-provider-spi` | 5 模块 | `SandboxProvider` trait 只有 `descriptor`/`health`/`allocate`/`start`/`stop`/`destroy`（`provider.rs:136`）；**无 `pause`/`resume`/`snapshot`** |
| Lifecycle Service | `crates/sdkwork-intelligence-sandbox-service` | 9 模块 | `SandboxSessionState` 只有 `Created/Starting/Running/Stopping/Stopped/Failed/Destroying/Destroyed`（`model.rs:13`）；**无 `Pausing/Paused/Recovering`** |
| Memory Repository | `crates/sdkwork-intelligence-sandbox-repository-memory` | 1 模块 | 内存适配器编译在树中，但**无任何消费点**：无 crate 在 `Cargo.toml` 里依赖它，也无测试引用它。这条消费空档即 §3.2 第 1 行登记的治理阻塞 |
| PostgreSQL Repository | `crates/sdkwork-intelligence-sandbox-repository-sqlx` | candidate | 4 张表：`sandbox_session` / `sandbox_session_operation` / `sandbox_runtime_binding` / `sandbox_session_lease`；**无 template / snapshot / pool / quota / node / event 表** |
| Local Provider | `crates/sdkwork-sandbox-provider-local` | 15 行 `lib.rs` | 生产模块 `pub mod host_boundary;`（`lib.rs:12`，2026-09-24 授权切片：纯数据边界规则，无 IO 无 spawn）；Fake 边界保留为 `mod fake_host_boundary;`（`lib.rs:15`） |
| Service Host | `crates/sdkwork-sandbox-service-host` | 5 行 | 只有 doc comment（`crates/sdkwork-sandbox-service-host/src/lib.rs`），**无 composition、无 wiring** |
| CLI | `crates/sdkwork-sandbox-cli` | 3 行 | `fn main() {}`（`main.rs:3`）——**零命令** |
| API Assembly | `crates/sdkwork-api-sandbox-assembly` | 骨架 | `ROUTE_CRATE_COUNT: usize = 0`（`generated.rs:3`）+ `Router::new()`（`bootstrap.rs:26`）——**零路由** |
| Command Executor | — | 不存在 | `crates/**/*.rs` 里没有 `SandboxCommandExecutor` 实现；它只被 `crates/sdkwork-sandbox-service-host/specs/sandbox-service-host-composition.contract.json` 声明为 `sandbox_required_bindings` 的一项，即**已声明、未绑定**。`apis/commands/*.json` 只有契约 |
| Template / Snapshot / Fork / Pool | — | 不存在 | `crates/` 下无 `template` / `snapshot` / `fork` / `pool` 同名 crate，全仓无对应实现，也无产品级 `REQ-*` |
| SDK | `sdks/` | 目录 + README | **零生成产物**，`apis/` 无权威 OpenAPI |

本节标题里的"可点证据"是一句**断言**，不是形容：每一行都由 `tools/check-sandbox-e2b-parity-matrix.mjs` 的第 11 条规则族核验，四个口径都写死在这里。

- **路径**：`路径` 格里的反引号路径必须真的存在（末段含 `*` 时按集合展开，所以 `apis/commands/*.json` 是被当作一组文件核对的）；`真实状态` 里任何以 `crates/` `apis/` `sdks/` `specs/` `docs/` `tools/` `tests/` 开头的反引号 token 同样要存在。**声明某组件不存在的行（`规模` 为 `不存在`）必须在 `真实状态` 里点名"不存在的是什么"**——一个不得出现在 `crates/**/*.rs` 里的标识符，或一个不得匹配任何 crate 目录名的词——否则这条"没有"无法从树里重新推导出来，只能被相信。
- **规模**：两种写法可核算。`N 模块` 等于该 crate `src/` 下的 `.rs` 文件数（含 crate 根 `lib.rs`/`main.rs`）；`N 行` 等于所点文件的物理行数，口径与 `wc -l` 一致（末行有换行时不计多一行），未点名文件时取该 crate `src/` 下唯一的 `.rs`。其余取值（`不存在` / `骨架` / `candidate` / `目录 + README` 等）不含数字断言。
- **行号锚**：`file.rs:N` 里的裸文件名相对该行的 crate 的 `src/` 解析，含 `/` 的相对仓库根解析；`N` 必须在文件行数内，且**该行自己点名的东西必须真的出现在 `N` 起的几行窗口里**——一个仍然解析得通、却已指向无关代码的行号是这类腐化最安静的一半：读者跟过去，看到一段像样的声明，就信了这一行。
- **证据**：每行的 `真实状态` 至少含一个反引号物证。整节标题承诺"可点"，那么点不到任何东西的行与点得到的行在读感上没有区别，这正是本规则族存在的理由。

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

每行的 `基准已取证 …` 引用指向 `specs/sandbox-e2b-capability-baseline.json` 的同一行号，那里给出该行 E2B 侧取自哪个来源、抽取到哪些标识符。`基准仅索引` 是**已退役的标记**：它曾表示该行只依据 `llms.txt` 的页面标题口径判定、未逐字核对字段名。全部分类现已逐页取证，该标记不得在本分节中再出现。

`第 N 行（M 项）` 里的 `N` 只能是**本行自己的行号**，`M` 只表示一个量：**该基准行的整行抽取面**，即 `e2bFields` + `e2bFacts` + `e2bClis` 三者长度之和——与 `tools/check-sandbox-e2b-field-parity.mjs` 判定"该行有无抽取面"用的是同一个数，因此可被重算。要额外给出更窄的量（例如只数 `e2bFields`）必须写出来（`其中 M 个字段`）：同一个写法不许承载两个含义。本仓曾在此处有 **4 行**把 `e2bFields` 单独当作 `项`，与其余 23 行口径不同，而**没有任何门禁看得见**——这正是 `document-join` 规则族现在逐行核验这一对数字的理由。

### 2.1 Sandbox 生命周期

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 1 | `Sandbox.create()`（template / `envs` / `metadata` / `timeoutMs` / `network` 参数） | `SandboxSessionLifecyclePort::create_sandbox_session`（`port.rs:10`）+ 4 张 PG 表 | 🟡 | 领域服务候选；无 Provider 实现、无入口。E2B 的 `metadata` 在本仓**无对应字段**。基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 1 行（25 项，其中 11 个字段，含 `GET /envs [getEnvVars]`） |
| 2 | `Sandbox.connect()`（暂停自动恢复；TTL 只延长不缩短） | 无 | ❌ | — |
| 3 | `setTimeout()` / `keepAlive`（运行中改 TTL） | 无 | ❌ | 本仓只有 **Lease** 过期时间（`repository.rs:57`），语义是生命周期控制权租约，不是沙箱 TTL，别混为一谈 |
| 4 | `getInfo()`（`templateId`/`name`/`metadata`/`startedAt`/`endAt`） | 无 | ❌ | `get_sandbox_session` 只返回领域聚合，无查询 API，无 metadata |
| 5 | `kill()` | `destroy_sandbox_session` → `Destroyed` 终态 | 🟡 | 领域候选；无入口 |
| 6 | `Sandbox.list()`（`state`/filter + paginator） | 无 | ❌ | — |
| 7 | Lifecycle events API（事件流） | `apis/async/sandbox-events.asyncapi.json` + `sandbox-event-catalog.json` 契约 | 🟡 | 仅契约，无 runtime exporter/worker |
| 8 | Lifecycle webhooks | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 8 行（18 项） |
| 9 | Auto-resume on request | 无 | ❌ | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 9 节；无 `REQ-*`〔§3.4/5〕 |
| 10 | SSH access（WebSocket 代理） | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 10 行（6 项） |
| 11 | Secured access / 访问令牌门控 | 无 | ❌ | 本仓有 `SandboxFencingToken`，对象是控制权竞争而非访问面，形态不同 |

### 2.2 持久化（Pause / Resume）

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 12 | `pause()`（同时保存文件系统**与内存**） | `REQ-2026-0008`/`REQ-2026-0021` 门禁（draft） | ❌ | PRD 状态机含 `Pausing/Paused`，实现枚举**没有**；`SandboxProvider` trait 无 `pause` |
| 13 | Filesystem-only pause（`keepMemory: false`） | 无 | ❌ | — |
| 14 | `connect()` 恢复（热恢复） | 无 | ❌ | — |
| 15 | Reboot-on-resume（`resume-without-memory`） | 无 | ❌ | — |
| 16 | Paused 无限期保留、无 TTL、无自动删除 | 无 | ❌ | **形态相反**：`REQ-2026-0020` 定义的是**有界**热状态投影与终态保留窗口，不是无限期保留 |
| 17 | Auto-pause on timeout（`onTimeout: 'pause'`） | 无 | ❌ | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 9 节；无独立 `REQ-*`〔§3.4/5〕 |
| 18 | Pause 被拒语义（HTTP 503 `ServiceBusyError`，沙箱保持运行可重试） | `SandboxLifecycleError::LeaseUnavailable` / `LeaseLost` | 🟡 | 形态不同：本仓的拒绝对象是**生命周期控制权竞争**，不是快照拥塞；E2B 那套快照背压语义本仓无对应 |
| 19 | 暂停/恢复性能承诺（约 4 s/GiB RAM；恢复约 1 s） | 无 | ❌ | [TECH-performance-and-capacity.md](TECH-performance-and-capacity.md) 有恢复时延目标，但无参考硬件与测量 |

### 2.3 Snapshot 与 Fork

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 20 | `createSnapshot()`（含内存与文件系统；原沙箱短暂暂停后继续，ID 不变） | 无产品级能力 | ❌ | 仅有 Workspace Checkpoint 与 Firecracker Snapshot 的 Gate 0 候选（`REQ-2026-0021`、`REQ-2026-0008`） |
| 21 | `Sandbox.create(snapshotId)`（从快照派生沙箱） | 无 | ❌ | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 5 节 |
| 22 | `listSnapshots()` / `deleteSnapshot()` | 无 | ❌ | — |
| 23 | `fork`（一次调用在原地快照并派生 N 个沙箱） | 无 | ❌ | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 6 节；无 `REQ-*`、无 `ADR`、一致性语义未定〔§3.4/4〕 |
| 24 | Snapshot 与原沙箱并行运行、一个快照派生多个 | 无 | ❌ | — |

### 2.4 Template

Template 是 E2B"快速创建 + 快速部署"的**唯一基础设施**：预构建镜像 + start command 常驻 + 构建缓存 + 层复用。本仓整类零承载。

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 25 | 声明式 Template 定义（`Template().fromBaseImage()` / `fromTemplate()` / `copy()` / `setEnvs()` / `setStartCmd()`） | **无任何承载** | ❌ | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 4 节；`REQ-*` 为零〔§3.4/1〕 |
| 26 | `e2b template init` / `build` / `deploy` | 无 CLI | ❌ | `crates/sdkwork-sandbox-cli/src/main.rs:3` = `fn main() {}`。基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 26 行（38 项，其中 15 个字段：Templates REST 建 / 查 / 改 / 删 + 构建流水线与构建产物） |
| 27 | Start / Ready command（沙箱创建时长驻进程**已在运行**，首命令零等待） | 无 | ❌ | — |
| 28 | 构建缓存与层级复用（`fromTemplate()` 复用已缓存基础层） | 无 | ❌ | PRD 第 4 节要求 Template 缓存 Hot/Warm/Cold + 淘汰策略；无 `REQ-*`〔§3.4/2〕 |
| 29 | Template tags / versioning / names | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 29 行（35 项，其中 10 个字段：tags 端点 + `GET /templates/aliases/{alias} [getTemplatesAlias]`，alias 即版本化命名机制） |
| 30 | Base image / 私有 registry 接入 | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 30 行（30 项） |
| 31 | 构建限额（1 h / 8 vCPU / 8 GiB / 10 GiB / 20 并发） | 无 | ❌ | — |
| 32 | 以 Dockerfile 或构建脚本作为**构建输入** | 无（产品要求已写） | 🟡 | [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 4 节已写"构建输入允许使用 Dockerfile 或构建脚本"；无 `REQ-*`〔§3.4/3〕 |
| 33 | 以 Docker 作为运行时依赖或隔离边界 | 明确不做 | ⛔ | `PRD.md` 非目标原话："不把 Docker 作为运行时依赖或隔离边界；Docker 只允许作为 Template 的构建输入格式" |

### 2.5 Filesystem

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 34 | `files.read()` 单文件读取 | 无 | ❌ | `REQ-2026-0007` 只有命令契约，无文件系统端口 |
| 35 | `files.write()` / `writeFiles()` 批量写入 | 无 | ❌ | — |
| 36 | `files.getInfo()` / stat / 存在性 | 无 | ❌ | — |
| 37 | 文件自定义 metadata（上传时 `X-Metadata-<key>` → xattr） | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 37 行（22 项） |
| 38 | `files.watch()` / `WatchDir` 变更流 | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 38 行（14 项） |
| 39 | upload / download（含目录、`X-Metadata-` 头） | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 39 行（22 项） |
| 40 | `listDir` / `makeDir` / `move` / `remove` | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 40 行（4 项） |

### 2.6 Volumes

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 41 | Volume 创建 / 列举 / 检视 / 销毁（独立于沙箱生命周期的持久存储） | `REQ-2026-0013` Workspace Block Device（draft） | 🟡 | 形态不同：本仓的字节权威在 Drive / 批准的 Volume Authority，语义权威在 `sdkwork-agents`，Sandbox 不拥有 Volume |
| 42 | 创建沙箱时把 Volume 挂载到自定义路径 | `REQ-2026-0004` Agents Workspace Attachment（accepted） | 🟡 | 只有 Attachment 边界；无挂载参数面、无入口 |
| 43 | Volume 内读写 / 上传 / 下载 | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 43 行（25 项） |
| 44 | Volume 间迁移（挂载两个 Volume + rsync） | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 44 行（12 项） |

### 2.7 Commands 与 Process

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 45 | `commands.run()`（`envs` / `cwd` / `user` / `timeoutMs`） | `apis/commands/sandbox-command-contract.json`（`implementationAuthorized: false`） | 🟡 | 仅契约：全仓无 `SandboxCommandExecutor`。契约的 `executionModes` 只有 `executable-argv`，`forbiddenExecutionModes` 显式禁止 `shell-string` |
| 46 | 流式 stdout/stderr（`onStdout`/`onStderr`） | 无 | ❌ | — |
| 47 | 后台进程（`background: true` + `commands.list` + `commands.kill`） | 无 | ❌ | — |
| 48 | stdin 输入 / `CloseStdin` | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 48 行（3 项） |
| 49 | `SendSignal` / 进程 `Update` | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 49 行（2 项） |

### 2.8 PTY

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 50 | Interactive terminal（PTY） | `specs/sandbox-interactive-terminal-session.contract.json` | 🟡 | 仅契约，且 `materialization` 显式禁止物化：`ptyOrConptyAllowed: false`、`processSpawnAllowed: false`、`rustPortOrTypesAllowed: false`（`REQ-2026-0024`） |

### 2.9 Code Interpreter

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 51 | `runCode()` / 代码上下文（`contexts`） | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 51 行（11 项） |
| 52 | 多语言执行（python / js / ts / r / java / bash） | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 52 行（23 项） |
| 53 | 图表与可视化预置库 | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 53 行（21 项） |

### 2.10 Network

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 54 | 出网开/关（`allowInternetAccess`，默认开启） | `REQ-2026-0014` 的 `DenyAll` 门禁（draft） | 🟡 | 契约方向**相反**：本仓默认拒绝，E2B 默认允许。仅契约，无 network runtime |
| 55 | allow / deny 列表（IP / CIDR / 域名 / 通配） | `specs/sandbox-firecracker-network-isolation.contract.json`（draft） | 🟡 | 仅契约 |
| 56 | per-host rules / header 注入（`network.rules`，public beta） | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 56 行（33 项） |
| 57 | 运行中 `updateNetwork`（替换式，不合并） | 无 | ❌ | — |
| 58 | 端口暴露（public URL / `getHost`） | 无 | ❌ | 产品要求见 [PRD-sandbox-surfaces.md](../../product/prd/PRD-sandbox-surfaces.md) 第 8 节；无 `REQ-*`〔§3.4/6〕 |
| 59 | 限制公开访问（`allowPublicTraffic` / `maskRequestHost` / `httpsPorts`） | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 59 行（12 项） |
| 60 | 自定义域名 | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 60 行（6 项） |
| 61 | 出网代理隧道 / BYOP SOCKS5 | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 61 行（31 项） |

### 2.11 Secrets 与 IAM

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 62 | Secret 存储（create / update / delete / list / rotate；**无读值面**） | `REQ-2026-0025` value-free opaque grant（draft） | 🟡 | 仅契约，无 Secret Authority |
| 63 | 出网代理注入 Secret（值不进沙箱） | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 63 行（10 项） |
| 64 | Workload identity（JWT-SVID 短期令牌） | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 64 行（13 项） |

### 2.12 Metrics 与 Telemetry

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 65 | `getMetrics()`（`cpuUsedPct` / `cpuCount` / `memUsed` / `memTotal` / `diskUsed` / `diskTotal`，5 s 采样） | `apis/async/sandbox-observability-catalog.json`（32 个指标契约） | 🟡 | 仅契约，无 runtime。[PRD-sandbox-surfaces.md](../../product/prd/PRD-sandbox-surfaces.md) 第 13 节的 13 个指标族已与本契约建立机器映射（`metrics.productFamilies`：6 控制面 / 7 运行面），命名后缀与 `catalogMetrics` 解析由 `tools/check-sandbox-requirement-traceability.mjs` 第 6 条规则族核验；运行面族尚无契约对应物，已按层登记缺口归属。基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 65 行（12 项，其中 6 个字段，含聚合端点 `GET /metrics [getMetrics]`） |
| 66 | Team 级 metrics | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 66 行（3 项） |
| 67 | OTel telemetry export | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 67 行（16 项） |

### 2.13 CLI

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 68 | `e2b` CLI（auth / sandbox list·create·connect·fork·exec·shutdown / template init·build / snapshot / metrics） | `crates/sdkwork-sandbox-cli` | ❌ | `main.rs:3` = `fn main() {}`；零命令、零参数解析 |

### 2.14 SDK

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 69 | 官方 JS/TS + Python SDK（同步/异步） | `sdks/` 目录 | ❌ | 只有 README，零生成产物；`apis/` 无权威 OpenAPI，`ROUTE_CRATE_COUNT: 0` |
| 70 | Code Interpreter SDK / Desktop SDK | 无 | ❌ | 基准已取证 `specs/sandbox-e2b-capability-baseline.json` 第 70 行（43 项） |
| 71 | 分页器与错误类型族（`ServiceBusyError` 等） | 无 | ❌ | — |
| 72 | 多语言同语义 SDK 的生成链 | `PRD.md` 目标已写 | ❌ | `PRD.md` 目标列出"Rust、TypeScript、Python 三语言同语义 SDK 消费面，由权威契约生成，不手写方言"；但 `apis/` 无权威 OpenAPI，`sdks/` 零生成产物 |

### 2.15 MCP Gateway

| # | E2B 能力 | 本仓对应 | 状态 | 证据 |
| --- | --- | --- | --- | --- |
| 73 | MCP Gateway（200+ servers / custom templates / custom servers） | 仅 Transport 级 | ❌ | [PRD-capabilities.md](../../product/prd/PRD-capabilities.md) 第 5 节只有传输级描述；无独立 `REQ-*`〔§3.4/7〕 |

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

本表是**逐用例**的：工作区里每一个 `#[test]` / `#[tokio::test]` 都必须在表中出现且只出现一次。这不是声明而是断言——`tools/check-sandbox-e2b-parity-matrix.mjs` 的第 9 条规则族自己遍历 `crates/**/*.rs` 把测试点出来，再与本表做**双向记账**：本表引用的实现路径必须存在（带 `:line` 时该行必须在文件内）、引用的用例必须由所引测试文件声明、而工作区声明的每个测试都必须被本表认领恰好一次。

**这张表此前是不完整的，这正是第 9 条规则族的由来。** 它声称自己是"本仓全部真实实现"，实际只覆盖了 3 个 crate 的 48 个可运行用例（外加把 `…-repository-sqlx` 唯一的 `#[ignore]` 集成用例记在已覆盖侧，凑成 49），而工作区有 10 个文件、68 个用例：两个仓储 crate（`…-repository-memory` 5 个、`…-repository-sqlx` 除该 `#[ignore]` 外的 14 个，共 19 个用例）**根本没出现在表里**。也就是说整份审计里最让人安心的一张表，恰好是没有任何东西核过的一张。补齐的过程还顺手暴露了一个抽取口径缺陷：`#[tokio::test(flavor = "multi_thread", worker_threads = 4)]` 带参数，而只认 `#[tokio::test]` 的正则会**静默丢掉它**——第一版门禁数出 67 个测试，工作区实际是 68 个。这和"operationId 抽取漏了点号 ⇒ 一度报出 36 个未覆盖、真值 19"是同一类缺陷：**任何覆盖率结论，必须先对抽取规则做正反例自检再报数。**

| 实现面 | 实现点 | 测试文件 | 用例 |
| --- | --- | --- | --- |
| Provider descriptor 能力与隔离等级的 fail-closed 协商 | `crates/sdkwork-sandbox-provider-spi/src/provider.rs:12` | `crates/sdkwork-sandbox-provider-spi/src/provider.rs` | `sandbox_provider_descriptor_fails_closed_on_capability_and_assurance` |
| 隔离强度全序（安全梯子）锁定 | `crates/sdkwork-sandbox-provider-spi/src/capability.rs:14` | `crates/sdkwork-sandbox-provider-spi/src/capability.rs` | `isolation_assurance_declaration_order_is_the_security_ladder` |
| Readiness 必须要求 Workspace Attachment 且 Policy 生效 | `crates/sdkwork-sandbox-provider-spi/src/provider.rs:101` | `crates/sdkwork-sandbox-provider-spi/src/provider.rs` | `sandbox_provider_readiness_requires_workspace_attachment_and_policy_enforcement` |
| Tenant 标识拒绝 path-like 取值 | `crates/sdkwork-sandbox-provider-spi/src/identity.rs:43` | `crates/sdkwork-sandbox-provider-spi/src/identity.rs` | `rejects_path_like_tenant_identifiers`、`rejects_empty_and_over_bound_opaque_identifiers` |
| Provider Allocation 引用与密钥材料的 Debug 脱敏 | `crates/sdkwork-sandbox-provider-spi/src/identity.rs:110` | `crates/sdkwork-sandbox-provider-spi/src/identity.rs` | `sandbox_provider_reference_debug_output_is_redacted` |
| Fencing Token 拒绝 0 与有符号上溢 | `crates/sdkwork-sandbox-provider-spi/src/identity.rs:88` | `crates/sdkwork-sandbox-provider-spi/src/identity.rs` | `sandbox_fencing_token_rejects_zero_and_signed_maximum_overflow` |
| 会话状态机、恢复重放与版本上界 | `crates/sdkwork-intelligence-sandbox-service/src/model.rs` | `crates/sdkwork-intelligence-sandbox-service/src/model.rs` | `sandbox_session_state_transition_matrix_matches_the_documented_state_machine`、`sandbox_session_transition_away_from_failed_clears_last_failure`、`sandbox_session_replay_distinguishes_matching_conflicting_and_missing_operations`、`sandbox_session_version_fails_closed_at_the_persistence_maximum` |
| 快照校验、受保护分配引用与租约上界 | `crates/sdkwork-intelligence-sandbox-service/src/repository.rs` | `crates/sdkwork-intelligence-sandbox-service/src/repository.rs` | `sandbox_snapshot_validation_accepts_the_persisted_state_matrix`、`sandbox_snapshot_validation_rejects_invalid_cross_field_combinations`、`sandbox_snapshot_restore_rejects_invalid_state_before_decryption`、`sandbox_snapshot_rejects_versions_above_the_persistence_maximum`、`sandbox_protected_allocation_reference_enforces_storage_bounds`、`sandbox_session_lease_rejects_non_positive_expiry` |
| Lifecycle 幂等 / Lease / Fencing / Readiness 端到端行为 | `crates/sdkwork-intelligence-sandbox-service/src/service.rs` | `crates/sdkwork-intelligence-sandbox-service/src/tests.rs` | `sandbox_create_rejects_sandbox_session_id_reuse_across_operations`、`sandbox_create_recovers_when_a_concurrent_identical_create_commits_first`、`sandbox_workspace_context_is_preserved_across_provider_attachment_requests`、`sandbox_lifecycle_commands_are_idempotent_without_duplicate_provider_effects`、`sandbox_stop_provider_failure_records_failed_operation_and_keeps_binding`、`sandbox_destroy_provider_failure_records_cleanup_failure_and_keeps_binding`、`sandbox_provider_selection_fails_closed_for_capability_assurance_and_health`、`sandbox_readiness_gate_cleans_binding_and_records_failed_operation`、`sandbox_retry_start_releases_failed_binding_before_allocating_again`、`sandbox_start_persists_recoverable_binding_intent_before_provider_allocation`、`sandbox_reconciler_recovers_after_allocation_persistence_failure`、`sandbox_tenant_scope_and_invalid_transitions_are_enforced`、`sandbox_reconciler_recovers_transient_sessions_with_bounded_pagination`、`sandbox_reconciler_omits_a_cursor_when_the_final_page_is_exactly_full`、`sandbox_reconciler_rejects_invalid_page_sizes_before_repository_access`、`sandbox_reconciler_skips_an_actively_leased_session`、`sandbox_reconciler_preserves_provider_failure_when_sandbox_lease_release_fails`、`sandbox_reconciler_reloads_authoritative_session_after_acquiring_the_sandbox_lease`、`successful_sandbox_lifecycle_maps_sandbox_lease_release_failure_to_lease_lost`、`sandbox_provider_call_maps_sandbox_lease_renewal_failure_to_lease_lost`、`sandbox_persistence_maps_sandbox_lease_conflict_to_lease_lost`、`sandbox_provider_timeout_is_bounded_and_persisted_as_a_typed_failure`、`sandbox_lifecycle_service_rejects_invalid_operation_policy_and_duplicate_providers`、`sandbox_allocation_protection_metadata_rejects_unsafe_key_identity`、`sandbox_lifecycle_guard_matrix_matches_the_documented_operation_contract`、`sandbox_lifecycle_replay_reports_an_in_progress_sandbox_operation`、`sandbox_stop_fails_closed_when_a_running_sandbox_session_has_no_runtime_binding`、`sandbox_lifecycle_create_start_benchmark`、`sandbox_reconciler_degrades_an_unregistered_provider_session_instead_of_aborting_the_page`、`sandbox_reconciler_reports_an_unreadable_session_and_keeps_the_page_converging`、`sandbox_provider_selection_renews_the_lease_before_every_health_probe`、`sandbox_replaying_a_succeeded_operation_after_later_operations_returns_the_current_session`、`validate_sandbox_session_persisted_invariants_rejects_a_ledger_that_replays_to_another_state`、`sandbox_reconciler_reports_a_vanished_session_as_vanished_not_lease_unavailable`、`sandbox_reconciler_reports_a_session_vanished_before_lease_acquisition_as_vanished`、`sandbox_reconciler_fails_closed_when_the_page_list_is_unavailable`、`sandbox_reconciler_fails_closed_when_a_session_load_is_unavailable` |
| Session Repository 内存实现：tenant 隔离、CAS、租约竞争与接管 | `crates/sdkwork-intelligence-sandbox-repository-memory/src/lib.rs` | `crates/sdkwork-intelligence-sandbox-repository-memory/src/lib.rs` | `isolates_sandbox_sessions_by_tenant_and_indexes_sandbox_create_operations`、`rejects_stale_sandbox_session_compare_and_swap`、`rejects_invalid_sandbox_reconciliation_page_sizes`、`sandbox_session_insert_conflicts_are_scoped_by_tenant`、`enforces_sandbox_lease_competition_takeover_and_stale_token_rejection` |
| SQLx 编解码：状态 / 能力 / 操作族的规范化往返与未知值拒绝 | `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/codec.rs` | `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/codec.rs` | `sandbox_capability_codec_is_canonical_and_rejects_duplicates`、`sandbox_state_codec_round_trips_every_state_and_rejects_unknown_values`、`sandbox_operation_kind_codec_round_trips_every_kind_and_rejects_unknown_values`、`sandbox_operation_outcome_codec_round_trips_every_outcome_and_rejects_mismatches`、`sandbox_session_failure_and_isolation_assurance_codecs_reject_unknown_values` |
| SQLx 加密的 Provider 恢复元数据：上下文绑定、密钥身份与材料上界 | `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/encryption.rs` | `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/encryption.rs` | `sandbox_allocation_reference_is_encrypted_redacted_and_context_bound`、`sandbox_allocation_restore_rejects_wrong_key_identity`、`sandbox_allocation_key_debug_output_redacts_key_material`、`sandbox_allocation_key_rejects_unsafe_key_identity_and_invalid_material_bounds` |
| SQLx 有界重加密的分页版本漂移拒绝 | `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/reencryption.rs` | `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/reencryption.rs` | `sandbox_reencryption_target_rejects_page_version_drift` |
| SQLx 错误映射：唯一约束、完整性与瞬时码、连接失败的分类 | `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/repository.rs` | `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/repository.rs` | `sandbox_sqlx_error_mapping_classifies_unique_violations_by_constraint`、`sandbox_sqlx_error_mapping_classifies_integrity_and_transient_codes`、`sandbox_sqlx_error_mapping_maps_connection_failures_to_unavailable` |
| SQLx 持久化生命周期（需外部 PostgreSQL） | `crates/sdkwork-intelligence-sandbox-repository-sqlx/tests/postgres_repository.rs` | `crates/sdkwork-intelligence-sandbox-repository-sqlx/tests/postgres_repository.rs` | `sandbox_postgres_destructive_test_requires_matching_non_echoing_database_urls`、`sandbox_postgres_repository_enforces_durable_lifecycle_contract` |
| Local Fake Host Boundary：类型化参数、路径逃逸、环境上界 | `crates/sdkwork-sandbox-provider-local/src/fake_host_boundary/mod.rs` | `crates/sdkwork-sandbox-provider-local/src/fake_host_boundary/tests.rs` | `sandbox_fake_host_boundary_preserves_typed_arguments_without_shell_parsing`、`sandbox_fake_host_boundary_rejects_path_escape_and_windows_path_hazards`、`sandbox_fake_host_boundary_denies_command_strings_and_ambient_credentials`、`sandbox_fake_host_boundary_enforces_argument_and_environment_bounds`、`sandbox_fake_host_boundary_enforces_environment_entry_bound` |
| Local Host Boundary 生产纯数据规则（2026-09-24 授权切片） | `crates/sdkwork-sandbox-provider-local/src/host_boundary/mod.rs` | `crates/sdkwork-sandbox-provider-local/src/host_boundary/tests.rs` | `accepts_a_well_formed_sandbox_command_request`、`rejects_an_executable_that_is_not_a_bare_name`、`rejects_an_executable_outside_the_allowlist`、`rejects_argument_overruns_and_forbidden_bytes`、`rejects_working_directory_escapes_and_windows_hazards`、`rejects_environment_entries_that_break_the_boundary`、`rejects_environment_count_overruns`、`displays_every_error_variant_without_panicking` |

表内共 **87 个用例**（17 行），与工作区静态清点一致；其中 `sandbox_postgres_repository_enforces_durable_lifecycle_contract` 带 `#[ignore]`，是唯一不进默认运行的用例（它声明需要 `SDKWORK_DATABASE_TEST_POSTGRES_URL` 与一个已初始化的 PostgreSQL）。因此 `cargo test --workspace` 的读数是 **86 passed / 0 failed / 1 ignored**，87 = 86 + 1，两侧对得上。

计数（2026-09-22 实测）：

```bash
cargo test --workspace
```

`86 passed / 1 ignored`（另 0 failed；1 ignored 是声明需要外部 PostgreSQL 的测试）。契约测试：

```bash
node --test tests/contract/*.test.mjs
```

`673 pass / 0 fail`（其中 E2B 矩阵门禁 172 个、E2B 基准门禁 30 个）。这两个数字都不是手写的：契约数由 `tools/check-sandbox-e2b-field-parity.mjs` 打开 `tests/contract/*.test.mjs` 逐文件重算（含逐文件明细，所以"总数对了但某个文件的数错了"同样会红），Rust 读数无法静态推导，因此与产生它的命令一起落盘在 `specs/sandbox-e2b-capability-baseline.json` 的 `testInventory.rustWorkspace` 里再比对。本节此前一直写着 406 与 63，而两个真值分别是上一段的两个数——覆盖章是整份审计里唯一会执行的部分，它对不上号就是在对自己说谎。

### 3.2 覆盖空档

按"先高价值后低价值"的优先级：

| 优先级 | 空档 | 性质 | 取证 | 说明 |
| --- | --- | --- | --- | --- |
| 1 | **500 ms 热分配目标没有测量者，三份性能文档互不 join** | 治理阻塞 | `REQ-2026-0019` | 目标（`PRD.md` 第 6 节）说的是"公开参考环境中 Pool 到 Workspace 绑定 p95 小于 500 ms"；基线（`docs/architecture/tech/TECH-performance-baseline.md`）自述**发布门禁资格：不合格**，且其第 0.2 条明确"分子（编排）已测、分母（真实沙箱启动）不存在"，测的是编排地板；容量分片（`docs/architecture/tech/TECH-performance-and-capacity.md`）声明"全部数值都是工程目标"。三份文件各说各话、无人同时读它们。而 500 ms 对应的 Pool 路径由 `REQ-2026-0019` 承载且仍是 `draft`，所以**这里连可测的实现都还没有**，只能先作为阻塞登记 |

**本表此前有一行是假的，这正是新增门禁的由来。** 原第 5 行写作「"快速创建/快速部署"的性能断言全为零测试」，性质一栏写着"既无参考硬件也无 Benchmark 套件，且该目标没有任何需求承载"——**这句有一半不成立**：`REQ-2026-0019` 承载的正是这个目标，`tools/bench-sandbox-lifecycle.mjs` 与两平台原始样本也都在树里（样本落在 gitignore 的 `target/` 下，是证据不是缓存）。一张"缺什么"的清单如果不可被目录列举推翻，它就会越写越旧。所以本节改成带性质的表，并由门禁按性质**反向核验**：`缺产物` 点名的路径必须**不存在**、`缺门禁` 点名的路径必须**存在**、`治理阻塞` 必须点名一份**记录在案且尚未 `ready`** 的需求——三者问的都是"这句话能不能被证伪"，不是措辞问题。

**六个空档已于本轮闭合并从本表移除**，各自的门禁落点：

| 已闭合空档 | 门禁落点 | 闭合方式 |
| --- | --- | --- |
| PRD 状态机 ⊋ 实现状态机（`Pausing`/`Paused`/`Recovering` 无标记） | `tools/check-sandbox-requirement-traceability.mjs` 第 5 条规则族 | PRD 第 3 节补 `目标态标记` 行；门禁双向核验：图中状态必须被实现枚举包含**或**被标记，标记集合必须恰好等于差集，目标态进了代码而标记未收缩同样转红 |
| 指标契约与指标族名不相交 + `*_latency` 命名违规 | `tools/check-sandbox-requirement-traceability.mjs` 第 6 条规则族 | PRD 第 13 节 13 个指标族按 `OBSERVABILITY_SPEC.md` 命名规则重写；`apis/async/sandbox-observability-catalog.json` 新增 `metrics.productFamilies` 机器映射（6 控制面 / 7 运行面），双向一一对应、`catalogMetrics` 必须可解析 |
| `PRD-capabilities.md` 第 11 节与 E2B 基准之间无门禁 | `tools/check-sandbox-e2b-field-parity.mjs` 第 8 条规则族（capability-matrix-join） | 基准 JSON 逐行携带 `capabilityMatrixRows`，78 行与 34 行双向记账（57 映射 / 21 登记无产品行；25 被判定 / 9 登记无基线行），审计文档自己的 34/78 标题句被解析比对 |
| §5「我们比 E2B 强的地方」的 9 行优势断言没有门禁 | `tools/check-sandbox-e2b-parity-matrix.mjs` 第 11 条规则族（形状取证扩至 §5） | 每行证据格必须含至少一个**可反证**引用（仓库路径 / Rust 文件 / `REQ-*`/`ADR-*` 记录 / 在 crates 或 database 树中出现的裸标识符）；裸 Rust 文件名拒绝（改名即孤儿）；行锚必须在界内且**该行引用的其他物证至少一项在锚点窗口内可见**（与 §1.2 同一配对法）。上线首跑抓到 5 处真缺陷：两处行锚已漂移（`identity.rs:88`→实为 :97、`model.rs:24`→实为 :276）、两个裸文件名、一行证据格只有文档链接没有可反证 token，均已按真值改写 |
| PRD 第 8 节等跨文档「无需求承载」断言只有计数、没有逐条归属 | `tools/check-sandbox-e2b-parity-matrix.mjs` 第 8 条规则族（§3.5 归属账） | 句型清单拓宽收编 `无独立 REQ-*`（普查 15→19 行、新增第 5 个文档），归属账把 19 行断言逐条判到封闭词表（口径句 / 确认无承载 / 已证伪见更正账）；门禁按文档核对归属行数与普查声明数相等、候选记录可解析、且每对（关键词，候选）词法不命中——新增需求记录一旦拥有某行关键词即转红，语义判定因此被账本化而不是散文化 |
| 已实现面没有任何消费点测试（真实 Provider 调用序列） | `REQ-2026-0003`/`0007`/`0008` 的实现授权（2026-09-24 人审签署）+ `specs/sandbox-local-provider-host-boundary.contract.json` 翻转 | 阻塞解除：三条需求进入 `ready`、两条 ADR 进入 `accepted`、Host Boundary 契约授权实现。剩余部分不再是审计空档而是交付工作本身，由 roadmap 交付顺序与 `sandbox-provider-delivery-gates.contract.json`（其余 packet 签署后翻转）接管 |

**本表此前有一行是假的，这正是新增门禁的由来。** 原第 5 行写作「"快速创建/快速部署"的性能断言全为零测试」，性质一栏写着"既无参考硬件也无 Benchmark 套件，且该目标没有任何需求承载"——**这句有一半不成立**：`REQ-2026-0019` 承载的正是这个目标，`tools/bench-sandbox-lifecycle.mjs` 与两平台原始样本也都在树里（样本落在 gitignore 的 `target/` 下，是证据不是缓存）。一张"缺什么"的清单如果不可被目录列举推翻，它就会越写越旧。所以本节改成带性质的表，并由门禁按性质**反向核验**：`缺产物` 点名的路径必须**不存在**、`缺门禁` 点名的路径必须**存在**、`治理阻塞` 必须点名一份**记录在案且尚未 `ready`** 的需求——三者问的都是"这句话能不能被证伪"，不是措辞问题。

**本轮没有新增实现用例**，因为没有获批的实现可测——2026-09-24 起授权状态开始翻转：机器契约里 `specs/sandbox-local-provider-host-boundary.contract.json` 已授权实现（对应 packet 已签署），其余 23 份 `*.contract.json` 中 21 份显式声明 `implementationAuthorized: false`，第 23 份 `specs/sandbox-commercial-readiness.contract.json` 是发布决定记录而非能力契约，它没有该字段、但独立声明 `runtimeImplementationAuthorizationGranted: false` 且 `releaseDecision.status: "no-go"`（缺字段在 `check-sandbox-human-review-signoff.mjs` 里按未授权处理，该处用 `value.implementationAuthorized === true` 判定）；另有两份不以 `.contract.json` 命名的机器契约：`apis/commands/sandbox-command-contract.json` 已于 2026-09-24 随 `REQ-2026-0007` 进入 `ready` 授权实现，`apis/async/sandbox-observability-catalog.json` 仍为 `false`；8 类未授权能力被 `PRD.md` 第 8 节明文列入"尚无需求承载"。在实现授权到位前写"用例"只能写成断言契约文本，属于假门禁。

### 3.3 本轮新增的门禁与用例（含变异结果）

矩阵本身是一个会被反复引用的**数字**，而本仓此前没有任何东西读它。本轮补上：

| 新增物 | 内容 | 用例数 | 变异自证 |
| --- | --- | --- | --- |
| `tools/check-sandbox-e2b-parity-matrix.mjs` | 12 条规则族：词表 / 编号与形状 / 状态 / 分类对应 / 普查算术 / 引用解析与登记 / 空档与取证 / 零需求断言登记 / 实现面覆盖 / 自描述计数 / 形状取证 / 结论数字一致性 | — | 见下 |
| `tests/contract/e2b-parity-matrix-tool.contract.test.mjs` | 143 个用例，每个规则族声明的每一条反面用例都经变异验证会转红 | 143 | 143/143 pass、逐族置空 12/12、95/95 条声明用例转红 |
| `tools/check-sandbox-e2b-field-parity.mjs` | 11 条规则族：基准形状 / 逐来源 provenance / 逐行证据 / 分类对齐 / 与文档逐行 join / `基准仅索引` 棘轮 / operation 覆盖记账 / 能力矩阵 join / 登记 / 测试清单 / 自描述计数 | — | 见下 |
| `tests/contract/sandbox-e2b-field-parity-tool.contract.test.mjs` | 30 个用例，11 条规则族各有一条能变红的反面用例，另加对照组与解析器回归 | 30 | 30/30 pass |
| `tools/check-sandbox-requirement-traceability.mjs`（既有工具，本轮扩展） | 第 5 条规则族（产品状态机 join）：PRD 第 3 节图 ↔ `SandboxSessionState` 枚举，`目标态标记` 集合必须恰好等于「图有而实现没有」的差集，实现态不得脱离图；第 6 条规则族（指标族 join）：PRD 第 13 节 ↔ `metrics.productFamilies` 一一对应，`kind` 与命名后缀互锁、`catalogMetrics` 必须可解析、空映射必须登记承载缺口 | — | 反面用例见下 |
| `tests/contract/requirement-traceability-tool.contract.test.mjs` | 27 个用例（未标记目标态 / 过期目标态 / 未知目标态 / 未列出实现态 / 缺映射条目 / 多余映射条目 / kind 后缀矛盾 / 悬空 `catalogMetrics` / 无承载 note，各一条能变红） | 27 | 27/27 pass |

变异自证（2026-09-22 实测，落盘改后跑，跑完还原并核验）：

```bash
node tools/check-sandbox-e2b-parity-matrix.mjs
```

把合计行 `| **合计** | **78** | **0** | **16** | **60** | **2** |` 的第二个数字改成 `**1**`（只脏这一个点）后，门禁 **exit 1** 并给出三条各自独立的诊断：与分类行求和不符、与矩阵逐行重算不符、合计行不再是 `N` 的划分。还原后 **exit 0**，文档 SHA256 前缀 `d654a2095335acaa398f46c7b687eb2a8477266ae76c45fc553dcdd77392c7ba` 与改前逐字节一致。

**门禁自身踩到并修掉的静默漏洞**：合计行写作 `| **合计** | **78** | … |`，初版解析器用 `^\d+$` 判行，星号使其不被识别为合计行 ⇒ 全部合计断言被跳过，门禁把**重算值**当作**已核对值**打印。这正是本仓最在意的那类缺陷（"看着通过、实际什么都没查"），已改为先剥离 Markdown 强调再解析，并加"没有合计行即失败"与"计数非数字即报告而非静默丢弃"两条规则锁住。

**矩阵门禁的逐族耦合自证**：契约套件里"每个规则族有一条反面用例"只证明用例存在，不证明**是这条规则**在报。于是再逐族把该族自己的判据做最小置空（`if (x) {` → `if (false) {`，或删掉 push 点），要求"转红的恰好是本族的反面用例"。12 / 12 全部成立：**95 条**声明为本族的用例**全部**转红、**异族 0 条**、套件总数全程 143 不变、跑完 `Buffer.compare` 逐字节还原。

**自证机制自己也被查出 10 条假覆盖**：此前的收敛判据只要求"每族至少转红 1 条"（`ownFlipped.length > 0`），于是 6 个族里共 **10 条**"声明为本族"的用例**从未被任何族的置空触发过**。分两种成因。一种是把规则写成了**行循环之外的守卫**因而锚点从未被列入编辑集：`RESIDUAL GAPS` 的"格子数不对"（走 `gaps.malformed`）与"声明无空档"（走 `rows.length === 0`）两条都在行循环之外；`SHAPE EVIDENCE` 的头表校验、缺节、无组件、锚点可见性四条同理；`CENSUS ARITHMETIC` 的"计数非数字"与"无合计行"两条也是。另一种是**直接调用解析器**的单元用例：SELF-DESCRIPTION 的两条按内联字符串断言 `parseLineScopedRuleFamilies` 的返回值，任何对判据的置空都不可能让它们转红。前者把锚点补进对应族，后者从"反面用例"改列为"编辑前即为绿"的对照用例——**一条永远不会转红的用例不是覆盖证据，把它记成覆盖正是本次要拒绝的那类断言**。另有一例既不是错标也不是漏锚点，而是**同族锚点互相掩盖**：`HEADLINE NUMBERS` 里"§3.3 引文不算断言"这条的 finding 与同族"数字与来源不符"那条落在同一行，聚合置空时两者一起消失，于是该用例改为单跑一个锚点来证明。收敛判据随之收紧为 `everyDeclaredCaseProved`——每族**每一条**声明都必须转红，某条声明过期即 `exit 3`，因为"声明了一份没人核对的清单"与"清单里有一条是假的"是同一个缺陷。变异报告落盘在 `target/matrix-mutation-proof.json`（`target/` 被 gitignore，是证据不是缓存）。

**这次置空顺手抓到一个真缺陷**：把"矩阵小节数与 census 分类数必须一一对应"这条判据置空后，越界读取 `census.categories[index]` 让门禁**抛 TypeError** 而不是报告——静态门禁在畸形输入上崩溃时，读起来像工具坏了而不是文档坏了。已补一层"取不到就跳过"的兜底，再复跑，转红集合重新变成"恰好本族"。

**基准门禁的耦合自证**：把 `add(rule, message)` 这个唯一的收敛点按规则族逐个置空，要求契约套件**逐族转红**，跑完用内存字节快照还原并核验逐字节相等。10 / 10 全部转红、还原 `Buffer.compare` 相等——也就是说十条规则族都不是死代码，每一条都有用例在真读它。第 8 条（能力矩阵 join）落地时沿用了同一判据：把该族两处判据置空（`if (false && …)`）后，契约套件**恰好只有数据驱动的变异用例转红**，且首条失败信息是 `capability-matrix-join mutation #N stayed green`——该族声明的反面用例失去红色，正是因为让它们变红的是规则本身而不是夹具巧合；其余 29 个用例不受影响，套件总数 30 不变，源文件还原后 `Buffer.compare` 逐字节相等。

**基准门禁自己踩到并修掉的两个错**：

1. **自描述解析器的贪婪**。它要读"本门禁有几条规则族"，初版用 `(\w+)\s+rule famil`，于是根 `README.md` 里的 "the gate then holds seven rule families" 被读成 `holds`，门禁反过来指责文档"声明了一个无法识别的计数"——解析器把自己的贪婪算在文档头上。改成以计数词本身做锚（`seven|eight|nine|ten|…`）后两个形态都能读对。
2. **测试清单的口径**。第九条要求文档声明的用例数必须为真，但"真值"从哪来：契约数能打开 `tests/contract/*.test.mjs` 逐文件重算，Rust 读数不能——它只在构建并跑完之后存在。于是两者用两种口径：契约数**重算**，Rust 读数与产生它的命令一起**落盘**再比对，和来源 sha256 同一种诚实模型（抓取时刻的陈述，不是可离线复算的摘要）。
3. **自描述计数的"第一处即答案"**。第十条要在五个表面里读"本门禁有几条规则族"，初版取文件里的**第一处**匹配。本轮给 `tools/README.md` 与门禁视图加了矩阵门禁（七条）的段落，位置都在基准门禁段**之前**，于是同一个文件里出现了两个不同的规则族数，门禁立刻把七算到了自己头上并报"declares 7, implements 10"。这不是文档写错，是解析器把"本文件里第一个数字"当成了"本门禁的数字"：一个表面同时描述两条门禁时，计数必须**按提及本门禁的位置定界**（本版取最后一段点名本门禁的块及其后一块；矩阵门禁补齐同名规则时又发现这个取法仍有两个洞，见上文第十条规则族），而**本门禁自己的源码**不需要定界（它不会把自己的数记到别人头上），所以那一面显式 `own: true` 退出定界。这件事本身就是第十条的加强版教训：**声明与归属要一起长大**。

**第九条规则族：实现面覆盖（§3.1 从散文变成断言）。** §3.1 声称自己是"本仓全部真实实现"，但**没有任何东西读它**——它只覆盖 3 个 crate 的 48 个可运行用例（外加 1 个 `#[ignore]` 集成用例记在已覆盖侧凑成 49），而工作区有 10 个文件、68 个用例：两个仓储 crate（`…-repository-memory` 与 `…-repository-sqlx`）连同 19 个用例根本没进表。也就是说整份审计里最让人安心的一张表，恰好是唯一没人核过的一张。现在该表是逐用例的，门禁自己遍历 `crates/**/*.rs` 把测试点出来，再做**双向记账**：本表引用的实现路径必须存在（带 `:line` 时该行必须在文件内）、引用的用例必须由所引测试文件声明、而工作区声明的每个 `#[test]` / `#[tokio::test]` 必须被认领恰好一次。

写这条规则时**又踩到同一类抽取口径缺陷**：`#[tokio::test(flavor = "multi_thread", worker_threads = 4)]` 带参数，只认 `#[tokio::test]` 的正则会**静默丢掉它**，第一版数出 67 个测试而工作区是 68 个。这与"operationId 抽取漏掉点号 ⇒ 一度报出 36 个未覆盖、真值 19"完全同源。连续两次栽在同一处，说明这不是偶然失误而是一个**必须写进检查表的动作**：任何覆盖率/缺口数结论，先对抽取规则做正反例自检（本轮的正例就是那条带参数的 `#[tokio::test]`，已锁进契约测试）。68 这个数与 `cargo test --workspace` 的 `67 passed / 1 ignored` 对得上：68 = 67 + 1，唯一的 `#[ignore = "..."]`（需要外部 PostgreSQL）不进默认运行。

**第十条规则族：自描述计数。** 门禁必须能被查问"你到底实现了多少条规则族"，答案必须由代码里的 `RULE_FAMILIES` 注册表推导，而不是手打的散文。加完第九条后，`tools/README.md` 与门禁视图**两个表面仍写着"八条"**，门禁立刻转红——这条门禁低估自己的覆盖范围，恰好就是它拒绝文档做出那类断言。读这个数还要求解析器分清散文与代码围栏：一节的结尾若是它自己的 usage 围栏，文件名会出现在声明计数那句话**之后**，把围栏当成"点名本门禁的块"会让定界晚一块，门禁于是对一节写得很清楚的散文报「没有声明规则族数」；而"命名块 + 其后一块"的取法还有第二个坑——`README.md` 的矩阵段与基准段只隔一个空行，矩阵段自己没有计数时会**借走**基准段的"ten"而静默通过。修法是**由窄到宽**：先只读命名块，读不到计数器才允许并入后一块，且后一块若点名了另一条门禁（即它属于那一节）则不并入。四处散文表面（`tools/README.md`、根 `README.md`、门禁视图，以及**本文档自己**）加上门禁的头注释，现在都各自声明同一个数。

**第十条规则族补上了它自己的漏网之鱼：本文档。** 上面那句"四处散文表面"在加第十一条之前只数到三处——本文档描述本门禁时既写了总数（本节表格那行）又**逐名列出了全部规则族**，却不在被读的表面清单里。也就是说，最常被评审引用的那份描述，恰好没有被任何东西核过。补它的难点在于**本文档无法按块定界**：它把两条门禁写进同一张表，本门禁那行与基准门禁那行之间没有空行，按块取会把两个数读成同一个声明，而且永远分不清是哪一个错了。于是这一面改成**按行定界**：只有同时点名本门禁、又写出 `N 条规则族` 的行才算声明。按行还有一个必须处理的形态——本文档多处用"第 9 条规则族""第 8 条规则族"指代**某一条**而非总数，这类序数与计数只差一个 `第`；`第 9 条规则族` 靠前一个字符就能排除，但**`第 11 条规则族` 不能**：`\d+` 会从第二个 `1` 开始匹配，把"十一条"读成"一条"。修法是两个断言一起加：既排除前一个字符是 `第` 或数字（挡住 `第 11` 与数字中段），又排除前面紧邻 `第` + 空白的写法（挡住 `第 9`）。这条缺陷是**新写的散文自己触发的**——本节这段说明里就有一句"第 11 条规则族"，门禁当场上报「本文档第 77 行把总数读成了 1」。

**第十一条规则族：形状取证（§1.2 的"可点证据"从形容变成断言）。** §1.2 是整份审计里**被信得最快、被核得最少**的一段：每行给出组件、路径、规模与真实状态，状态格里还带 `provider.rs:136` 这样的行号。此前没有任何东西解析过其中一个锚点。它的腐化不会报警——`crates/sdkwork-sandbox-provider-spi` 长出一个生产实现、状态枚举长出 `Pausing`、行号因为上面插了几行而整体下移，这张表都会继续断言旧的形状，而它正是被引用来回答"本仓到底有什么"的那张表。现在四个口径全部核验：

- **路径可解析**：反引号路径必须存在，末段含 `*` 按集合展开（`apis/commands/*.json` 是被当作一组文件核对的），含 `**` 则走子树。**声明某组件不存在的行必须点名"不存在的是什么"**——一个不得出现在 `crates/**/*.rs` 里的标识符，或一个不得匹配任何 crate 目录名的词。否则这句"没有"只能被相信，不能被重新推导。
- **规模可重算**：`N 模块` 等于该 crate `src/` 下的 `.rs` 文件数；`N 行` 等于所点文件的物理行数，口径与 `wc -l` 一致（末行有换行时不计多一行）。**这条第一次跑就抓到了真缺陷**：Provider SPI 那行写着"4 模块"，而 `src/` 下是 5 个 `.rs`（`capability` / `error` / `identity` / `lib` / `provider`）；同一张表里 Lifecycle Service 的"9 模块"却是**含 crate 根**的口径。相邻两行用了两套口径，必然有一行是错的，而此前没人算得出来。
- **行号锚可归因**：`file.rs:N` 必须在文件行数内，**且该行自己点名的东西必须真的出现在 `N` 起的几行窗口里**。一个仍然解析得通、却已指向无关代码的行号是这类腐化最安静的一半：读者跟过去，看到一段像样的声明，就信了这一行。`crates/sdkwork-sandbox-provider-local` 那行的 `lib.rs:7` 指向 `#[cfg(test)] mod fake_host_boundary;` 被拆开的两行，所以窗口按空白归一化后匹配，而不是要求同行。
- **每行都有物证**：整节标题承诺"可点"，那么点不到任何东西的行与点得到的行在读感上没有区别——这正是本规则族存在的理由。首跑就抓到两行（Memory Repository、Service Host）只有判词、没有任何可打开的东西。
- **点名的否定要能被反证**：加粗否定短语（`**无 …**` / `**零…**`）里凡是点名了标识符的，那些标识符必须在**该行所锚的那个文件**里不存在。范围取"所锚文件"而不是全仓，是因为这句话说的就是"那个文件里的那个构造没有"——第 2 行断言状态枚举没有 `Pausing`/`Paused`/`Recovering`，而这一行正是 §3.2 第 1 条治理阻塞的实现侧：门禁打开前写不了它，门禁打开后它会被静默改掉，所以这条否定的真假必须每次重算。没点名标识符的否定（`零命令` / `零路由` / `零生产实现`）不靠短语自证，而由行号锚承担——`main.rs:3` 上真的只有 `fn main() {}`，才是"零命令"可被核对的原因。

同一轮还顺手把 3 行判词改精确了：Command Executor 那行原写"全仓无 `SandboxCommandExecutor`"，实际上它**被一份契约声明为 `sandbox_required_bindings` 的一项**——`crates/**/*.rs` 里确实没有实现（这一点现在被门禁核验），但"全仓无"是错的，准确的形状是"已声明、未绑定"；Template / Snapshot / Fork / Pool 那行原写"全仓无对应实现"，没有点名任何可反证的东西，现在点名 `crates/` 下四个不得出现的同名 crate。

**形状取证随后扩到了 §5**（"我们比 E2B 强的地方"）：这 9 行与 §1.2 是同一类"可点证据"句子，但方向相反——§1.2 过期会**低**估本仓，§5 过期会**高**估本仓，而高估的那张表正是被引用来回答"追赶时哪些差异必须保住"的表。门禁对 §5 施加与 §1.2 同配对的核验：每行证据格至少一个可反证引用（仓库路径 / Rust 文件 / `REQ-*`/`ADR-*` 记录 / 在 `crates/` 或 `database/` 树中出现的裸标识符，`sandbox_session_lease` 表名因此从"一个名字"变成"一棵可被改名杀死的声明"）；裸 Rust 文件名拒绝，因为它在 crate 改名时恰好幸存下来指向虚无；行锚必须在界内，且该行引用的**其他物证至少一项**在锚点起 4 行的窗口内可见——§1.2 的引文配对法原样照搬。这条上线首跑就抓到 5 处真缺陷：`identity.rs:88` 与 `model.rs:24` 两个行锚已漂移（构造真身在 :97 与 :276，两者都仍落在"行号在文件内"的旧口径里，所以此前的在界检查看不见它们）、`provider.rs:50` 与 `capability.rs:2` 两个裸文件名、以及一行证据格只有 markdown 链接没有任何可反证 token。

**第十二条规则族：结论数字（§1.1 的每一个数字都变成可比对的断言）。** §1.1 标题是「直接回答」，是整份审计里**被评审引用得最多**的一段——它说的就是"能力集到底对齐没有"。这一段里的每个数字都是**别处某个数字的副本**：下一节的普查表、需求记录、决策记录、每份机器契约上的授权字段、证据注册表。此前没有任何东西把副本与原值放在一起，而这一段还会**自我重复**：那段"23 份 `*.contract.json` 中 22 份未授权"的话在 §1.1 与 §3.2 各出现一次，一处过期就会被引用两次。现在读的是**全文档的每一处**出现（不只是第一处），且 §1.1 赖以成立的五个数字必须出现在本节——数字悄悄消失留下的是一段"读起来仍像答案、其实什么都没断言"的文字。

- **普查数字比对的是矩阵行本身**，不是 §1.3 的普查表：两个副本一起写错同一个数照样转红，拿副本比副本只能证明两个副本彼此一致。契约测试里就有这条：把 §1.3 合计行与 §1.1 同时改成 4，门禁仍报"矩阵里是 3"。
- **需求与决策数字重算自记录**：`docs/product/requirements/` 与 `docs/architecture/decisions/` 逐份读 `status`。那句 ADR 明细（"28 份 `ADR` 中 25 份 `proposed`（3 份 `accepted`）"）里的**每个数字**也核——一份记录被推进 `accepted` 不改变数量，却会让这句话从"治理阻塞"变成一句不成立的话。
- **契约面既核数量也核身份**："23 份里 22 份声明未授权"只有在**第 23 份被点名**时才有意义，所以凡是不声明该字段的契约必须被文档点名；两份不以 `.contract.json` 命名的 `apis/` 机器契约按名单逐个对上（顺序无关，按集合比）；"没有任何一份授权实现"则对**两个**被本仓门禁承认的授权字段（`implementationAuthorized`、`releaseDecision.runtimeImplementationAuthorizationGranted`，分别由 `check-sandbox-human-review-signoff.mjs` 与 `check-sandbox-commercial-readiness.mjs` 读）逐份核，而不是从计数反推。
- **证据数字比对注册表的 `acknowledged` 块**（该块由 `check-sandbox-evidence-traceability.mjs` 保证等于活契约），**不在这里重算**——再写一份"什么算作被要求的证据 id"的抽取规则，正是证据门禁存在的意义所要防止的那种漂移。
- **来源读不到就报红**：注册表存在但没有 `acknowledged` 块时，门禁报"该数字无法被反驳"，而不是当作通过。这是"查过了"与"什么都没查"的分界。

首跑即暴露一个真缺陷，而且是**新规则自己的**：`apis/` 名单初版按排序后的字面串比对，而文档按散文顺序列出（`commands/` 在前、`async/` 在后），于是两处都报假红——是规则脆，不是文档错。已改成排序集合比对，并把这条写进契约测试，正是这条"先对抽取规则做正反例自检再报数"的老教训。

### 3.4 零需求断言

本文档有十几处断言某个能力"背后没有需求"。这类句子的宾语是**需求目录**，所以它和 §3.2 一样可以被目录推翻：§3.2 第 5 行就这么写错过一次。因此本文档**只在这一节里**断言"没有需求承载"，正文每一处这类句子都必须标明依据哪一行（`〔§3.4/N〕`）。

| # | 主题 | 关键词 | 说明 |
| --- | --- | --- | --- |
| 1 | 声明式 Template 定义与构建（`fromTemplate()` / `fromBaseImage()` / `copy()` / `setEnvs()` / `setStartCmd()`） | `template` `fromtemplate` `frombaseimage` `setstartcmd` | 产品要求见 [PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 4 节。矩阵第 3、25 行与 §4 P0 都断言它无需求承载 |
| 2 | Template 构建缓存与层级复用（Hot/Warm/Cold + 淘汰策略） | `cache` `layer` | 矩阵第 28 行；产品要求同上第 4 节 |
| 3 | 以 Dockerfile 或构建脚本作为构建输入 | `dockerfile` | 矩阵第 32 行；PRD 该节已写"构建输入允许使用 Dockerfile 或构建脚本" |
| 4 | Snapshot / Fork（含 `Sandbox.create(snapshotId)`） | `snapshot` `fork` | 矩阵第 23 行与 §4 P1；Fork 一致性语义未定 |
| 5 | Auto-resume / Auto-pause（Idle 收敛） | `resume` `pause` `autopause` `auto-pause` | 矩阵第 9 行；产品要求见 PRD 该文件第 9 节 |
| 6 | 端口暴露（public URL / `getHost`） | `port` `public` `gethost` | 矩阵第 58 行；产品要求见 [PRD-sandbox-surfaces.md](../../product/prd/PRD-sandbox-surfaces.md) 第 8 节 |
| 7 | MCP 执行面与 Skills | `mcp` `skill` | §4 P2；本仓只有传输级描述 |

这张表由 `tools/check-sandbox-e2b-parity-matrix.mjs` 的**第 8 条规则族**核验，判据三条：

1. **只能从这里断言**。正文里每处"无需求承载"的句子必须带 `〔§3.4/N〕` 引用，且**每一行至少被引用一次**——两向记账，与基准门禁对 71 个 operation 的做法一致。没有引用的引用与没有引用的行**都会转红**。
2. **关键词对着需求记录反证**。每个关键词都会去比对**每一条需求记录**的 id、文件名 slug 与 title；一旦命中，该行转红并点出是哪份记录。反证即验证：若以 `allocation` 登记"性能基准无需求承载"，门禁会因 `REQ-2026-0019-sandbox-runtime-pool-and-fast-allocation` 转红——**上一轮那个假断言正是这种形态**。
3. **口径只取 id / slug / title，不取正文**，这是刻意收窄的。实测各记录的正文提到这些词时说的都是别的事（进程 `suspended` spawn、密钥轮换的 `Pause/Resume`、流式响应的 `resume cursor`、`runtime recovery`），把它们当作"承载"会让整张表因为错误的原因变红。口径窄，所以它写在这里，而不是留给读者猜。

反过来，这条规则**不**判断"该能力是否真的没人承载"——那需要读正文语义。它把一句不可证伪的散文变成一句对着目录可反证的陈述，并让新增的同类句子无处可藏。

### 3.5 跨文档零需求断言对账

第 3.4 节登记的是**本文档**的断言。「某能力无需求承载」这个句型不是本文档独有的：逐字扫 `docs/**`（时点证据目录除外），句型一共出现在 6 个文档的 30 行上，而第 3.4 节的两向记账只覆盖本文档那 12 行。其余 5 个文档的 18 行由本节记账——其中一行已经被证伪，并在下表第二张里留了账。

**句型清单本身也是实测对象。** 2026-09-23 的归属判定轮发现：`无独立 ` + backtick + `REQ-*` + backtick + `（如 PRD.md 第 8 节 MCP 行、PRD-capabilities.md 第 11 节 Auto Pause 行、PRD-sandbox-surfaces.md 第 1 节 Port Exposure 行）是**同类断言**，却因决定词与 `REQ-*` 之间隔了一个形容词而被旧句型静默漏数——与 `#[tokio::test(...)]` 带参数被丢、operationId 含点被丢是同一类抽取口径缺陷。本轮把 `无独立` 并进句型并重算：跨文档断言从 15 行变为 **19 行**、新增第 5 个文档（其后 `REQ-2026-0028` 登记并按更正账第 2 行移出 SDK 家族断言，现值 **18 行**）；本文档自己的两行（矩阵第 17、73 行）按第 3.4 节规则补了 `〔§3.4/N〕` 引用。任何覆盖率结论必须先对抽取规则做正反例自检再报数——这条纪律第三次同向验证。

下表按文档逐段计数。`断言数` 是**重算值**：门禁在该段落（同级或更浅的下一节标题之前）重新数句型出现次数，再与声明值比对——加一句、删一句、或把整节搬走，都会转红。本文档自己的断言由第 3.4 节负责，故不在本表内。

| # | 文档 | 段落 | 断言数 |
| --- | --- | --- | --- |
| 1 | `docs/product/prd/PRD.md` | `尚未拆分的能力` | 11 |
| 2 | `docs/product/prd/PRD-capabilities.md` | `11. 能力对齐矩阵 (Capability Alignment Matrix)` | 4 |
| 3 | `docs/architecture/tech/TECH_ARCHITECTURE.md` | `2. 技术选型 (Technology Choices)` | 1 |
| 4 | `docs/architecture/views/gate-zero-current-state.md` | `验证门禁` | 1 |
| 5 | `docs/product/prd/PRD-sandbox-surfaces.md` | `1. 能力面对齐范围` | 1 |

**归属账（逐条真值判定）。** 普查证明每条断言被**数到**，不证明它**为真**。下表把上表的每一行断言逐一判到需求目录上：`判定` 取封闭词表（`口径句，不指能力`——定义性句子而非能力断言；`确认无承载`——已对目录核对、无记录承载；`已证伪，见更正账`——该断言已被推翻，由更正账留探针）；`已核对候选` 点名核对时最可疑的记录，`关键词` 是该能力的代表词。门禁核验三件事：候选记录必须可解析；对每对（关键词，候选），`requirementOwnsKeyword` 必须不命中——**命中即说明候选就是承载者，该行判定为假**；每文档归属行数必须与普查声明数相等，断言增删而判定不同步即转红。词法核验只保"候选不承载"这半句；"目录中无任何记录承载"这半句是逐条人工判断，随目录增长必须复核——这正是把它登记成账而不是写成散文的原因。

| # | 文档 | 能力 | 关键词 | 判定 | 已核对候选 |
| --- | --- | --- | --- | --- | --- |
| 1 | `docs/product/prd/PRD.md` | （§8 引言口径句，不指具体能力） | — | 口径句，不指能力 | — |
| 2 | `docs/product/prd/PRD.md` | 运行模式分层（Mode 0 / Mode 1） | `mode` | 确认无承载 | `REQ-2026-0002`、`REQ-2026-0008` |
| 3 | `docs/product/prd/PRD.md` | Template 与构建链 | `template` | 确认无承载 | `REQ-2026-0008`、`REQ-2026-0012` |
| 4 | `docs/product/prd/PRD.md` | Fork | `fork` | 确认无承载 | `REQ-2026-0002`、`REQ-2026-0021` |
| 5 | `docs/product/prd/PRD.md` | 按需内存与写时复制根文件系统 | `memory` | 确认无承载 | `REQ-2026-0008`、`REQ-2026-0013` |
| 6 | `docs/product/prd/PRD.md` | 端口暴露 | `port` | 确认无承载 | `REQ-2026-0014`、`REQ-2026-0023` |
| 7 | `docs/product/prd/PRD.md` | 网络 `shared` 模式 | `shared` | 确认无承载 | `REQ-2026-0014` |
| 8 | `docs/product/prd/PRD.md` | Sandbox 内 Agent 运行时 | `agent` | 确认无承载 | `REQ-2026-0024` |
| 9 | `docs/product/prd/PRD.md` | MCP 执行面 | `mcp` | 确认无承载 | `REQ-2026-0023`、`REQ-2026-0024` |
| 10 | `docs/product/prd/PRD.md` | Skills | `skill` | 确认无承载 | `REQ-2026-0023` |
| 11 | `docs/product/prd/PRD.md` | Node Drain 与迁移 | `migration` | 确认无承载 | `REQ-2026-0017` |
| 12 | `docs/product/prd/PRD-capabilities.md` | （§11 引言口径句） | — | 口径句，不指能力 | — |
| 13 | `docs/product/prd/PRD-capabilities.md` | Egress Policy 行的 `shared` 模式 | `shared` | 确认无承载 | `REQ-2026-0014` |
| 14 | `docs/product/prd/PRD-capabilities.md` | Auto Pause | `pause` | 确认无承载 | `REQ-2026-0019`、`REQ-2026-0020` |
| 15 | `docs/product/prd/PRD-capabilities.md` | MCP | `mcp` | 确认无承载 | `REQ-2026-0023`、`REQ-2026-0024` |
| 16 | `docs/architecture/tech/TECH_ARCHITECTURE.md` | 边缘路由与端口暴露 | `port` | 确认无承载 | `REQ-2026-0023` |
| 17 | `docs/architecture/views/gate-zero-current-state.md` | Benchmark 套件与容量基线 | `benchmark` | 已证伪，见更正账 | `REQ-2026-0019` |
| 18 | `docs/product/prd/PRD-sandbox-surfaces.md` | Port Exposure | `port` | 确认无承载 | `REQ-2026-0023` |

第 18 行说明：gate-zero 视图那句关于「`REQ-*` 计数为零」的历史措辞是对已修正断言的**转述**（它讲的就是那条规则为何存在），断言本体已在更正账第 1 行留探针；按「已证伪断言全仓皆假」的口径，这里判到同一条更正上。第 15 行的 Auto Pause 是本轮句型拓宽后新入账的断言：`REQ-2026-0019`（池化）与 `REQ-2026-0020`（热状态保留）都不含 pause 语义，PRD-runtime-execution-model 第 9 节的产品要求仍无需求承载。

被证伪的断言必须在这里留账，且**原文必须已经消失**：`缺失探针` 是一个不得再出现在该文档里的字面串。它是这条「已修正」声明的**反证物**——与第 1.2 节要求「不存在」必须点名不存在什么，是同一条规则；承载需求必须能在需求目录里解析到记录。

| # | 文档 | 能力 | 承载需求 | 缺失探针 |
| --- | --- | --- | --- | --- |
| 1 | `docs/product/prd/PRD.md` | Benchmark 套件与容量基线 | `REQ-2026-0019` | `无参考硬件定义` |
| 2 | `docs/product/prd/PRD.md` | SDK 家族与 API 权威 | `REQ-2026-0028` | `REQ-*；无`、`apis/ 权威契约` |

第 1 行的来龙去脉：`PRD.md` 第 8 节曾把「Benchmark 套件与容量基线」列为无需求承载，并断言「无参考硬件定义」。该断言已被推翻——`REQ-2026-0019` 的 Goals 与 Performance 行要求「在公开参考环境和固定工作负载中证明 Pool Claim 到 Sandbox Running Ready 的 p50/p95/p99」并记录固定硬件，`tools/bench-sandbox-lifecycle.mjs` 与 [TECH-performance-baseline.md](TECH-performance-baseline.md)（两平台实测）也都在树里。该行已改写为「已有承载 + 仍缺什么」，探针保证旧措辞不会悄悄回来。

**普查表给出覆盖，归属账给出真值判定，两者都不是免检结论。** 覆盖的判据（句型清单）在本轮就被抓到漏数 4 行；真值判定的词法半句由门禁逐对复算，语义半句（"目录中确实无人承载"）随每份新增需求记录增长而必须复核——新增记录若与某行关键词冲突，门禁会在候选之外转红（第 3.4 节的目录反查对全目录生效）。

## 4. 缺口清单

每条标注改动性质：**纯增量**（新增能力，不动既有模型）或**设计级**（需要新的权属模型、共享类型变更或跨仓契约，成本差一个数量级）。

### P0 — 阻塞"能创建任何沙箱"

| 缺口 | 性质 | 说明 |
| --- | --- | --- |
| 零运行入口（无 HTTP/RPC、无 CLI、无 Service Host wiring） | 设计级 | `ROUTE_CRATE_COUNT: 0`、`fn main() {}`、service-host 5 行。需要 `REQ-2026-0023`（internal control plane）与 `REQ-2026-0009`（service host）进入 `ready` |
| 零真实 Provider（Local 只有 fake host boundary） | 设计级 | `REQ-2026-0003` 的 5 条 Readiness Blocker 全是人工评审/接受 |
| 无 Template（含定义、构建、缓存、tags、start command） | **设计级** | E2B 快速创建与快速部署的**全部**依赖它。本仓零 `REQ-*`〔§3.4/1〕；与 Firecracker 制品元组的权威边界未定（见 `PRD.md` 第 9 节待决问题） |
| 无 Command / Terminal / Filesystem 执行面 | 设计级 | `REQ-2026-0007`、`REQ-2026-0024` 仅契约且显式禁止物化 |

### P1 — 阻塞"创建得快"

| 缺口 | 性质 | 说明 |
| --- | --- | --- |
| 无 Runtime Pool（`PreparedSlot` / `WarmMicroVmSlot` / fenced Claim） | 设计级 | `REQ-2026-0019`（draft）。无 Pool 则每次都是冷启动，"快"无从谈起 |
| 无 Snapshot / Fork（含 `Sandbox.create(snapshotId)`） | 设计级 | 产品要求已写，`REQ-*` 为零〔§3.4/4〕；Fork 一致性语义未定 |
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
| 无 MCP 执行面与 Skills | 设计级 | 仅传输级描述，无 `REQ-*`〔§3.4/7〕 |

### P3 — 平台与集成

| 缺口 | 性质 | 说明 |
| --- | --- | --- |
| 无 BYOC / 多区域 / 限额执行 | 设计级 | `REQ-2026-0026` 与 `0015`/`0016`/`0018` 均为 draft |
| 无 Code Interpreter / Desktop / Browser 能力面 | 设计级 | PRD 非目标明确 Browser 与 Computer Use 不在第一阶段 |
| 无性能基准套件与参考硬件基线 | 纯增量 | 所有性能数字都是工程目标，无测量则不能写入 Release Evidence |

### 操作面覆盖记账（70 / 71）

本节此前只回答"已枚举的 78 项对齐得怎么样"，不回答"E2B 的操作面是否枚举完整"。`specs/sandbox-e2b-capability-baseline.json` 的 `operationCoverage` 现在**逐操作**记账：捕获到的 71 个 OpenAPI operation 中，**70 个被某条矩阵行以 operationId 引用**，仅 1 个例外。

这个数字是**查出来的，不是假设的**。首轮测量报出 19 个"无行判定"，其中 18 个其实是**记账错误**——它们本就属于某条行判定的能力，只是没有作为证据挂上去：

| 曾报未判定的 operation | 实际归属行 | 说明 |
| --- | --- | --- |
| Templates REST 建 / 查 / 改 / 删 + 构建流水线 + 构建产物（15 个） | **行 26**（`e2b template init` / `build` / `deploy`） | 该行判定的正是 Template 的创建与构建生命周期：`postTemplates*`、`getTemplate`、`listTemplates*`、`patchTemplate*`、`deleteTemplate`、`postTemplateBuild*`、`getTemplateBuildStatus`、`getTemplateBuildLogs`、`getTemplateFile` 是这条能力的 REST 面 |
| `getTemplatesAlias` | **行 29**（Template tags / versioning / names） | alias 就是版本化命名机制，与 tags 同属一行 |
| `getEnvVars` | **行 1**（`Sandbox.create()` / 环境变量） | 环境变量能力的读取端点 |
| `getMetrics` | **行 65**（`getMetrics()`） | 该行已判 `listSandboxesMetrics` / `getSandboxMetrics`，聚合端点同属一族 |

剩余 1 个**确实没有行判定**，已在案登记：

| 未判定的 operation | 为什么没有行覆盖它 |
| --- | --- |
| `getHealth`（`GET /health`） | 控制面存活探针。E2B 并不把它作为沙箱能力对外承诺；本仓的对应面是 Service Host readiness（`REQ-2026-0009`），不在"沙箱能力行"的范围内 |

⚠️ **另一个教训：抽取口径写窄，赤字会凭空翻倍。** 首轮还用 `[A-Za-z0-9_]+` 抽 operationId，而 `filesystem.Filesystem.Stat`、`process.Process.Start` 这类 id **含点**，被静默丢弃 ⇒ 一度报出 **36** 个未覆盖，真值 19。任何"覆盖率 / 缺口数"结论，必须**先对抽取规则做正反例自检再报数**。

`node tools/check-sandbox-e2b-field-parity.mjs` 的第七条规则要求这 71 个 operation **恰好被计入一次**：要么被某条行的 `e2bFields` 以 `[operationId]` 引用，要么在上表带非空理由登记。**两个方向都红**：既未被引用也未被登记 = 红；既被引用又登记为未判定 = 红。将来补写行引用某 operationId 时，对应的 `unjudged` 条目必须删除，否则门禁立刻转红。

### 解锁路径（唯一路径，且是人工决策）

本仓不是"有些功能没做完"，而是**治理门禁未打开**。四条硬门禁互相依赖：

1. 28 份 `REQ-*` 中 3 份 `ready`（5 `accepted` / 20 `draft`）→ 其余逐份人工评审进 `ready`。
2. 28 份 `ADR` 中 25 份 `proposed`（3 份 `accepted`）→ 其余需 `accepted`。
3. 机器契约全部未授权（23 份 `*.contract.json` + 2 份 `apis/` 机器契约，全部 `implementationAuthorized: false` 或独立声明 `runtimeImplementationAuthorizationGranted: false`）→ 需人工评审签字后翻转。
4. 8 份契约声明的 127 个证据 id 中 125 个无产出者 → 需真实 runner 与人工评审闭合。

当前签字积压（机器读数）：28 份评审记录中 22 份为 `pending-human-review`，其中 **14 份被机器契约点名门控**。完整清单与 5 步签字程序见 [human-review-signoff-backlog.md](../../engineering/human-review-signoff-backlog.md)。

`node tools/check-sandbox-commercial-readiness.mjs` 输出 `NO-GO`：6 个交付切片 blocked、5 个缺 `ready` 契约、4 个跨仓权威 blocked。这是**预期行为**，不是缺陷；任何把它读成"就差一点"的解释都是错的。

## 5. 我们比 E2B 强的地方

这一节对决策同样重要：以下能力是本仓**已有**、E2B 公开文档中**没有等价承诺**的，属于应当保留而不是在追赶中丢掉的差异。

| 本仓能力 | 证据 | 为什么保留 |
| --- | --- | --- |
| 单写者 Lease + 单调 Fencing Token 防止双重活动所有权 | `crates/sdkwork-sandbox-provider-spi/src/identity.rs:97`（`SandboxFencingToken`）、`crates/sdkwork-intelligence-sandbox-service/src/service.rs`、`sandbox_session_lease` 表 | E2B 未公开等价机制。多控制器竞争下的 Provider 副作用去重是自建平台必须自证的 |
| 稳定 `sandbox_operation_sequence` + 恢复重放校验 + 幂等 ledger | `crates/sdkwork-intelligence-sandbox-service/src/model.rs:276`（`replay_sandbox_operation`）、`REQ-2026-0020` | 恢复时先重放 Create/Start/Stop/Destroy 并校验组合，非法组合关闭失败。E2B 不对外承诺这一层 |
| Tenant-scoped 加密的 Provider 恢复元数据 + 有界密钥轮换/重加密 | `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/encryption.rs`、`REQ-2026-0006` | 已有候选实现**与测试**，是本仓少数可点的实现面 |
| Provider 无关 SPI + fail-closed Capability/IsolationAssurance 协商 | `crates/sdkwork-sandbox-provider-spi/src/provider.rs:55`（`satisfies_sandbox_requirements`）、`crates/sdkwork-sandbox-provider-spi/src/capability.rs:2`（`RuntimeCapability`） | 禁止静默降级到更弱隔离；E2B 是单一 microVM 层，不存在这层协商 |
| 显式运行模式分层（Shared / Namespace / MicroVM）+ 禁止回退 | `docs/product/prd/PRD-runtime-execution-model.md` 第 2 节 | 成本分层能力；E2B 只有一种隔离强度 |
| 数据驻留与恢复 Gate（Local `device-local-persistence` / Cloud region tuple） | `REQ-2026-0022`、`REQ-2026-0026` | 企业私有化与合规场景的硬要求 |
| 工作区业务权威在 `sdkwork-agents`，Sandbox 只拥有运行投影 | `REQ-2026-0004`、`ADR-20260728-agents-workspace-and-sandbox-attachment-ownership` | `Workspace ≠ Sandbox` 不等式：销毁执行环境不连带销毁用户数据 |
| 跨仓不可变 Release Set 与多维兼容矩阵 | `REQ-2026-0027` | 四仓联合发布的可追溯性 |
| 127 个证据 id 的机器可读注册表 + 双向漂移门禁 | `specs/sandbox-real-evidence-registry.json`、`tools/check-sandbox-evidence-traceability.mjs` | 把"声称完成"与"有证据"分开，是本仓最重要的自证机制 |

在这些维度上，本仓的设计**比 E2B 更严**。问题不在设计，在于**没有一行运行时代码把它们跑起来**。

## 6. 复核方式

```bash
node tools/check-sandbox-e2b-parity-matrix.mjs
node tools/check-sandbox-e2b-field-parity.mjs
node tools/check-sandbox-requirement-traceability.mjs
node tools/check-sandbox-evidence-traceability.mjs
node tools/check-sandbox-human-review-signoff.mjs
node tools/check-sandbox-commercial-readiness.mjs
cargo test --workspace
node --test tests/contract/*.test.mjs
```

`check-sandbox-e2b-parity-matrix.mjs` 证明本文档**内部自洽**（标记词汇、行号与形状、状态格、census 分区、`REQ-*`/`ADR-*` 可解析，以及第 3.2 节每条空档的性质与取证方向）；`check-sandbox-e2b-field-parity.mjs` 证明本文档**确实读过基准**（78 行逐行的 E2B 字段级证据、来源 provenance、与本文档逐行 join、`基准仅索引` 棘轮、71 个 operation 的覆盖记账，以及本文档自己声明的用例数——契约数逐文件重算、Rust 读数与命令一起落盘后比对）。前者全绿时后者仍可能红——那正是"census 加得起来、却全压在页面标题上"的形态。反过来，后者无法保证前者：基准全绿而第 3.2 节写过一句不存在的"零需求承载"，就是本轮实际发生的事。

全局标准在 `../sdkwork-specs/` 下保持权威，本分片只引用不复制。
