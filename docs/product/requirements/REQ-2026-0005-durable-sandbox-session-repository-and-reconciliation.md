---
id: REQ-2026-0005
title: Deliver durable Sandbox Session persistence and crash reconciliation
owner: SDKWork Runtime Platform
status: accepted
source: reliability
problem: The lifecycle candidate stores Sandbox Session state only in process memory, so operation idempotency, Runtime Binding ownership, and transient lifecycle recovery do not survive process restart or concurrent control-plane replicas.
goals:
  - Make PostgreSQL the authoritative store for Sandbox Session, operation, Runtime Binding, and recovery-lease state.
  - Preserve tenant isolation, operation uniqueness, optimistic concurrency, and monotonic fencing under concurrent replicas.
  - Recover Starting, Stopping, and Destroying Sandbox Sessions after process interruption without exposing Provider-private allocation data.
non_goals:
  - Execute a real Local, Docker, microVM, Kubernetes, or Remote VM Provider.
  - Access host paths, Agent Workspace content, terminal, network, browser, Git, build, port, snapshot, or Secret values.
  - Add HTTP, RPC, generated SDK, Service Host deployment, IAM authorization, quota, metering, or production release packaging.
users:
  - SDKWork Runtime Platform maintainers
  - Sandbox control-plane operators
  - Future SDKWork Kernel integrators
affected_surfaces:
  - rust-components
  - backend
  - database
  - composition
---

# REQ-2026-0005: 交付持久化 Sandbox Session Repository 与崩溃恢复

## 验收标准

- `database/database.manifest.json` 声明 `schemaVersion: 2`、`databaseRole: authoritative-server`、且引擎严格为 PostgreSQL；数据库契约、Prefix/Table Registry、Migration、Seed Manifest、Drift Policy、Fixture 与 Contract Test 使用 SDKWork 标准目录。
- PostgreSQL 分别持久化 `SandboxSession`、`SandboxSessionOperation`、`SandboxRuntimeBinding` 与 Session Recovery Lease；所有在线查询和唯一约束以 `tenant_id` 领先，禁止创建第二套 Agents Workspace/Session Registry。
- 数据库字段遵循固定产品术语和实现映射：`sandbox_workspace_id`、`sandbox_session_id`、`sandbox_session_state`、`sandbox_operation_id`、`sandbox_runtime_binding_id`、`sandbox_provider_id`、`sandbox_lease_owner_id` 与 `sandbox_fencing_token`；共享 `TenantId`、`OperationId`、`RuntimeCapability`、`IsolationAssurance` 不创建重复类型别名。
- `sdkwork-intelligence-sandbox-repository-sqlx` 通过注入的 `sdkwork-database-sqlx::DatabasePool` 使用 PostgreSQL；Repository 不创建连接池、不执行迁移、不读取环境变量，也不依赖 `sdkwork-kernel` 或 `sdkwork-agents`。
- Insert/Save 在单个事务中持久化 Aggregate、Operation 和当前 Runtime Binding；`sandbox_operation_id` 在 Tenant 内唯一，同一 ID 绑定不同 Session 或 Kind 时返回 `SandboxSessionRepositoryError::DuplicateOperation`。
- `sandbox_session_operation` 使用从 `0` 开始、Tenant+Session 内唯一且连续的 `sandbox_operation_sequence` 保存 Aggregate Operation 顺序；Restore 只按该字段排序并拒绝缺口、重复或重排，禁止用事务内相同的 `created_at` 和随机 Operation ID 推导状态机顺序。
- Save 使用 `WHERE tenant_id = ... AND sandbox_session_id = ... AND version = expected` 原子 Compare-and-swap，并区分 Not Found 与 Version Conflict；SQLSTATE 按约束名称或标准状态码映射，禁止匹配数据库错误文本。
- Session Version 的统一逻辑范围为 `0..=BIGINT_MAX`；Domain、Memory 与 PostgreSQL Adapter 在递增或恢复超出该范围时必须关闭失败，禁止溢出、回绕或产生适配器语义分叉。
- Service 拥有窄化的 Repository Snapshot/Restore Boundary；Restore 在解密 Provider Allocation 前重放并验证 Create/Start/Stop/Destroy Operation 顺序、状态、Typed Failure、Runtime Binding 与 Allocation 组合不变量，非法持久化组合统一返回 `InvalidStoredData`。普通 Domain Model 不派生 Serde，也不提供 Provider Allocation Reference 的普通公开 Getter。
- `SandboxProviderAllocationRef` 只在受控持久化边界内交给注入的保护器；落库内容为 Ciphertext、Key ID、Key Version 与 Crypto Version，不存储明文。实现复用 `sdkwork-utils-rust` 的 HKDF-SHA256 与 AES-256-GCM，解密或身份上下文校验失败时关闭失败。
- Ciphertext 的派生上下文绑定 `tenant_id`、`sandbox_session_id`、`sandbox_runtime_binding_id` 与 Crypto Version，防止跨 Tenant/Session/Binding 搬移密文；密钥材料只从未来 Service Host Secret Port 注入，不进入 Source、Database Asset、普通 Config、Debug、Log、Event 或 Wire。
- 生命周期调用在 Provider Side Effect 前取得 Session Lease；Lease Acquisition 使用 PostgreSQL 数据库时钟，过期接管时原子递增非零 `SandboxFencingToken`，Renew/Release 必须同时匹配 Tenant、Session、Owner 与 Token。`SandboxFencingToken` 达到 PostgreSQL `BIGINT` 上限后必须关闭失败为 `LeaseConflict`，不得回绕或伪装成临时 Lease 竞争；Memory 与 PostgreSQL Adapter 语义必须一致。
- Provider Allocate/Start/Stop/Destroy Request 携带 `sandbox_fencing_token`；同一 `SandboxRuntimeBindingId` 对较旧 Token 必须拒绝，避免 Lease 过期后旧控制器继续写入。
- 每次 Provider Side Effect 前 Renew 当前 `SandboxSessionLease`；Provider Operation Timeout 必须非零且不超过 Lease Duration 的一半，Timeout 映射为 `SandboxProviderErrorKind::Timeout` 并持久化 Typed Failure。取得 Lease 后，Renew 失败、Renew 返回不匹配 Identity、Save 返回 `LeaseConflict`、或成功业务后的 Release 失败统一映射为 `SandboxLifecycleError::LeaseLost`；若业务已产生 Provider/Readiness 错误且 Release 同时失败，保留原业务错误，Lease Lost 时不得继续 Provider 调用或 Save。
- Start 在首次 Allocate 前原子持久化 `Starting`、In-progress Start Operation 以及带 Sandbox/Binding/Provider Identity 且无 Allocation Reference 的 `SandboxRuntimeBinding` Intent；任何持久化 `Starting` 都必须拥有可恢复 Binding。Failed/Stopped Retry Start 必须先在原稳定状态下幂等销毁旧 Allocation，成功后才持久化本次新 Intent；清理失败直接记录 Typed `Failed(Cleanup)`，不得让 Reconciler 把旧 Allocation 当作本次 Start 目标。Allocate 以稳定 Identity 幂等，消除“Provider 已分配但数据库没有可恢复身份”的崩溃窗口。
- Reconciler 仅按显式 `tenant_id` 分页读取 `Starting`、`Stopping`、`Destroying` 状态，逐 Session 取得 Lease 后必须重新读取权威 `SandboxSession`，只对重读后仍为瞬态状态的 Session 发出 Provider Side Effect；已由其他控制器推进为稳定状态的陈旧候选直接按当前状态收敛。页大小只允许 `1..=200`，非法请求统一返回 `InvalidPageRequest`，Continuation 只在有后继行时返回。它不依据 Lease 前快照执行、不执行无界全表收集、不绕过 Tenant Scope。
- `Starting` 恢复重放幂等 Allocate/Start 并执行 Readiness Gate；`Stopping` 恢复幂等 Stop；`Destroying` 恢复幂等 Destroy。成功后完成原 Operation，失败则进入 Typed Failure；旧 Token 不得提交状态。
- 测试覆盖 PostgreSQL 空库 Migration、Tenant 隔离、Operation 冲突与稳定顺序、Version CAS、非法跨字段存储篡改关闭失败、密文非明文/上下文绑定/错误密钥关闭失败、Lease 竞争/过期接管/Token 单调性与上限耗尽、精确分页边界、Lease 前陈旧候选、Renew/Save/Release 控制权故障、瞬态状态重启恢复和关键崩溃点 Failure Injection。
- Cargo Format、Test、Clippy、Database Framework、Component、Layering、Naming、Documentation 与 Repository Baseline 检查通过；真实 PostgreSQL 证据缺失时本需求不得标记 `accepted`。

## 非功能需求

| 领域 | 要求 |
| --- | --- |
| Security | Provider 私有 Allocation Reference 使用注入密钥进行应用层加密；普通日志、错误、事件、Projection、Wire 和测试快照不得出现明文。跨 Tenant 查询与密文搬移测试必须失败。 |
| Privacy | 只持久化恢复所需最小 Sandbox Runtime Metadata；不复制 `AgentWorkspace`/`AgentSession` 业务数据、Workspace 内容或 Provider Payload。 |
| Performance | Session/Operation/Binding/Lease 查询使用 Tenant-leading B-tree Index；恢复扫描有界且分页。P0/P1 Query Plan 在发布前记录 `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)`。 |
| Reliability | PostgreSQL 是唯一 Server Authority；Operation、CAS、Lease/Fencing、有界 Provider Timeout 与瞬态状态恢复必须通过并发、重启和故障注入证据。RPO/RTO、Backup/Restore 和多副本运行证据由后续 Service Host/Release Requirement 完成。 |

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `DATABASE_SPEC.md`, `DATABASE_FRAMEWORK_SPEC.md`, `MIGRATION_SPEC.md`, `APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`, `COMPONENT_SPEC.md`, `CODE_STYLE_SPEC.md`, `NAMING_SPEC.md`, `RUST_CODE_SPEC.md`, `SECURITY_SPEC.md`, `TEST_SPEC.md`.

Components: `crates/sdkwork-intelligence-sandbox-service`, `crates/sdkwork-intelligence-sandbox-repository-memory`, `crates/sdkwork-intelligence-sandbox-repository-sqlx`, `database/`, `tests/contract/`.

Decision: [ADR-20260728: PostgreSQL Sandbox Lifecycle Persistence And Reconciliation](../../architecture/decisions/ADR-20260728-postgresql-sandbox-lifecycle-persistence-and-reconciliation.md).

## Verification

```bash
cargo fmt --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node ../sdkwork-specs/tools/check-database-framework-standard.mjs --root .
node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root . --strict
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-identity-naming.mjs --root .
node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root .
node ../sdkwork-specs/tools/audit-repository-baseline.mjs --root .
```

Live PostgreSQL Migration、Repository、Concurrency、Lease/Fencing、Recovery、Query Plan 与 Backup/Restore Evidence 必须在进入生产 Release Gate 前归档；静态 Schema 检查或 Memory Test 不替代该证据。

## Current Evidence

2026-07-29 已通过临时 PostgreSQL 17 的空库 Migration/幂等重跑、Status/Drift、Repository Round-trip、Tenant/Operation/CAS、稳定 Operation 顺序、非法状态/Failure/Binding 篡改拒绝、密文落库、并发 Lease 竞争、过期接管、Token 单调性与上限耗尽、旧 Lease 拒绝、重建实例后的 Reconciliation、精确分页边界与 Query Plan 执行。Start 顺序审计进一步消除了两个崩溃恢复窗口：首次 Start 不再先保存缺失 Binding 的 `Starting`，Retry Start 不再先保存携带旧 Allocation 的 `Starting`。Snapshot Capture/Restore 均拒绝缺失 Binding Intent 的 `Starting`；故障注入证明 Allocate 成功但 Allocation Save 失败后，Provider 清理完成、持久状态仍为无 Allocation 的稳定 Intent，Reconciler 使用更高 Fencing Token 重新 Allocate 并只启动新 Allocation。Reconciler 在取得 Lease 后重新读取权威 Session，陈旧候选不会触发 Provider 调用；Renew 失败、Save Lease Conflict 与成功业务后的 Release 失败统一为 `LeaseLost`，而已有 Provider/Readiness 失败不会被并发 Release 错误覆盖。2026-07-30 的边界审计统一了 Domain、Memory 与 PostgreSQL 的 `BIGINT_MAX` Session Version 上限，并以领域和持久化快照回归测试证明超界关闭失败且不改变当前版本；同日交付的仓库内证据运行器又在 PostgreSQL 16/17 上重复通过 Migration `1/0`、Status/Drift、完整 Repository 测试、自定义格式 Backup/Restore、`11/20/9/11` 计数一致和 Allocation 明文零匹配，并以双 URL 安全闩阻止破坏性测试回落默认开发库。Kernel 已对 `SandboxLifecycleError::LeaseUnavailable`、`SandboxLifecycleError::LeaseLost` 及全部 Repository Error 建立无通配分支的显式安全映射；`sdkwork-intelligence-agents-service -> sdkwork-agent-kernel -> sdkwork-intelligence-sandbox-service` 已通过锁定依赖编译与 Cargo Dependency Tree 验证。完整环境、命令、结果和未关闭门禁见 [REVIEW-20260728: Sandbox PostgreSQL Persistence Verification](../../engineering/reviews/REVIEW-20260728-sandbox-postgresql-persistence-verification.md)。本需求定义的持久化与恢复技术范围已经验收为 `accepted`；这组证据不替代真实 Provider Fencing、Service Host Secret/KMS、多副本长稳、PITR、SLO、CI 证据归档或人工架构/安全/跨仓库集成评审。

## Release And Review Boundary

本需求只交付生产候选的持久化与恢复边界，不激活真实 Sandbox Provider、Service Host 或公开 Transport。其技术范围状态为 `accepted`；数据库数据所有权、Provider Private Reference 加密、Fencing Contract、跨组件公共命名及 `sdkwork-agents -> sdkwork-kernel -> sdkwork-sandbox` 集成仍需要人工架构/安全评审，在评审和生产 Provider 证据完成前不得据此宣称生产或商业发布就绪。

完整 Operation 历史 hydrate/replay 是本需求已验收候选的当前行为，不是商业化保留策略。其长生命周期成本、最大 Operation 数、最大 Session 生命周期、终态幂等保留、Late Retry 和安全 Migration 由 [REQ-2026-0020](REQ-2026-0020-sandbox-lifecycle-hot-state-and-idempotency-retention.md) 单独门禁；该 Requirement 获批并完成真实 PostgreSQL 迁移证据前，不得删除、截断或过期当前幂等记录。
