# Integrator Guide

SDK consumption, API boundaries, and integration examples.

See `DOCUMENTATION_SPEC.md` section 2.

## 概述

本指南面向需要在以下场景与 sdkwork-sandbox 对接的团队：

- 其他 Kernel (如 sdkwork-specs、sdkwork-cli、sdkwork-realtime) 集成 Sandbox 能力
- 下游 SDK 消费生成的 SDK stub
- Serverless/FaaS Runtime 使用 Sandbox 进行代码执行
- SaaS 平台对接 Sandbox Provider

## 集成边界

### L0 契约消费

| 契约类型 | 路径 | 用途 |
| --- | --- | --- |
| API Command Schema | `apis/commands/*.schema.json` | 命令请求/响应序列化 |
| API Event Schema | `apis/async/*.schema.json` | Outbox 事件订阅 |
| Machine Contract | `specs/*.contract.json` | 组件级 Gate 验证 |

上游仓库**必须**通过以下方式消费契约：

```json
{
  "$ref": "https://sdkwork.internal/api/commands/sandbox-command-execution-request.schema.json"
}
```

**禁止**直接读取源文件作为契约；禁止 fork/copy 契约到本地（使用 repository 引用或发布 artifact）。

### L3 Provider SPI 消费

```rust
use sdkwork_sandbox_provider_spi::SandboxProvider;

#[async_trait::async_trait]
impl SandboxProvider for MyProvider {
    async fn allocate(&self, tenant_id: TenantId) -> Result<SandboxProviderAllocation, SandboxProviderError> { ... }
    async fn describe(&self, allocation_ref: SandboxProviderAllocationRef) -> Result<SandboxProviderDescriptor, SandboxProviderError> { ... }
    async fn release(&self, allocation_ref: SandboxProviderAllocationRef) -> Result<(), SandboxProviderError> { ... }
}
```

### L2 Service 消费

通过 SDK stub 调用 L2 Service Port：

```rust
use sdkwork_sandbox_lifecycle::SandboxLifecyclePort;

let sandbox = sandbox_lifecycle.start(
    req.tenant_id,
    req.provider_id,
    req.workspace_id,
).await?;
```

## SDK 集成

### Rust SDK

```toml
# Cargo.toml
[dependencies]
sdkwork-sandbox = { version = "0.1.0", registry = "sdkwork" }
```

```rust
use sdkwork_sandbox::prelude::*;

let client = SandboxClient::with_provider(MyProvider::new());
let session = client.start_session(StartSessionRequest {
    tenant_id,
    provider_id,
    workspace_id,
    ..Default::default()
}).await?;
```

### Web/JS SDK

```js
import { SandboxClient } from '@sdkwork/sandbox-js';

const client = new SandboxClient({ endpoint: 'https://sandbox.api.internal' });
const session = await client.startSession({
  tenantId: 't_xxx',
  providerId: 'local',
  workspaceId: 'ws_yyy',
});
```

## 跨仓库集成模式

### 模式 1: 直接 Port 集成

```
┌───────────────┐     ┌─────────────────────┐
│ Kernel Repo │ --> │ sdkwork-sandbox     │
│ (lifecycle────┘      └─────────────────────┘
```

上游仓库直接依赖 `sdkwork-sandbox-provider-spi` crate。

### 模式 2: 间接 SDK 集成

```
┌───────────────┐     ┌──────────────┐     ┌─────────────────────┐
│ Kernel Repo │ --> │ Generated    │ --> │ sdkwork-sandbox     │
│ (lifecycle────┘      └──────────────┘      └─────────────────────┘
                     (SDK Stub)
```

上游仓库通过 published SDK stub 与 Sandbox 交互。

### 模式 3: HTTP API 集成

```
┌───────────────┐     HTTP      ┌─────────────────────┐
│ Kernel Repo │ --> (REST) --> │ sdkwork-sandbox     │
│ (lifecycle────┘      JSON        └─────────────────────┘
```

上游仓库通过 HTTP/JSON 调用 Published Command API。

## 类型映射

### Rust <-> JS

| Rust 类型 | JS 类型 | 备注 |
| --- | --- | --- |
| `SandboxId` (newtype Uuid) | `string` (uuid format) | 自动序列化 |
| `SandboxSessionState` | 如 schema 定义的 enum | camelCase variants |
| `SandboxFencingToken` (zeroized) | opaque `string` | 不 leak token bytes |
| `SandboxProviderAllocationRef` (SecretCarrier) | ❌ 永远不序列化 | 仅 inner 句柄可见 |

### Rust <-> REST API

遵循 JSON Schema 规范：
- UUID 使用 8-4-4-4-12 格式
- DateTime 使用 RFC 3339
- byte arrays 使用 base64
- 敏感字段在 HTTP 响应中恒为 null

## Repository Interaction

### Write Path

```
[Runtime] -> Lease Acquire -> Fencing CAS -> Save
         -> Audit -> Metrics -> Event -> Outbox
```

### Read Path

```
[Runtime] -> Lease Acquire -> Read (with Fencing Validation)
         -> Audit [optional] -> Response
```

### 并发控制

- 使用 Fencing Token 避免 ABA (compare-and-swap on version column)
- Lease 过期自动失效 (由 Tokio background task 清理)
- DB 行锁仅在 Lease Active 期间应用

## 错误处理

### 错误类型

| 错误 | 含义 | Retryable |
| --- | --- | --- |
| `SandboxSessionTerminated` | 会话已终止 | No |
| `SandboxLeaseExpired` | Lease 已过期 | Yes (重连) |
| `SandboxFencingConflict` | 版本冲突 | Yes (重新读取) |
| `SandboxProviderUnavailable` | Provider 不可用 | Yes (带 backoff) |
| `SandboxQuotaExceeded` | 配额耗尽 | No (需人工) |
| `SandboxPolicyDenied` | 策略拒绝 | No (需修正请求) |

### 重试策略

```
ExponentialBackoff {
    initial_interval: 100ms,
    max_interval: 5s,
    multiplier: 2.0,
    max_attempts: 5,
    jitter: 0.1,
}
```

### 幂等性

- 所有 POST 命令支持 `Idempotency-Key`
- 重复请求返回相同 response (200 / 201)
- 幂等窗口默认 24 小时

## 集成检查清单

```text
- [ ] 确认 sdkwork-sandbox 版本与契约兼容
- [ ] 导入类型映射表并验证 UUID / DateTime / byte 序列化
- [ ] Token / Allocation Ref 永不 log、永不持久化到未加密层
- [ ] 错误处理覆盖全部可 retryable 错误
- [ ] 配置 Sandbox Provider Registry 与 fallback provider
- [ ] 集成 contract testing (Node.js 契约测试套件)
- [ ] 集成 conformance testing (SDKWork Common Conformance)
- [ ] 人工架构评审并通过
```

## 常见错误

| 错误 | 原因 | 修复 |
| --- | --- | --- |
| Allocation Ref 泄露到日志 | Debug 未 redact | 检查 `#[redact("secret")]` 标注 |
| 契约 schema 过期 | 上游 sandbox 更新 | 重新拉取最新 schema 并刷新本地映射 |
| 并发冲突 | 未正确处理 fencing token | 使用提供的 Token 做 CAS，不可自生成 |
| Provider 注入失败 | Provider Registry 未配置 | 在 Composition 已实现后注入 `SandboxProvider` |

## 参考

- [技术架构概述](../../architecture/tech/TECH_ARCHITECTURE.md)
- [Traceability Map](../../architecture/views/traceability-map.md)
- [Command Contract](../../../apis/commands/README.md)
- [`API_SPEC`](../../../../sdkwork-specs/API_SPEC.md)
- [`INTERNAL_API_SPEC`](../../../../sdkwork-specs/INTERNAL_API_SPEC.md)
