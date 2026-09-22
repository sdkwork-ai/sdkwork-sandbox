# MIG-2026-0001: Tenant ID TEXT → BIGINT Normalization

Status: candidate
Owner: SDKWork Runtime Platform
Date: 2026-07-29
Requirement: REQ-2026-0018 (PostgreSQL Quota/Capacity Reservation Persistence)
Module: sandbox
Engine: postgres

## 1. Purpose

将现有 `sandbox_session`、`sandbox_session_operation`、`sandbox_runtime_binding`、`sandbox_session_lease` 四张已注册表上的 `tenant_id` 字段从 `TEXT` 类型规范迁移到 `BIGINT`。同时解决租户业务主键、外部 ID 映射与内部存储层表达分离。

当前 TEXT `tenant_id` 保持了灵活性但破坏了以下约束：
- 索引与 Join 性能依赖 text_pattern_ops 或更宽泛的对比函数。
- 下游 Billing / Quota 不能通过 `tenant_id BIGINT FK` 强制引用一致。
- 与 sdkwork-kernel 其它模块的 Tenant 主键策略不兼容。

## 2. Affected Tables

| 表名 | 当前 tenant_id 类型 | 目标类型 |
| --- | --- | --- |
| `sandbox_session` | TEXT NOT NULL | BIGINT NOT NULL |
| `sandbox_session_operation` | TEXT NOT NULL | BIGINT NOT NULL |
| `sandbox_runtime_binding` | TEXT NOT NULL | BIGINT NOT NULL |
| `sandbox_session_lease` | TEXT NOT NULL | BIGINT NOT NULL |

这四张表由 `database/contract/table-registry.json` 登记为唯一活跃清单；本迁移不得引入第五张表。Runtime Binding 的 intent 字段位于 `sandbox_runtime_binding`，不存在独立的 binding-intent 表。

后续 REQ-2026-0018 新增的四张表 (`sandbox_quota_state`、`sandbox_admission_reservation`、`sandbox_capacity_reservation`、`sandbox_node_capacity`) 直接以 `tenant_id BIGINT NOT NULL` 创建，不在本迁移修改范围。

## 3. Migration Strategy

采用 BIGINT 主键表 + 逐步切流 + 映射表的“双写 + 映射”策略：

1. 新增 `sandbox_tenant_mapping(tenant_id BIGINT PK, external_tenant_id TEXT UNIQUE NOT NULL)` 作为业务租户到内部 BIGINT 的唯一映射。
2. 新增 `tenant_id_bigint BIGINT` 兼容列到每一张受影响表；写路径先写 TEXT 后补写 BIGINT，读路径优先 TEXT 列、BIGINT 列空时回查映射表回填。
3. 批量回填 `tenant_id_bigint`：分批 SELECT DISTINCT tenant_id，通过 `sandbox_tenant_mapping` 回填。
4. 切流验证双写一致性后，将外键与索引从 `tenant_id TEXT` 切换到 `tenant_id_bigint`。
5. 稳定一个 Release 后删除 `tenant_id TEXT` 列。

全部变更需在线执行、有限锁（批处理窗口 ≤ 每批 1000 行）、始终维护前向回滚脚本。

## 4. Reversibility

按 `forward-fix` 模式设计：新增列、新增映射表与切换阶段都可以 forward-fix 直接修正。删除 TEXT 列最后阶段标记 `reversible: false`；执行前必须完成数据校验快照。

Rollback：
- 切流阶段：写双端都保留；反向切换入口表。
- 删列阶段：不可逆，恢复只能依赖 pg_dump 快照或备份恢复。

## 5. Validation

- `pg_is_in_replication` 期间禁止迁移改变。
- 每个批次后 SELECT count(*) 校验 tenant_id 与 tenant_id_bigint 分布相等。
- 迁移后 EXPLAIN ANALYZE 验证 BIGINT 索引在 `SandboxSession` Join 上的 cardinality 与估算一致。
- 业务 E2E：Allocate → Start → Reconcile 全流程通过。

## 6. Dependencies

- `database/ddl/baseline/postgres/0001_sandbox_baseline.sql`（已物化；`baseline-plus-migrations` 的不可变引导锚点，原 `0001_create_sandbox_lifecycle.up.sql` 已合并至此）
- `0002_tenant_id_mapping.up.sql`（本迁移前置新增；将成为 `database/migrations/postgres/` 下第一条 post-baseline 有序迁移）
- `REQ-2026-0018` 四表创建（后续迁移，不在 MIG-2026-0001 内执行）

## 7. Evidence

迁移 Owner 必须记录：
- 批处理窗口起止时间与行数
- BIGINT 列 NOT NULL 切换前后的 vacuum 与 index bloat 快照
- 双写一致性抽样报告（≥ 1% 行数）
- 回滚演练日志