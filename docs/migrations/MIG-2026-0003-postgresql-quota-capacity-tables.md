# MIG-2026-0003: PostgreSQL Quota And Capacity Reservation Tables (REQ-2026-0018)

Status: candidate
Owner: SDKWork Runtime Platform
Date: 2026-07-29
Requirement: REQ-2026-0018 (Sandbox PostgreSQL Quota And Capacity Reservation Persistence)
Module: sandbox
Engine: postgres
Adr: docs/architecture/decisions/ADR-20260729-sandbox-postgresql-quota-and-capacity-reservation-persistence.md
Contract: specs/sandbox-quota-and-capacity-persistence.contract.json
Review: docs/engineering/reviews/REVIEW-20260729-sandbox-postgresql-quota-and-capacity-persistence.md

## 1. Purpose

按 REQ-2026-0018 物化 Quota/Admission/Capacity/Node 四张 PostgreSQL 持久化表：`sandbox_quota_state`（租户配额开销事实）、`sandbox_admission_reservation`（Admission 原子预留）、`sandbox_capacity_reservation`（Provider Allocate 前原子预留）、`sandbox_node_capacity`（Fabric 节点当前剩余能力）。

当前这些对象只存在于 draft `sandbox-quota-and-capacity-persistence.contract.json`，仅供 Human Review 输入；本迁移把它们落地为 PostgreSQL 存储，并保持与 MIG-2026-0001 (tenant_id BIGINT)、MIG-2026-0002 (Provider Registry) 一致性。

## 2. New Tables

### 2.1 `sandbox_quota_state`

```sql
CREATE TABLE sandbox_quota_state (
    tenant_id             BIGINT      NOT NULL,
    quota_name            TEXT        NOT NULL,
    quota_dimension       TEXT        NOT NULL,
    quota_scope           TEXT        NOT NULL,
    quota_used            BIGINT      NOT NULL,
    quota_reserved        BIGINT      NOT NULL,
    quota_limit           BIGINT      NOT NULL,
    protection_version    BIGINT      NOT NULL,
    last_reservation_id   TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT pk_sandbox_quota_state PRIMARY KEY (tenant_id, quota_name),
    CONSTRAINT ck_quota_dimension
        CHECK (quota_dimension IN ('instance_count', 'cpu_cores',
                                    'memory_bytes', 'storage_bytes',
                                    'session_seconds')),
    CONSTRAINT ck_quota_scope
        CHECK (quota_scope IN ('tenant', 'workspace', 'session')),
    CONSTRAINT ck_quota_monotone_amounts
        CHECK (quota_used >= 0 AND quota_reserved >= 0
               AND quota_limit >= 0),
    CONSTRAINT ck_quota_reserved_not_exceed_used
        CHECK (quota_reserved <= quota_used),
    CONSTRAINT ck_quota_used_not_exceed_limit
        CHECK (quota_used <= quota_limit)
);
```

### 2.2 `sandbox_admission_reservation`

```sql
CREATE TABLE sandbox_admission_reservation (
    tenant_id               BIGINT      NOT NULL,
    admission_reservation_id TEXT       NOT NULL,
    reservation_state       TEXT        NOT NULL,
    requested_dimensions    JSONB       NOT NULL,
    granted_dimensions      JSONB,
    reserve_fencing_token   TEXT        NOT NULL,
    reservation_version     BIGINT      NOT NULL,
    expires_at              TIMESTAMPTZ NOT NULL,
    resolved_at             TIMESTAMPTZ,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT pk_admission_reservation
        PRIMARY KEY (tenant_id, admission_reservation_id),
    CONSTRAINT ck_reservation_state
        CHECK (reservation_state IN ('pending', 'granted',
                                      'expired', 'released')),
    CONSTRAINT ck_admission_expires_after_creation
        CHECK (expires_at > created_at),
    CONSTRAINT ck_granted_dimensions_required_when_granted
        CHECK (reservation_state <> 'granted'
               OR granted_dimensions IS NOT NULL)
);

CREATE INDEX idx_admission_reservation_evict
    ON sandbox_admission_reservation (tenant_id, expires_at)
    WHERE reservation_state = 'pending';
```

### 2.3 `sandbox_capacity_reservation`

```sql
CREATE TABLE sandbox_capacity_reservation (
    tenant_id                BIGINT      NOT NULL,
    capacity_reservation_id  TEXT        NOT NULL,
    reservation_state        TEXT        NOT NULL,
    provider_id              TEXT        NOT NULL,
    requested_dimensions     JSONB       NOT NULL,
    granted_dimensions       JSONB,
    granted_provider_binding TEXT,
    reserve_fencing_token    TEXT        NOT NULL,
    reservation_version      BIGINT      NOT NULL,
    expires_at               TIMESTAMPTZ NOT NULL,
    resolved_at              TIMESTAMPTZ,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT pk_capacity_reservation
        PRIMARY KEY (tenant_id, capacity_reservation_id),
    CONSTRAINT fk_capacity_provider
        FOREIGN KEY (provider_id)
        REFERENCES sandbox_provider_profile (provider_id)
        ON DELETE RESTRICT,
    CONSTRAINT ck_capacity_state
        CHECK (reservation_state IN ('pending', 'granted', 'bound',
                                      'expired', 'released')),
    CONSTRAINT ck_capacity_expires_after_creation
        CHECK (expires_at > created_at)
);

CREATE INDEX idx_capacity_reservation_evict
    ON sandbox_capacity_reservation (tenant_id, expires_at)
    WHERE reservation_state IN ('pending', 'bound');
```

### 2.4 `sandbox_node_capacity`

```sql
CREATE TABLE sandbox_node_capacity (
    provider_id           TEXT        NOT NULL,
    node_id               TEXT        NOT NULL,
    dimension_name        TEXT        NOT NULL,
    total_quantity        BIGINT      NOT NULL,
    allocated_quantity    BIGINT      NOT NULL,
    reserved_quantity     BIGINT      NOT NULL,
    unhealthy_flag        BOOLEAN     NOT NULL DEFAULT FALSE,
    protection_version    BIGINT      NOT NULL,
    inventory_fencing_token TEXT      NOT NULL,
    last_heartbeat_at     TIMESTAMPTZ NOT NULL,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT pk_sandbox_node_capacity
        PRIMARY KEY (provider_id, node_id, dimension_name),
    CONSTRAINT fk_node_provider
        FOREIGN KEY (provider_id)
        REFERENCES sandbox_provider_profile (provider_id)
        ON DELETE CASCADE,
    CONSTRAINT ck_node_dimension
        CHECK (dimension_name IN ('cpu_cores', 'memory_bytes',
                                   'storage_bytes', 'instance_slots')),
    CONSTRAINT ck_node_allocation_bounds
        CHECK (allocated_quantity + reserved_quantity <= total_quantity
               AND allocated_quantity >= 0 AND reserved_quantity >= 0)
);

CREATE INDEX idx_node_capacity_healthy
    ON sandbox_node_capacity (provider_id, dimension_name,
                              unhealthy_flag)
    WHERE unhealthy_flag = FALSE;
```

## 3. REVOKE Strategy（RLS/Role）

按 REQ-2026-0018 与 `ADR-20260729-sandbox-postgresql-quota-and-capacity-reservation-persistence`：

```sql
ALTER TABLE sandbox_quota_state           ENABLE ROW LEVEL SECURITY;
ALTER TABLE sandbox_admission_reservation ENABLE ROW LEVEL SECURITY;
ALTER TABLE sandbox_capacity_reservation  ENABLE ROW LEVEL SECURITY;

CREATE POLICY tenant_isolation_quota
    ON sandbox_quota_state
    USING (tenant_id = current_setting('app.current_tenant')::BIGINT)
    WITH CHECK (tenant_id = current_setting('app.current_tenant')::BIGINT);

-- admission/capacity 策略同模式，均基于 tenant_id 隔离。
-- node_capacity 不启用 RLS，由 provider_id 服务身份访问控制。

GRANT SELECT, INSERT, UPDATE ON sandbox_quota_state,
                                  sandbox_admission_reservation,
                                  sandbox_capacity_reservation
    TO sandbox_service;

GRANT SELECT ON sandbox_node_capacity TO sandbox_service;
```

## 4. Reversibility

`reversible: false`（新表上线）。回滚通过 DROP 实现；数据保留需 pg_dump 或逻辑复制备份。

Rollback: `DROP` 顺序遵循 FK 反依赖：
1. `sandbox_capacity_reservation`
2. `sandbox_admission_reservation`
3. `sandbox_quota_state`
4. `sandbox_node_capacity`（在 sandbox_provider_binding 后 DROP）

## 5. Validation

- 静态 seed 数据：每个表插入 1 行人工构造的 tenant_id=0 记录，校验 CHECK 约束。
- 违反约束场景：`quota_used > quota_limit` → 错误代码 23514。
- RLS 场景：`current_setting('app.current_tenant')` 切换时无法读取其它 tenant 行。
- CAS UPDATE：`UPDATE ... WHERE version = :old_version` 行数必须为 1，否则程序判定冲突。
- Expires 字段：`expires_at <= now()` 时由清理任务将状态翻转为 `expired`。

## 6. Dependencies

- MIG-2026-0001 (tenant_id BIGINT)
- MIG-2026-0002 (sandbox_provider_profile)
- `REQ-2026-0018` Review Approved + ADR Accepted
- `sandbox_provider_profile` seed 数据就绪后才能启动 `sandbox_capacity_reservation` FK 校验。

## 7. Evidence

- RLS 隔离测试：跨 tenant 拒绝 + 同 tenant 允许。
- CAS 冲突测试：dual-writer 仅一个成功。
- Expiry orphan 回收：定时任务将过期 pending 状态转为 `expired` 同时释放 quota_reserved。
- FK Registry：未在 `sandbox_provider_profile` 登记的 provider_id 不能插入 reservation。
- Lock/Statement timeout 与 VACUUM 窗口报告。