# REVIEW-20260923: E2B 上游源码级能力对齐研究

Status: active

Owner: SDKWork Runtime Platform

Date: 2026-09-23

Outcome: 把 E2B 上游**源码**（四个仓库，浅克隆，HEAD 见 §1）vendored 到本仓 gitignore 的 `external/` 下，并以源码而非文档页为基准重做对照。**四项发现推翻了文档级审计看不见的形状**：（1）基准里的「71 operations」不是单一 REST 面，而是**控制面 REST 与沙箱内 envd 数据面的聚合**，两个面各自有独立契约、独立鉴权、独立传输；（2）基准记录的 SDK 版本（JS v2.38.2 / Python v2.37.1）已落后上游 tag `e2b@2.51.0` 十三个 minor；（3）E2B 基础设施**全部是 Go，零 Rust**，`firecracker/` 只是内核与 VMM 版本的工具目录；（4）E2B 的沙箱创建**内部就是一次快照恢复**，且**不存在沙箱级预热池**。这四项都不改变「78 行能力 ✅ 0」的结论，但改变**对齐的工作定义**：REST/SDK 对齐必须按两个面拆开立项，基准需要按上游当前版本重抓。本报告不改变任何实现授权状态，不构成对任何 Provider、API、SDK、调度器、隔离策略或部署 Profile 的实现授权（`AGENTS.md` Agent Execution Rules）。

## 0. 目的与边界

既有 [E2B 能力对齐审计](../../architecture/tech/TECH-e2b-capability-parity.md) 的基准 `specs/sandbox-e2b-capability-baseline.json` 是从 **E2B 文档页抓标识符**得来的（101 个来源逐页记录 url / 字节数 / sha256）。它的 `captureNote` 自己声明了边界：「Only identifiers … are recorded; E2B page bodies are not vendored.」

本报告补的正是这条边界：**读上游实现本身**。因此判据从「文档页的小节标题里出现过这个词」升级为「源码里存在这个符号、常量和调用链」。

分工不变：能力对齐审计回答「能力集对齐没有」，[功能走查](REVIEW-20260922-sandbox-functional-module-walk.md)回答「实现逻辑是否有洞」，[Prompt 对齐](REVIEW-20260923-sandbox-implementation-prompt-alignment.md)回答「外部实施指令与 Canon 是否冲突」，本报告回答 **「上游实现层面的真实形状是什么，文档级基准在哪些地方看不见它」**。

**本报告不记录任何上游代码正文。** 只记录标识符、常量名与值、文件路径与行号、组件职责——与既有基准同一种诚实模型。

## 1. vendored 参考源码（取证）

四个仓库浅克隆（`--depth 1 --single-branch`）到 `external/`，共约 52 MB。上游代码不是本仓财产，`external/` 因此被 gitignore；本仓通过 `specs/` 里的标识符记录对齐，不通过 vendored 正文。

| 仓库 | HEAD | 上游提交时间 | 规模 |
| --- | --- | --- | --- |
| `external/infra` | `0c21aa2277b59a1d040761ed3fbbb29a78775470` | 2026-09-23T06:44:31Z | 28 MB |
| `external/E2B` | `ccaf9fc0ffe6ac39c7ec786af7608ab1de19467b` | 2026-09-18T12:06:48Z | 12 MB |
| `external/desktop` | `17ddc44f31080af9f2d0fa0fa767525fefd9882c` | 2026-09-11 | 11 MB |
| `external/code-interpreter` | `f56a1edf750e20a96f847df57e0063eeeb5e13c3` | 2026-09-10 | 1.2 MB |

**浅克隆是一个必须声明的取证限制**：没有历史，因此本报告**不能**回答「某能力是何时引入的」「docs 与源码谁先动」这类演进问题。§2.1 的方向性差异因此只能记录为「两向差异都存在」，不能归因。需要演进结论时必须重抓全量历史。

**vendoring 暴露的一个真缺陷（已修）**：`tools/check-sandbox-doc-integrity.mjs` 走文件系统而不读 gitignore，于是把 `external/` 下的 69 份上游 markdown 当作本仓文档校验，报出 4 条本仓无权修的死链，并连带把该门禁的契约套件转红。`.gitignore` **不足以**把一个目录排除在门禁之外。修法是按既有约定把 `external` 加入该门禁的 `SKIPPED_DIRECTORIES`（该表已有 `target`、`.workbuddy`、`node_modules` 三个同类目录——判据是**内容归属**而不是内容是否有趣）。变异自证：去掉该条目后**恰好 2 条**用例转红（新增的排除用例 + 「仓库自身必须干净」），异族 0 条，套件总数不变，还原后逐字节一致。

## 2. 四项推翻性发现

### 2.1 基准的「71 operations」横跨两个契约面

基准把 71 个 operation 记成一个集合，并在 `operationCoverage` 里按「被某条矩阵行引用 / 登记为未判定」做双向记账。源码显示这个集合**是两条独立契约面的并集**：

| 面 | 权威文件 | 形态 | 鉴权 |
| --- | --- | --- | --- |
| 控制面 | `external/infra/spec/openapi.yml` | 74 个 operation，**无 `operationId` 字段** | `X-API-KEY`（团队级 API key） |
| 数据面 | `external/E2B/spec/envd/envd.yaml` + `spec/envd/{filesystem,process}/*.proto` | Connect RPC（protobuf）+ 少量 REST | `X-Access-Token`（沙箱级 envd token）+ URL 签名 |

基准所依据的 `https://docs.e2b.dev/openapi-public.yaml` 正是把两者**拼在一起**的聚合文档。因此：

- **源码有、基准无 = 28 个**：其中 2 个是 SDK 公开面（`POST /v2/sandboxes`、`POST /v2/sandboxes/{sandboxID}/connect`，两者都出现在 JS 生成客户端 `api/schema.gen.ts` 与 Python 生成客户端里），26 个是内部/管理面（admin、api-keys、nodes、clusters/rigs、events/webhooks）。
- **基准有、源码无 = 25 个**：21 个是 envd 数据面 operation（属 §2.1 另一面，不在控制面契约里），4 个是**已弃用并已从源码删除**的控制面旧路径（`POST /templates`、`POST /templates/{templateID}`、`POST /templates/{templateID}/builds/{buildID}`、`POST /v2/templates`），源码已由 `POST /v3/templates` 与 `POST /v2/templates/{templateID}/builds/{buildID}` 取代。
- 附加契约**六份**，**均不对 SDK 公开**，分属两个仓库：
  - `external/infra/spec/` 三份：`openapi-edge.yml`（14 ops / 14 paths，边缘与节点面）、`openapi-hyperloop.yml`（2 ops / 2 paths，沙箱内 sidecar 面向 API）、`openapi-dashboard.yml`（46 ops / 38 paths，Web 控制台后端）。
  - `external/E2B/spec/` 三份：`openapi-volumecontent.yml`（独立于沙箱生命周期的 volume 内容面，`security: VolumeJWT`，与沙箱级 envd token 不同）、`mcp-server.json`（163 KB，JSON Schema 形态而非 OpenAPI，MCP Gateway 的服务器目录）、`envd/envd.yaml`（envd REST 面，见下）。

**同一份控制面契约在上游被 vendoring 了两份，且已经分叉**：`external/infra/spec/openapi.yml`（控制面自己的副本）与 `external/E2B/spec/openapi.yml`（SDK 仓库携带的副本）**operation 面完全一致（各 74 ops / 57 paths），但 schema 正文有 22 行差异**——SDK 那份把 `outstandingWork` / `maxSandboxes` 写进 required 列表且描述更严（"Node-scoped configured sandbox admission limit. Nonpositive values reject creation. Omitted when unknown or not an orchestrator."），控制面那份是 "Cached node-scoped sandbox admission limit." 且两个字段不在 required 里。本报告的 operation 记账读的是 `infra` 那份；但 **SDK 是从 `E2B` 那份生成的**，因此凡涉及 node 准入字段是否可缺省的回答，必须指明读的是哪一份——只读一份会让另一份的 operation 面漂移无人看见。采样器现对两份都取数（`twinControlPlaneContracts`）。

**对对齐工作的影响**：「REST API 对齐」不是一个工作项。控制面 REST 与数据面 RPC 的能力、鉴权、错误语义、版本策略**各自独立**，必须拆成两个对齐面分别立项；把 envd 的数据面 operation 记进控制面基准，会让「已判定 70/71」这个读数在错误的口径上自洽。

### 2.2 基准记录的上游版本已落后十三个 minor

| 组件 | 基准记录 | 上游当前（源码 `package.json` / `pyproject.toml`） |
| --- | --- | --- |
| JS/TS SDK | `v2.38.2` | **2.51.0** |
| Python SDK | `v2.37.1` | **2.51.0** |
| CLI | `v2.16.1` | **2.20.0** |
| Code Interpreter JS / Python | `v2.7.0` / `v2.9.0` | **2.8.0** / **2.10.0** |
| Desktop JS / Python | `v2.3.1` / `v2.4.2` | **2.4.0** / **2.6.0** |

版本号不是「文档页里写着」而是 tag 与清单文件里的字段，因此这条差异是可直接复核的事实。基准的 `capturedAt` 是 `2026-09-22T09:36:31Z`——**同一天**，说明这是**基准抓取口径**的问题（抓的是文档页引用的版本，不是上游当前版本），不是时间流逝。

### 2.3 E2B 基础设施全部是 Go，零 Rust

`external/infra` 全仓 `.rs` 数量为 **0**，无任何 `Cargo.toml`；`go.work` 统一了 **12** 个 Go module（`packages/` 下 11 个 + `tests/integration`）。这个 12 值得说明来源：`use ( … )` 块里 `tests/integration` 前有一空行，按 `packages/` 前缀数会读到 11，必须按 `use` 块的**条目数**数（判据：`go.work` 的 `use` 块中非空行条数，实测 12）。`firecracker/` 目录**不是 VMM 源码**，而是 FC 内核配置、busybox 与 VMM 版本的工具与配置目录（shell / Python / kernel config），构建产物上传到对象存储供 orchestrator 按版本拉取——真正的 Firecracker 是**外部二进制**，本仓只引用版本常量。

本仓是 Rust 工作区，E2B 是 Go 工作区加外部 VMM 二进制。这条不影响能力对齐，但影响**对照的方式**：可对照的是能力、契约、常量与机制，**不是** crate 划分、模块布局或错误类型设计。任何「把 E2B 的包结构搬进本仓」的读法都是误读——它同时会违反本仓 `NAMING_SPEC.md` 对职责命名的要求。

### 2.4 沙箱创建就是一次快照恢复，且没有沙箱级预热池

`docs/ARCHITECTURE.md` 明写两条设计主线，其一即「sandbox = 恢复的 snapshot（懒加载内存 + COW rootfs）」。源码印证：**fresh create 内部走的就是 resume 路径**（恢复模板 base snapshot），只有 filesystem-only 模板/构建或显式请求才 cold boot。

更反直觉的是：全仓搜索 `prewarm` / `warm pool` / `sandbox pool` **没有实现命中**——E2B **不存在**沙箱级预热/复用池。它把「快」建立在**快照恢复**上，复用只发生在三个资源池：网络槽位池（New 32 / Reused 100 / 归还延迟 3 s）、NBD 设备池、模板本地缓存（TTL 25 h）。

**与本仓的关系**：本仓 `REQ-2026-0019` 承载的是 **tenant-neutral `PreparedSlot` + fenced Claim 的运行时池**，这与 E2B 的形态**不同**——E2B 用「快照恢复 + 资源槽位池」达成快速创建，本仓用「预热池 + 有界租赁」。这是**有意的形态差异而非缺口**：两者都以「创建延迟」为指标，但机制不同，对照时必须按指标而不是按机制判对齐，否则会把一项已由产品需求承载的设计判成「E2B 有而本仓没有」。

### 2.5 基准漂移的实测（`recaptureCommand` 的第一次执行）

基准的 `recaptureCommand` 写好了完整重抓命令，但此前从未执行过。本轮执行了其中成本最低、权威最高的两个文件：

| 来源 | 基准记录 | 本轮重抓 | 判定 |
| --- | --- | --- | --- |
| `https://docs.e2b.dev/openapi-public.yaml` | 166293 字节 / `84d2bfe1…dea7` | 166293 字节 / `84d2bfe1…dea7` | **逐字节一致，未漂移** |
| `https://docs.e2b.dev/llms.txt` | 33167 字节 / `88725aeb…dc73` | 33174 字节 / `818d97f2…23db` | **已漂移**（+7 字节） |

完整 sha256：`openapi-public.yaml` = `84d2bfe140d38295d963efcde6586933d3a81b0774828c2f7d4ebda76915dea7`（两侧相同）；重抓的 `llms.txt` = `818d97f2040676ed4d9c2a1586e75afc135e4e47f70820c0e232d62e24fd23db`。抓取落盘在 gitignore 的 `target/e2b-baseline/`（证据，不是缓存）。

**这条结果把重抓变成一件有边界的事**：71 个 operation 的**权威文件没有移动**，因此 operation 记账与 §2.1 的两向差异**不需要重算**；会动的只有索引与 99 个文档页，即承载 SDK 方法名与页面小节标题的那些行。重抓的风险因此集中在「文档页字段」而不是「操作面」——这与 §2.2 的版本漂移是同一处漂移的两个来源。按 `recaptureCommand` 的写法，重抓必须逐来源记新 sha256 并**比对**，而不是覆盖：哈希是抓取时刻的陈述，不是可离线复算的摘要。

## 3. 上游组件设计（源码级）

`go.work` 的 12 个 module 的职责与端口。本表 12 行与 12 个 module **不是一一对应**，对账如下：`db` 与 `clickhouse` 合并为一行（两者是同一个 module 下的两个包组），`otel-collector` **不是 module** 而是纯配置目录，故合并与新增相加后仍为 12 行。

| 组件 | 职责 | 接口 |
| --- | --- | --- |
| `api` | 控制面入口：生命周期、放置、鉴权、配额 | REST `:80`；内部 gRPC `:5009`、edge gRPC `:5109` |
| `orchestrator` | 单二进制双角色：跑 Firecracker 沙箱 / 构建模板，角色由 `ORCHESTRATOR_SERVICES` 选 | gRPC `:5008`、代理 `:5007` |
| `envd` | **每个 VM 内的 agent**：进程与文件系统 API | `:49983`，Connect RPC + REST |
| `client-proxy` | 边缘路由：`<port>-<sandboxID>.<domain>` → 正确节点 | 代理 `:3002`、健康 `:3003` |
| `dashboard-api` | Web 控制台后端；**从不连 orchestrator** | REST `:3010` |
| `db` / `clickhouse` | PostgreSQL 迁移与查询 / ClickHouse schema 与批量写入 | 库 |
| `auth` | 鉴权库（API key、OIDC JWT、admin JWKS），被 api 与 dashboard-api 复用 | 库 |
| `shared` | protos、遥测、存储客户端、代理引擎、feature flags、服务发现 | 库 |
| `local-dev` | 本地全栈 compose + 数据库播种 | compose |
| `nomad-nodepool-apm` | Nomad Autoscaler 插件：节点池感知扩缩 | 插件 |
| `otel-collector` | Collector 配置（无源码，非 module） | 配置 |
| `tests/integration` | 跨组件集成测试 module（唯一非 `packages/` 前缀者） | 测试 |

**控制面 / 数据面分界是硬边界**：数据面流量（client → client-proxy → Redis 查 `sandbox:catalog:{id}` → 节点代理 `:5007` → veth/tap 进 VM）**从不经过 API**。envd 同时承载数据面（process/filesystem）与控制面（`/init`、freeze/thaw）两类接口，靠契约里的 `x-internal: true` 标记隔离——被标记的端点对沙箱代理一律 404，只有 orchestrator 经 host 网络与沙箱 slot IP 可达。这是「同一个 agent 进程同时是数据面服务与控制面执行器」的一种解法，值得在拆 `REQ-2026-0023`（内部控制面）时作为设计输入。

**这个隔离是「契约声明式」的，不是代理里手写的名单**，本轮已把它量到具体端点。`envd.yaml` 的 11 个 REST path 被标记切成两半：**6 个 orchestrator-only**（`/init`、`/freeze`、`/unfreeze`、`/collapse`、`/fsfreeze`、`/fsthaw`——生命周期冻结与文件系统冻结）与 **5 个客户端可达**（`/health`、`/metrics`、`/envs`、`/files`、`/files/compose`）。上游注释明写沙箱代理的拒绝列表**由该标记生成**，因此「契约里加一个标记」就是「对外关闭一个端点」的完整动作——信任域由 spec 派生，代理代码无法与契约漂移。这条对本仓的隔离策略是可直接借用的形态：**拒绝列表必须从契约派生而不是在代理里另写一份**，否则两处必然分叉。两个抽取陷阱已写进采样器：标记必须**行锚定**（`envd.yaml` 顶部解释该标记的注释里含 `x-internal: true` 字面量，子串搜索会多数一个并归给紧跟其后的那个端点），且必须**按 path 块限定作用域**（全文件计数无法说明是哪一个端点被关闭）。

## 4. 性能实现与常量（对照面）

E2B 的暂停/恢复是**机制涌现**，不是单一算法：UFFD（`userfaultfd`）懒加载内存 + COW rootfs 只导出脏块 + `memfd` 避免 `process_vm_readv` + 后台导出把 reflink/seal 移出暂停关键路径。文档承诺的「约 4 s/GiB 暂停、约 1 s 恢复」在源码里**没有任何常量直接编码**——它是上述机制共同作用的结果，官方标定来源未能确认。

下表**不是手抄清单**：21 个值全部由 `tools/audit-sandbox-e2b-upstream-source-parity.mjs` 的 `scanPerformanceConstant` 从 vendored 树上抽取，并与 `RECORDED_PERFORMANCE_CONSTANTS` 逐条比对；任一值变化、消失、或**值的种类**变化都会让它以 exit 1 报出。这张表因此是一次抽样输出的转录，而不是一段记忆。

**两类值必须分开读**——这是本节第一个结论，也是上一版表格的错误所在：

| 值种类 | 条数 | 含义 |
| --- | --- | --- |
| 编译期内建（`const`） | **13** | 只有重新构建才会变；可当作上限读 |
| 运行时 flag 默认值（`NewIntFlag(name, default)`） | **8** | flag 服务可在**不部署**的情况下改写；它是*默认值*，不是上限 |

把后者当上限读会得到一个与运行中的服务不一致的容量模型。8 个 flag 的**运行时键**（运维真正去设置的那个字符串，不是 Go 标识符）与默认值一并记录，因为键改名会打断所有既有覆盖、而默认值不变——只看值的读数是看不见这次漂移的。

| 主题 | 标识符 = 值 | 值种类 | 运行时键（仅 flag） |
| --- | --- | --- | --- |
| 原地 checkpoint | `inPlaceStateFlipTimeout` = `40 * time.Second` | const | — |
| UFFD 并发 | `maxRequestsInProgress` = `4096`；`maxWPResolvesInProgress` = `256` | const | — |
| 内存预热并行度 | `MemoryPrefetchMaxFetchWorkers` = `16`；`MemoryPrefetchMaxCopyWorkers` = `8` | flag | `memory-prefetch-max-fetch-workers` / `memory-prefetch-max-copy-workers` |
| 网络槽位池 | `NewSlotsPoolSize` = `32`；`ReusedSlotsPoolSize` = `100`；`ReturnDelay` = `3 * time.Second` | const | — |
| 单节点容量 | `MaxSandboxesPerNode` = `200`；`MaxStartingInstancesPerNode` = `3` | flag | `max-sandboxes-per-node` / `max-starting-instances-per-node` |
| 放置与超分 | `BestOfKMaxOvercommit` = `400`；`BestOfKAlpha` = `50`；`BestOfKSampleSize` = `3` | flag | `best-of-k-max-overcommit` / `best-of-k-alpha` / `best-of-k-sample-size` |
| 快照与内存粒度 | `HugepageSize` = `2 << 20`；`MemoryChunkSize` = `4 * 1024 * 1024` | const | — |
| 模板构建 | `buildTimeout` = `time.Hour`；`BuildBaseRootfsSizeLimitMB` = `25000` | const / flag | `build-base-rootfs-size-limit-mb` |
| 模板缓存 | `templateExpiration`、`buildCacheTTL` = `time.Hour * 25`；`templateExpirationBuffer` = `time.Hour`；`buildCacheDelayEviction` = `time.Second * 60` | const | — |

三条**已核实的约束**（值之外的、决定这些值为什么是这些值的东西）：

- `inPlaceStateFlipTimeout` 的 40 s **必须大于 Firecracker 自身的 30 s vcpu-ack 死锁检测**（`RECV_TIMEOUT_SEC`），否则 Go 侧的 trip 就不再意味着「FC 已放弃这次 flip」；而且触发它不只是调用失败——pause 路径会以 cleanup resume 响应，cleanup resume 再失败就**拆掉沙箱**（`ErrSandboxLost`）。
- `templateExpiration`（25 h）之所以取 25 h，注释给的理由是「应长于沙箱可能的最长寿命」，因此 `templateExpirationBuffer` = 1 h 是配它的余量；两者是一对，单独改一个没有意义。
- hugepage 不是评分函数里的一个权重项，而是**放置阶段的输入**（`HugePages: hasHugePages` 由 FC 版本能力 `HasHugePages()` 决定），并按节点导出池用量（`HugePagesTotal` / `HugePagesUsed` / `HugePagesReserved`）。上一版写的「hugepage 参与评分」过于含糊，已按源码改写。

**两处单位陷阱写在值里，不写在注释里**：`BestOfKAlpha` 是 **50**（α = 0.5，flag 存百分比）、`BestOfKMaxOvercommit` 是 **400**（4×）。抽取器有意返回**存储值**而非语义值——返回 0.5 等于把「要乘 100」这个约定替使用者抹掉，而两者在有人乘两次 100 之前都看不出差别。契约用例把这条固化下来。

**本版相对上一版更正四处**（都是同一类错误：把不同种类的东西放进同一栏，或凭印象补名字）：

1. `MaxOvercommit` **不存在**这个标识符，真名是 `BestOfKMaxOvercommit`（键 `best-of-k-max-overcommit`）。
2. 上一版把 21 个值里 8 个实际可在运行时改写的 flag 默认值，与 13 个编译期常量并列在同一栏「常量 = 值」。
3. 上一版「构建限额」行把 `1 h 总超时`（源码常量 `buildTimeout`，真在 `template_status.go`）与 `8 vCPU / 8 GiB / 20 并发 / 10 GiB disk` **并列**——后四项**不是源码常量**：它们读自 `data.Team.Limits`（`BuildConcurrency` / `MaxVcpu` / `MaxRamMb` / `DefaultFreeDiskSizeMb`），是**按团队存库的限额**，源码中只以 `types.TeamLimits{...}` 测试夹具的形式出现。对齐目标因此是「本仓需要一套等价的**按租户可配**限额」而不是「抄一组数字」。
4. `构建缓存淘汰延迟 60 s` 这个值是对的，但上一版**没有给出标识符**——它是 `buildCacheDelayEviction`。本版补上标识符后，抽取器才可能在它消失或改值时报警；只有值没有名字的数字是不可抽样的。同一行还漏了 `templateExpirationBuffer`。

**抽取器踩到的坑（已写进用例与变异自证）**：Go 的行尾 `//` 注释会以两种方式破坏读数——注释以 `)` 结尾时该行仍被识别为 flag，但注释被粘进值里；不以 `)` 结尾时该行**完全不再被识别为 flag**，退化成「值是一段未被求值的调用」的普通常量。上游 `flags.go` 里两种形态同时存在（`best-of-k-sample-size` 那行正是后者）。另外上游写 `time.Second * 60` 而不是 `60 * time.Second`，抽取器因此**原样返回表达式**而不做归一化：归一化会让读数对「上游把两个操作数换个顺序」这种重写失去敏感，而这类重写正是需要人看一眼的那一类改动。

**本仓当前没有可对照的机制面**：`SandboxProvider` trait 只有 `descriptor`/`health`/`allocate`/`start`/`stop`/`destroy`，无 `pause`/`resume`/`snapshot`；`SandboxSessionState` 无 `Pausing`/`Paused`/`Recovering`。因此这张表现在的用途是**未来 REQ 的设计输入**（`REQ-2026-0008`、`REQ-2026-0019`、`REQ-2026-0021`），不是可测对象的参照——按本仓纪律，未测数字不得进 live 文档的承诺位。

**本仓当前没有可对照的机制面**：`SandboxProvider` trait 只有 `descriptor`/`health`/`allocate`/`start`/`stop`/`destroy`，无 `pause`/`resume`/`snapshot`；`SandboxSessionState` 无 `Pausing`/`Paused`/`Recovering`。因此这张常量表现在的用途是**未来 REQ 的设计输入**（`REQ-2026-0008`、`REQ-2026-0019`、`REQ-2026-0021`），不是可测对象的参照——按本仓纪律，未测数字不得进 live 文档的承诺位。

## 5. SDK 与 REST 面的源码级增量

基准记录的是「文档页里出现的标识符」。源码读出的**增量面**（基准字段里没有这些符号）：

- **Git 模块**（`sandbox/git/index.ts` / `sandbox_sync/git.py`）：`clone` / `init` / `remoteAdd` / `remoteGet` / `status` / `branches` / `createBranch` / `checkoutBranch` / `deleteBranch` / `add` / `commit` / `reset` / `restore` / `push` / `pull` / `setConfig` / `getConfig` / `dangerouslyAuthenticate` / `configureUser`。**JS 侧整模块已标 `@deprecated`**，将在下个大版本移除；Python 侧未见同等级标记（未能确认是否一并移除）。
- **`Volume` 类**（`volume/index.ts` / `volume_sync.py`）：静态 `create` / `connect` / `getInfo` / `list` / `destroy`，实例 `list` / `makeDir` / `getInfo` / `exists` / `updateMetadata` / `readFile` / `writeFile` / `remove`。基准只以能力名提及整块。
- **`Secret` 类**：`create` / `update` / `getInfo` / `list` / `exists` / `destroy` / `fill` / `iamToken`。
- **IAM 不是类而是占位符机制**：`validateIamTokenName` + `iamTokenPlaceholders` 把 token 名映射成 `${e2b.identity.tokens.<name>}`，`tokenType` 取值含 `JWT-SVID`。
- **PTY**：`create` / `connect` / `sendInput` / `resize` / `kill`（本仓矩阵第 50 行判 🟡）。
- **MCP Gateway 的 SDK 面**：`getMcpUrl()` / `getMcpToken()`，端口 `mcpPort = 50005`。
- **`trafficAccessToken`**：用于受限公网流量的沙箱服务访问，与控制面 key、数据面 token 三者不同。
- **`getFullInfo`**：**JS 独有**（`sandboxApi.ts`），Python 侧无对应公开方法。
- **envd RPC 面**（决定数据面对齐的粒度）：`filesystem.Filesystem` 有 `Stat` / `MakeDir` / `Move` / `ListDir` / `Remove` / `WatchDir`(server-streaming) / `CreateWatcher` / `GetWatcherEvents` / `RemoveWatcher`；`process.Process` 有 `List` / `Connect`(server-streaming) / `Start`(server-streaming) / `Update` / `StreamInput`(client-streaming) / `SendInput` / `SendSignal` / `CloseStdin`。SDK 访问 envd 的端口是 `49983`，随请求带 `E2b-Sandbox-Id` 与 `E2b-Sandbox-Port`。
- **签名机制**：仅当存在 envd access token 时启用；算法为 `v1_` + base64(sha256(`"{path}:{operation}:{user}:{envdAccessToken}"`))，带过期时追加 `":{unixExpiration}"`，经 URL query 的 `signature` / `signature_expiration` 传递，`operation ∈ {read, write}`。
- **CLI 命令树**：`auth`（login/logout/info/configure）、`template`(别名 `tpl`：create/list/init/delete/publish/unpublish/migrate)、`sandbox`(别名 `sbx`：connect/info/list/kill/pause/resume/create/logs/metrics/exec/snapshot{create,list,delete}/fork)。
- **`external/code-interpreter` 与 `external/desktop` 是退役镜像，不是另一份实现**：两仓 README 顶部均明写 SDK 源码已迁入 monorepo 的 `packages/code-interpreter-*` 与 `packages/desktop-*`，本仓只剩模板、示例与图表抽取器。**对齐时只认 monorepo 那一份**；把退役仓当第二实现会读出重复能力。

**JS / Python 的两处系统性差异**（不是缺陷，是对齐时必须逐条处理的口径）：超时单位（JS `timeoutMs` 毫秒 vs Python `timeout` 秒）、命名风格（camelCase vs snake_case）、运行时模型（JS 单一 async vs Python sync/async 双实现共享基类）。三语言同语义是产品要求（`PRD-sandbox-surfaces.md` 第 12 节），因此这些差异属生成器必须处理的映射，不是可以各写一套的理由。

## 6. 对本仓的影响

上述源码级发现**不改变**能力对齐审计的结论（78 行 ✅ 0 / 🟡 16 / ❌ 60 / ⛔ 2），但改变三件事：

1. **对齐的工作定义要拆面。** 「REST API 与 SDK 完整兼容」必须展开为：控制面 REST 对齐（含 74 ops 中 28 个基准未收录者与 4 个已删除者的记账，并指明读的是 `infra` 还是 `E2B` 那份副本）、数据面 envd-RPC 对齐（**17** 个 RPC 方法 = Filesystem 9 + Process 8，加 11 个 REST 端点中 5 个客户端可达者，加传输与鉴权语义）、SDK 方法面对齐（含 CLI 与三个衍生 SDK）。三者有不同的权威文件与不同的验收方式。
2. **基准需要按上游当前版本重抓。** 版本落后十三个 minor，且 §2.1 的两向 operation 差异都已存在。重抓会同时解决 `recaptureCommand` 里已经写好但未执行的那一步。**这是本轮留下的最大单笔技术债**，且它是**可以自动化的**（`recaptureCommand` 已经是完整命令）。
3. **形态差异要先判「差异」还是「缺口」。** §2.4 的预热池是最好例子：E2B 用快照恢复、本仓用预热池，二者都有产品承载，按机制比会误判成缺口。对齐判定必须落在**能力与指标**上，并对每处形态差异显式写出「这是有意的形态差异，理由是 X」。

## 7. 实现授权边界与下一步

**本报告没有产生任何实现，也不产生实现授权。** 当前授权状态与既有记录一致且未变：`REQ-*` 27 份 = 0 `ready` + 5 `accepted` + 22 `draft`；ADR 27 份全 `proposed`；`specs/*.contract.json` 与 `apis/` 下的机器契约**没有任何一份** `implementationAuthorized: true`；`sdks/` 只有 README（零生成产物）、`apis/` 无权威 OpenAPI。`AGENTS.md` Agent Execution Rules 明文禁止在无 `ready` `REQ-*` 时实现 **Provider、API 路由、SDK、调度器、隔离策略、密钥注入机制或可部署 Profile**——而「让 sandbox 具备完整兼容 E2B 的功能实现、REST API 与 SDK 完整兼容」正是这一整类。

因此下一步有三条，**只有第三条需要代理之外的人做决定**：

| # | 动作 | 归属 | 前置 |
| --- | --- | --- | --- |
| A | 按上游当前版本重抓基准（`recaptureCommand` 已具备），并把两个契约面分开记账 | 代理可做 | 无 |
| B | 把 vendoring 与源码级对照做成可重跑的采样器（非门禁：`external/` 缺失时必须优雅退出） | 代理可做 | 无 |
| C | 把 `REQ-2026-0007`（命令契约）/`0008`（Firecracker Provider）/`0009`（Service Host）/`0023`（内部控制面）/`0019`（运行时池）等推进到 `ready`，并翻转相应契约的授权字段 | **人工** | 22 份 `pending-human-review` 签字、ADR 从 `proposed` 推进 |

**B 已在本轮交付**：`tools/audit-sandbox-e2b-upstream-source-parity.mjs` 每次运行都从 vendored 树重新推导全部读数并与记录值比对，退出码区分「漂移」(1)、「未 vendoring，无从采样」(3) 与「采样且干净」(0)，因此「采不出来」与「采完是干净的」不会长得一样。它**不是门禁**（`external/` 被 gitignore，门禁必须能在全新签出上跑），也不在 `_sdkwork:check` 链里；由 `tests/contract/e2b-upstream-source-parity-tool.contract.test.mjs` 的 24 条用例锁住其抽取规则与退出码语义（含 §8 的 SDK 面读数：20 个错误族的跨语言归一化、9 个 envd 版本闸的逐条相等、17 个 RPC 的流方向、7 个 gRPC code 的映射；以及 §4 的 21 个性能常量按「编译期常量 / 运行时 flag 默认值」两类分开读出、值取**存储值**而非语义值；§9 的 45 个公开方法成员与 4 个单侧成员、两条正交的成员面规则）。记录值一律由该工具的抽取器产出（必要时与 PyYAML 独立核对），**不得手抄**——本报告早先正是手抄出两处错（`go.work` 记 11 实为 12，envd RPC 方法数在 §6 被读成 8 实为 17），§4 这一轮又手抄出四处（见 §4），五处均已按实测更正，且 `go.work` 的抽取规则已改为按 `use` 块条目数而非 `packages/` 前缀数。计划的 23 条变异已全部证红并逐字节还原：抽取器级 16 条（`target/_parity-mutation-proof.py`）＋ 采样器级 7 条（`target/_performance-drift-mutation-proof.py`，临时改写 vendored 树后按 git 还原，并以 `git status --porcelain`、`git diff` 与 9 份文件摘要三重验证还原）。**两个驱动脚本与其余证据产物同处 `target/`，而 `target/` 被 gitignore**（与既有的 `target/_matrix-mutation-proof.mjs` 同惯例）：它们是**本地证据**，不是随仓交付的门禁，因此新签出里找不到它们是预期行为，不是缺失。

A 与 B 不触碰实现面，属研究基础设施；C 是任何 Provider / API / SDK 实现的前置条件，且在本仓治理里由人工拥有——这一点与 [Prompt 对齐报告](REVIEW-20260923-sandbox-implementation-prompt-alignment.md) 的裁决一致，不因本轮新增源码证据而改变。

## 8. 追加：SDK 面跨语言契约审计（2026-09-23 下午）

**为什么单独立一章**。用户要求「SDK 上也要完整对齐」「确保 REST API 和 SDK 能力完整兼容」。控制面 REST 是**对外**契约；而 JS / Python 两个 SDK 的**公开错误面、envd 版本闸、gRPC 状态表、分页语义**是**对内**契约——两个 SDK 之间、以及 SDK 与 envd 之间的约定。生成式 SDK 必须逐语言复现这一面，而它**既不在 `openapi.yml` 里，也不在 envd 的 proto 里**。本章逐项把它量下来，可自动化的部分已交给采样器（`sdk*` / `envd*` 读数，见 §7 的 B）。

### 8.1 错误分类：20 个族，名字一一对应，但继承关系有 2 处真分叉

| | JavaScript（`js-sdk/src/errors.ts`） | Python（`python-sdk/e2b/exceptions.py`） |
| --- | --- | --- |
| 族数 | 20 | 20 |
| 命名 | `XError` | `XException` |
| 剥离后缀后 | **完全一致，无单边成员** | |
| 不继承沙箱基类者 | 6 族：`SandboxError` 自身、`AuthenticationError`、`BuildError`、`ServiceBusyError`、`VolumeError`、`SecretError` | 同（对应族） |

⚠️ **2 处继承真分叉**（是继承关系不同，不是命名不同）：

| 族 | JS 父类 | Python 父类 | 后果 |
| --- | --- | --- | --- |
| `VolumeNotFound` | `VolumeError` | `NotFoundException` | `catch (e instanceof VolumeError)` 在 JS 抓到「卷不存在」，在 Python **抓不到**；`except NotFoundException` 恰好相反 |
| `VolumePathNotFound` | `VolumeError` | `NotFoundException` | 同上 |

**为什么要记成集合而不是逐条硬编码**：调用方把 handler 从一个语言搬到另一个语言，**代码照样编译、照样运行，只是静默不再捕获那个分支**。采样器把它记为 `RECORDED_SDK_INHERITANCE_DIVERGENCES`，因此「**新增**一条分叉」与「上游**消掉**一条」都会报出来——前者是陷阱，后者说明该清单已过期、需要重读。

⚠️ **归一化是这条规则的前提**：首轮量到 **8 处**分叉，其中 **6 处**是「JS `Error` vs Python `Exception`」——那是两个语言的**根类**，语义相同（都是「不属于沙箱层级」），属抽取器没归一化，**不是**分叉。把两个根类名折叠成同一个记号后剩 **2 处**。任何跨语言对比都要先做这一步，否则会把「两种语言的语法」当成「两边的分歧」。

### 8.2 同一状态码两种文案；同一个 code 两种归因

**控制面 HTTP**（`api/index.ts` 的 `apiErrorFromCode` vs `api/__init__.py` 的 `api_exception_from_code`）：映射的**类**一致（401 → 认证、429 → 限流、503 → 服务忙），但**文案不一致**：

| 状态 | JavaScript | Python |
| --- | --- | --- |
| 401 | `Unauthorized, please check your credentials.` | `401: Unauthorized, please check your credentials.` |
| 429 | `Rate limit exceeded, please try again later` | `429: Rate limit exceeded, please try again later.` |
| 503 | `Service temporarily unavailable, please retry` | `503: Service temporarily unavailable, please retry.` |

差异恰有**两处**：Python **前缀 `{status_code}: `**、**句尾多一个句号**；追加细节的来源也不同（JS 追加整个 error 对象，Python 追加 body 的 `message` 字段）。⇒ 凡断言错误文案的测试**不能跨语言复用**。

**数据面 gRPC**（`envd/rpc.ts` vs `envd/rpc.py`）：两侧**映射同样的 7 个 code**，归一化后集合相等（采样器已断言，且逐 code 比较，不只比个数）。但 **`Canceled` 的归因方向相反**：

- JS：`… This error is likely due to exceeding 'requestTimeoutMs'. You can pass the request timeout value as an option when making the request.`
- Python：`… The request was cancelled by the server or a proxy while it was in flight — for example when the sandbox is paused or shut down.`

**同一次取消，JS 让运维去调超时，Python 让运维去看沙箱是不是被暂停或关闭了。** 这不是措辞差异，是**诊断方向相反**——照着 JS 的提示去调大 `requestTimeoutMs` 解决不了「沙箱已被暂停」。`DeadlineExceeded` 措辞也不同（JS 分开讲 `timeoutMs` 与 request timeout，Python 把 `timeout` / `request_timeout` 合并讲）。

**采样器刻意不断言文案**：上游改措辞是常态，断言会把每一次改写都变成假红，而这条规则的价值不在「措辞有没有变」而在「归因方向是否有矛盾」。因此这一条只在本报告登记，靠人读；采样器只守**code→族**的对应关系。

### 8.3 「连接中途断开」的检测：两个语言用了**不同机制**

同一个语义（请求飞行中连接被断），两侧实现方式**结构不同**：

| | 机制 |
| --- | --- |
| JavaScript | 匹配 **5 个运行时的文案片段**：Node/undici `terminated`、Bun `The socket connection was closed unexpectedly`、Deno `error reading a body from connection`、Cloudflare Workers `Network connection lost`、浏览器 `network error`。命中条件 = 是 `ConnectError` **且** `code === Unknown` **且** 文案含片段 |
| Python | 按**异常类型**判（`pyqwest` 的 `ReadError` / `StreamError` / `WriteError`），并显式排除 `DEADLINE_EXCEEDED`；再用一次**健康探测** `sandbox_running` 消歧（探测确认沙箱已消失才判超时）；并把 asyncio 的取消**还原成 `CancelledError`**，以保住 `asyncio` 语义 |

**对生成式 SDK 的意义**：这是**唯一一个无法靠「照抄语义」复现的面**。JS 走字符串匹配 ⇒ 每新增一个运行时（新的 fetch 实现）就要加一条片段，漏了就退化成「未知错误」；Python 走类型匹配 ⇒ 更稳但绑定 `pyqwest`，且多出一个 JS 没有的概念（健康探测消歧）。本仓若生成 SDK，**必须先决定采用哪一种机制并写进 SDK 生成契约**，不能两个语言各随各的。

### 8.4 envd RPC 契约（数据面对齐的粒度）

17 个方法、2 个 service。**流方向**决定客户端要实现的形态（采样器已记数与记名）：

| service | 方法 | 流 |
| --- | --- | --- |
| `Filesystem` | `Stat` / `MakeDir` / `Move` / `ListDir` / `Remove` | 一元 |
| | `WatchDir` | **server-streaming** |
| | `CreateWatcher` / `GetWatcherEvents` / `RemoveWatcher` | 一元（`WatchDir` 的非流式版本） |
| `Process` | `List` / `Update` / `SendInput` / `SendSignal` / `CloseStdin` | 一元 |
| | `Connect` / `Start` | **server-streaming** |
| | `StreamInput` | **client-streaming** |

⇒ **3 个 server-streaming（`WatchDir` / `Connect` / `Start`）+ 1 个 client-streaming（`StreamInput`）**。注意 `WatchDir` 与 `StreamInput` 的 `oneof` 里各有**显式 `KeepAlive` 分支**——**长连接的存活由协议层定义**，不依赖 TCP 超时。

几处对 wire 兼容有硬影响、必须原样照做的事实：

- **`enum Signal` 只有两个值，且用真实 Linux 信号号**：`SIGNAL_SIGTERM = 15`、`SIGNAL_SIGKILL = 9`。**没有** SIGHUP / SIGINT / SIGQUIT 等。号码必须原样，不能自定义编号。
- **`ProcessSelector` 是 `oneof { uint32 pid | string tag }`** —— 不是两者都可给，也不是两个可选字段。
- **`ProcessEvent.EndEvent.exit_code` 是 `sint32`**（zigzag 编码），不是 `int32`。按 `int32` 解析非零退出码会出错。
- **`EntryInfo.metadata` 其实存在 xattr 里**：键位于 `user.e2b.` 命名空间，**前缀被剥掉后**才出现在该字段；其他工具写的普通 `user.*` xattr **不会**出现。要读 E2B 的 per-file 元数据必须写对命名空间。
- 🔴 **`StartRequest.stdin` 的默认值已被官方宣布要翻转**。proto 注释原文：「This is optional for backwards compatibility. We default to true. New SDK versions will set this to false by default.」⇒ 这是**已声明的未来破坏性变更**，不是猜测。本仓若实现数据面，**必须显式发送该字段**而不要依赖默认值，否则上游翻默认值的那一天，行为会**静默**改变（stdin 从「可用」变成「关闭」）。

### 8.5 envd 版本协商：9 个闸，且**别读 `envd.yaml` 的 `info.version`**

两侧 SDK 各声明 **9 个 `ENVD_*` 版本闸**，**名字与值逐条相同**（采样器断言的是**逐条相等**，不只是个数相等）：

`ENVD_VERSION_RECURSIVE_WATCH 0.1.4`、`ENVD_DEBUG_FALLBACK 99.99.99`、`ENVD_COMMANDS_STDIN 0.3.0`、`ENVD_DEFAULT_USER 0.4.0`、`ENVD_ENVD_CLOSE 0.5.2`、`ENVD_OCTET_STREAM_UPLOAD 0.5.7`、`ENVD_FILE_METADATA 0.6.2`、`ENVD_VERSION_FS_EVENT_ENTRY_INFO 0.6.3`、`ENVD_VERSION_WATCH_NETWORK_MOUNTS 0.6.4`

机制：`GET /envs` 返回 envd 版本 ⇒ 客户端按闸比较 ⇒ 不满足时**明确报错并要求重建模板**，例如 `Sandbox envd version X doesn't support closeStdin. Please rebuild your template to pick up the latest sandbox version.`

🔴 **陷阱**：`external/E2B/spec/envd/envd.yaml` 的 `info.version` 写的是 **0.1.3**，而闸要求到 **0.6.4**。**那个 0.1.3 是这份契约文档自身的修订号，不是 envd 的版本。** 按它对齐会把能力面判成 0.1.3 那一代（远早于 stdin / 文件元数据 / entry-info / 网络挂载这几代能力）。⇒ **判 envd 能力面只认 `ENVD_*` 闸。**

### 8.6 分页：游标在响应头里，不在 body 里

`PAGINATION_SPEC` 的对齐对象。两侧形状一致：

- 游标来源 = 响应头 **`x-next-token`**；**`hasNext` 没有独立字段**，就是 `!!nextToken`。⇒ 客户端**不能只看 body**；中间若有代理丢掉该响应头，分页会**静默结束**（而不是报错）——这是「列表看起来变少了」这类缺陷的根因形态。
- 沙箱列举的过滤集：`metadata`（**先 URL-encode 每个 k/v、再整体 urlencode**）、`state`、`started_after`、`template`、`order`、`limit`、`next_token`。
- `order` 取值 **`asc` / `desc`**；非法值抛 `InvalidArgumentException` / `InvalidArgumentError`（Python 侧消息：`Invalid order {x!r}, expected 'asc' or 'desc'`）。
- 快照列举另有 `sandbox_id` / `name` 两个过滤。

### 8.7 本章对「SDK 完整对齐」的收敛结论

「SDK 对齐」不是一个工作项，而是**五个面**，各有不同权威文件与不同验收方式：

| 面 | 权威文件 | 已量到的规模 |
| --- | --- | --- |
| 控制面 REST | `infra/spec/openapi.yml`（＋ SDK 仓的 `E2B/spec/openapi.yml` 副本） | 74 ops / 57 paths；两向差异 28 / 25 |
| 数据面 envd | `spec/envd/`（proto ＋ `envd.yaml`） | 17 RPC（3 srv-stream / 1 cli-stream）＋ 11 REST（6 内部 / 5 可达）＋ 9 版本闸 |
| 语言间公开面 | 两 SDK 的 `errors` / `envd/versions` / `envd/rpc` | 20 错误族（**2 处继承分叉**）＋ 7 gRPC code |
| 语言间方法成员面 | 两 SDK 的客户端类（`git` / `volume` / `secret` / `pty`） | 45 个公开成员；**4 个单侧成员**（详见 §9） |
| 分页与会话语义 | 两侧 paginator ＋ `x-next-token` | 游标在头；`order ∈ {asc, desc}`；`hasNext = !!token` |

生成式 SDK 的验收**不能只对 OpenAPI 打勾**：上面第二、三、四、五行**都不在 OpenAPI 里**，而它们正是「同一个能力在两种语言里表现是否一致」的所在。

⚠️ **本章不改变 §7 的授权边界。** 以上全部是**读源码得到的事实**，不构成任何实现授权；步骤 C 仍归人工。

## 9. 追加：SDK 公开方法成员面（2026-09-23 傍晚）

§8 对齐的是「同一个能力在两种语言里**报错 / 协商 / 分页**是否一致」。还有第五个轴，前四个都覆盖不到：**同一个能力在两种语言里是否以同一个名字公开**。这是把 E2B 的用法从一种语言搬到另一种语言时唯一真正会挡路的差异——它既不在 OpenAPI 里（方法名是生成物之外的手写面），也不属于错误族。

### 9.1 面的定义与测量口径

上游客户端把能力挂在四个类上，逐类比较公开成员：

| 面 | TS 权威文件 | Python 权威文件 | TS 成员 | Python 成员 | 单侧 |
| --- | --- | --- | --- | --- | --- |
| `git` | `packages/js-sdk/src/sandbox/git/index.ts` | `packages/python-sdk/e2b/sandbox_sync/git.py` | 19 | 19 | 0 |
| `volume` | `packages/js-sdk/src/volume/index.ts` | `packages/python-sdk/e2b/volume/volume_sync.py` | 11 | 9 | 2 |
| `secret` | `packages/js-sdk/src/secret.ts` | `packages/python-sdk/e2b/secret/base.py` ＋ `secret/secret_sync.py` | 9 | 9 | 0 |
| `pty` | `packages/js-sdk/src/sandbox/commands/pty.ts` | `packages/python-sdk/e2b/sandbox_sync/commands/pty.py` | 5 | 5 | 2（一对改名） |

并集 **45 个成员**。口径（多一条少一条都会改变结论，故逐条说明）：

- **排除 `private` / `protected`**：`private async runGit` 不算公开面。不排除会凭空造出 5 条假分歧。
- **允许类型参数、但参数不是名字**：`static create<V extends typeof Volume>(` 是成员，`V extends typeof Volume` 不是。
- **过滤语句关键字**：成员缩进位置上的 `if (` / `for (` 是函数体语句而非成员——`secret.ts` 里的 `validateSecretName` 就会踩到，不处理会多出一个名叫 `if` 的成员。
- **`@property` 不是方法**：`Volume.volume_id` / `name` / `token` 在 Python 是属性、在 TS 是 `readonly` 字段，两侧是同一个公开面。
- **一语言多文件求并集**：Python 的 `secret` 面被拆在 `base.py`（`fill`、`iam_token`）与 `secret_sync.py`（REST 成员）。按「一语言一文件」读只会读到 6 个成员，真值是 9 个。
- **命名归一化后再比**：`make_dir` 与 `makeDir` 是同一个成员。不归一化，蛇形/驼峰会淹掉真差异。

### 9.2 量到的 4 个单侧成员，分两类（性质完全不同）

| 成员 | 类型 | 事实 |
| --- | --- | --- |
| `pty.sendInput`（TS） | **改名** | Python 侧同能力为 `send_stdin`（`commands/pty.py:83`，公开）。两侧各 5 个公开成员、4 个同名 |
| `pty.send_stdin`（PY） | **改名** | 同上，是同一对的另一半 |
| `Volume.getInfo`（TS） | **可见性** | TS 为公开静态（`volume/index.ts:189`）；Python 同能力是私有类方法 `_class_get_info`（`volume_sync.py:176`） |
| `Volume.list`（TS） | **可见性** | TS 为公开静态（`volume/index.ts:230`）；Python 是私有类方法 `_class_list`（`volume_sync.py:219`） |

🔴 **「可见性」这一类必须与「未实现」分开读**：Python 的 `_class_get_info` / `_class_list` 是**有实现的**，内部经生成式 core API 客户端（`get_volumes_volume_id` / `get_volumes`）打同一批 REST 端点，另有私有实例方法 `_instance_get_info` / `_instance_list`。所以这是**公开面差异**，不是能力缺口；把它记成「Python 缺 volume 查询」是错的。这条区分直接决定工作量：改名与补公开面，不是补实现。

### 9.3 两条规则，故意看不同的东西

| 规则 | 抓什么 | 结构上抓不到的 |
| --- | --- | --- |
| `sdk-method-divergences` | 单侧公开成员（当前 4 条） | 两侧**同步**新增的成员 |
| `sdk-method-inventory` | 成员清单本身变化（当前 45 条） | ——（清单变化它都管） |

第二条存在的原因正是一类结构盲区：上游若同时在两种语言加一个同名成员，`divergences` 会报「干净」，而任何手写的成员清单已经过期（`git` 面 19 个成员全靠清单守着）。变异自证专门造了这个场景——**同时**给两个文件加一个同名成员——并断言 `sdk-method-inventory` 必须红、`sdk-method-divergences` 必须**不**红。两条规则的正交性由此被证明，而不是被声称；「两边同步新增」也因此被显式归为「覆盖度新闻」而非分歧。

### 9.4 对「SDK 完整对齐」的增量

- 「SDK 对齐」的面数从四个改为**五个**（§8.7 表已同步）。
- 4 个单侧成员中：**1 对是改名**（`sendInput` / `send_stdin`，加别名即可抹平）、**1 对是可见性**（`Volume.getInfo` / `list`，补齐公开面即可）。**没有任何一个属于「上游有、E2B 客户端没有」**——这条轴的工作量因此落在「补公开面与命名」，不在「补实现」。
- ⚠️ 与 §8.7 同理：本节的 45 / 4 是**读数**，不构成任何实现授权；`AGENTS.md` 对 Provider / API / SDK 的实现禁令不因本节松动，步骤 C 仍归人工。

## 10. 追加：基准漂移全量实测（2026-09-23 傍晚）

§2.5 只重抓了 2 个来源就停了，`recaptureCommand` 里"re-fetch every `sources[].url`"那一步从未执行。本节把它**执行完**，并把 §7-A（"按上游当前版本重抓基准"）从一句待办变成一份有读数的现状说明。

### 10.1 方法与边界

- 重抓 `specs/sandbox-e2b-capability-baseline.json` 的**全部 101 个** `sources[].url` 到 `target/e2b-baseline-recheck/`，逐文件比 `(bytes, sha256)`。落盘读数 `target/e2b-baseline-drift-report.json`。
- **只比对，不覆盖**：基准未被改写，`target/e2b-baseline/` 里的原始抓取副本也未被改写（唯一的例外见 §10.4，那是上一轮留下的，不是本轮）。§2.5 定的规矩是"逐来源记新 sha256 并比对，而不是覆盖"，本节照此执行。
- 执行器 `target/_e2b-baseline-drift-recheck.py` 与读数同处 `target/`（gitignore，与 `target/_parity-mutation-proof.py`、`target/_performance-drift-mutation-proof.py` 同惯例）：它是**本地证据**，不是随仓交付的门禁，新签出里找不到它是预期行为。退出码区分 `0` 无漂移 / `1` 有漂移或采不全 / `2` 用法错，"采不出来"与"采完干净"不会长得一样。
- 取样时刻 `2026-09-23T09:08:18Z`（UTC，落盘在读数 JSON 的 `recheckAt`），基准 `capturedAt` 为 `2026-09-22T09:36:31Z`。连跑两次读数一致（97 / 3 / 1），是本节的复现判据。

### 10.2 读数：97 未动 / 3 漂移 / 1 抓取失败

| 来源 id | 种类 | 记录值 | 实测值 | 差 |
| --- | --- | --- | --- | --- |
| `e2b-docs-index` | index | 33167 B / `88725aeb…` | 33174 B / `818d97f2…` | **+7 B** |
| `e2b-network-byop` | doc-page | 10788 B / `b3aa2aa8…` | 12418 B / `e9bb0d03…` | **+1630 B** |
| `e2b-network-internet-access` | doc-page | 18921 B / `247b54ee…` | 19622 B / `2a788615…` | **+701 B** |
| `e2b-sdk-reference-python-sdk-v2-37-1-sandbox-async` | doc-page | 见基准 | **HTTP 404** | 页面已不存在 |

完整 sha256（记录值 → 实测值，逐字节取自读数 JSON，非手抄）：

- `llms.txt`：`88725aeb33f69cdf474efb11b5c2c8510f967214e682016ff21bf0fad237dc73` → `818d97f2040676ed4d9c2a1586e75afc135e4e47f70820c0e232d62e24fd23db`
- `network/byop.md`：`b3aa2aa836d40f23fdb3bf2a1fc188bc8e60bf928449f424284e0c20d6c3f9d0` → `e9bb0d03793659bd39ef10b6a5d67ca30d9e5f80395bf79340fb98df2c65549d`
- `network/internet-access.md`：`247b54ee1e30f528bb4955cfb3438f6897fa537e0e56d9a4409ff4e3d9b0f4c9` → `2a788615d078e9d4111f9e79b0fb70d6575caa230891bbd3dd3aa60066fc277e`

`llms.txt` 侧的 7 字节差值得单独说明：它在上一轮重抓时已被**原地覆盖**（§10.4），因此磁盘上那一份与我本轮的新抓取**逐字节相同**（`cmp` exit 0，均 33174 B）——它能证明"上游现在是 33174"，**不能**用来 diff"相对原始的 33167 改了什么"。索引里的 URL 集合两次完全一致（无新增/删除页面），所以**来源清单 101 项仍然完整**，那 7 字节落在描述文字里。

`openapi-public.yaml` **未漂移**（166293 B，两侧 sha256 相同）——与 §2.5 一致。因此 §2.1 的 74-op 记账与两向差异 28/25 **不需要重算**；会动的只有"文档页字段"那一层。101 个来源里 97 个仍逐字节复现，说明基准的 **operation 面**是稳的，**页面字段面**不是。

### 10.3 漂移的内容级含义：两行 `e2bFacts` 已过期

两个漂移页不是改错别字，它们**新增了小节**：

- `network/internet-access.md` 新增 `#### Catch-all wildcard`：`'*'` 匹配 80 端口 HTTP `Host` 与 443 端口 TLS SNI，且**API 要求 deny 列表里必须有 `0.0.0.0/0`（`ALL_TRAFFIC`），否则创建返回 HTTP 400**。这是一条**带错误码的请求校验约束**，不是措辞。
- `network/byop.md` 新增 `### Using a wildcard with BYOP`：`allowOut: ['*']` + `denyOut: ['0.0.0.0/0']` + `egressProxy` 的组合用法。

同时一处**语义细化**：`ATYP=domain` 只用于主机名目的地——**IP 字面量目的地（含被 `'*'` 匹配到的数字型 HTTP `Host`）按 SOCKS5 IP 地址发送**。

这两页正是**矩阵第 54 行**（`sourceIds` 含 `e2b-network-internet-access`）与**第 61 行**（含 `e2b-network-byop`）的字段证据来源。基准里这两行的 `e2bFacts`（该页小节标题清单）分别是 9 与 14 条，**都不含**上述新标题，因此"逐行字段级证据"在这两行**已不再完整**。这不是本仓的实现缺口（Network 全族本就是 `❌`/`🟡` 仅契约），而是**基准这一层的过期**。

### 10.4 一个自相一致的缺陷：上一轮重抓原地覆盖了原件

`target/e2b-baseline/llms.txt` 与 `target/e2b-baseline/openapi-public.yaml` 的 mtime 是 **2026-09-23 15:54**，其余 99 个文件是 **2026-09-22 17:32–17:35**。即 §2.5 那次重抓把新字节**写回原名**，覆盖了 33167 B 的原始件。后果是确定的：**那 7 个字节改了什么，现在无法再 diff 出来**——`cmp` 只能证明"新副本与我的新抓取一致"，无法回答"相对原始件动了哪一行"。§2.5 自己写的规矩是"比对，而不是覆盖"，执行时违背了它。本轮的 §10.1 因此把新抓取放进**独立目录**。

### 10.5 耦合：这说明"刷新基准"不是改三个哈希

矩阵文档有 **27 行**复述了该行的项数。⚠️ **本节的上一版写的是「23 行」，那是错的，成因正是本节要防的那类**：首轮抽取用 `第\s*\d+\s*行（\d+\s*项）`，**要求闭括号紧跟**，于是 4 行带后续说明的行（`（6 项，含 …）`）被静默丢弃。放宽为 `第\s*(\d+)\s*行（\s*(\d+)\s*项` 后读到 27。⇒ 凡"覆盖率 / 缺口数"结论，先对抽取规则做正反例自检再报数——这条纪律在本轮**又**救了一次。

**而这 27 行分两组口径**，这是补门禁时用正反例自检查出来的：

| 组 | 行数 | 数的量 | 证据 |
| --- | --- | --- | --- |
| 多数 | 23 | **整行抽取面** `len(e2bFields)+len(e2bFacts)+len(e2bClis)` | 第 56/59/60/61/63/64/66/67 行逐条吻合：33/12/6/30/10/13/3/16 |
| 少数 | **4**（第 1 / 26 / 29 / 65 行） | **只数 `e2bFields`** | 这 4 行的文案各自点名了一个 `operationId`，且该符号确实在 `e2bFields` 里：行 1 `GET /envs [getEnvVars]`、行 29 `GET /templates/aliases/{alias} [getTemplatesAlias]`、行 65 `GET /metrics [getMetrics]`；行 26 的 `e2bFields` 恰好是 **15** 个 Templates REST 操作，与"15 项：Templates REST 建/查/改/删 + 构建流水线与构建产物"逐字对应 |

⇒ **同一个写法 `（N 项）` 承载了两个含义，且没有任何门禁看得见**。这 4 行不是错，是在数另一个量；但读者无法区分。第 61 行现为「30 项」，若补入上游新标题即为 31。

🔴 **这 27 处复述数字在本轮之前没有任何门禁校验**：`grep -n "项）" tools/*.mjs` 零命中。`check-sandbox-e2b-parity-matrix.mjs` 只校验 §1.1 answer section 的 14 个复述数字，§2.x 的逐行项数不在其内。⇒ 刷新基准时"改数据"与"改正文里跟着变的数字"之间**没有机器约束**。该前置项已在 §11 关闭。

### 10.6 本节结论

- **基准已实测漂移**（3 源移动、1 源 404），且漂移落在**页面字段面**而非 operation 面。
- **§7-A 仍开放，且比 §7 写它时更具体**：它包含三件事，① 三个来源的 `bytes`/`sha256` 与两行 `e2bFacts` 的更新（机械但需同步正文项数）；② 给那 27 处复述数字补校验（否则改错了没人知道）——**已完成，见 §11**；③ `python-sdk/v2.37.1/sandbox/async.md` 的 **404 政策决定**——换到当前版本页等于**重导第 69/72 行的 SDK 字段**，那是语义改动而非哈希更新，须人工定夺。①②代理可做，③不是。（**读到这里请续看 §12：① 已收口；且"两行 `e2bFacts`"这个判断本身是错的，实际只有一行要改——见 §12.3。**）
- ⚠️ **本节与 §2、§8、§9 一样，不产生任何实现，也不产生实现授权。** 漂移读数不改变 §7 的授权边界：`REQ-*` 27 份仍 0 `ready`，ADR 仍全 `proposed`，机器契约仍无一份 `implementationAuthorized: true`。§7 步骤 C 仍归人工。

## 11. 追加：把 §10.5 的前置项关掉，并把复述项数纳入 `document-join`

§10.5 说"补这层校验是刷新工作的前置项，不是收尾项"。本节把它做完。

### 11.1 落点：扩既有族，不新增族

先确认归属，而不是先写代码：

- `check-sandbox-e2b-parity-matrix.mjs` **完全不读基准**（`grep -n capability-baseline` 零命中），而这条判据的右侧正是基准行的抽取面 ⇒ 放这里就没有可比对象。
- `check-sandbox-e2b-field-parity.mjs` 有 `BASELINE_PATH` 与现成的 `document-join` 族（"文档必须与基准逐行相等"），语义完全同类。

⇒ 落在 `document-join` 族**内部**。因此 `RULE_FAMILIES` 仍为 **11** 条，`RULE_FAMILY_SURFACES` 的 4 个自描述面（门禁头注释 / `tools/README.md` / 根 `README.md` / Gate-0 视图）**一个都不用动**——新增族会把族数推到 12，那才是要五处联改的动作。

### 11.2 规则形状：三条判据，五个分支

`第 N 行（M 项）` 只在矩阵行里出现，所以它的 `N` 只能是**本行自己的行号**：

1. **引用必给数**：某行引用了 `specs/sandbox-e2b-capability-baseline.json` 却不写项数 ⇒ 红（"这条引用覆盖了多少，读不出来"）。
2. **给数必引用**：某行写了项数却不点基准文件 ⇒ 红（"这个数字没有可被反驳的来源"）。
3. **N 只能是本行、M 必须等于该基准行的整行抽取面**（`e2bFields`+`e2bClis`+`e2bFacts`）⇒ 否则红（行号指错邻居、或数字与源不符）。

判据 1 与 2 之所以**双向**都写：当前 27 行恰好构成对称集合（引用者全给数、给数者全引用），双向编码因此是一条**棘轮**——它只能被一次明说的编辑放松，不能悄悄松掉。抽取面算式提成 `baselineRowSurfaceCount()` 单一实现，`row-evidence` 族与本节的新判据共用，杜绝"同一公式写两遍然后各自漂移"。

### 11.3 它一上线就抓到 4 处真缺陷

**规则首次运行即转红 4 行**（第 1 / 26 / 29 / 65 行），即 §10.5 量到的两组口径。修法**不是改数字**——那 4 行的数字与自己的文案是自洽的（各自点名了一个 `operationId`，且该符号确实在 `e2bFields` 里），单改数字会让数字与文案矛盾。正确修法是**统一口径并保住信息**：

- §2 前言新增一句，定义 `（M 项）` **只表示该基准行的整行抽取面**，并要求更窄的量必须写出来（`其中 M 个字段`）。
- 4 行改写为 `（整行 N 项，其中 M 个字段，…）`，窄口径作为文案保留。例：行 1 `（11 项，含 …）` → `（25 项，其中 11 个字段，含 …）`；行 26 `（15 项：Templates REST …）` → `（38 项，其中 15 个字段：Templates REST …）`。

⇒ 一处写法，一个含义；且现在有机器在看着。

### 11.4 自证读数

| 项 | 读数 |
| --- | --- |
| 契约用例（该门禁） | **30 / 30**，`test()` 数**不变**（新增的 5 条是既有 `cases` 数组与既有 `expectRule` 测试里的条目，不是新 `test()`）⇒ `REAL_CONTRACT_FILES = 41` / `REAL_CONTRACT_TESTS = 644` / `testInventory` **全部无需改动** |
| 新判据的 5 个分支 | 逐条进变异表，全部转红（数字漂移 / 行号指错 / 引用无数字 / 数字无引用 / 行号不存在）；另加 2 条文案定点断言（`which holds 1`、`states no item count`） |
| 族级耦合自证 | **11 / 11** RED；门禁还原**逐字节一致** |
| 全链回归 | 12 条 sandbox 门禁 + **6** 条 specs 门禁 + `cargo fmt --check` / `cargo check --workspace --all-targets` / `cargo clippy --workspace --all-targets -- -D warnings` 全 `EXIT=0`；契约套件 **644 / 644**；`cargo test --workspace` = 78 passed / 0 failed / 1 ignored。⚠️ 本条原写「7 条 specs 门禁」，错因是把只在散文里出现、并不在根 `README.md` Verification 块里的 `check-database-framework-standard.mjs` 算了进去；块内实测 **6** 条，已更正 |

### 11.5 顺带修掉的两个驱动缺陷（都在 gitignore 的 `target/`）

1. 🔴 **族清单是手抄的，已经过期**。`target/_e2b-mutation-proof.mjs` 的 `RULES` 列了 **10** 族，而门禁实现 **11** 族——漏掉 `capability-matrix-join`，于是自证打印"all 10 rule families coupled"，**对一个实现了 11 族的门禁低报了它自己的覆盖**。这恰是本仓判定为最严重的那类失败。实测该族**是**耦合的（中和它 ⇒ 29 pass / 1 fail）⇒ 缺陷在驱动、不在门禁。修法不是补一行，而是**从门禁 `import { RULE_FAMILIES }` 派生**——一份只能靠人维护才正确的清单，就是一份必然过期的清单。改后读数 **11 / 11**。
2. 🔴 **还原无保证**。驱动原本只在正常路径末尾写回原件，循环中途抛异常会把门禁**留在变异态**。已改为 `finally` 内还原，并把退出码绑定 `byteExact`（还原不逐字节一致即 `exit 1`）。

### 11.6 边界不变

本节只动了**审计基础设施**（一个门禁族内的判据 + 契约用例 + 本地自证驱动 + 文档口径定义与 4 处数字订正）。**没有产生任何实现，也不产生实现授权**：`REQ-*` 27 份仍 0 `ready`，ADR 仍全 `proposed`，机器契约仍无一份 `implementationAuthorized: true`，`check-sandbox-commercial-readiness.mjs` 仍 `NO-GO`。§7-A 的 ①（基准刷新）与 ③（404 政策）仍开放，③ 归人工。（**读到这里请续看 §12：① 已收口，只剩 ③ 归人工。**）

## 12. 追加：§7-A ① 基准刷新收口（2026-09-23）

§11 关掉的是刷新工作的**前置项**（27 处复述项数的校验）。本节把 §7-A ① 本体做完，并**推翻 §10.3 的一半结论**。

### 12.1 先钉住 `e2bFacts` 的抽取口径，再谈"过期"

要判断"上游页面多了个小节"是否等于"基准该多一条 fact"，得先知道 `e2bFacts` 到底收什么。§10.3 是直接下结论的，本节先反向核验：`target/_e2b-facts-recompute.py` 从 `capturedFile` 复算每一行的 `e2bFacts` 与基准逐行比对（本地证据脚本，与其余驱动同处 gitignore 的 `target/`）。

**首轮口径连错两次**，两次都是"多收"：按"全部 `#` 级标题"只复现 69/78，多出来的正是两类噪声——① fenced code block 里的 `#` 行被当成标题（新页 ```bash 段里的 `# Through BYOP: …`、`# Allowed: TLS SNI matches the wildcard` 全被误收）；② SDK 参考页的结构标题（`Call Signature` / `Parameters` / `Methods` / `Returns` / `Type declaration` …）。修正口径后**条数复现 78/78**（75 行逐字一致，3 行仅差 `\_` 转义的写法）：

| 口径 | 内容 |
| --- | --- |
| 收录 | 该行 `sourceIds` 中每个 doc-page / index 来源的 **1–3 级** ATX 标题（**含页面 H1 标题**） |
| 不收录 | **4 级及更深**（`####`）、fenced code block 内的 `#` 行 |
| 文本 | 剥掉反引号与 `**`；markdown 转义**按原样保留**（记 `get\_info` 而非 `get_info`） |

⇒ **`####` 从来不在 `e2bFacts` 里。** 这是本节推翻 §10.3 的唯一依据，且它现在是可复算的，不再是印象。

### 12.2 逐文件实测增量（权威来源是字节，不是 diff 的观感）

对三个漂移来源分别算"记录值对应字节 → 重抓字节"的 H1–H3 集合差：

| 来源 | H1–H3 条数 | 新增 | 移除 |
| --- | --- | --- | --- |
| `network/internet-access.md` | 9 → **9** | — | — |
| `network/byop.md` | 8 → **9** | `Bring your own proxy (BYOP)`、`Using a wildcard with BYOP` | `Bring your own proxy` |
| `llms.txt` | 2 → **2** | — | — |

另：**没有任何一行的 `sourceIds` 引用 `e2b-docs-index`**（实测为空集），所以 `llms.txt` 那 +7 字节无论落在大纲还是链接里，都动不到任何一行的 `e2bFacts`。

### 12.3 🔴 推翻 §10.3：第 54 / 55 / 56 行**不需要改**

§10.3 写的是「这两页正是**矩阵第 54 行**与第 61 行的字段证据来源……因此"逐行字段级证据"在这两行**已不再完整**」。**前半句对，后半句错**：

- `internet-access.md` 新增的 `#### Catch-all wildcard` 是 **H4**；
- 该页**原本就有 3 个 H4**（`#### Matching hosts`、`#### Limits`、`#### Transform callbacks`），基准**一个都没收**；第 54 行的 9 条 fact 正是该页的 9 个 H1–H3；
- ⇒ 新 H4 属于**收录口径内本就不该出现的东西**，它的出现不是"基准过期"，而是"上游多了个更深的子节"。

**错误成因**（与 §10.5 同一类，本轮第二次）：§10.3 看到 diff 里冒出新小节标题，就直接下了"基准过期"的结论，**没有先问这个标题的层级在不在收录口径内**。正确判据是**层级**，不是"有没有新标题"——而层级这件事当时没人量过。

⇒ 修正后的结论：**行 54 / 55 / 56 不动，只有行 61 动。**

### 12.4 落地的 4 处改动

| # | 落点 | 改动 |
| --- | --- | --- |
| 1 | `specs/sandbox-e2b-capability-baseline.json` 的 `sources[]` | 3 个来源的 `bytes` / `sha256` → 实测值（33167→**33174** / `818d97f2…`、10788→**12418** / `e9bb0d03…`、18921→**19622** / `2a788615…`） |
| 2 | 同一文件的 `rows[61].e2bFacts` | 14 → **15**：`Bring your own proxy` → `Bring your own proxy (BYOP)`，并插入 `Using a wildcard with BYOP`（按字典序落位，仍有序） |
| 3 | 同一文件的 `captureNote` | 记下"部分刷新"这件事及其依据：刷了哪 3 个来源、为何行 61 变而 54–56 不变（H1–H3 口径）、其余 98 源仍是 2026-09-22 的字节、404 页保留记录值 |
| 4 | `docs/architecture/tech/TECH-e2b-capability-parity.md` 第 224 行 | `第 61 行（30 项）` → `（31 项）`，与基准整行抽取面 15+15+1=**31** 对齐（守这一对的正是 §11 新加的 `document-join` 判据） |

**`capturedAt` 刻意不动。** 它表示 101 个来源的**整次**抓取时刻，且被 **6 处**复述：矩阵文档的「基准完整性」行、本报告 §2.2 与 §10.1、契约测试里 3 处（常量 `REAL_CAPTURED_AT` 与两处夹具）。仅刷新 3 个来源就把时间戳前移，等于同时让这 6 处失真；部分刷新这件事落在**没有第二处复述**的 `captureNote` 里。

改动是**定点文本替换**，不是 JSON 反序列化再重排：`git diff --numstat` = **12 insert / 11 delete**，即没有把 4300 行文件整体重排；行尾仍是**纯 LF**（`crlf=0`）。落地前脚本对每个替换点断言"恰好命中一次"，落地后再解析一次并断言 4 个读数（3 源的 `(bytes,sha256)`、行 61 面 = 31、`capturedAt` 与 `testInventory` 未动）。被替换掉的旧字节已另存 `target/e2b-baseline-drift-preimages/`——§10.4 的教训（原地覆盖让漂移无法再 diff）不在这一轮重演。

### 12.5 收口前后的读数

| 项 | 刷新前（§10.2 / §10.3） | 刷新后 |
| --- | --- | --- |
| 漂移复检 | 97 未动 / **3 漂移** / 1 抓取失败 | **100 未动 / 0 漂移 / 1 抓取失败** |
| 行 61 `e2bFacts` | 14（页面标题已改名，未同步） | **15** |
| 行 61 整行抽取面 | 30 | **31** |
| 逐行 fact 复算 | 该口径当时不存在 | **78/78 条数复现**（75 行逐字一致，3 行仅差 `\_` 写法） |
| 判"基准过期"的判据 | 只看字节 | 字节 **＋ 标题层级**（`####` 不计） |

复检退出码**仍是 1**：那 1 个抓取失败（`sdk-reference/python-sdk/v2.37.1/sandbox/async.md` 上游 HTTP 404）让"采不全"继续与"采完干净"区分开，与该工具既定语义一致——刷新清掉的是**漂移**，不是**采不到的来源**。

### 12.6 验证与边界

刷新后按根 `README.md` 的 Verification 块**整条链**复跑，全部 `EXIT=0`：12 条 sandbox 门禁 + 6 条 specs 门禁（`--root .`）+ `cargo fmt --check` + `cargo check --workspace --all-targets` + `cargo test --workspace`（78 passed / 0 failed / 1 ignored）+ `cargo clippy --workspace --all-targets -- -D warnings` + 契约套件 **644 / 644**；另有上游采样器 `audit-sandbox-e2b-upstream-source-parity.mjs` 报 **No drift**（20 项读数全复现、vendored 提交未动），四个变异自证驱动复跑 11/11、4/4、16/16、7/7 全红且**逐字节还原**。

**本节没有产生任何实现，也不产生实现授权。** §7-A 至此 **① 收口（② 在 §11 已收）**，**③ 仍归人工**：把 404 那页换到当前版本页会**重导第 69 / 72 行的 SDK 字段**（两行共 98 条 fact 里相当一部分来自该页），那是语义改动而非哈希更新。基准的 `captureNote` 已把该页标注为「保留记录值，待人工决定替换页」——所以它不是被遗忘的烂尾，而是一条**写下来的、有人负责的**开放项。

### 12.7 顺带修掉的第三个同族缺陷：变异驱动的锚点也是手抄的

`target/_doc-count-mutation-proof.py` 把锚点写死成 `第 61 行（30 项）`。本节把行 61 刷成 31 之后，该锚点在文档里**命中 0 次**，而驱动的语义是 `occurrences != 1 → SKIPPED`——于是它会打印 **4/4 通过**（分母随跳过数缩小）而实际只验了 4 刀，**悄悄降级而不报错**。这与 §11.5 的族清单、§4 的手抄读数**是同一类**（本仓判定为最严重的那类失败，这是第三次）。

修法同 §11.5：**从权威处派生**。驱动现在从基准 JSON 取该行的 `e2bFields` / `e2bClis` / `e2bFacts` 现算整行抽取面与字段数，再从文档里按行号定位 `` 第 N 行（M 项 `` 子句——锚点与期望文案**全部现算**，一行手抄数字都不留；`occurrences != 1` 也从"跳过"改成 **`FAILED` 并计入红色数**（分母固定为变异集大小）。

这面镜子照出的第二个缺陷是**变异被累积施加**：行 61 的项数在一组里出现两次（一刀改数字、一刀去引用），前一刀把后一刀的锚点改掉了，表现为 `anchor occurs 0 time(s)`。已改为**每一刀都作用在原始文本上**——独立变异本应如此。

读数：**5 / 5 RED**（比原版多一条，补上了"给数必引用"这个反向分支，与 §11.2 的两向棘轮对齐），文档还原**逐字节一致**（sha256 `07afa5da…`）。


## 13. 追加：§3.2 空档 4 收口——§5 优势断言表纳入形状取证（2026-09-23 晚）

§3.2 第 4 行登记的空档（§5「我们比 E2B 强的地方」的 9 行优势断言没有门禁）本轮闭合。这是 §6 列出的、代理可做的最后两项之一（另一项 §3.2 第 2 行的逐条归属判定仍开放）。

### 13.1 首跑即抓到 5 处真缺陷——其中两处是"在界即通过"的盲区

门禁把 §1.2 的形状取证扩到 §5 后第一次运行就转红，且每一条都是真缺陷：

| 行 | 缺陷 | 真值 |
| --- | --- | --- |
| §5 行 1 | 行锚 `identity.rs:88` 漂移 | `SandboxFencingToken` 真身在 **:97**（`pub struct`）；:88 处是一排 `opaque_id!` 宏 |
| §5 行 2 | 行锚 `model.rs:24` 漂移 | `replay_sandbox_operation` 真身在 **:276**；:24 处是 `SandboxSessionOperationKind` 枚举 |
| §5 行 1 | 裸文件名 `service.rs` | 未写所属 crate |
| §5 行 4 | 裸文件名 `provider.rs:50`、`capability.rs:2` | 未写所属 crate |
| §5 行 5 | 证据格只有 markdown 链接，无任何可反证 token | 改为反引号仓库路径 |

两处漂移行锚**恰好落在此前所有核验的盲区里**：§3.1 覆盖表与 §1.2 形状表对行锚的机器要求是"行号在文件内"（§3.1）或"行号在界内且引文可见"（§1.2 自己的行）。§5 从未被核过，而它的 `:88` 与 `:24` 都仍解析得通——与 §2.4 的教训同构：**一个仍然解析得通、却已指向无关代码的行号是这类腐化最安静的一半**。

### 13.2 落地的判据（family 11 扩展，族数保持 12）

`parseAdvantageClaims` 解析 §5 表（三列：本仓能力 / 证据 / 为什么保留），逐行核验：

1. **能力格非空**；**证据格至少一个反引号 token**（一行点不开的优势是偏好，不是优势）。
2. **至少一个可反证引用**：仓库路径（`REPO_PATH_PREFIXES` 前缀）、Rust 文件、`REQ-*`/`ADR-*` 记录、或在 `crates/`+`database/` 树中真实出现的裸 snake_case 标识符。最后一类使 `sandbox_session_lease` 表名从"一个字符串"变成"一次改名就能杀死的声明"（`identifierOccursInRepository` 扫 `crates/**/*.rs` 与 `database/**/*.{sql,rs}`）。
3. **裸 Rust 文件名拒绝**——`provider.rs:50` 这类名字在 crate 改名时恰好幸存而指向虚无。判据对**行锚的文件部分**同样生效（`lib.rs:4` 的文件部分是 `lib.rs`），因为 `.rs$` 正则读不到行号后缀，而真表用的恰是 `provider.rs:50` 这种带行号的裸名形态。
4. **行锚三查**：文件存在、行号在界内、**该行引用的其他物证至少一项在锚点起 4 行窗口内可见**（§1.2 的引文配对法原样照搬）。§5 与 §1.2 的结构差异只有一个：§5 没有每行 crate 列，所有路径天生仓库根相对。

对齐修法与 §11 一脉相承：**不是把数字改对，而是把口径统一并保住信息**。两处漂移锚按构造真值改写并括注应在该行可见的标识符（`identity.rs:97`（`SandboxFencingToken`）、`model.rs:276`（`replay_sandbox_operation`）、`provider.rs:55`（`satisfies_sandbox_requirements`）、`capability.rs:2`（`RuntimeCapability`））——括注不是装饰，它就是配对判据的右侧。

### 13.3 一个自伤事故：被杀死的变异驱动把门禁留在变异态

§11.5 给驱动补的 `finally` 还原覆盖的是**异常**路径；这一轮证明它不覆盖**进程被杀**：本轮在驱动运行中途中止了它（外部工具超时处置），而它正停留在 SHAPE EVIDENCE 族的某一刀上——`finally` 没有机会执行，门禁被留在 `if (false) {` 的变异态。契约套件**当场抓住**（对应反面用例转红、其余全绿），恢复方式是**定点把那一行改回**而非 `git restore`——因为该文件同时载有本轮未提交的作者改动，`git restore` 会把两者一起抹掉，恰是 `ROLLBACK_RESTRICTION_SPEC.md` 禁止的那种"修复"。随后对全文件扫变异签名（`if (false)` / `false &&` / `void id;` / `for (const row of [])` / `if (true) continue`）确认无其他残留，契约套件复绿。教训记为：**驱动只许跑到完成；对它的任何中止都隐含一次对门禁源的怀疑，必须以签名扫描收尾**。

### 13.4 驱动与套件的同步扩展

- 契约套件：`e2b-parity-matrix-tool.contract.test.mjs` 143 → **159**（新增 16 条 = 14 条 §5 反面用例：缺节 / 空表 / 错列头 / 错格数 / 无能力名 / 无证据 / 无可反证锚 / 裸文件名 / 锚路径不存在 / 非锚路径不存在 / 锚越界 / 锚漂移 / REQ 无记录 / 标识符树上不存在，另加 2 条对照）。总套件 644 → **660**。
- 耦合驱动（`target/_matrix-mutation-proof.mjs`，本地证据）：SHAPE EVIDENCE 族 `own` 清单 15 → 29 条（全族 29/29 转红、异族 0、套件总数全程 159），每条新判据一把独立编辑锚（全部实测在门禁源码中恰好命中一次），隔离运行验证裸名判据不被路径判据掩盖；两个 §5 对照用例列入 POSITIVE（不声明翻转）。为让 §1.2 与 §5 的同形语句不互相污染锚点计数，§5 实现使用独立命名的局部量（`citations` / `lineAnchor` / `anchorFile` / `anchorWindow` 等）——这既是驱动的要求，也让两段判据在源码里一眼可分。
- 基准 `testInventory` 同步：`tests: 660`、矩阵门禁套件 `tests: 159`（基准门禁的 test-inventory 规则首跑即报两条，同步后转绿；分批落地途中的中间读数 657/156 未落盘，以免活文档断言一个已不存在的套件）。
- 基准门禁自己的契约夹具（`sandbox-e2b-field-parity-tool.contract.test.mjs`）同步：`REAL_CONTRACT_TESTS` 644 → 660 及其报告匹配串——该夹具把真实仓库的读数常量化，套件扩容后不同步就会在**全量**运行时转红（单跑该文件绿，全量跑才暴露，恰是只跑单文件的回归会漏掉的形态）。

### 13.5 复核读数与本轮边界

矩阵门禁 `parity matrix: consistent`，新增读数行 `advantage claims: 9 row(s), 4 line-numbered anchor(s) resolved`；§3.2 空档 4 → 3（闭合表新增第四行）；field parity 全绿（660 契约 / 78 Rust / 11 族 4 表面）；上游采样器 `No drift`（vendored 提交 0c21aa2 / ccaf9fc / 17ddc44 / f56a1ed 未动，上游当日无新提交）；全部 sandbox 与 specs 门禁 `EXIT=0`。

**本节没有产生任何实现，也不产生实现授权。** §5 的 9 行优势从"散文"变成"可反证的声明"，但它们描述的仍是候选契约与测试，不是运行时能力：`REQ-*` 27 份仍 0 `ready`，ADR 仍全 `proposed`，机器契约仍无一份 `implementationAuthorized: true`。§7 的 ③（404 页政策）与 C（人审推进）仍归人工。
