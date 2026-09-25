# CHANGELOG-2026-09-24: Runtime-Breaking SQL Fix, Internal-API Authority Alignment, Real Command Execution, And Audit Remediation

Date: 2026-09-24

Phase: Phase 0 candidate remediation across persistence, lifecycle service, provider execution, and HTTP surface

## Summary

本变更是一次全仓审计后的整改落地面。它修复一个会让权威库所有事务在运行时失败的 P0 SQL 缺陷、把首个 HTTP 面从 `app-api` 双令牌形态纠正为规范规定的 `internal-api` ingress-token 形态、把实例列表从 OFFSET+COUNT 改为 keyset cursor 分页并补齐排序索引、把 Local 命令执行从"仅有 seam 无实现"落成真实 tokio 进程切片（有界流式输出、硬超时 kill+reap、取消注册表、fencing 校验），并为生命周期服务补齐仓库超时、补偿 destroy、reconcile 单项降级与操作账本上限。所有新表仍未上线，按 pre-launch 零债务规则直接采用规范形态，无兼容层。

## Fixed

### P0：PostgreSQL 事务超时语句运行时必坏

- `crates/sdkwork-intelligence-sandbox-repository-sqlx/src/repository.rs`：`SET LOCAL statement_timeout = $1` / `SET LOCAL lock_timeout = $1` 以绑定参数执行，而 PostgreSQL 的 `SET` 语法不接受参数，Parse 阶段即报 42601——所有经 `enforce_sandbox_transaction_timeouts` 的事务路径（读取/插入/保存）在真实 PostgreSQL 上必然失败。改为 `SELECT set_config('statement_timeout'|'lock_timeout', $1, true)`，同样事务本地生效且接受绑定参数。CHANGELOG-2026-08-05 所称"事务超时契约强制执行"自该缺陷引入后从未在真库上成立，本次以代码修复关闭

### P1：HTTP 面权威对齐 internal-api（INTERNAL_API_SPEC §2/§4/§6）

- crate `sdkwork-routes-sandbox-app-api` 重命名为 `sdkwork-routes-sandbox-internal-api`；路由前缀从 `/app/v3/api/sandbox/*` 改为锁定的 `/internal/v3/api/intelligence/sandbox/*`
- 认证从 dual-token 改为 `HttpRoute::ingress_token`（`X-SDKWork-Ingress-Token`）；internal-api 为 operator-trusted，不再声明未执行的 IAM 权限码（旧 manifest 声明 `web.sandbox.read/write` 而框架对 app-api 仅观测，构成授权夸大）
- 删除硬编码回退租户 `"100001"`（`SDKWORK_SANDBOX_TENANT_ID`/`DEFAULT_TENANT_ID`）与公开裸 router 导出（`business_router`/`router`/`build_router*`/`gateway_mount*`）：无验证租户上下文的请求一律 403 fail-closed；仅保留经 web framework 层包装的组装入口
- `assembly-manifest.json` surface 改为 `internal-api`，`component.spec.json` 同步；`pnpm api:assembly:materialize` 重生成
- assembly 与 database-host 的 `Result<_, String>` 升级为 thiserror 类型化错误（`SandboxAssemblyError` / `SandboxDatabaseHostError`，保留 source 链）

### P1：分页对齐 PAGINATION_SPEC（§3/§5/§6/§12）

- 实例列表由 `page`/`pageSize`（camelCase 违禁线名）+ `LIMIT/OFFSET` + 每页 `COUNT(*)` 改为 cursor 模式：查询参数 `cursor`/`page_size`/`sandbox_instance_owner_id`/`sandbox_instance_state`（lower_snake_case），`page_size` 缺省 20、越界 400 拒绝而非夹取；排序键 `(created_at, sandbox_instance_id)` DESC 的 keyset seek，仓库层超取 1 行探测 `has_more`，opaque base64url cursor 服务端解码并校验（伪造/过期一律 400）
- 响应 `pageInfo.mode = "cursor"`，`nextCursor` 仅在还有后续行时返回；不再有每页 `COUNT(*)` 与深翻页劣化
- `database/ddl/baseline/postgres/0001_sandbox_baseline.sql` 新增 `idx_sandbox_instance_listing (tenant_id, created_at DESC, sandbox_instance_id DESC)` 支撑 keyset 扫描；契约三件套经 `db:materialize:contract` 重生成且可逐字节复现

### P1：生命周期服务健壮性

- 新增 `BoundedSandboxSessionRepository` 装饰器与 `instance_service` 调用级超时（30s，与 statement_timeout 基线一致）：被卡死的数据库以可重试 `Unavailable` 上浮，而不是让持有租约的请求挂到租约过期
- reconcile 的 Starting 分支：allocate 成功后 persist 失败现在执行补偿 destroy（镜像主 start 流程），不再泄漏孤儿 Provider 资源；start 流程的补偿 destroy 失败不再静默丢弃，记 `tracing::error`
- reconcile 单项降级补齐：租约在项中途丢失或瞬时不可用时按 `LeaseUnavailable` 上报并继续收敛该页，不再让一个会话冻结整页（最多 199 个后续会话）
- 会话操作账本写侧上限：`model.rs` 导出 `MAX_SANDBOX_SESSION_OPERATIONS = 10_000`（sqlx 仓库改为复用该单一来源），账本触顶后 `begin_sandbox_operation` fail-closed，消除"写侧无界增长、读侧拒绝加载"的变砖路径

### P1：Local 命令执行真实实现（REQ-2026-0007 授权切片）

- SPI `SandboxCommandOutcome` 增加有界 `sandbox_stdout`/`sandbox_stderr` 二进制捕获（契约 result schema 的 `sandboxStdoutBase64`/`sandboxStderrBase64` 在 SPI 层的承载）
- 新增 `crates/sdkwork-sandbox-provider-local/src/process_runner.rs`：`SandboxLocalTokioProcessRunner` 以 tokio::process 真实执行——provider-owned 根目录解析裸可执行名（环境不可影响解析、canonicalize 校验防符号链接逃逸）、`env_clear` + 准入 allowlist、工作目录在 workspace root 下重校验、8KiB 分块流式读取并在字节上限处截断（防满管道死锁与无界缓冲）、硬超时 kill+cleanup reaping、Unix `process_group(0)`、`kill_on_drop(true)`
- `command_executor` 重写：live 注册表（上限 1024，按 operation id 幂等冲突、不同指纹告警）、`sandbox_cancel` 落地（token 匹配通知取消、未知/终态幂等 no-op、stale token 拒绝）、fencing token 0 作为畸形请求拒绝、`ExecutableDenied` 映射为 `PolicyDenied`
- 边界补强：保护环境名单加入 `LD_PRELOAD`/`LD_LIBRARY_PATH`/`DYLD_INSERT_LIBRARIES` 等注入向量，敏感段名单加入 `KEY`（`API_KEY`/`GPG_KEY` 不再漏过）
- 诚实边界：descendant containment（Windows suspended Job Object、Linux delegated cgroup v2）仍未实现，Terminal capability 保持未声明，等待 host-boundary 契约的 real-evidence 门禁

### P2

- `instance.rs` 仓库的 `unwrap_or(i32::MAX)`/`unwrap_or(i64::MAX)` 静默钳制改为 fail-closed（`InvalidStoredData`），与全仓转换纪律一致
- `validate_expires_at` 补上小数位形态校验（`...T00:00:00.//-Z` 之类畸形不再可持久化）
- `update` 返回存储行（含存储侧 `updated_at`），与 `create` 的返回契约一致；内存适配器 `save` 补齐 owner-name 唯一性（镜像 PG `uk_sandbox_instance_owner_name` 对 UPDATE 的约束）

## Dependencies

- workspace 新增 `base64 = "0.22"`（cursor 编码）；tokio features 增加 `io-util`、`process`
- `cargo update` 至当前兼容最新（thiserror 2.0.21、zerocopy 0.8.58 等）

## Verification

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
```

`123 passed / 0 failed / 1 ignored`（ignored 为需外部 PostgreSQL 的证据测试，读数已同步 `specs/sandbox-e2b-capability-baseline.json` 的 `testInventory.rustWorkspace` 与 `tools/README.md`）。

文档同步：`TECH_ARCHITECTURE.md`（模块计数、internal-api 面现状）、`TECH-e2b-capability-parity.md`（模块清单、证据表 25 行 124 用例、相关引用行号）、`tools/README.md`。`node tools/check-sandbox-e2b-parity-matrix.mjs`、`node tools/check-sandbox-e2b-field-parity.mjs`、`node tools/check-sandbox-database-contract-reproducibility.mjs` 全部通过。
