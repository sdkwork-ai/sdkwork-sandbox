# REVIEW-20260922: Sandbox 功能模块逻辑走查

Status: active

Outcome: 首批走查完成 — 23 个功能模块逐一定判，登记 15 项发现（1 项已实测复现的级联失败、**1 项租约不覆盖 Provider 选择循环、在策略允许的最大 Provider 超时下三次健康探测即耗尽租约**（已实测复现）、**1 项读路径把「单会话操作历史超界」升格为「整页对账永久失败」而写路径完全不设上界**、1 项未锁定的隔离强度全序、1 项**不在门禁链上**的 SQL 证据路径、1 项两个 L4 适配器的写入校验不对称、1 项评测入口文档的清单漂移与风险低估（已修复并由门禁守住）、1 项索引契约测试的解析缺陷（已修复）、1 项**门禁自身文档里过期的套件读数**（已修复并由该门禁守住））。治理面读数：27 条 `REQ-*` 中 **0 条 `ready`、5 条 `accepted`**（REQ-2026-0001/0002/0004/0005/0006），27 份 ADR 全部 `proposed`，24 份 `specs/*.json` 中 0 份 `implementationAuthorized: true`。本报告不改变任何实现授权状态，它是一份跨产品的持续评估，不绑定单一 `REQ-*`/ADR。**§4 给出最小可批范围：本轮发现里 6 项落在已 `accepted` 需求的范围内，不需要新签署即可修；只有 1 项真正需要先批 `REQ-*`。**

Owner: SDKWork Runtime Platform

Date: 2026-09-22

## 0. 目的与分工

本报告回答一个既有材料没有回答的问题：**已落地的每一块实现逻辑本身是否完整、是否有洞。**

仓库此前已有三张表，分工如下，本报告不重复它们：

| 既有材料 | 回答的问题 | 是否受门禁保护 |
| --- | --- | --- |
| [`TECH-e2b-capability-parity.md`](../../architecture/tech/TECH-e2b-capability-parity.md) §1.2 | 每个组件的**形状**（规模、存在与否） | 是，第 11 条规则族 |
| 同上 §3.1 | 每个实现的**用例覆盖** | 是，第 9 条规则族 |
| 同上 §3.2 | **覆盖空档**及其性质 | 是，第 7 条规则族 |
| **本报告** | **实现逻辑正确性**（走查而非清点） | 否，本报告首次提出该问题 |

本报告不是设计评审，也不是授权评审。它不改变任何 `REQ-*` 状态、任何 `implementationAuthorized` 取值，也不构成对任何 Provider、API、SDK、调度器、隔离策略或部署 Profile 的实现授权（`AGENTS.md` Agent Execution Rules）。

走查方法：逐个读 `crates/*/src` 的真实源码 → 与 `Cargo.toml` 依赖面、`specs/component.spec.json` 声明面、`database/contract/table-registry.json` 写入者面三方对账 → 对可疑逻辑用只读探针在真实 API 上复现（`target/_probe-module-walk/`，位于 gitignored 目录）。每个模块的判定依据集中在 §1.6 的索引里，**没有依据的判定不算判定**；判定依据只能是被引用的 `path:line`、可复现的命令读数，或探针读数。

## 1. 功能模块清单

判定词表：**✅ 完整**（相对其已授权范围无缺口）／**🟡 部分**（有逻辑但存在已确认的缺口或不一致）／**❌ 未落地**（骨架或零实现）／**⛔ 刻意不做**（等待治理，`REQ-*`/ADR 未 `ready`）。

### 1.1 L3 契约与领域 Port

| # | 功能模块 | 真实规模 | 已实现逻辑 | 缺口或漏洞 | 判定 |
| --- | --- | --- | --- | --- | --- |
| M01 | 能力与隔离强度词汇表 | `capability.rs` 20 行 | `RuntimeCapability` 8 变体、`IsolationAssurance` 5 变体 | **隔离强度全序由枚举声明顺序隐式决定**，全仓仅 **1 对**断言覆盖 HostUser/Container（`provider.rs:187-188`），另 3 个变体零断言 → **F-02** | 🟡 |
| M02 | Sandbox 标识与围栏令牌 | `identity.rs` 185 行 | 5 个解析式不透明 id + 4 个可生成 id，字符白名单 + 128 字节上界；`SandboxFencingToken` 拒 0 与有符号上溢；分配引用 Debug 脱敏、非法时零化、Drop 零化 | 无 `TenantId::parse("")` 空串与 128/129 长度边界的显式断言（`validate_opaque_id` 逻辑正确，只是未被测试锁定） | ✅ |
| M03 | Provider 描述符与资格匹配 | `provider.rs:11-58` | 能力集合 `BTreeSet` + 隔离等级双条件 `satisfies_sandbox_requirements`，失败关闭 | 同 M01：该函数是全仓唯一的隔离强度消费点（`service.rs:1237`），但比较语义无全序测试 | 🟡 |
| M04 | Provider 生命周期 Port | `provider.rs:60-157` + `error.rs` 58 行 | `health`/`allocate`/`start`/`stop`/`destroy` 五个异步操作，typed error（provider id + operation + kind 三类） | `SandboxProviderOperation`/`ErrorKind` 的组合无约束（任意 operation 可配任意 kind）；`allocate`/`start` 无幂等键入参，幂等完全靠上层 operation 账本 | ✅ |

### 1.2 L2 服务

| # | 功能模块 | 真实规模 | 已实现逻辑 | 缺口或漏洞 | 判定 |
| --- | --- | --- | --- | --- | --- |
| M05 | 会话状态机与版本 CAS | `model.rs:12-388` | 8 态 13 条合法迁移；`next_sandbox_version` 前置返回期望版本；`MAX_SANDBOX_SESSION_VERSION = i64::MAX` 失败关闭 | `Running` 无直达 `Failed` 迁移（只能经 `Stopping`），配合 **F-05** 使运行态异常无法自动收敛 | ✅ |
| M06 | 生命周期编排 | `service.rs:186-1199` | 创建/启动/停止/销毁四条命令，含：租约获取→重放检查→状态校验→绑定意图先落盘→Provider 调用→补偿销毁→终态落盘 | 编排骨架完整；缺陷集中在失败的**聚合行为**上，见 M08/M10 与 F-01 | ✅ |
| M07 | 幂等与操作账本 | `model.rs:46-77,260-320` + `service.rs:1201-1227` | 账本按 operation id 去重；`InProgress`/`Succeeded`/`Failed` 三态重放；同 id 不同 kind 判 `IdempotencyConflict`（`model.rs:271-275`）；`create` 走 `find_by_operation` + `DuplicateOperation`/`VersionConflict` 双路恢复 | 账本无上界（`MAX_SANDBOX_SESSION_OPERATIONS` 只在下游仓储强制，见 **F-09**/**F-15**）；另有一处**未被锁定、也未被文档回答**的语义问题：重放 `Succeeded` 返回的是 `sandbox_session.clone()`（`service.rs:1213`）即**当前**状态，而非「该操作发生时」的状态 ⇒ 同一 operation id 在后续无关操作之后重放，会得到与首次不同的响应。既有测试 `tests.rs:1173` 只钉住「不重复产生 Provider 副作用 + 分配与启动用同一围栏令牌」这两条**重要**性质，且是**紧接着重放**；`tests.rs:2725` 只覆盖 `InProgress`。故这是**测试与文档的空档，不是已确认的实现错误**（返回当前资源状态是可辩护的设计），与 M02 同类处理 | ✅ |
| M08 | Provider 选择 | `service.rs:1229-1281` | 先按 id 稳定排序拒重复 → 按能力+隔离筛选 → 逐个查健康取首个 Ready；区分 `NoEligibleProvider` 与 `NoHealthyProvider` | 无负载/容量感知（属未授权范围）；`sandbox_provider_by_id` 在 transient 路径失败会**中止整页对账** → **F-01**；选择循环的健康探测**不续租**，探测次数一多即耗尽租约 → **F-14** | 🟡 |
| M09 | 租约与围栏（服务侧） | `service.rs:107-184,1283-1304` | **除健康探测外**的每次 Provider 调用前续租（`execute_sandbox_provider_call`，滑动窗口 `now + duration`）并校验租约身份四元不变；超时统一映射为 typed `Timeout`；`LeaseConflict` → `LeaseLost` | 租约时长与 Provider 超时的约束（1ms–300s、超时 ≤ 租约一半）在构造函数三处独立校验，一致；但 15 处 Provider 调用里 **14 处**经续租包裹、第 15 处（选择循环的健康探测 `service.rs:1248`）**未包裹**，且它是唯一可被反复执行的 → **F-14** | 🟡 |
| M10 | 对账收敛 | `service.rs:289-702` + `reconciliation.rs` 69 行 | 仅处理 `Starting/Stopping/Destroying`；逐项取租约、重读权威快照、执行补偿、回报三种结果 | **单条陈旧 Provider 引用可永久卡死整页**（F-01，已实测）；`LeaseUnavailable` 被复用于「已消失/跳过」（F-06）；`Running` 健康探测未实现（F-05） | 🟡 |
| M11 | 持久化快照与不变量 | `repository.rs:19-760` | 落盘前把操作账本**重放**成状态 + 最后失败，要求与字段一致；操作 id 唯一；首条必须是 `Create(Succeeded)`；8 态各自的绑定/分配引用一致性矩阵 | 该强不变量**只有 sqlx 适配器在写入时执行**，内存适配器完全跳过 → **F-04** | 🟡 |

### 1.3 L4 适配器

| # | 功能模块 | 真实规模 | 已实现逻辑 | 缺口或漏洞 | 判定 |
| --- | --- | --- | --- | --- | --- |
| M12 | Allocation Reference 保护 | `encryption.rs` 477 行 | AES-256-GCM，密钥经上下文盐派生（tenant+session+binding 长度前缀拼接）；密文/keyId/keyVersion/cryptoVersion 四元组；密钥身份校验；历史版本可解、退版后失败关闭；Debug 脱敏 | 无缺口。一处**边界事实**（非缺陷）：`随机 nonce、同明文不同密文` 这一安全属性由**兄弟仓**保证——`sdkwork-utils-rust/src/crypto.rs:75-93` 每次加密用 `rand::thread_rng()` 生成 12 字节 nonce 并前置存储，且该仓有 `aes_gcm_encrypt_produces_different_ciphertexts` 断言；本仓自己的 4 条用例（`encryption.rs:341`、`:394` 等）只能验证上下文绑定、密文脱敏与退版失败关闭，**无法**验证 nonce 随机性 | ✅ |
| M13 | PostgreSQL 持久化适配器 | `repository.rs` 1233 行 | 4 张表的读写；每事务 `statement_timeout=30s` + `lock_timeout=2s`；租约行 `FOR UPDATE` 锁定；`INSERT … ON CONFLICT … WHERE` 做能力受限 CAS；操作历史 `ROW_NUMBER` 窗口 + `MAX+1` 探测做有界加载；SQLSTATE 分类映射 | **全部 SQL 无任何执行期覆盖**（F-03）：集成套件是单个 `#[ignore]` 函数；**操作历史上界只在读取路径生效，且一个超界会话会击穿整页并掐断 cursor**（写路径 `capture` 不检查条数）→ **F-15** | 🟡 |
| M14 | 密钥轮换重加密 | `reencryption.rs` 313 行 | 按 `sandbox_runtime_binding_id` 游标分页（≤200）；筛出非当前版本的密文；解密后用旧四元组做 CAS 更新，冲突计入 `conflict_count` | 更新不在事务内，冲突只计数不重试（可接受，但无文档） | ✅ |
| M15 | 内存持久化适配器 | `lib.rs` 697 行 | tenant 二级 `BTreeMap` 隔离、操作索引、CAS 版本、租约竞争/接管/陈旧令牌拒绝、围栏令牌单调递增、分页校验 | 不做落盘不变量校验（F-04）；**没有任何操作条数上界**（`MAX_SANDBOX_SESSION_OPERATIONS` 零出现）⇒ 与 sqlx 在**读取方向**同样不对称（F-15）；**无任何 crate 依赖它**（仅自身测试消费） | 🟡 |
| M16 | 编解码词汇表 | `codec.rs` 349 行 | 8 态/4 操作族/3 失败类/8 能力/5 隔离等级的双向映射；重复能力拒绝；`failed` 必须带失败原因、非 `failed` 必须不带 | 无 | ✅ |

### 1.4 L4/L5/L6 未接线面

| # | 功能模块 | 真实规模 | 已实现逻辑 | 缺口或漏洞 | 判定 |
| --- | --- | --- | --- | --- | --- |
| M17 | 主机边界负向校验夹具 | `fake_host_boundary/mod.rs` 253 行 + `tests.rs` 317 行 | 可执行文件白名单、参数类型化（无 shell 解析）、逻辑相对路径（拒绝对路径/盘符/`..`/保留设备名）、环境变量白名单 + 保护名 + 敏感段拒绝、7 类上界 | 整个模块是 `#[cfg(test)]`（`lib.rs:7`）——**零生产实现**，其 5 个用例验证的是测试替身 | ❌ |
| M18 | API 装配 | `bootstrap.rs` 43 行 + `generated.rs` 6 行 | `ApiAssemblyContribution` 装配骨架与 `WebModule` 出口 | `ROUTE_CRATE_COUNT: usize = 0`（`generated.rs:3`）、`Router::new()`（`bootstrap.rs:19`）、`HttpRouteManifest::from_owned_routes(Vec::new())` → **零路由** | ❌ |
| M19 | 服务宿主组合 | `lib.rs` 5 行 | 仅 doc comment | **`Cargo.toml` 零依赖**——组合根无法组合任何东西；无 bootstrap、无 wiring、无 readiness | ❌ |
| M20 | CLI | `main.rs` 3 行 | `fn main() {}` | **零命令**，`Cargo.toml` 零依赖 | ❌ |

### 1.5 服务层契约面与装配边界（本轮补齐）

首轮清单只列了 20 个模块，覆盖 20 个源文件；本轮把余下 3 个实质源文件补为模块，并把 4 个 crate `lib.rs` 与 2 个测试文件显式登记（见下方封闭账目）。

| # | 功能模块 | 真实规模 | 已实现逻辑 | 缺口或漏洞 | 判定 |
| --- | --- | --- | --- | --- | --- |
| M21 | 生命周期命令契约 | `command.rs` 23 行 | `CreateSandboxSessionCommand`（6 字段：tenant / workspace / session / operation / 能力集合 / 最低隔离）与 `SandboxSessionLifecycleCommand`（3 字段），均 `Clone + Debug + Eq + PartialEq` | 无 | ✅ |
| M22 | 生命周期端口 | `port.rs` 36 行 | `SandboxSessionLifecyclePort` 的 5 个方法；由 `service.rs:1403` 实现、经 `lib.rs:18` 再导出 | **crate 外零消费者**——`service-host` 零依赖（M19/F-08），所以「Kernel 只消费该端口」目前只是契约意图，`sandbox-service-host-composition.contract.json` 已把它登记为 `sandbox_constructed_port` | 🟡 |
| M23 | 服务层类型化错误 | `error.rs` 61 行 | 15 个变体，每变体带结构化上下文（id / 状态 / 操作族）而非字符串，2 个 `#[from]` 透传仓储与 Provider 错误 | `LeaseUnavailable` 与 `LeaseLost` 被复用于不同语义 → **F-06**；且 `LeaseLost` 还会被用于**本地租约过期**而非「被他人夺走」 → **F-14** | 🟡 |

**清单口径核对（29 个 `.rs` 文件 / 12054 行全覆盖）**：工作区 `crates/*/src` 与 `crates/*/tests` 共 29 个 `.rs` 文件。逐文件归属如下，无遗漏：

| 归属 | 文件数 | 文件 |
| --- | --- | --- |
| M01–M20（首轮） | 20 | `capability.rs`、`identity.rs`、`provider.rs`、`provider-spi/error.rs`、`model.rs`、`service.rs`、`reconciliation.rs`、`service/repository.rs`、`encryption.rs`、`sqlx/repository.rs`、`reencryption.rs`、`repository-memory/lib.rs`、`codec.rs`、`fake_host_boundary/{mod,tests}.rs`、`assembly/{bootstrap,generated,lib}.rs`、`service-host/lib.rs`、`cli/main.rs` |
| **M21–M23（本轮补齐）** | 3 | `service/{command,port,error}.rs` |
| crate 装配边界（`lib.rs` 再导出，非独立功能模块） | 4 | `provider-spi/lib.rs`、`service/lib.rs`、`sqlx/lib.rs`、`provider-local/lib.rs` |
| 验证面（模块自带测试与证据运行器目标） | 2 | `service/tests.rs`（2909 行，全仓最大单文件）、`sqlx/tests/postgres_repository.rs`（1439 行，单条 `#[ignore]`，见 F-03） |
| **合计** | **29** | |

**清单自身的完整性缺口（本报告自我更正）**：首轮清单是「20 个模块 / 29 个文件」，但**未列 `command.rs`、`port.rs`、`error.rs` 这 3 个实质源文件（合 120 行）**，并把 4 个 `lib.rs` 与 2 个测试文件当作隐含背景。这不是仓库缺陷，是本报告自己的清单缺陷：一张 20 行的表配 29 个文件的代码面，读者无法判断「没列出来的部分是漏了还是本不需要列」——与 F-07「人读清单没有门禁读」同形，只是这次清单就是本报告。已按上表补成可算术核对的封闭账目（20 + 3 + 4 + 2 = 29）。

**其他口径**：工作区 8 个 `crates/` 目录，8 份 crate 级 `component.spec.json`（另 1 份仓库级 → `check-sandbox-component-contract-alignment.mjs` 报 9，口径一致）。`crates/` 下无 `template`/`snapshot`/`fork`/`pool`/`quota`/`node`/`event` 同名 crate，与 §1.2 表述一致。

### 1.6 逐模块判定依据索引（每个判定都能被独立复核）

§1.1–§1.5 的每个判定都必须能被第三方按下面这一列独立复核；**没有依据的判定不算判定**。

| # | 判定 | 依据（可独立复核的引用） |
| --- | --- | --- |
| M01 | 🟡 | `capability.rs:14-20`（`derive(Ord)`，全序 = 声明顺序）；唯一比较点 `provider.rs:55`；全仓仅 `provider.rs:185-192` 一个测试覆盖该比较，且只覆盖 HostUser/Container 一对 → F-02 |
| M02 | ✅ | `identity.rs` 全文（字符白名单 + 128 字节上界 + 围栏令牌拒 0/拒上溢）；本次复核未在该文件或测试里找到空串 / 128↔129 的显式边界用例 |
| M03 | 🟡 | `provider.rs:11-58`；`grep -rn 'satisfies_sandbox_requirements' crates/` → 定义 `provider.rs:50`、唯一生产消费 `service.rs:1237` → 与 M01 同一处风险 |
| M04 | ✅ | `provider.rs:60-157`（5 个异步操作）+ `provider-spi/error.rs` 58 行（id + operation + kind 三元组） |
| M05 | ✅ | `model.rs:327-351`：`matches!` 分支**恰 13 条**，且确无 `(Running, Failed)` 直达 |
| M06 | ✅ | `service.rs:186-1199`（`create` 在 `:186`、`persist` 在 `:1283`），四条命令路径逐段读 |
| M07 | ✅ | `model.rs:260-278`（kind 校验 `:271-275` ⇒ `IdempotencyConflict`）；`service.rs:1201-1227`；测试 `tests.rs:1173`（不重复 Provider 副作用 + 同一围栏令牌）、`tests.rs:2725`（`InProgress` 重放） |
| M08 | 🟡 | `service.rs:1229-1281`；F-01 探针读数 + F-14 探针读数 |
| M09 | 🟡 | `service.rs:139-183`（续租 → 四元校验 → 有界 timeout）；`grep -n 'execute_sandbox_provider_call' …/service.rs \| wc -l` = **14**，而同文件 Provider 调用 **15** 处 → F-14 |
| M10 | 🟡 | `service.rs:289-702`（`reconcile_sandbox_sessions` 在 `:289`、逐项封装在 `:435`）；降级分支 `:396-399`、注释意图 `:411-419`；F-01 探针 |
| M11 | 🟡 | `service/repository.rs:367-525`（账本重放 + 8 态引用矩阵）；`grep -n 'Snapshot::capture' sqlx/repository.rs` → 仅 `:731`、`:795`；内存侧零调用 → F-04 |
| M12 | ✅ | `encryption.rs:104-190`（派生 + 加解密 + 四元组）；随机 nonce 由兄弟仓 `sdkwork-utils-rust/src/crypto.rs:75-93` 保证 |
| M13 | 🟡 | `sqlx/repository.rs` 全文；`grep -c MAX_SANDBOX_SESSION_OPERATIONS service/repository.rs` = **0** → F-15；集成套件单条 `#[ignore]` → F-03 |
| M14 | ✅ | `reencryption.rs:72-130`（页大小守卫 `:78`、`+1` 探测 `:116`、`has_more` `:116`、截断 `:118-120`） |
| M15 | 🟡 | `repository-memory/lib.rs` 全文；`grep -c MAX_SANDBOX_SESSION_OPERATIONS` = **0**；`grep -rn '…-repository-memory' --include=Cargo.toml crates/` 无消费者 → F-04/F-15 |
| M16 | ✅ | `codec.rs` 全文双向映射（8 态 / 4 操作族 / 3 失败类 / 8 能力 / 5 隔离等级） |
| M17 | ❌ | `provider-local/src/lib.rs:7` 的 `#[cfg(test)]` 前缀；该 crate 无其它生产源文件 |
| M18 | ❌ | `assembly/generated.rs:3`（`ROUTE_CRATE_COUNT: usize = 0`、`:6` 空包列表）+ `bootstrap.rs:19`（`Router::new()`） |
| M19 | ❌ | `service-host/lib.rs` 5 行（仅 doc comment）；`Cargo.toml` 零依赖 |
| M20 | ❌ | `cli/main.rs` 3 行（`fn main() {}`）；`Cargo.toml` 零依赖 |
| M21 | ✅ | `service/command.rs` 全文（23 行，2 个命令结构） |
| M22 | 🟡 | `port.rs:10-36` + 实现 `service.rs:1403` + 再导出 `lib.rs:18`；`grep -rn 'SandboxSessionLifecyclePort' --include=Cargo.toml crates/` → **无命中**（crate 外零消费者） |
| M23 | 🟡 | `service/error.rs:14-61`（15 个变体，2 个 `#[from]`）→ F-06/F-14 的语义复用落点 |

## 2. 走查发现

### F-01 对账页可被单条陈旧 Provider 引用永久卡死（严重，已实测复现）

**位置**：`service.rs:452-453`（`sandbox_provider_by_id(...)?`）→ `service.rs:426`（`Err(sandbox_lifecycle_error) => return Err(...)`）。

**机理**：对账逐项收敛，但 `reconcile_sandbox_session_with_lease` 解析 Provider 时的失败会经 `?` 冒泡成**整页错误**。调用方只把 `Provider` 与 `ProviderReadinessRejected` 降级为「本项失败」（`service.rs:396-399`），`InvariantViolation` 落到通用分支直接返回。由于 `list_sandbox_sessions_requiring_reconciliation` 始终按 id 升序返回 `starting|stopping|destroying`（`repository.rs:1036`、memory `lib.rs:80-85`），同一条坏会话每次都在页首 → **该租户的对账永久失败，所有 transient 会话永不收敛**。

**实测**（探针 `target/_probe-module-walk/`，同一页两条会话，唯一变量是 Provider 是否注册）：

```text
--- CONTROL  provider 'p-registered' IS registered
    outcome      = Ok
    items        = 2
    next cursor  = None
    detail       = session-a=Running/Reconciled, session-b=Running/Reconciled
    session-a persisted state = Running
    session-b persisted state = Running

--- SUBJECT  provider 'p-unregistered' is NOT registered
    outcome      = Err
    error        = sandbox session state is internally inconsistent: sandbox runtime binding references an unregistered sandbox provider
    session-a persisted state = Starting
    session-b persisted state = Starting
```

对照组两条会话均收敛到 `Running`；实验组整页 `Err`，且 `session-b` **一次都没被尝试**——证明是整页中止而非单项失败。

**可达性**：会话由 A 部署的 Provider 集合创建后，若该 Provider 从配置中移除（`standalone` 与 `cloud` 两个 profile 各有独立 Provider 集合，见 composition contract），遗留会话即触发。

**最小修法**：把 transient 路径的 Provider 解析失败降级为「本项 `Failed`」——为对账结果增加一个能表达「本项无法处理」的取值，或复用 `Failed` 并同时把会话推进到 `Failed`/`Cleanup`，而不是让 `?` 冒泡。

### F-02 隔离强度全序没有测试锁定（严重，潜在）

**位置**：`capability.rs:14-20`（`#[derive(... Ord ...)] pub enum IsolationAssurance`）→ 全序 = 变体声明顺序；唯一比较点是 `provider.rs:55` 的 `self.sandbox_isolation_assurance >= sandbox_minimum_assurance`；唯一生产消费点是 `service.rs:1237`。

**证据（本项计数已更正）**：全仓只有**一个**测试碰过 `satisfies_sandbox_requirements`——`provider.rs:171` 的 `sandbox_provider_descriptor_fails_closed_on_capability_and_assurance`，内含 3 条断言：`:185-186`（要求 HostUser ⇒ 通过）、`:187-188`（要求 Container ⇒ 拒绝）、`:192`（能力子集不满足 ⇒ 拒绝）。**只有前两条与隔离强度比较有关**，因此该比较在全仓被覆盖的是一个**单对**（HostUser vs Container，各一个方向）；**`UserSpaceKernel`、`MicroVm`、`DedicatedVm` 在任何测试的断言里零出现**（`codec.rs` 的往返测试覆盖了它们的编解码，不覆盖次序）。本报告初稿把此处写成「2 处断言」，实际是 **3 条断言、其中 2 条**属该比较——结论不变，计数更正。

**影响**：这个枚举的全部意义就是安全强度序。任何人重排变体（新增一个中间层级、把 `MicroVm` 移到 `DedicatedVm` 之后）都会静默反转隔离保证，而 18 条门禁 + 612 个契约用例全绿。

**最小修法**：加一条 5×5 全序断言（`DedicatedVm ≥ MicroVm ≥ UserSpaceKernel ≥ Container ≥ HostUser`），成本约 15 行。

### F-03 SQL 层的证据存在，但不在门禁链上（严重，验证面空档）

**位置**：`crates/…-repository-sqlx/tests/postgres_repository.rs:437` —— 整个 1439 行集成套件是**单个** `#[ignore]` 函数。

**实测读数**：`cargo test --workspace` → `67 passed / 0 failed / 1 ignored`；逐 crate 分布：

| crate | 用例 |
| --- | --- |
| `sdkwork-api-sandbox-assembly` | 0 |
| `…-repository-memory` | 5 |
| `…-repository-sqlx`（lib） | 13 |
| `…-repository-sqlx`（集成） | 2（1 通过 + 1 忽略） |
| `…-service` | 38 |
| `sdkwork-sandbox-cli` | 0 |
| `sdkwork-sandbox-provider-local` | 5（全部验证 `#[cfg(test)]` 代码） |
| `sdkwork-sandbox-provider-spi` | 5 |
| `sdkwork-sandbox-service-host` | 0 |

**影响**：`repository.rs` 里的全部 SQL——`ON CONFLICT … DO UPDATE … WHERE`、`RETURNING … EXTRACT(EPOCH …)`、`ROW_NUMBER() OVER (PARTITION BY …)` 窗口、`SELECT … FOR UPDATE`、`make_interval(secs => …)`、`SET LOCAL statement_timeout`——**不在任何门禁或 CI 检查链的射程内**。`package.json` 的 `_sdkwork:check` 链 18 条命令不含它；根 `README.md` 的 Verification 块 18 条门禁也不含它。而 composition contract（`sandbox-service-host-composition.contract.json:393`）把该 crate 定为 `serverAuthority`。

**但要准确地说：证据路径是存在的，不是零覆盖。** 仓库内有一个 419 行的证据运行器 `tools/testing/sandbox-postgres-evidence.mjs`，它在一次性容器（前缀 `sdkwork-sandbox-pg`）里起 PostgreSQL 16/17 并跑真实仓储测试；根 `README.md:92-93` 以单独段落列出它的两条命令；REQ-2026-0005 的 `Current Evidence` 记录该运行器在 2026-07-30 通过（Migration `1/0`、Status/Drift、Backup/Restore、`11/20/9/11` 计数一致、Allocation 明文零匹配）。

**因此真正的缺口是「证据一次性、门禁链不带」**：SQL 正确性依赖一条**历史证据记录**，而不是一条**可连续复现的检查**。三个具体表现：① 它不在 `_sdkwork:check` 里；② 它需要 Docker，而本机（Windows 宿主）`docker` 不存在，所以在本环境下**无法复现**；③ 那份 `#[ignore]` 的 Rust 集成套件读起来像"有个测试只是被忽略了"，实际含义是"这条路径的默认验证强度为零"。

**最小修法**：不要新建证据路径——已有的那条足够。把 `tools/testing/sandbox-postgres-evidence.mjs` 纳入一条具名的验证轮（它现在只在 README 里出现，不在任何检查链），并把每轮读数落盘为可复核证据；同时把该文件里那条 `#[ignore]` 集成用例的**存在意义写进注释**（"默认不跑，需经证据运行器显式触发"），否则下一位读者会把它读成"被忽略的普通测试"。

### F-04 内存与 sqlx 适配器的写入校验不对称（中，潜在）

**位置**：sqlx 在 `insert_sandbox_session`（`repository.rs:731`）与 `save_sandbox_session`（`repository.rs:795`）都调用 `SandboxSessionRepositorySnapshot::capture`，`capture` 内部执行 `validate_sandbox_persisted_invariants`（`repository.rs`(service) `:368-525,607`）；内存适配器全文**没有** `capture` 或该校验的任何调用。

**为何是潜在而非现行**：`SandboxSession` 只能由 `create`（`model.rs:154`）或 `restore`（`repository.rs:611`，其自身也校验）产生，服务 API 无法构造违例会话。因此今天不可达。

**为何仍要修**：内存适配器是 dev/test 权威。两个 L4 适配器对同一份写入给出不同判据，意味着**任何新增写入路径都可能先在内存侧通过、在 sqlx 侧失败**——这是差分对拍（differential oracle）的基本要求。

**最小修法**：让内存 `insert`/`save` 也走 `capture`（或至少调用校验），两侧判据合一。

### F-05 `Running` 会话没有健康对账（中，已文档化的范围缺口）

**位置**：筛选条件是 `sandbox_session_state IN ('starting','stopping','destroying')`（`repository.rs:1036`）与同义的内存过滤（`lib.rs:80-85`）；`reconcile_sandbox_session_with_lease` 对非 transient 状态直接报 `InvariantViolation("reconciler received a non-transient sandbox session")`（`service.rs:698-700`）。

**影响**：Provider 分配被外部回收/宿主机重启后，会话会**永远停在 `Running`**，直到有人显式 `stop`。对账只承诺 transient 状态，这是自洽的；风险在于 §1.2/§3.2 把它读成「异常已覆盖」。

**最小修法**：在文档里显式登记「运行态健康探测未实现」，或为运行态增加独立的健康探测轮（需 `REQ-*`）。

### F-06 `LeaseUnavailable` 被复用于「已消失/跳过」（中，语义）

**位置**：`service.rs:339-344`（取不到租约）、`service.rs:373-377`（列表后消失）、`service.rs:415-419`（失败收敛中消失）三处都报 `SandboxSessionReconciliationOutcome::LeaseUnavailable`（`reconciliation.rs:9` 只有三个取值，没有「跳过」）。

**影响**：「租约被他人持有」与「数据已不存在」是两种完全不同的运维工单，聚合计数里却混在一起。

**最小修法**：新增一个 `Skipped`/`Vanished` 取值。

### F-07 模块清单漏登 1 个 crate，且门禁不校验该清单（中，文档漂移）

**位置**：`TECH-modules-and-contracts.md` §2「当前已物化组件」表 **7 行**、§5 `Repository Layout` 代码块的 `crates/` 子树 **7 项**，均缺 `sdkwork-api-sandbox-assembly`；实际 `crates/` 有 **8** 个目录。

**证据**：`ls -d crates/*/ | wc -l` → 8；8 份 crate 级 `component.spec.json`；该 crate 已在 `TECH-e2b-capability-parity.md:72` 被点名（说明是漏登，不是不知道）。

**为何门禁放行**：`check-sandbox-component-contract-alignment.mjs` 只校验每份 `component.spec.json` 的 `root` 等于 `crates/<name>`（`exit=0`，报 9），**不校验 §5 那份人读清单**。这正是「门禁清单本身没有门禁」的形态——与上一轮 `PRD.md:306` 那条假断言同类。

**最小修法**：补 §2 与 §5 两处；更彻底的做法是让 component-contract 门禁反向核验「§5 列出的 crate 集合 == 实际目录集合」的双向记账。

### F-08 §4 计划依赖图 6 条沙箱内边 0 条落地（中，文档漂移）

**位置**：`TECH-modules-and-contracts.md` §4 声明 `HOST --> SERVICE`、`HOST --> PROVIDERS`、`HOST --> STORES`、`CLI --> HOST`、`ASSEMBLY --> ROUTE`。

**事实**：`crates/sdkwork-sandbox-service-host/Cargo.toml` **零依赖**、`crates/sdkwork-sandbox-cli/Cargo.toml` **零依赖**、`sdkwork-api-sandbox-assembly` 只依赖 `sdkwork-web-bootstrap`/`sdkwork-web-core`（无 route crate 存在）。

章节标题已写「计划」，因此不是错误陈述；但表中**没有任何一列标出落地状态**，读者无法区分计划与现状。

**最小修法**：为该图加一列「当前落地状态」，或分段（已落地 / 计划）。

### F-09 操作历史上界的失败模式是「整会话不可读」（中，已文档化的高后果选择）

**位置**：`repository.rs:35`（`MAX_SANDBOX_SESSION_OPERATIONS = 10_000`）、`:267`（窗口限 `MAX+1`）、`:366-368`（超界即 `InvalidStoredData`）。

**影响**：超过 1 万条操作历史的会话，`get_sandbox_session` **每次都失败**。即：把「保留策略缺口」升级为「数据永久不可读」，并被服务层一路冒泡为 `Repository(InvalidStoredData)`。一个每 2 条操作 = 一次启停周期，约 5000 次启停即触界。

**⚠️ 更正（本文档同一轮内自查）**：本项初稿断言这条后果「值得显式登记，而不是只写在 doc comment 里」。**该断言错误。** REQ-2026-0005 的 `## Release And Review Boundary` 段已经显式登记了它，且措辞比本报告更强——"完整 Operation 历史 hydrate/replay 是本需求已验收候选的当前行为，不是商业化保留策略 …… 该 Requirement 获批并完成真实 PostgreSQL 迁移证据前，**不得删除、截断或过期当前幂等记录**"。所以这**不是**一处未登记的高后果选择，而是**已被治理文档明确冻结的行为**，并且它正确地把修复权锁在 REQ-2026-0020 后面。

**保留此项的理由只剩一条**：登记发生在**需求文档**里，代码侧只有 `repository.rs:35` 的 doc comment 点名 `REQ-2026-0020`，没有指向上面那段"不得删除/截断/过期"的禁令。这是**同一事实在两个位置的强度不对等**，不是缺失登记。修法随之降级为纯注释工作：在常量注释里补一句指向该边界段。**本项不构成实现缺陷，也不构成需要新签署的理由。**

### F-10 §3.1 叙述段的历史数字与当前表差 1（低，数字口径）

**位置**：`TECH-e2b-capability-parity.md:282` 称「只覆盖了 **3 个 crate 的 49 个用例**」且遗漏的是「…-repository-memory **5** 个、…-repository-sqlx **14** 个，共 **19** 个」。

**核对**：`49 + 19 = 68` ✅ 与 `5 + 14 = 19` ✅ 自洽；但同文档 §3.1 表给该 crate 的是 **15** 个用例（codec 5 + encryption 4 + reencryption 1 + repository 3 + 集成 2），得 `5 + 15 = 20`。且非仓储 crate 只有 `service 38 + spi 5 + local 5 = 48`，凑不出 49。

**结论**：只有「当时那条 `#[ignore]` 的 sqlx 集成用例算在已覆盖侧的 49 里」才能同时成立，即该句的「3 个 crate 的 49 个用例」实际是「3 个 crate 48 个 + 另一 crate 1 个」。属叙述精度问题，非事实错误；但这句话没有门禁保护（§3.1 表本身有），建议收紧为可复核的写法。

### F-11 文档索引契约测试的解析缺陷：末位条目不可见（严重，已实测复现并修复）

**位置**：`tests/contract/documentation-status-alignment.contract.test.mjs:13-41`（`parseIndexEntries`）。

**机理**：该解析器按固定缩进抽 `  - id:` / `    path:` / `    status:`，但在 `entries:` 段结束后**不重置游标**。而 `docs/INDEX.yaml` 在 `entries:` 之后还有 `domains:` 段，其子项同样带四空格 `path:` 键（`    path: docs/product/prd/`）。于是 `entries:` 里**最后一条**目的 `path` 会被 `domains:` 的第一条 `path` 覆盖。

**后果**：`every REQ ADR PLAN and REVIEW working document is indexed`（同文件 `:79`）从结构上**不可能**看到注册在末位的 working document——这个断言对「排在最后一个」的文档恒为假，而它正是"每个工作文档都必须登记"这条规则的唯一执行者。

**实测**（`node -e` 直接复现该解析函数，唯一变量是否重置游标）：

```text
fix=false entries=115 targetIndexed=false entry=null
fix=true  entries=115 targetIndexed=true  entry={"id":"REVIEW-20260922-FUNCTIONAL-MODULE-WALK","path":"docs/engineering/reviews/REVIEW-20260922-sandbox-functional-module-walk.md","status":"active"}
```

两侧条目总数相同（115），说明修复没有增删任何条目，只让末位条目拿回它自己的 `path`——即这个修复是**承重**的，不是顺手改绿的。

**已修复**：在解析循环开头对顶层键（无前导空白）重置游标，并写入注释说明原因。该修复只改解析逻辑、不增删用例，故契约套件由 `607 pass / 1 fail` 变为 **608 pass / 0 fail**（总数仍是 608，`testInventory` 无需为此变动）；本轮 F-12 再补 4 条用例后，末态为 **612 pass / 0 fail**（见 §5）。

**为何值得单列**：这是一个「门禁自己写错了」的实例，且它与 F-07 同形——F-07 是「有一份人读清单没有门禁读」，F-11 是「有一条门禁读不懂它负责的清单」。两者都不会被 18 条静态门禁或 612 个契约用例中的任何一个发现。

### F-12 评测入口文档漏登 5 个包、低估 4 项风险、计数过期两月（严重，已修复并由门禁守住）

**位置**：`docs/engineering/gate-zero-exit-readiness-package.md` —— 人工评审者据以决定评审顺序的入口文档。它此前 `Updated: 2026-07-30`，`## Gate 0 退出条件` 第 3 条写「当前全部 **17** 个相关 Review Packet 状态均为 `pending-human-review`」。

**实测差集**（`node tools/check-sandbox-human-review-signoff.mjs --json` 的 `backlog` vs 该页表格）：

| 项 | 该页 | 实测 | 差异 |
| --- | --- | --- | --- |
| pending 计数 | 17 | **22** | 少 5 |
| 表格行 | 17 | 22 | 漏登 5 个包 |
| 风险值 | — | — | 4 行低估 |

**漏登的 5 个包**（既不在该页表中，也不被任何 `specs/` 契约具名，只以文件形式存在）：`REVIEW-20260731-sandbox-interactive-terminal-session`（REQ-2026-0024）、`REVIEW-20260731-sandbox-internal-control-plane`（REQ-2026-0023）、`REVIEW-20260801-sandbox-runtime-secret-projection`（REQ-2026-0025）、`REVIEW-20260801-sandbox-cloud-data-residency-and-recovery`（REQ-2026-0026）、`REVIEW-20260801-sandbox-cross-repository-version-compatibility`（REQ-2026-0027）。**这 5 个包全部 `Risk: critical`**，且全部处于 `pending-human-review`。

**低估风险的 4 行**：`REVIEW-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain`、`…-workspace-block-device-attachment-and-sanitization`、`…-firecracker-resource-isolation`、`…-postgresql-quota-and-capacity-persistence` —— 该页写 `high`，包内 `Risk:` 头写 `critical`。另有 `…-observability-event-audit-outbox` 该页写 `high`，而该包**没有 `Risk:` 头**（22 个 pending 包中唯一缺失者）。

**为何两个月的漂移无人发现**：`docs/engineering/human-review-signoff-backlog.md` 有门禁双向对账（契约具名集合 vs 页面具名集合），但**退出包这篇没有**。实测确认：`gate0:readiness:check` 对应的 `tools/check-sandbox-commercial-readiness.mjs` 只读 `specs/sandbox-commercial-readiness.contract.json`，全文不含 exit-readiness 路径；18 条静态门禁与 612 个契约用例也都不读它。

**后果**：一个只读退出包的评审者会**漏掉 5 个 critical 包**、并把 4 个 critical 当 high 排序——即漏掉的恰好是本页表尾之后新增的那批。这是 F-07/F-11 同形的第三次出现，而这次落在**评审流程真正照做的那份文档**上。

**已修复（两处）**：
1. 该页按包内声明补齐：计数 17 → 22；补入第 18–22 行；4 行风险 `high` → `critical`；observability 行改写为 `未声明`（因为它确实没声明，不由本页代为判定）；并加注说明该表是包内 `Status:`/`Risk:` 头的投影、不再手工维护。
2. 一致性改由门禁守住，而不是靠人记得。`tools/check-sandbox-human-review-signoff.mjs` 新增第 **9** 类判据（`parseExitPackage` + `exitPackageDocument` 输入）：漏登任一 pending 包 → 红；某行状态与包内 `Status:` 不一致 → 红；某行风险与包内 `Risk:` 不一致（无声明时须写 `未声明`）→ 红；正文计数与实测 pending 数不等 → 红。

**变异自证**：把新判据的入口条件置空后重跑契约套件，**恰好 3 条规则用例转红**（漏登 / 风险不一致 / 计数过期），异族 0 条，套件总数 22 不变，还原后 `sha256` 逐字节一致（`8072d947c5dd1c2b…`）。即该规则不是恒绿装饰。

**为何值得单列**：F-07 是「有一份人读清单没有门禁读」，F-11 是「有一条门禁读不懂它负责的清单」，F-12 是「**评审流程的入口文档**没有人读」。三者同形、逐级更靠上游，且都不被任何既有门禁发现。

### F-13 门禁自己文档里的套件读数不受自己的规则约束（严重，已实测复现并修复 + 门禁化）

**位置**：`tools/README.md:159` —— E2B Field Parity Gate 章节的 `Current reading:` 段，原文写「the audit document's quoted suite readings (**592** contract tests recomputed from `tests/contract/*.test.mjs`, 67 Rust tests measured by `cargo test --workspace`) are checked against the files and the recording」。

**机理**：`check-sandbox-e2b-field-parity.mjs` 第 9 条规则族（`test-inventory`）的职责正是「文档声明的套件读数必须为真」，但它读取的外部散文只有一处——审计文档 `TECH-e2b-capability-parity.md` 里的 `` `N pass / N fail` ``（基线 `testInventory` 是它的比对记录，不是散文）。门禁**自身章节**里那句 restatement 描述的恰恰就是这条检查，却不在被检查之列。因此该句在套件从 592 长到 612 的全过程中始终写着 592，而 **18 条门禁 + 612 个契约用例全绿**。

**证据链**（`git show HEAD` 的读数即起点，逐步可复核）：

| 阶段 | 矩阵契约用例 | 签核契约用例 | 套件总数 | 声明同步处 |
| --- | --- | --- | --- | --- |
| `HEAD` | 127 | 18 | **592** | 三处均为 592 |
| 矩阵契约扩写 | **143** | 18 | 608 | 三处同步到 608 |
| F-12 补 4 条 | 143 | **22** | 612 | 三处同步到 612 |
| `tools/README.md` 该句 | — | — | — | **全程停在 592** |

差额 20 条没有任何规则发现，因为读它的规则不存在。

**已修复（两处）**：
1. `tools/README.md` 该句 592 → 612，并在第 9 条规则描述里写明该 restatement 现已受检。
2. 门禁新增 `readGateReadmeSection(root, GATE_FILE_NAME)`：把扫描范围限定到 `tools/README.md` 中**点名本门禁的那个 `## ` 章节**（实测全文件只有 `## E2B Field Parity Gate` 一节点名它，所以相邻门禁的章节不可能提供数字），在该章节内校验 `N contract tests recomputed` == `testInventory.tests`、`N Rust tests measured` == `testInventory.rustWorkspace.passed`。**章节里没有这句 restatement 也算失败**，所以删掉该句并不能让规则静默。

**变异自证**（在真实树上做，跑完还原）：

```text
# 把该句的 612 改回 592
sandbox E2B field parity: 1 finding(s)
  [test-inventory] tools/README.md restates 592 contract test(s) recomputed from tests/contract, the suite declares 612
exit=1

# 把该句的 67 改成 64
sandbox E2B field parity: 1 finding(s)
  [test-inventory] tools/README.md restates 64 passing Rust test(s), the baseline records 67
exit=1
```

两次各只产出一条 finding、不级联；还原后 `tools/README.md` 与改前**逐字节一致**。契约侧新增 4 个变异条目（过期契约数 / 过期 Rust 数 / 无 restatement / 无本门禁章节）与 2 条定向消息断言，但**套件总数不变（612 → 612）**——它们是既有 `test-inventory` 规则族内的新条目，不是新的 `test()`。这正是本项不引发 §5 那套四处数字同步的原因。

**为何值得单列**：这是 F-07/F-11/F-12 链条的**第四环，也是最内环**——前三者分别是「有一份人读清单没有门禁读」「有一条门禁读不懂它负责的清单」「评审入口文档没有人读」，而这一条是**门禁自己写下的、关于自己的那句声明，不受自己的规则约束**。它同时证明本报告 §5 那句「声明点必须是单点的」在写下时**就是错的**：真实同步点是三处**加这句 restatement**，共四处。

### F-14 租约不覆盖 Provider 选择循环：策略允许的最大超时下三次探测即耗尽租约（严重，已实测复现）

**位置**：`service.rs:1245-1256`（`select_sandbox_provider` 的探测循环，健康探测在 `:1248`）→ 唯一的续租点 `execute_sandbox_provider_call`（`service.rs:149-156`）。

**机理**：`SandboxLifecycleService` 的租约模型是「滑动窗口 + 每次 Provider 调用前续租」——`renew` 把到期时间设为 `now + duration`（内存适配器 `lib.rs:319`），`execute_sandbox_provider_call` 在每次 Provider 调用前先续租、再校验租约身份四元不变、再用有界 timeout 执行（`service.rs:149-183`）。**因此租约的语义是「干活期间把锁续活」，不是「整条命令的截止时间」**——若后者成立，每次调用前续租就会当场自毁。

而 `select_sandbox_provider` 正是这个模型里的唯一例外：它对每个**资格匹配但未 Ready** 的 Provider 发一次健康探测，用 `tokio::time::timeout(self.sandbox_provider_operation_timeout, …)`（`:1246-1249`），**期间不续租**；第一次续租要等到选择成功、进入 `execute_sandbox_provider_call`（`:789`、`:849` 起）。

构造函数只约束了**单次调用的时长上界**（`service.rs:61-71`：`timeout × 2 ≤ lease`），**没有约束一条命令能发起多少次调用**。于是选择阶段的耗时上界是 `探测次数 × timeout`，只要它越过租约就整体失败。

**实测**（探针 `target/_probe-module-walk/src/bin/lease.rs`；只读，位于 gitignored 的 `target/`）。三个场景的**每个慢 Provider 探测延迟都是 600 ms**（必然烧满其 timeout），唯一变量是「排在健康 Provider 之前的慢 Provider 数量」与操作策略：

```text
--- CONTROL  lease 1000 ms, timeout 200 ms, 2 slow providers
    slow providers probed  = 2
    selection cost         = 400 ms
    elapsed                = 434 ms
    outcome                = Err(Provider(error) -- the provider WAS reached)

--- SUBJECT  lease 1000 ms, timeout 200 ms, 7 slow providers
    selection cost         = 1400 ms
    elapsed                = 1472 ms
    outcome                = Err(LeaseLost)

--- BOUNDARY lease 1000 ms, timeout 500 ms (= lease/2, the largest the policy allows), 3 slow
    selection cost         = 1500 ms
    elapsed                = 1535 ms
    outcome                = Err(LeaseLost)
```

CONTROL 走到了健康 Provider 并调用了它（`allocate` 返回可区分的 `Unavailable`，因此「到达」是可观测的）；SUBJECT 与 BOUNDARY 都在**选择阶段**就因租约过期而中止，集群里那个健康 Provider **一次都没被调用**。

**阈值是闭式可算的**：租约 `L`、Provider 超时 `T`（构造时已保证 `T ≤ L/2`），失败阈值是 `⌈L/T⌉` 次探测。取**策略允许的最大** `T = L/2` 时阈值降到 2 次——BOUNDARY 用 3 个慢 Provider 即确定性地越过（2 次探测恰好等于 `L`，落在 `expires_at <= now` 的边界上）。也就是说：**不需要「很多」坏 Provider；一个正常的 2:1 策略加 3 个资格匹配但暂时未 Ready 的 Provider，就足以让 `start` 在健康集群上报 `LeaseLost`。**

**可达性**（逐条核对调用序，不是推测）：
- Provider 选择**只发生在 `start`**。`create_sandbox_session` 全路径不调用 `select_sandbox_provider`（实测 `service.rs:186-340` 内无该调用），它只记录绑定意图。
- `start_sandbox_session` 在 `:728` 先取租约，再进 `start_sandbox_session_with_lease`；`:765-774` 的 match 里只有**不带 allocation 的既有绑定**才走 `sandbox_provider_by_id`，其余（全新 `Created` 会话，以及 `Stopped`/`Failed` 之后重启）都走 `select_sandbox_provider`。
- 因此**每次 `create → start`、每次 `stop → start` 都会重新选择**，这不是罕见分支。
- 触发条件仅为「资格匹配的 Provider 里有若干未 Ready」，即滚动发布、部分实例降级、Provider 健康端点变慢这类日常状态。

**为什么既有测试没有发现**（实测证据）：
- `service/tests.rs` 的 3 条 `LeaseLost` 断言（`:2230`、`:2268`、`:2313`）**全部是注入仓储故障**（release 失败 / renew 失败 / save 冲突），没有一条是「本地租约过期」。
- 这些测试的 Provider 集合都是 `vec![FakeSandboxProvider::ready(...)]`（`tests.rs:914`、`:926`），**选择永远在第一次探测即命中**，即选择成本恒为零。
- 全文件只有一处时间延迟（`tests.rs:803`，服务于 `start` 调用本身），且**没有 `tokio::time::pause()`**，测试时间轴无法被确定性地推进——这正解释了为什么该缺口只能被带真实延迟的探针照出来。

**二阶后果：错误名指向错误的原因。** `LeaseLost` 的文案是「sandbox session lifecycle lease was lost before the operation completed」（`error.rs:55-56`），而本场景里租约**从未被任何竞争者夺走**——是本地把它耗到了过期。`renew` 返回 `None` 的两类原因（所有者/令牌不匹配 vs 已过期）在适配器里被折叠成同一个分支（内存适配器 `lib.rs:309-318`）。这与 **F-06** 同族：一个把「数据已消失」写成 `LeaseUnavailable`，一个把「自己让它过期」写成 `LeaseLost`。

**最小修法**（两档，都不触碰公开签名）：
1. **让选择循环也走续租**——把健康探测也经 `execute_sandbox_provider_call`（或等价地，先续租再 `timeout`）。因为 `renew` 是滑动窗口，续租后每次探测都从当前时刻续满，`⌈L/T⌉` 这个阈值就消失了。这也与其余 14 处保持一致，是最小改动。
2. **或为选择阶段设独立预算**，并把它写成显式常量（而不是让它隐式等于 `探测次数 × timeout`），超预算时返回一个能自解释的错误而非 `LeaseLost`。

若采纳第 2 档，建议同时把「本地过期」与「被他人夺走」拆成两个错误取值——否则失败模式修好了，错误名仍然是错的。

**为何值得单列**：这是本轮唯一一个**在策略完全合规的前提下、由日常集群状态触发、且错误名指向错误原因**的功能性缺陷。它不是 F-13 那类验证面缺陷，而是实打实的实现逻辑洞：M09 原先判定为 ✅ 的判据「每次 Provider 调用前续租」**是错的**——15 处 Provider 调用里恰有 1 处没续租，而它偏偏是唯一能被反复执行的那一处。本报告据此把 M09 由 ✅ 降为 🟡（§1.2），这是本轮第二次自我更正。

### F-15 单会话操作历史超界 → 整页对账永久失败（严重；读路径已逐行核实，写路径无上界）

**位置**（三条链，全部 path:line 可点）：
- 上界**只存在于读取路径**：`repository-sqlx/src/repository.rs:35`（常量）、`:267`（窗口取 `MAX + 1` 行）、`:366-368`（`row_number > MAX` ⇒ `Err(InvalidStoredData)`）。
- 该 `Err` 会击穿**整批**：`:1032-1039` 的页查询只取 id，`:1064-1069` 把**整页 id** 交给同一个 `load_sandbox_session_snapshots`；其中任一会话超界即在 `:366-368` 直接 `return Err`，经 `:1069` 的 `?` 冒泡出该方法。
- 服务层再冒泡一次：`service.rs:298-305` 的 `list_sandbox_sessions_requiring_reconciliation(…).await?` ⇒ 整个 `reconcile_sandbox_sessions` 调用失败。

**机理**：操作行是按 `PARTITION BY sandbox_session_id` 编 `row_number` 的**跨会话同批查询**，所以「某一个会话超界」没有局部出口——它只能表现为整批失败。

**为什么今天可达（写路径不拦）**：上界**只在读取时检查**。写路径 `insert_sandbox_session`（`:727-731`）与 `save_sandbox_session`（`:783-795`）都只做 `SandboxSessionRepositorySnapshot::capture(…)`；而 `capture` → `validate_sandbox_persisted_invariants`（`service/repository.rs:367-525`）校验的是「账本重放后状态一致、首条必须是 `Create(Succeeded)`、8 态的绑定/分配引用矩阵」——**全文不含任何操作条数上界**（`MAX_SANDBOX_SESSION_OPERATIONS` 在整个 service crate 里零出现）。所以第 **10001** 条操作能被正常写入。

**为什么失败是永久的**：该页按 `sandbox_session_id` 升序返回（`:1038`），而 `next_sandbox_session_id` 只在**成功加载之后**由 `service.rs:306-325` 计算。于是：① 拿不到 `next_cursor`；② 下次调用仍从同一位置取到同一页；③ 坏会话 id 不变 ⇒ **该租户的对账永久失败，所有 transient 会话永不收敛**。与 F-01 完全同形，只是触发源不是陈旧 Provider 引用、而是超长操作历史。

**一个刺眼的对照（同一文件已写下相反的意图）**：`service.rs:411-419` 的降级分支里专门有注释——"The sandbox session disappeared while the failed provider reconciliation was in flight; **skip the item instead of aborting the whole page**"。也就是说「不要因单项失败而中止整页」**是这份代码自己写明的意图**；但 `:396-399` 只把 `Provider(_)` 与 `ProviderReadinessRejected { .. }` 降级，其余（含 `Repository(InvalidStoredData)`）在 `:426` 一律 `return Err` —— 而该错误**在更早一层的页加载就发生**，连 `:396` 的降级分支都进不去。

**证据强度声明（避免过度声称）**：本项证据是**代码路径逐行核实**（上面每条都有 path:line），**不是实测复现**——复现需要真实 PostgreSQL，而本机没有 Docker（这正是 F-03 记录的约束）。因此本项**没有 CONTROL/SUBJECT 读数**，与 F-14 不同；但它**也不需要运行**即可判定：`return Err` 与 `?` 的传播链是静态确定的。

**与 F-04 / F-09 的关系（本项是对 F-09 的升级，不是重复）**：
- F-09 已登记「超过 1 万条操作历史的会话 `get_sandbox_session` 每次都失败」，并说明该行为被 REQ-2026-0005 的边界段冻结——但它描述的是**单会话不可读**。
- 本项补上三件 F-09 没写的事：① **写路径不设上界**，所以「怎么越过来的」有答案；② **爆炸半径是整页**——连带最多 `sandbox_page_size`（≤200）个正常会话一起不可读，并**掐掉 cursor** 使失败永久化；③ **内存适配器完全没有这个上界**（`MAX_SANDBOX_SESSION_OPERATIONS` 在 `repository-memory/src/lib.rs` 零出现）⇒ F-04 那类「dev 侧通过、生产侧失败」的不对称在**读取方向**上有第二个实例。
- 一句话：F-09 说「会话读不出来」，F-15 说「**一个会话读不出来，会让整个租户的对账也读不出来**」。

**最小修法（分两半，只有一半需要新签署——这个区分很关键）**：
1. **收缩爆炸半径：不需要新 `REQ-*`。** 把页加载改成「逐会话加载、单项失败降级为本项 `Failed`（或新增 `OverBound` / `Vanished` 取值）」——这正是 `service.rs:411-419` 已写明的意图。落点在 `repository-sqlx`（REQ-2026-0005 覆盖的 crate，`accepted`）与 `service.rs`（REQ-2026-0002 `accepted`）；**它不删除、不截断、不过期任何幂等记录**，因此不触碰 REQ-2026-0005 边界段的那条禁令。
2. **改保留策略本身（放宽/抬高/取消上界）：需要 REQ-2026-0020。** 那才是 F-09 背后被冻结的行为。

把这个区分写清楚很重要，否则本项会被读成「又一个必须等 REQ-2026-0020 的问题」，而实际上**最坏的那一半（整页停机）现在就修得了**。

**为何值得单列**：它是 F-01 的**同形重现但更靠下**——F-01 在 `service.rs` 把单项失败升格为整页失败，F-15 在 `repository-sqlx` 做同一件事，而**两层都没有任何门禁会读**（F-03：SQL 证据不在门禁链上）。它也说明「失败关闭」这一正确原则，一旦落在**批量读取**路径上，就会把「一条数据的保留策略缺口」放大成「一个租户的运维面停机」。

## 3. 判定

按模块汇总（判定词表见 §1）：

| 判定 | 模块 |
| --- | --- |
| ✅ 完整 | M02 M04 M05 M06 M07 M12 M14 M16 M21 |
| 🟡 部分 | M01 M03 M08 M09 M10 M11 M13 M15 M22 M23 |
| ❌ 未落地 | M17 M18 M19 M20 |
| 合计 | 9 + 10 + 4 = **23** |

**结论**：已落地的 19 个模块，其逻辑主干是自洽且防御性良好的（版本 CAS、租约+围栏、失败关闭的状态矩阵、补偿销毁、密文上下文绑定都实现了）。走查确认的不是「实现普遍写错了」，而是六类此前无人读过的东西：

1. **级联失败：单点把整批击穿**——F-01（`service.rs` 内，已实测复现）与 F-15（`repository-sqlx` 的批量读取，读路径已逐行核实、写路径无上界、内存侧无上界，两层都无门禁覆盖）**同形出现两次**——M08/M10/M13/M15 的判定依据。
2. **一块已落地逻辑在策略完全合规时仍会失败，且错误名指向错误原因**（F-14，实测：`⌈L/T⌉` 次探测即耗尽租约，取策略允许的最大 Provider 超时时只需 3 次）——M08/M09/M23 的判定依据。
3. **一块安全语义依赖枚举声明顺序且无测试锁定**（F-02）——M01/M03。
4. **最大的一块 SQL 实现，其证据不在门禁链上**（F-03，表述已收窄：证据存在且曾被采集，缺的是可复现性）——M13。
5. **两个 L4 适配器对同一份写入给出不同判据，而需求明确要求它们一致**（F-04）——M11/M15。
6. **验证面自身的四环同形缺陷**：评审流程的入口文档没有人读（F-12，实测并已修复 + 门禁化）、有一条门禁读不懂它负责的清单（F-11，已修复）、有一份人读清单没有门禁读（F-07），以及**门禁自己文档里的套件读数不受自己的规则约束**（F-13，实测并已修复 + 门禁化）——四环逐级更靠内，都不是模块缺陷，而是验证面的缺陷。

这六类都不在 `TECH-e2b-capability-parity.md` §1.2（形状）、§3.1（用例覆盖）、§3.2（覆盖空档）任何一张表的射程内，因此 18 条门禁 + 612 个契约用例全绿。**本报告的核心结论是：仓库缺的不是更多门禁，而是「实现逻辑走查」这一层审查本身**——F-11、F-12 与 F-13 尤其说明，门禁与评审流程自身也需要被走查，而最内一环（F-13）已经是**门禁关于自己的那句声明**。F-14 则说明另一半：**走查不能只读代码，还要在真实 API 上按策略边界跑一遍**——这条缺陷在静态阅读下完全合理（「每次调用前续租」读起来是对的），只有把 `T` 推到策略上限、把坏 Provider 放到 3 个时才现形。

同时必须说清一个**不构成缺陷**的边界：M17–M20 的 ❌ 是人工评审阻塞（22 个包 `pending-human-review`），不是工程欠账。而**已 `accepted` 的 5 条需求（REQ-2026-0001/0002/0004/0005/0006）其评审记录是自洽的**——实测 7 份非 pending 包恰好是那 5 条 + 2 份 `active` 持续评估，无一错配。

## 4. 最小可批范围：哪些发现现在就能修，哪些真的需要先批 `REQ-*`

这一节回答「要不要为此批一批需求」。判据是**修复落点所属需求的状态**——`AGENTS.md` 禁止的是「没有 `ready` REQ 就实现 Provider/API/SDK/调度器/隔离策略/密钥注入/可部署 Profile」，而不是禁止触碰已被 `accepted` 需求覆盖的代码。

权威映射来自需求文档自己的 `Trace → Components`，不是本报告的推断：

| 需求 | 状态 | 其 `Trace → Components` 覆盖的 crate |
| --- | --- | --- |
| REQ-2026-0002 | **accepted** | `sdkwork-sandbox-provider-spi`、`…-intelligence-sandbox-service`、`…-repository-memory` |
| REQ-2026-0005 | **accepted** | `…-intelligence-sandbox-service`、`…-repository-memory`、`…-repository-sqlx`、`database/`、`tests/contract/` |

据此逐项判定：

| 发现 | 落点（需求） | 需求状态 | 结论 |
| --- | --- | --- | --- |
| **F-01** 对账页级联失败 | `service.rs` 对账路径（REQ-2026-0002 + 0005） | accepted | **现在可修**。且它违反 REQ-2026-0005 验收标准第 46/47 条的逐会话收敛语义 |
| **F-14** 选择循环不续租、健康集群上报 `LeaseLost` | `service.rs` 选择路径（REQ-2026-0002 + 0005） | accepted | **现在可修**。修法 1（探测前续租）是既有租约模型的一致性补全，不新增能力、不改签名；修法 2（独立预算 + 拆分「本地过期」错误取值）会触及错误枚举，需先做跨仓消费者核对（同 F-06） |
| **F-15** 超界会话击穿整页对账 | `repository-sqlx` 批量读取（REQ-2026-0005）+ `service.rs`（REQ-2026-0002） | accepted | **收缩爆炸半径现在可修**：逐会话加载 + 单项降级，落点就是 `service.rs:411-419` 已写明的意图；**不删除/截断/过期任何幂等记录**，故不触碰 REQ-2026-0005 边界段的禁令。**改保留策略本身（放宽/取消上界）仍需 REQ-2026-0020** |
| **F-02** 隔离强度全序无锁定 | `provider-spi/src/capability.rs`（REQ-2026-0002） | accepted | **现在可修**（加断言≈15 行） |
| **F-04** 内存/ sqlx 写入校验不对称 | `repository-memory`（REQ-2026-0002）+ `repository-sqlx`（REQ-2026-0005） | accepted | **现在可修**。REQ-2026-0005 验收标准第 42 条明确要求「Memory 与 PostgreSQL Adapter 语义必须一致」，故这是**未满足已验收标准**，不是新特性 |
| **F-06** `LeaseUnavailable` 语义复用 | `service.rs` / `reconciliation.rs`（REQ-2026-0002 + 0005） | accepted | 可修，但**给它加枚举取值可能触及已发布契约**，需先做跨仓消费者核对 |
| **F-03** SQL 证据不在门禁链 | 验证面（REQ-2026-0005 的 Verification 段） | accepted | **现在可做**：把已存在的证据运行器纳入一条具名验证轮，不需要新代码 |
| **F-05** `Running` 无健康对账 | 新能力 | — | **需要新 `REQ-*`**（探活语义 = 新 Provider 调用） |
| **F-09** 操作历史上界 | `repository.rs` 常量（REQ-2026-0020） | draft | **不可动**——REQ-2026-0005 边界段明文禁止在此之前「删除、截断或过期当前幂等记录」。⚠️ 但 F-15 表明该禁令只冻结了**保留策略本身**；**收缩爆炸半径**（不让一个超界会话击穿整页）不受其约束，那一半现在就能修 |
| F-07 / F-08 / F-10 / F-11 / F-12 / F-13 | 纯文档与测试 | — | 已在本轮修复或建议修复，无需授权 |

**P0 最小可批范围 = 1 个包。**

唯一真正被 `REQ-*` 卡住、且仓库自己已把它标为「立即可执行」的是 **REQ-2026-0020**（`docs/engineering/gate-zero-exit-readiness-package.md` 的 `Phase 0.5` 写「批准 REQ-2026-0020 的最大 Operation 数、最大活动 Session 生命周期、终态保留、Late Retry、Repository 命名与 `MIG-*` 后，以 expand/backfill/verify/cutover 方式……收敛」）。对应包为 `REVIEW-20260730-sandbox-lifecycle-history-and-idempotency-retention`（`Risk: high`，受 `sandbox-lifecycle-history-and-idempotency.contract.json` 具名约束）。

选它的三条理由，都是可复核的：
1. 它是仓库自己的 Phase 表中唯一标注「立即可执行」的一项，不需要我先造顺序。
2. 它的范围是**封闭数值**（最大 Operation 数、最大生命周期、保留与 Late Retry 策略）——评审者可以在没有新证据的前提下给出裁决；而 Provider 那 11 个包都需要真实平台证据。
3. 它解开的是**当前唯一被显式冻结的行为**（F-09 背后的 `MAX_SANDBOX_SESSION_OPERATIONS` 失败关闭），而不是一个尚未实现的能力。

**因此本轮结论的执行建议是**：F-01/F-02/F-03/F-04 属已验收范围内的修复，不必等签署；要推进能力落地，最小的一步是只批 REQ-2026-0020 一个包。第二梯队再考虑 `REQ-2026-0009`（Service Host 组合；其包与 `REQ-2026-0003` 一起决定 M17–M20 能否从 ❌ 转出）。

## 5. 附录：本报告的实测读数

```text
$ cargo test --workspace
test result: ok. 67 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
（逐 crate 分布见 F-03）

$ node --test tests/contract/*.test.mjs
# tests 612
# pass 612
# fail 0
（F-11 修复前为 607 pass / 1 fail；本轮为 F-12 新增 4 条用例，故 608 → 612。F-13 只向既有 `test-inventory` 规则族追加变异条目与定向断言、不新增 `test()`，故总数不变）

$ node tools/check-sandbox-e2b-field-parity.mjs
  Declared suite size: 612 contract test(s) recomputed from tests/contract, 67 Rust test(s) recorded from `cargo test --workspace`; 10 rule families declared consistently in 4 surface(s)
（F-13 新增的校验读的就是 `tools/README.md` 本门禁章节内的 restatement，与上句的两个数逐一比对；它此前不在任何规则的射程内，所以那句停在 592）

$ node tools/check-sandbox-human-review-signoff.mjs
review packets on record: 29
pending human review: 22
named as required by a contract: 14 (14 still pending)
coherence: consistent (consistent is not approved; no review outcome has been accepted by this check)

$ node tools/check-sandbox-component-contract-alignment.mjs
Component contract alignment: 9 component spec(s) declare resolvable canonical specs, truthful languages and existing manifests

$ node tools/check-sandbox-doc-integrity.mjs
Documentation integrity: 637 relative link(s) across 199 markdown document(s) resolve, and 157 code block(s) across 163 live document(s) prescribe 309 existing script target(s)
（本报告创建前为 633 / 198：本报告自身是 +1 文档 +2 链接，F-12 的两条交叉引用再 +2 链接）

$ node tools/check-sandbox-requirement-traceability.mjs
Requirement traceability: 1184 requirement and 256 decision reference(s) across 220 live document(s) resolve to 27 requirement and 27 decision record(s), and every record is cited by another live document

$ cd target/_probe-module-walk && cargo run --quiet
（输出见 F-01；探针位于 gitignored 的 target/，不改动受版本控制的树）

$ cd target/_probe-module-walk && cargo run --quiet --bin lease
（输出见 F-14；同一只读探针工程的第二个 bin，三个场景共用同一个服务构造函数）

$ find crates -name '*.rs' | wc -l                 # 29
$ find crates -name '*.rs' -exec cat {} + | wc -l  # 12054
（§1.5 的封闭账目：20 + 3 + 4 + 2 = 29）

$ grep -n 'execute_sandbox_provider_call' crates/sdkwork-intelligence-sandbox-service/src/service.rs | wc -l
（F-14：14 个续租包裹点；同文件 15 处 Provider 调用，缺口恰为 `service.rs:1248` 那处健康探测）

$ grep -rn 'MAX_SANDBOX_SESSION_OPERATIONS' crates/ | wc -l   # 4，且全部落在 sqlx
$ grep -rln 'MAX_SANDBOX_SESSION_OPERATIONS' crates/          # 仅 …-repository-sqlx/src/repository.rs
$ grep -c 'MAX_SANDBOX_SESSION_OPERATIONS' crates/sdkwork-intelligence-sandbox-service/src/repository.rs   # 0
$ grep -c 'MAX_SANDBOX_SESSION_OPERATIONS' crates/sdkwork-intelligence-sandbox-repository-memory/src/lib.rs # 0
（F-15：上界只存在于 sqlx 的读取路径；**写路径（service 的 `capture` / `validate_sandbox_persisted_invariants`）
与内存适配器都不含它** ⇒ 第 10001 条可写入，且内存侧读得出来、sqlx 侧整页读不出来）
```

**套件规模的声明点不是单点的，本轮实测是四处**：给套件加用例会让 `check-sandbox-e2b-field-parity.mjs` 先报 `test-inventory` 红（本轮实测原文 `declares 608, the suite declares 612`），因为规模要同步到三处可复算处——`tests/contract/sandbox-e2b-field-parity-tool.contract.test.mjs` 的 `REAL_CONTRACT_TESTS`、`specs/sandbox-e2b-capability-baseline.json` 的 `testInventory.tests`、`TECH-e2b-capability-parity.md:316` 引用的那句读数——**外加此前无人读的第四处** `tools/README.md:159` 的 restatement（F-13）。本报告初稿断言只有「两处 + 一句」，是被自己的走查推翻的：那句 restatement 在套件 592 → 612 的全过程中一直写着 592。本轮确实**先红了、后同步**，这正是该模型按设计工作：声明与重算不一致就报错，而不是静默接受一个过期数字。F-13 修复后第四处也进入受检范围，于是「要同步到几处」不再需要靠人记。

**F-12 的变异自证**（落盘改后跑，跑完还原并核验）：

```bash
node --test tests/contract/human-review-signoff.contract.test.mjs
```

把新判据的入口条件 `if (exitPackageDocument !== null) {` 置为 `if (false) {` 后，**恰好 3 条**规则用例转红（漏登一行 / 风险不一致 / 计数过期），**异族 0 条**，套件总数 22 不变；还原后该文件 SHA256 与改前**逐字节一致**（前缀 `8072d947c5dd1c2b`）。

治理边界复核（`2026-09-22`）：`REQ-*` 27 条中 **0 条 `ready`、5 条 `accepted`**（REQ-2026-0001/0002/0004/0005/0006），27 份 ADR 全部 `proposed`，24 份 `specs/*.json` 中 **0 份** `implementationAuthorized: true`。因此 §1 里 M17–M20 的 ❌ 与 M01–M16 中标注「等待治理」的部分**不是工程欠账，而是人工评审阻塞**；本报告不改变该状态。而 §4 说明的是另一半：那 5 条 `accepted` 所覆盖的 crate 里发现的问题，**不需要新签署就能修**。
