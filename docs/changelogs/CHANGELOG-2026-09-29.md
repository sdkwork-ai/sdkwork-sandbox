# CHANGELOG-2026-09-29

主题：生命周期写放大与账本触界变砖修复（M1/M2）、命令执行租户分区（M3），以及全量审计 MINOR 闭环。

## 生命周期持久化（M1 + M2）

- `crates/sdkwork-intelligence-sandbox-service/src/model.rs`：
  - `SandboxSession` 携带持久化 frontier（`sandbox_persisted_operation_count` + 末项 outcome 指纹），`sandbox_operations_to_persist()` 只暴露本次保存必须持久化的账本尾部（追加条目 + 恰在保存前解析的末项 `InProgress` 条目）；完全持久化的终态账本窗口为空。
  - `begin_sandbox_operation` 的保留界新增终态宽限：`Destroy` 类型在 `MAX_SANDBOX_SESSION_PERSISTED_OPERATIONS`（= `MAX_SANDBOX_SESSION_OPERATIONS + 1`）内始终可开始，普通类型仍在保留界 fail-closed。触界 Session 不再"无法销毁"，Provider 分配不再因账本触界泄漏。
  - 新导出 `MAX_SANDBOX_SESSION_PERSISTED_OPERATIONS`。
- `crates/sdkwork-intelligence-sandbox-service/src/repository.rs`：`SandboxSessionRepositorySnapshot` 携带 `sandbox_persisted_operations` 增量（`capture` 填充，行构造保持 None 回退全量），`sandbox_operations_to_persist()` 为公共读取面。
- `crates/sdkwork-intelligence-sandbox-service/src/service.rs`：`persist_sandbox_session` 与 create 成功路径调用 `mark_sandbox_operations_persisted()`；create 的并发重复插入处理合并 `DuplicateSandboxSession` 与 `VersionConflict` 两个来源。
- `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/repository.rs`：
  - `save_sandbox_session` 只 upsert 增量条目（绝对序号 = 全量长度 − 增量长度 + 偏移），消除"每次状态转移重放全量账本"的 O(n²) 写放大；`insert_sandbox_session` 仍一次性写全量（一次性路径）。
  - 读窗口提升为 `MAX_SANDBOX_SESSION_PERSISTED_OPERATIONS + 1` 行、`> MAX_SANDBOX_SESSION_PERSISTED_OPERATIONS` 失败关闭。
  - `acquire_sandbox_session_lease` 拆出单语句 `try_take` 辅助；分类显示租约恰在失败后过期时做一次有界接管重试，消除误报冲突。
  - SQLSTATE 23505 按约束细分：`pk_sandbox_session` → `DuplicateSandboxSession`、`uk_sandbox_runtime_binding_sandbox` → `RuntimeBindingConflict`、未命名约束 → `InvalidStoredData`（不再笼统映射 `VersionConflict`）。
  - 新增服务驱动的 PostgreSQL 集成测试钉死增量契约（账本行数恒等于操作数、序号连续 0..n、四阶段生命周期终态一致）。
- `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/instance.rs`：23505 仅 `uk_sandbox_instance_owner_name` → `DuplicateName`，主键/未命名约束 → `InvalidStoredData`。

## Local Provider 命令执行（M3）

- `crates/sdkwork-sandbox-provider-local/src/command_executor.rs`：活跃执行注册表新增 per-tenant 分区（`MAX_SANDBOX_LIVE_COMMANDS_PER_TENANT = 128`、`MAX_SANDBOX_LIVE_OUTPUT_BYTES_PER_TENANT = 256 MiB`）；节点级 1024 / 1 GiB 保持。预留字节总量与租户预算增量维护，准入检查从 O(n) 扫描降为 O(1)；空租户桶即时回收。单一租户不再能占满节点预算使其他租户饥饿。
- `crates/sdkwork-sandbox-provider-local/src/process_runner.rs`：可执行文件与工作目录解析（阻塞文件系统元数据）移入 `spawn_blocking`，不再占用 Tokio worker；reader join 改按引用，清理预算超时路径 `abort` 两个 reader 任务，逃逸孙进程不再留下无界存活的 detached 任务与缓冲。
- 测试确定性加固：`command_executor_tests.rs` 的三个注册表/预算测试从固定 `sleep` 后断言改为 runner 进入即发信号的确定性同步（tokio mpsc + 有界超时等待），消除高负载下的偶发失败；节点预算测试随分区设计改为 8 租户 × 128 MiB 填满节点预算（原"单租户填满节点"的前提已被 M3 的租户分区正确禁止）。`docs/architecture/tech/TECH-platform-support.md` 同步声明网关二次信号退出码的 `cfg-macro` 平台标记（20 → 21）。

## 依赖

- `process-wrap` 9.1 → 10.0.1（本仓独有依赖；零 API 变更，全部 33 个 provider-local 测试通过）。
- 兼容范围内全部依赖已为最新（`cargo update` 锁定 0 项变更）。
- `sha2` 0.10 → 0.11 与 `base64` 0.22 → 0.23 为跨仓生态决策：sdkwork-iam / sdkwork-database / sdkwork-web-framework / sdkwork-utils 均钉在 0.10/0.22（且 `rsa 0.9` 依赖 sha2 0.10 feature），本仓单方面升级会在最终二进制中产生双份密码学/编码栈。留待兄弟仓统一的生态迁移，不属于单仓可消除的债务。

## sdkwork-utils 复用审查结论

`sdkwork-utils-rust` 已被 service、memory/sqlx repository、routes 四个 crate 采用（serde 助手等）。剩余手写逻辑均为领域特化实现，与 utils 的一次性助手不构成重复：命令指纹是流式多字段长度前缀哈希（非一次性 `sha256_hash`）、实例时间校验是严格 RFC3339-UTC + ≤6 位小数语法（严于通用 `parse_datetime`）、分页 cursor 是 no-pad URL-safe base64（utils 的 `base64url_*` 为标准填充）。无冗余实现需要收敛。

## API 网关与装配

- `crates/sdkwork-api-sandbox-assembly`：
  - 新增 `readiness.rs` `SandboxDrainGate`（实现框架 `ReadinessCheck`）：begin_drain 后 `/readyz` 立即失败，负载均衡在排水期间停止导流；装配 readiness 组合池探针 + drain 探针。
  - 新增 `assemble_api_router_with_drain` / `assemble_api_router_with_pool_and_drain`；原函数以一次性 gate 委托，行为不变。
  - 补齐 `[lints] workspace = true`（此前是唯一未继承 workspace lint 的成员）。
- `crates/sdkwork-api-sandbox-standalone-gateway`：首个信号开启排水并翻转 gate；第二个信号以 130/143 常规信号退出码强制退出卡死的排水，Windows 亦受 Ctrl+C 双击保护。

## 文档与契约

- `specs/sandbox-commercial-readiness.contract.json`：`standalone-local-developer-runtime` 的 blockers 更新为当前事实（命令执行切片已授权实现；terminal/filesystem/network/browser 未实现；组合缺失；驻留证据缺失）。总体 NO-GO 结论不变。
- `database/README.md`：registry 表数 4 → 5（`sandbox_instance` 已在册）。
- `README.md`、`docs/architecture/tech/TECH-security-and-operations.md`、`docs/architecture/tech/TECH-modules-and-contracts.md`、`docs/engineering/gate-zero-exit-readiness-package.md`：读取界与写侧增量语义同步。

## 评估后不实施（记录理由）

- 过滤列表复合索引（owner/state + created_at）：契约物化器以单 baseline 文件为源（`db:materialize:contract --baseline`），post-baseline 迁移不会进入 `contract/schema.yaml`，会造成注册契约与有效 schema 的真实 drift。正确时机是 REQ-2026-0018 的 `tenant_id TEXT` → `BIGINT` 人审 refold，届时与 `(tenant_id, owner, created_at DESC, id DESC)`、`(tenant_id, state, created_at DESC, id DESC)` 两个复合索引一并纳入；当前无过滤路径的扫描由租户与 page_size 双重有界，不构成缺陷。

## 治理边界（本次未实施，须人审解锁）

- Service Host 组合、session 生命周期 HTTP 面（REQ-2026-0023）、E2B OpenAPI 权威与 SDK 生成（REQ-2026-0028）、Firecracker/调度/池/配额（REQ-2026-0008、0016~0019）、账本保留与 Late Retry（REQ-2026-0020）、`tenant_id` BIGINT 迁移（REQ-2026-0018）均为 draft REQ 门禁锁定项，未经人审批准不得实施。

## 商业化路径门禁推进（2026-09-29 所有者结构化决策）

- `REQ-2026-0028`（E2B-Compatible API And SDK Family Authority）经所有者于本会话的书面指令（逐字记录于 `REVIEW-20260924` 附录）由 `draft` 提升为 `ready`；`ADR-20260924` 进入 `accepted`。授权切片：`apis/` 权威契约 + 双向 parity ledger 物化与生成链（`SDK_WORKSPACE_GENERATION_SPEC.md`）；服务面路由/RPC/envd、公共命名与边缘归属仍需各自机器契约翻转与评审。
- `REVIEW-20260924` 评审包追加 2026-09-29 授权扩展节（逐角色 Human Outcome 表，仓库所有者以单一身份行使各列名角色）。
- 钉面同步：PRD 链接行与缺口行、PRD-roadmap 进度段、`TECH_ARCHITECTURE` §5、`TECH-e2b-capability-parity` 普查行（[28, 4, 5, 19] / decisions [28, 24, 4]）、`docs/INDEX.yaml` 两行状态、`e2b-parity-matrix-tool` / `e2b-field-parity-tool` / `sandbox-platform-code-tool` 契约测试钉数（ready=4、draft=19、proposed=24、accepted-ADR=4、Rust 读数 143/0/2、declared tests=145、ignored 两个、平台标记回 20）。
- 网关二次信号强杀撤回：Gate 0 交付门禁止控制面出现 `std::process`（`provider-delivery-gate.contract.test.mjs`），force-exit watcher 移除；drain 期间 readiness 摘流保留，卡死排空交由监督者 stop-timeout（`terminationGracePeriodSeconds` + SIGKILL，标准实践）。平台标记 21 → 20。

## REQ-2026-0028 授权切片 v0：apis/ 权威契约与双向 parity ledger 物化

- `apis/internal-api/intelligence/sandbox-internal-api-authority.openapi.json`：权威 OpenAPI v0（OpenAPI 3.1），覆盖当前已拥有的 sandbox 实例注册表面（5 条路由）；每个 operation 携带 `x-e2b-reference` 映射到钉死的 E2B 参考基线；`SandboxVersion` 遵循 int64-as-string（`API_SPEC.md` §13.6，`x-sdkwork-int64-string: true`）；ingress-token 安全方案仅限 internal-api 面。
- `apis/internal-api/intelligence/sandbox-e2b-parity-ledger.json`：71 个 E2B 参考操作的双向 ledger——4 个 `mapped`（实现面）+ 67 个 `pending-gate`（无实现能力，按基线类别登记，绝无静默遗漏）。由 `tools/generate-sandbox-e2b-parity-ledger.mjs` 从钉死基线确定性推导，`--check` 即 AC8 回归命令。
- `tests/contract/sandbox-e2b-parity-ledger.contract.test.mjs`：四向钉死（完备性、映射指向真实权威操作、int64/安全方案规则、可复现性）。
- 钉面同步：基线 `testInventory`（42 文件/678 测试）、`REAL_CONTRACT_FILES/TESTS`、tools/README 与 parity 文档套件读数（678 pass / 0 fail）。
