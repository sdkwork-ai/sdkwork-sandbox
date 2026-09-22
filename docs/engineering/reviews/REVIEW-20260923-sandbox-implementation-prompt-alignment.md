# REVIEW-20260923: 全自动落地实施 Prompt 与仓库 Canon 对齐

Status: active

Owner: SDKWork Runtime Platform

Date: 2026-09-23

Outcome: 对一份 58 节的「`sdkwork-sandbox` 全自动落地实施 Prompt」（下称 **Prompt**）做了逐节对齐：其功能面**已被 Canon 近乎全覆盖**（5 条 `accepted` + 22 条 `draft` 的 `REQ-*`、27 份 ADR、25 份机器契约、PRD 四分片），无一项能力在 Canon 中完全无承载；但 Prompt 按**自己的优先级第 1、2 条**（`sdkwork-specs` > 本仓规范 > Prompt）必须在 **6 处**按 Canon 改写后才能作为实施输入（crate 命名、指标名、`cargo fmt --all`、公开 REST 面、Linux-only Native Sandbox、未测性能数字），其中第 2 处 Prompt 引用的正是本仓已废弃的旧指标名。**「完美可商业化」的判定权在本仓治理里是人工的**：`node tools/check-sandbox-commercial-readiness.mjs` 输出 NO-GO 是设计行为，解锁需要 22 份 `pending-human-review` 签字、27 份 ADR 从 `proposed` 推进、25 份契约翻转授权字段、127 个证据 id 中 125 个由真实 runner 或人工闭合——这四件事**没有一件是代理可以合法代做的**。本报告不改变任何实现授权状态，不构成对任何 Provider、API、SDK、调度器、隔离策略或部署 Profile 的实现授权（`AGENTS.md` Agent Execution Rules）。

## 0. 目的与分工

Prompt 要求「进入 Autonomous Implementation Mode，直接完成工程化落地」。本报告回答的问题先于该要求：**Prompt 描述的功能面与仓库 Canon 是否一致，不一致处以谁为准，通往商业化的剩余路径由谁拥有。**

Prompt 第一节自己规定了权威序：

```text
1. sdkwork-specs
2. 本项目现有代码规范
3. 本 Prompt
4. PRD / 技术设计文档
5. 工程判断
```

因此本报告的全部裁决都援引优先级高于 Prompt 的材料：`../sdkwork-specs/*_SPEC.md`、本仓 `AGENTS.md`、`docs/product/`、`specs/*.json` 契约与既有评审记录。已有材料的分工不变：[功能走查](REVIEW-20260922-sandbox-functional-module-walk.md)回答"实现逻辑是否有洞"，[跨平台验证](REVIEW-20260922-sandbox-cross-platform-run-and-deploy.md)回答"能否在两平台构建运行"，E2B 对齐审计回答"能力集对齐没有"，本报告回答 **"外部实施指令与 Canon 的对齐与冲突"**。

## 1. 冲突裁决：Prompt 必须按 Canon 改写的 6 处

以下每条都是"Prompt 第 3 优先级 输给 第 1/2 优先级"的实例。裁决不是否决 Prompt 的意图，而是把意图改写成合规形态。

| # | Prompt 位置 | Prompt 原文（摘要） | 冲突规范 | 裁决 |
| --- | --- | --- | --- | --- |
| C1 | §6 目标 Workspace | crate 列表 `sdkwork-sandbox-core` / `-runtime` / `-orchestrator` / `-network` / `-security` / `-resource` 等；`tools/sandbox-cli`；`jobs/`、`configs/`、`apps/` 目录 | `NAMING_SPEC.md` §3.1 与 `AGENTS.md` Code Style：新 crate 禁用泛化后缀 `core`/`runtime`/`manager`/`backend`；本仓字典无 `jobs/`/`configs/`/`apps/`（`apis/`、`sdks/`、`etc/` 已有定义） | 按**职责命名**：Provider 适配器沿用 `sdkwork-sandbox-provider-{local,firecracker}`（local 已存在）；CLI 已是 `crates/sdkwork-sandbox-cli`；编排/调度面按 `REQ-2026-0016`/`0023` 获批时的职责名拆分，不按能力清单直推 crate（[TECH_ARCHITECTURE.md](../../architecture/tech/TECH_ARCHITECTURE.md) 第 3 节明文）。后台任务属组件而非新顶层目录，归 `MODULE_BIN_SPEC.md` 的 `bin/` 九入口或具体 crate |
| C2 | §39 指标清单 | `sandbox_create_latency`、`runtime_pool_hit/miss`、`template_cache_hit/miss`、`cpu_usage`、`network_rx/tx` 等 | `OBSERVABILITY_SPEC.md` Metric naming：计数器必须 `_total`、时长必须带单位（`_duration_seconds`）、尺寸必须带单位；布尔/双态用标签不拆指标 | **Prompt 引用的是本仓已废弃的旧 PRD 指标名**（2026-09-23 前的 `PRD-sandbox-surfaces.md` 第 13 节）。现行权威是 13 个规范名 + `apis/async/sandbox-observability-catalog.json` 的 `metrics.productFamilies` 机器映射（6 控制面 join 现有 32 指标 / 7 运行面登记承载缺口），由 `tools/check-sandbox-requirement-traceability.mjs` 第 6 条规则族核验。Prompt 实施时必须改用该清单；把 `*_latency` 写回任何 live 文档会同时违反规范与门禁 |
| C3 | §48 编译检查 | `cargo fmt --all -- --check` | `AGENTS.md`：`--all` 会格式化本地路径依赖，报告兄弟仓库（`sdkwork-web-framework`、`sdkwork-utils`、`sdkwork-database`）拥有的格式偏差，本仓不得编辑 | 门禁是 `cargo fmt --check`（无 `--all`）。该差异已被 `tools/check-sandbox-doc-integrity.mjs` 固化：live 文档里出现 `cargo fmt --all` 即红 |
| C4 | §26 API Gateway | 公开 REST 面 `POST /v1/sandboxes`、`GET /v1/sandboxes/{id}`、files/… 通配路由 | API 权威未定且未授权：内部控制面由 `REQ-2026-0023`（draft）承载；`PRD.md` 第 9 节待决「HTTP internal-api、internal RPC 还是并存」；`API_SPEC.md`/`SDK_SPEC.md` 要求权威契约先行、生成器产出、不手写 | 路由表是**未来 REQ 的设计输入**，不是现在的 API 权威。实施顺序必须是：`REQ-2026-0023` 进 `ready` → `apis/` 权威 OpenAPI → 生成 SDK（三语言同语义，`PRD-sandbox-surfaces.md` 第 12 节）→ 任何路由实现。手工先写路由 = 违反 `AGENTS.md` Agent Execution Rules |
| C5 | §9/§52 Native Sandbox | 第一阶段用 Linux 六类 Namespace + Cgroup v2 + Seccomp 自主实现 Sandbox | 平台故事与承载：Local Provider（`REQ-2026-0003`，`HostUser`，Windows Job Object / Linux cgroup v2 / macOS 明确拒绝 Terminal）是优先 1；Namespace Sandbox 是运行模式 **Mode 1**（[PRD-runtime-execution-model.md](../../product/prd/PRD-runtime-execution-model.md) 第 2 节），仓库 Canon 对 Mode 1 尚未拆分独立需求记录（缺口已在 `PRD.md` 第 8 节「尚未拆分的能力」表登记，本报告引用该登记而非另立断言） | Linux namespace 方案是 Mode 1 的 Linux 子集，方向与 Canon 一致，但**实施前必须先拆分 Mode 1 的独立 `REQ-*` 与 ADR**（含 `IsolationAssurance::Container` 承载、跨平台矩阵、与 Local Provider 的边界）。在本仓 Windows 开发环境上该代码不可编译验证；真实 containment 证据（`REQ-2026-0007` Conformance、`REQ-2026-0003` 平台证据）只能在真实 Linux 上产生 |
| C6 | §36 性能目标 | `Native P50 < 10ms / P95 < 30ms`、`Warm MicroVM P50 < 100ms`、`单 Node 10,000+ Sandbox`、`Idle < 10MB` | `PRD.md` 第 6 节：未完成参考硬件测量前不得把容量/时延数字写成已实现 SLO；[TECH-performance-baseline.md](../../architecture/tech/TECH-performance-baseline.md) 自述发布门禁资格不合格（分子已测、分母不存在） | 这些数字作为**候选工程目标**可进入未来 REQ（`REQ-2026-0019` 已要求记录固定硬件/工作负载/统计方法并出具 p50/p95/p99）的 Performance 行；在此之前不得出现在任何 live 文档的承诺位。参考硬件基线定义是 `PRD.md` 第 9 节待决问题 |

## 2. 逐节对齐：Prompt 功能面 → Canon 载体

58 节按主题归并为 22 行。`载体` 列全部可解析到真实记录；`状态` 沿用 Canon 口径（`accepted`/`draft`/未授权/刻意不做）。

| Prompt 节 | 功能 | Canon 载体 | 状态 |
| --- | --- | --- | --- |
| 一~五 | 权威序、执行模式、规范阅读 | `AGENTS.md`、`SOUL.md`、`../sdkwork-specs/README.md` 任务矩阵 | 已生效（Prompt 亦受其约束） |
| 六 | Workspace 布局 | 见冲突 C1 | 冲突已裁决 |
| 七 | 七条解耦不等式 | `PRD.md` 第 4 节（九条不等式，是 Prompt 七条的超集）；`PRD-runtime-execution-model.md` 第 1 节 | 已承载且更严 |
| 八 | `SandboxBackend` trait | `SandboxProvider` SPI（`crates/sdkwork-sandbox-provider-spi`，descriptor/health/allocate/start/stop/destroy + fail-closed 协商）；`pause`/`resume`/`fork`/`snapshot` 属 `REQ-2026-0008`/`0021`（draft）与 Provider 契约扩展 | 候选实现；扩展未授权 |
| 九 | Native Sandbox（namespaces） | 冲突 C5；Local Provider `REQ-2026-0003`（accepted）；Mode 1 需求拆分未闭合（见 `PRD.md` 第 8 节登记） | 部分承载（Local）；Mode 1 未拆分 |
| 十、十一 | Filesystem 隔离与 API | `REQ-2026-0007`（draft，命令契约）；`PRD-sandbox-surfaces.md` 第 3 节（14 项能力面）；OverlayFS/COW 见 `PRD-runtime-execution-model.md` 第 7.2 节（需求拆分未闭合，`PRD.md` 第 8 节登记） | draft + 未授权 |
| 十二 | Process Runtime | `REQ-2026-0007`（Executable+Argv、禁 shell-string、Fencing、有界输出）；PTY 拆分由 `REQ-2026-0024`（draft）固定 | draft |
| 十三、十四 | 网络三模式与出网 | `REQ-2026-0014`（draft，`DenyAll`、永久拒 Metadata/Host/横向）；`PRD-sandbox-surfaces.md` 第 6、7 节；`shared` 模式的需求拆分未闭合（`PRD.md` 第 8 节登记） | draft + 部分未拆分 |
| 十五 | Resource（cgroup v2） | `REQ-2026-0015`（draft：CPU/Memory/PID/IO、Effective Readback、immutable Usage Fact） | draft |
| 十六 | Security（seccomp 等） | `REQ-2026-0003`/`0008`/`0011`（draft）+ `SECURITY_SPEC.md`；Local 的安全边界契约已机检候选 | draft |
| 十七、十八 | Runtime Pool 与 Warm 模型 | `REQ-2026-0019`（draft：tenant-neutral `PreparedSlot`、fenced Claim、Sanitization/Quarantine、fast-allocation evidence）；`PRD-runtime-execution-model.md` 第 3 节（Warm/Idle/Busy/Draining、min/max、背压） | draft（与 Prompt 模型一致） |
| 十九、二十 | Template 与 Builder | `PRD-runtime-execution-model.md` 第 4 节（版本化/不可变/构建输入允许 Dockerfile、Docker 仅构建输入——与 Prompt §20 完全一致）；需求拆分未闭合（`PRD.md` 第 8 节登记；E2B 矩阵行 25-33） | 产品要求已写，未拆分 |
| 二十一~二十三 | Snapshot / COW / Lazy Memory | `PRD-runtime-execution-model.md` 第 5、7 节；Checkpoint/Firecracker-Snapshot Gate 0 为 `REQ-2026-0021`/`0008`（draft）；产品级 Snapshot/Fork/按需内存的需求拆分未闭合（`PRD.md` 第 8 节登记） | 部分承载 |
| 二十四 | Firecracker Backend | `REQ-2026-0008` + `REQ-2026-0012`（供应链元组）+ `REQ-2026-0011`（Host Broker）+ `REQ-2026-0013`/`0014`（块设备/网络），全部 draft | draft |
| 二十五 | Agent Runtime（`agentd`） | `PRD-sandbox-surfaces.md` 第 9 节（未授权）；`REQ-2026-0024` 明确 Guest Agent Stream 未批准 | 刻意等待治理 |
| 二十六、二十七 | API 与 Control Plane | 冲突 C4；`REQ-2026-0023`（draft）+ `REQ-2026-0016`/`0018`（draft） | draft |
| 二十八 | Node Agent | `REQ-2026-0017`（信任侧 draft）；Node Agent runtime 未批准（`REQ-2026-0016`/`0017` 边界） | draft（信任侧） |
| 二十九~三十二 | Scheduler / 索引 / 公平 / 背压 | `REQ-2026-0016`（draft：Hard Placement Filter、Tenant-aware Fairness、Atomic Reservation）；背压在 `PRD-runtime-execution-model.md` 第 3 节与 `REQ-2026-0019`；O(log N) 索引与加权公平队列属该 REQ 获批后的设计节内容 | draft |
| 三十三 | Multi-Tenant | `REQ-2026-0016`/`0017`/`0018`（draft）；Cross Tenant = Deny 是 Canon 全局默认拒绝原则的实例 | draft |
| 三十四 | 存储分层 L1/L2/L3 | `PRD-runtime-execution-model.md` 第 8 节（与 Prompt 一致，含对等缓存与区域约束 `REQ-2026-0026`） | 产品要求已写，实现未授权 |
| 三十五~三十七 | 性能/基准/禁误优化 | `tools/bench-sandbox-lifecycle.mjs` + `TECH-performance-baseline.md`（门禁资格不合格）+ `TECH-performance-and-capacity.md`（工程目标）；禁误优化条目与 `RUST_CODE_SPEC.md`（无阻塞等待、有界重试、锁不跨 `.await`）一致；数字见冲突 C6 | 已承载（目标口径） |
| 三十八 | 错误处理 | `RUST_CODE_SPEC.md`（thiserror + source 链）；已实现面为 typed error（`SandboxLifecycleError` 等，见功能走查 M01-M10） | 已承载且有候选实现 |
| 三十九 | 可观测性 | `REQ-2026-0010`（draft，32 指标契约）+ 冲突 C2 的 13 指标族映射 | draft |
| 四十~四十二 | 测试/安全/混沌 | `TEST_SPEC.md`；68 个 Rust 用例 + 616 契约用例；安全测试=各契约 evidence id（127 个，125 被真实 runner/人审阻塞）；混沌场景落在 `REQ-2026-0005`（恢复）与 `REQ-2026-0016`/`0019`（Quarantine/背压）语义内 | 部分承载，证据被阻塞 |
| 四十三 | CLI | `crates/sdkwork-sandbox-cli` 存在（`fn main() {}`，E2B 矩阵行 68 判 ❌）；命令面属 `REQ-2026-0009`/`0023` 获批后工作项（readiness 包 Phase 4） | 骨架，未授权 |
| 四十四 | 部署 | `MODULE_BIN_SPEC.md` 九入口 + `DOCKER_SPEC.md`/`DEPLOYMENT_SPEC.md`；「Docker 部署 Node 而非 Sandbox 本身」与 `PRD.md` 非目标（Docker 仅构建输入）**兼容**；生产拓扑 Linux+KVM+NVMe 与 `REQ-2026-0008` 一致 | 已承载（规范侧） |
| 四十五 | 文档 | `DOCUMENTATION_SPEC.md` + `docs/INDEX.yaml` 登记门禁；文档与代码同步由 traceability/parity 门禁族保障 | 已生效 |
| 四十六~四十九 | Phase 顺序与完成标准 | 与 [gate-zero-exit-readiness-package.md](../gate-zero-exit-readiness-package.md)「评审退出后立即可执行的工作项」（Phase 0.5→5）一致度高于与 Prompt 十八阶段顺序的一致度；**本仓顺序以签字解锁为前置**，不是自由推进 | 已承载（更严） |
| 五十 | Git 提交纪律 | 与本仓提交规范一致；提交须按仓库惯例分拆 | 已生效 |
| 五十一 | 禁伪实现 | `RUST_CODE_SPEC.md` 禁 `todo!()`/`unwrap`/伪装成功 + 本仓证据注册表（127 id）机制——**Prompt 的要求与 Canon 同向且 Canon 更严** | 已承载且更严 |
| 五十二 | 特权集中管理 | `REQ-2026-0011` Host Isolation Broker（draft：特权固定操作、短期 Grant、Fencing、Audit） | draft |
| 五十三、五十四 | E2E Demo | `demos/` 已有语言矩阵演示（上一轮）；完整 Sandbox E2E（create→exec→pause→snapshot→restore）依赖 P0 缺口（零运行入口/零真实 Provider，E2B 审计 §4） | 阻塞于授权 |
| 五十五~五十八 | 最终形态与交付清单 | `PRD.md` 第 7 节 Phase 0-V4 与第 7 节能力建设顺序表；27 项交付物映射见本报告第 3 节 | 已承载 |

**对齐结论：58 节中没有一项功能在 Canon 中找不到载体**；Prompt 的真实增量是工程细节（userfaultfd 页画像、O(log N) 调度索引、加权公平队列、混沌场景清单），它们的正确去处是相应 `REQ-*` 进 `ready` 前的**设计节**，而不是现在就写代码。

## 3. 27 项交付物 → 6 个交付切片 → 解锁路径

Prompt §58 列出 27 项交付。`tools/check-sandbox-commercial-readiness.mjs` 的 6 个交付切片是其超集分组，当前全部 NO-GO：

| 交付切片（门禁读数） | 覆盖 Prompt 交付物 | 解锁所需签字（`pending-human-review`） |
| --- | --- | --- |
| governed-foundation（candidate-only） | 1/18/23/24/26/27 的规范与契约面 | 基础验证包已 accepted；Service Host + Observability 两包 |
| standalone-local-developer-runtime（blocked） | 2/3/6/7/22 | `REQ-2026-0003` Local Provider、`REQ-2026-0007` Command、`REQ-2026-0021` Workspace 事务、`REQ-2026-0022` 驻留 |
| cold-firecracker-cloud-runtime（blocked） | 12/13/14/15/26 | `REQ-2026-0008`/`0011`/`0012`/`0013`/`0014`/`0015`、`REQ-2026-0026` |
| runtime-pool-and-high-concurrency（blocked） | 8/9/10/11/15/16 | `REQ-2026-0016`/`0017`/`0018`/`0019`/`0020` |
| four-repository-integration（blocked） | 18/19/20/21/27 | `REQ-2026-0004`/`0005`（已 accepted 的边界）+ `REQ-2026-0023`/`0025`/`0027` |
| commercial-operations-and-release（blocked） | 23/26/27 | 商业发布决定记录（`sandbox-commercial-readiness.contract.json` `no-go`）+ `REQ-2026-0027` 发布集 |

完整清单与 5 步签字程序见 [human-review-signoff-backlog.md](../human-review-signoff-backlog.md)（22 份 pending，其中 14 份被机器契约具名门控）；签字后的工程序列以 readiness 包 Phase 0.5→5 为准，Prompt 的十八阶段只有映射到该序列才可执行。

## 4. 结论：代理侧已闭环，剩余路径是人工的

1. **代理可做的对齐已全部做完并受门禁保护**：E2B 78 行字段级基准与审计双向记账；PRD §11 ↔ E2B、PRD 状态机 ↔ 实现枚举、PRD 指标族 ↔ 契约映射三个 join 门禁；22 份签字包的机器读数；本报告将 Prompt 的 6 处规范冲突显式裁决归档。任何后续实施指令（包括 Prompt 本身）进入本仓时，都会撞上这些门禁而不是静默通过。
2. **「完美可商业化」在本仓的定义是机器可查的**：`check-sandbox-commercial-readiness.mjs` 从 NO-GO 翻转、`check-sandbox-human-review-signoff.mjs` 的 pending 归零、`check-sandbox-evidence-traceability.mjs` 的 127 个证据 id 全部有产出者。当前读数：NO-GO（6 切片 blocked / 5 缺 ready 契约 / 4 跨仓权威 blocked）、22 份 pending、125 个证据 id 无产出者。
3. **下一步不存在代理侧动作**。最高杠杆的推进是把 22 份签字包按风险排序提交人工评审（11 个 critical Provider/隔离包优先），每份签字解锁 readiness 包对应 Phase 的实现窗口；在那之前写任何 Provider/调度/隔离代码都违反 `AGENTS.md`，且在本机不可验证。

## 5. 本轮追加（2026-09-23）：已验收范围内缺陷的修复记录

按 2026-09-22 功能走查 §4 的「最小可批范围」，以下落在 `accepted`（REQ-2026-0002/0005）范围内的发现在本轮修复并配齐单元测试；这不需要任何新签署：

| 发现 | 修复 | 回归测试 |
| --- | --- | --- |
| F-01 对账页级联失败 | `service.rs` 对账循环把 `InvariantViolation`（如绑定引用已注销 Provider）与写入侧 `InvalidStoredData` 降级为本项 `Failed`，页继续收敛 | `sandbox_reconciler_degrades_an_unregistered_provider_session_instead_of_aborting_the_page` |
| F-14 选择循环不续租 | `select_sandbox_provider` 携带租约，健康探测经 `execute_sandbox_provider_call` 逐次续租；探测超时/失败按不健康继续，租约真丢失才 `LeaseLost` | `sandbox_provider_selection_renews_the_lease_before_every_health_probe`（3 探测 ×25ms vs 60ms 租约） |
| F-15 爆炸半径（保留策略不动） | 仓储列表改为轻量候选投影（id + 行级状态），不再在对账枚举中加载操作账本；不可读会话在循环内降级为 `Unreadable`，记录不删除/不截断/不过期 | `sandbox_reconciler_reports_an_unreadable_session_and_keeps_the_page_converging` |
| F-04 适配器写入校验不对称 | 校验核心抽为 `validate_sandbox_session_persisted_invariants` 并导出；内存适配器 insert/save 与 PostgreSQL 的 `Snapshot::capture` 同门槛 | `validate_sandbox_session_persisted_invariants_rejects_a_ledger_that_replays_to_another_state` |
| F-02 隔离强度全序无锁定 | `capability.rs` 全对全序断言（5 变体 10 对 + 反自反） | `isolation_assurance_declaration_order_is_the_security_ladder` |
| M02 标识边界未锁定 | `identity.rs` 空串/128 字节上界/129 越界断言 | `rejects_empty_and_over_bound_opaque_identifiers` |
| M07 重放语义空档 | 钉死「重放 `Succeeded` 返回当前权威状态」语义（无重复 Provider 副作用） | `sandbox_replaying_a_succeeded_operation_after_later_operations_returns_the_current_session` |

未动且理由明确：F-05（`Running` 健康对账 = 新能力，需新 `REQ-*`）；F-09 保留策略本身（REQ-2026-0020 draft，边界段冻结）；F-06 错误枚举拆分（触及已发布契约，需跨仓消费者核对）；F-03 的可复现轮（需真实 PostgreSQL）。

账目同步：工作区 Rust 用例 68 → 75（74 passed / 1 ignored，唯一的 `#[ignore]` 仍是 PostgreSQL 集成测试），审计 §3.1 表逐项认领并更新为 75 = 74 + 1；一处既有测试（`sandbox_provider_call_maps_sandbox_lease_renewal_failure_to_lease_lost`）按新续租时机更新断言——租约续租失败现在发生在首个 Provider 调用（选择探测）处，会话保持 `Created` 无任何副作用。

## 6. 本轮追加（2026-09-23，续）：F-06 结果语义与 F-03 具名证据轮

| 发现 | 修复 | 回归测试 |
| --- | --- | --- |
| F-06（结果枚举侧）对账把「会话已消失」误报为 `LeaseUnavailable` | `SandboxSessionReconciliationOutcome` 新增 `Vanished`：两条消失路径（租约获取后 get NotFound、失败收敛中消失）改报 `Vanished`，取不到租约仍报 `LeaseUnavailable`——两种运维工单从此分开计数 | `sandbox_reconciler_reports_a_vanished_session_as_vanished_not_lease_unavailable` |
| F-03 SQL 证据不在门禁链 | 已存在的真实证据运行器 `tools/testing/sandbox-postgres-evidence.mjs` 接入具名验证轮：`pnpm run gate0:postgres:evidence`，并在根 README Verification 清单登记（需 Docker + 可弃容器 PostgreSQL 16/17，不进默认门禁链，这是它的运行前提而非缺陷） | 运行器自带（容器化 PostgreSQL + `#[ignore]` 集成套件） |

F-06 的**错误枚举侧**（`SandboxLifecycleError` 拆分「本地过期」与「被夺走」）仍按走查建议缓行：该错误族是 PRD 第 4 节点名的领域契约，拆分前需跨仓消费者核对。

账目同步：工作区 Rust 用例 75 → 76（75 passed / 1 ignored），审计 §3.1、基准 `testInventory`、工具 README、根 README、Gate 0 视图五处计数已随测同步。

## 7. 本轮追加（2026-09-23，续二）：走查文档类发现闭环（F-07/F-08/F-10 + F-09 注释）

| 发现 | 修复 | 防回归 |
| --- | --- | --- |
| F-07 组件清单漏登 `sdkwork-api-sandbox-assembly` 且无门禁 | `TECH-modules-and-contracts.md` §2 组件表与 §5 Repository Layout 补登（8 个 crate 全数入账）；并实现走查给出的「彻底做法」：`check-sandbox-component-contract-alignment.mjs` 新增 **R7 规则**——`crates/*` 持 `Cargo.toml` 者必须有 `specs/component.spec.json`，`crates/` 下的组件 spec 指向的目录无 `Cargo.toml` 即描述不存在的 crate（双向记账，工具头部与 `tools/README.md` R 清单同步） | `component-contract-alignment-tool.contract.test.mjs` 新增 3 例（缺 spec 转红 / spec 无 crate 转红 / 空目录不误伤），20/20 pass |
| F-08 §4 依赖图计划与现状不可区分 | 图后新增「当前落地状态」清单：落地 2 条（`SERVICE → PORTS`、`STORES → PORTS`，依据各 crate `Cargo.toml` 实测），其余 11 条零落地逐条点名 | 静态散文；依赖事实可由 Cargo.toml 复核 |
| F-10 §3.1 叙述段历史数字口径 | 两处叙述收紧为可复核分解：「3 个 crate 的 48 个可运行用例 + `#[ignore]` 集成用例记在已覆盖侧 = 49；两仓储 crate 5 + 14 = 19」 | 散文；数字与 §3.1 表分解一致 |
| F-09 降级后的纯注释修复 | `MAX_SANDBOX_SESSION_OPERATIONS` 常量注释补指 REQ-2026-0005 Release And Review Boundary 的「不得删除/截断/过期」禁令（同一事实两处强度对齐） | 注释；行为不变 |

至此 2026-09-22 功能走查登记的全部 15 项发现状态：**9 项已修复并受测试/门禁锁定（F-01/02/03/04/06结果侧/07/08/10/14），F-11/12/13 上轮已修，F-15 已修爆炸半径一半；F-05（新能力）与 F-09（保留策略）持awaiting-human-review，F-06 错误枚举侧持awaiting-cross-repo-consumer-check** ——工程侧在授权范围内再无可修项。

## 8. 本轮追加（2026-09-23，续三）：acquire 侧消失级联（F-01 同族残留）与失败关闭边界锁定

在为 F-15 补边界测试时发现的**新残留**：对账循环在**租约获取**一步对消失会话同样以 `Repository(NotFound)` 击穿整页——`…-repository-memory`（显式 `NotFound` 分支）与 `…-repository-sqlx`（会话行消失 → `INSERT…SELECT` 空源 → 状态查询 `None => NotFound`）两个适配器行为一致。这是 F-01 同族级联的第三个入口（前两个：get 阶段消失、provider 引用失效，均已修），位于「列表之后、取租约之前」。

| 修复 | 测试 |
| --- | --- |
| 对账循环的 acquire 步骤改为显式 match：`NotFound` → 本项 `Vanished`（与 get 侧消失同票）；`Ok(None)` → `LeaseUnavailable`；其余仓储错误仍整页失败关闭 | `sandbox_reconciler_reports_a_session_vanished_before_lease_acquisition_as_vanished`（消失项 Vanished + 健康项 Reconciled） |
| 降级精确性锁定（负向）：列表级 `Unavailable`（基础设施故障）**不**降级为空页，而是失败关闭 | `sandbox_reconciler_fails_closed_when_the_page_list_is_unavailable` |
| 同上，会话加载级：仅 `InvalidStoredData` 降级，`Unavailable` 仍整页失败关闭 | `sandbox_reconciler_fails_closed_when_a_session_load_is_unavailable` |

R7（组件契约门禁 crate↔spec 双向记账）在本轮落地并配 3 例契约测试（20/20），`tools/README.md` R 清单同步。

账目：工作区 Rust 用例 76 → 79（78 passed / 1 ignored），五处计数表面与两份契约测试常量同步；契约套件 619/619；clippy `-D warnings` 首次纳入例行回归并通过。

## 9. 本轮追加（2026-09-23，续四）：P0 签署包决策清单与文档头同步

为把「商业化落地」的最短人工路径再缩短一步，[Gate 0 退出就绪包](../gate-zero-exit-readiness-package.md) 新增 **REQ-2026-0020 待决策清单**（D1–D7：Operation 上限、Session 生命周期、终态保留窗口、Late Retry Outcome、物理命名、MIG 策略、Kernel 映射），每行给出现状锚点（含代码临时上界 10_000 与其冻结禁令）与批准 Owner——评审者打开即可逐行落笔，无需先读 93 行需求与 1439 行集成测试。数值仍必须由 Owner 给出，清单不代填。

同轮修正文档陈旧：四个被实质编辑的技术/PRD 文档的 `Updated:` 头从 2026-07-30/2026-09-22 同步为 2026-09-23。

## 10. 本轮追加（2026-09-23，续五）：规范符合性专项审计（分页/SQLite/内存/锁/事务）

按 PAGINATION_SPEC、RUST_CODE_SPEC 与 Canon 边界对已实现面做专项审计，读数：

| 维度 | 审计结论 | 依据 |
| --- | --- | --- |
| 分页对齐 PAGINATION_SPEC | **合规**。服务层先验 `page_size`（1..=200，超界拒绝）、keyset 续传（`SandboxSessionId` 不透明游标，无数值 offset）、O(page) 有界、has-more 用 `LIMIT 1` 探测（优于 page_size+1 上界）；仓储层 SQL `WHERE tenant_id=$1` + `LIMIT`，租户隔离在查询内；内存适配器走 §5.3 认可模式（维护 BTreeMap 索引 + key-range + early-take）；§2.5 批量清扫豁免条款正好覆盖 Reconciler 场景 | `check-pagination.mjs --workspace .` 通过 + 逐条人工对照 |
| SQLite 完整性 | **Canon 明文不做服务端 SQLite**：REQ-2026-0022 只允许 SQLite 作为 BirdCoder/Kernel 的 `client-local` 有界设备状态，明确禁止作为 Server Fallback；全仓 grep 零 sqlite/rusqlite/sqlx-sqlite 引用。服务端持久适配器 = PostgreSQL（候选实现）+ 内存适配器（非生产、已标注）。"未实现 SQLite" 是边界遵从，不是缺陷 | REQ-2026-0022 + grep 读数 |
| 内存 / OOM | 全部列表路径 O(page ≤ 200)；单会话 hydrate 受 `MAX_SANDBOX_SESSION_OPERATIONS+1` 窗口约束、超界失败关闭（保留策略归 REQ-2026-0020，已冻结）；所有 Provider 调用经唯一超时包裹点（`tokio::time::timeout`）；sqlx 无无界 `fetch_all`（3 处均受 id 集 ≤200 / 窗口 ≤MAX+1 约束，1 处 `LIMIT ≤200`）。唯一有界性缺口 = 操作账本写路径无上界（F-09，REQ-2026-0020 冻结，已登记） | 代码路径 + grep 读数 |
| 锁与死锁 | 内存适配器全部为 tokio RwLock，锁内无 `.await`；服务层无进程内锁；DB 侧单行租约锁 + `lock_timeout=2s`/`statement_timeout=30s` + CAS（`ON CONFLICT … WHERE`），逐会话顺序取锁，无多锁环 | 代码路径 + RUST_CODE_SPEC 规则 |
| 并发 | 单调 Fencing Token（拒 0 与 i64::MAX 溢出）、版本 CAS、租约过期/接管/陈旧令牌拒绝均有实现与测试；多副本调度竞争语义在 REQ-2026-0016/0018（draft） | 走查 §1 + 测试 |
| 高可用 | 未实现 = Canon 阶段设计：HA 属 V2（Scheduler/Cluster Placement/Node Trust，REQ-2026-0016/0017 draft）；单机 Standalone 形态当前不存在 HA 承诺 | PRD §7 + REQ 状态 |
| 数据库事务与设计 | 4 表基线 + 可复现性门禁通过（`db:materialize:contract` 逐字节比对）；事务含超时与 REPEATABLE READ 读快照；`tenant_id TEXT→BIGINT` 迁移阻塞在 REQ-2026-0018 登记 | 门禁读数 |
| API 定义 | 零路由（`ROUTE_CRATE_COUNT: 0`，形状门禁核验）；内部控制面 REQ-2026-0023 draft；`apis/` 现有命令/可观测/事件三份 draft 契约，均未授权实现 | Canon |
| 虚假实现 | 无。伪 Host Boundary 仅测试编译；内存适配器明确标注非生产；全部"未实现"均被四态标记 + 形状门禁核验；clippy `-D warnings` 通过 | 审计 §1.2 + R7 后的门禁族 |

结论：在已授权实现范围内，未发现新的虚假实现、OOM 面、死锁面或分页违规；规模化集群能力（HA、多副本、容量执行）按 Canon 属 V2 阶段且承载 REQ 均为 draft——它的"缺失"是治理状态，不是实现隐藏缺陷。改进方案不变且已收敛：按 §4/§7 清单推进 22 份人工签署（首选 REQ-2026-0020），在有 Docker+PostgreSQL 的宿主机运行 `gate0:postgres:evidence`，签署后按 Phase 0.5→5 序列实现。

## 11. 本轮追加（2026-09-23，续六）：依赖新鲜度、sdkwork-utils 复用与 API 模式审计

| 维度 | 审计结论 | 依据 |
| --- | --- | --- |
| 依赖版本新鲜度 | `cargo update` 读数 **0 个可升级**——锁文件已在本仓声明的兼容范围内最新（thiserror 2、axum 0.8、sqlx 0.9、tokio 1.53 等均为当前大版本）。2 个「落后于最新」的传递依赖（`matchit 0.8.4` 被 axum 自身 semver 钉死、`generic-array 0.14.7` 属 TLS 加密栈传递）不可越权强升——覆盖上游钉死版本属于破坏供应链语义的伪优化 | `cargo update --verbose` 读数 |
| sdkwork-utils 复用 | 现有适用点已用（内存适配器 `sdkwork_utils_rust::datetime`）；对其 validation/string 模块逐 API 对照后结论：均为表单格式类校验（email/uuid/url/ipv4/e164）与大小写/mask 工具，**没有**能等价替换本仓两个安全边界谓词的能力（`validate_opaque_id` 的 path-like 拒绝 + 字符白名单 + 128 字节上界、`is_safe_sandbox_allocation_key_id` 的 ascii-graphic 白名单）——替换会削弱安全精度，属假复用。零可消除冗余 | 逐 API 对照 |
| API 输入输出标准 | `check-api-operation-patterns.mjs --workspace .` 通过：`apis/` 现有命令契约、可观测目录与事件 AsyncAPI 的输入输出模式符合 API_SPEC 模式规则（int64-as-string 等 wire 规则包含在内核工具的检查面内） | 工具读数 |

## 12. 本轮追加（2026-09-23，续七）：公共 API 文档质量收口

按 RUST_CODE_SPEC「公共 API 必须文档化、`#[must_use]` where applicable」完成两轮 pedantic 诊断后的收口：

1. **`#[must_use]` 79 处**：全部纯访问器/构造器（SPI、Service、Reconciliation、Assembly）补齐属性，`clippy::must_use_candidate` 清零。
2. **`# Errors` 文档 23 处**：SPI 标识宏（覆盖 9 个 parse 类型）、FencingToken/AllocationRef 构造器、Assembly bootstrap 三函数、快照 capture/restore/validate、Protector trait 四方法、Re-encrypt 分页、Service 五条公共命令与两个构造器——每处写明真实错误变体集合；`clippy::missing_errors_doc` 清零。
3. 行锚联动：插入导致 `bootstrap.rs:19` 锚失效，已按门禁指引更新为 `bootstrap.rs:26`——门禁迫使文档与代码同步的又一次实际运作。
4. 明确不采纳项：pedantic 的 `same-prefix-fields`（`sandbox_*` 前缀是 Canon 强制命名）、`u64→usize` 截断（平台为 64 位且值受 i64::MAX 约束）、`too-many-lines`（重构风险大于收益）；pedantic lint 不引入 workspace 强制（其会波及测试夹具构造器，`#[allow]` 噪音大于收益）。
