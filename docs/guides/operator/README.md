# Operator Guide

Deployment, monitoring, and incident response entrypoints.

See `DOCUMENTATION_SPEC.md` section 2.

## 概述

本指南面向 SDKWork Sandbox 平台运维与开发 (DevOps/SRE/PlatOps)，涵盖：

- 部署拓扑与运行时配置
- 监控、告警与日志收集
- 故障分类与恢复流程
- 配额与多租户调度
- 密钥轮换与 Provider 分配撤销

## Gate 0 运营边界

在 `implementationAuthorized: false` 期间，Operator 不得：

- 部署任何需要与外部 Provider Node 交互的组件
- 启用非 sandbox-provider-local 的 Provider
- 执行生产规模的 SaaS 准入 / 调度 / 配额 / 计费流程
- 泄露 Tenant / Allocation Reference / Key Material 到任何日志/事件系统

**Gate 0 期间允许工作**：

- 本地 cargo check + test 验证
- 文档与 Runbook 评审
- 监控面板 Schema 设计 (不部署)

## 部署拓扑

### 本地开发

```
┌──────────────┐
│ SDKWork CLI  │──┐
├──────────────┤  │         ┌─────────────────────────────┐
│ Service Host │──┼──────── │ PostgreSQL (本地)          │
├──────────────┤  │         └─────────────────────────────┘
│ Service      │──┘
├──────────────┤
│ SPI / Local  │
└──────────────┘
```

### V1 Local Runtime (当前)

```
┌──────────────┐
│ SDKWork CLI  │── 本地 TCP / 共享内存 ──┐
└──────────────┘                         │
                                         ▼
                                ┌─────────────────────┐
                              │ Service Host (单进程)  │
                                ├─────────────────────┤
                              │ Service + Memory/Postgre│
                                └─────────────────────┘
                              (依赖 PostgreSQL 作为持久化)
```

### V2 Isolated Cloud Runtime (待建设)

```
┌──────────────┐        HTTPS/gRPC         ┌─────────────────────┐
│ SDKWork CLI  │  ────-───────────────> │ Service Host (多副本)  │
└──────────────┘                         ├─────────────────────┤
                                       │ Service + PostgreSQL    │
                                         └─────────────────────┘
```

### V3 Elastic Platform (路线图)

```
 ┌──────────┐      ┌────────────────────────────────────────────────┐
│ Clients  │ ->  │ LB/API Gateway -> Service Host Pool              │
 └──────────┘      ├────────────────────────────────────────────────┤
                 │ Service Pool    │  Provider Registry               │
                 ├─────────────────┼─────────────────────────────────┤
                 │ Repository Pool │  Event Bus                       │
                 └─────────────────┴─────────────────────────────────┘
```

## 运行时配置

### 环境变量

| 变量名 | 默认 | 用途 | 敏感 |
| --- | --- | --- | --- |
| `SDKWORK_DATABASE_URL` | N/A | PostgreSQL 连接字符串 | Yes |
| `SDKWORK_SANDBOX_HTTP_PORT` | 8080 | API Server 监听端口 | No |
| `SDKWORK_SANDBOX_HTTP_BIND` | 127.0.0.1 | API 监听地址 | No |
| `SDKWORK_SANDBOX_LOG_LEVEL` | info | 日志级别 (debug/info/warn/error) | No |
| `SDKWORK_SANDBOX_PROVIDER_CONFIG_PATH` | ./etc/provider | Provider Profile 配置 | No |
| `SDKWORK_SANDBOX_SECRET_FROM_ENV` | N/A | Secret 注入源 (env/file/vault) | Yes |
| `SDKWORK_SANDBOX_KMS_KEY_ID` | N/A | KMS 主密钥 ID | No |

### 启动参数

```
Usage: sdkwork-sandbox-service-host [OPTIONS]

Options:
  -c, --config <FILE>              Configuration file path
      --database-url <URL>          PostgreSQL connection URL
      --bind <ADDR:PORT>            Socket address
      --provider <NAME>             Default provider ID [default: local]
      --log-level <LEVEL>           Log level flag
      --disable-metrics             (Gate 0 reserved) 禁用 metrics
      --disable-audit               (Gate 0 reserved) 禁用 audit
```

## 监控

### Metrics (OpenTelemetry)

| Metric 名 | 类型 | 单位 | 描述 |
| --- | --- | --- | --- |
| `sandbox.session.active` | UpDownCounter | - | 活跃 Session 数 |
| `sandbox.session.start.duration` | Histogram | duration | Session 启动耗时 |
| `sandbox.session.end.duration` | Histogram | duration | Session 结束耗时 |
| `sandbox.lease.acquire.duration` | Histogram | duration | Lease 获取耗时 |
| `sandbox.fencing.conflict.count` | Counter | - | 版本冲突次数 |
| `sandbox.repository.query.duration` | Histogram | duration | Repository 查询耗时 |
| `sandbox.provider.status` | Gauge | 0/1 | Provider 健康状态 |

### 日志规范

```
[INFO]  sandbox.session.start tenant=t_xxx ws=ws_yyy
[WARN]  sandbox.lease.expired session=s_xxx age=1250ms
[ERROR] sandbox.fencing.conflict session=s_xxx expected=42 got=43
```

- 日志恒为 `info` / `warn` / `error` 三级
- 敏感字段 (Tenant ID、Allocation Ref、Key Material) 全部 redact
- Trace ID 与 Span ID 通过 tracing context 透传

### 告警规则

| 告警 | 条件 | 严重度 |
| --- | --- | --- |
| `sandbox_session_start_failure_rate` | Error rate > 1% for 5min | P1 |
| `sandbox_fencing_conflict_rate` | Conflict rate > 5% for 3min | P2 |
| `sandbox_repository_query_latency_p99` | p99 > 500ms for 5min | P2 |
| `sandbox_provider_unavailable` | Provider 0/1 for 2min | P0 |

## 故障响应

### 故障分类

| 分类 | 触发 | 响应 | Runbook |
| --- | --- | --- | --- |
| `P0` | Provider / Repository / KMS 不可用 | 立即 pause + 值班 | `sandbox-provider-failure-recovery.md` |
| `P1` | 系统级错误率升高 | 自动熔断 + 人工介入 | `sandbox-provider-failure-recovery.md` |
| `P2` | 单点 Lease/Session 异常 | 自动重试 + 审计 | `sandbox-cli-destructive-operation.md` |
| `P3` | 配额 / 策略异常 | 工单系统 | workflow ticket |

### 常见 Recovery 流程

```
  异常信号
      │
      ▼
  识别异常来源 (Provider / Repository / Session / Fencing)
      │
      ▼
  执行对应 Runbook (Pause → 诊断 → 修复 → Resume)
      │
      ▼
  记录 Incident 与补救措施
```

## 配额与调度

### 租户配额维度

| 维度 | 描述 | 默认值 |
| --- | --- | --- |
| `max_active_sessions` | 同时活跃 Session 数 | 10 |
| `max_sessions_per_minute` | 每分钟新建 Session | 60 |
| `max_runtime_duration` | 单个 Session 最长运行时间 | 1h |
| `max_memory_per_session` | 每 Session 最大内存 | 1GB |
| `max_cpu_per_session` | 每 Session CPU share | 100m |
| `max_disk_per_session` | 每 Session 持久化 | 10GB |

### Provider 调度流程

```
 Tenant 请求
      │
      ▼
 SandboxProviderRegistry.match(tenant, requirement)
      │
      ▼
 Provider.allocate(tenant_id) -> SandboxProviderAllocation
      │
      ▼
 Binding (Tenant, Provider, Lease, Fencing)
      │
      ▼
 Session.lifecycle(Start / Execute / End)
```

## 密钥轮换 (Provider Allocation Key)

详情见 [对应 Runbook](../../runbooks/RUNBOOK-sandbox-provider-allocation-key-rotation.md)。

关键原则：

- 禁止 Ad-hoc SQL 改写 Ciphertext / Key ID / Key Version / Crypto Version
- 必须通过 `SandboxProviderAllocationProtector` 与 Repository CAS
- 必须在 KMS 保持 Historical Key 可解密时才可推进
- 每次轮换必须全 Tenant Scope 与 Full Dry Verification
- 所有步骤必须留痕 (Audit event / Failure Queue / Recovery Smoke Evidence)

## CLI 破坏性操作保护

详情见 [对应 Runbook](../../runbooks/RUNBOOK-sandbox-cli-destructive-operation.md)。

核心不变量：

- `session.end` / `provider.release` 必须确认
- 客户端 Fencing Token 丢失可被修复，服务端 Fencing Token 不会
- 所有破坏性操作必须记录与 Trace 关联
- 幂等复本与错误合并不会被允许 (若存在歧义 → 停止)

## 参考

- [技术架构概述](../../architecture/tech/TECH_ARCHITECTURE.md)
- [Traceability Map](../../architecture/views/traceability-map.md)
- [部署视图](../../architecture/views/deployment-view.md)
- [故障恢复 Runbook](../../runbooks/RUNBOOK-sandbox-provider-failure-recovery.md)
- [CLI 破坏性操作 Runbook](../../runbooks/RUNBOOK-sandbox-cli-destructive-operation.md)
- [`SECURITY_SPEC`](../../../../sdkwork-specs/SECURITY_SPEC.md)
- [`OBSERVABILITY_SPEC`](../../../../sdkwork-specs/OBSERVABILITY_SPEC.md)
