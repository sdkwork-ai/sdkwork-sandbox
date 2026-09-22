---
id: REQ-2026-0002
title: Deliver the provider-neutral sandbox lifecycle core
owner: SDKWork Runtime Platform
status: accepted
source: platform
problem: The Phase 0 repository has no executable lifecycle contract, provider negotiation, tenant-scoped session state, or evidence that retries avoid duplicate provider side effects.
goals:
  - Establish a provider-neutral lifecycle contract for create, start, stop, and destroy.
  - Enforce capability and isolation-assurance requirements without weaker-provider fallback.
  - Prove state transitions, idempotency, tenant scoping, and optimistic concurrency with behavior tests.
non_goals:
  - Execute host commands or expose filesystem, terminal, network, browser, Git, build, or port capabilities.
  - Claim hardened local or multi-tenant isolation.
  - Add HTTP, RPC, generated SDK, durable database, distributed coordination, recovery, or release packaging.
users:
  - SDKWork Runtime Platform maintainers
  - Sandbox provider authors
  - Future SDKWork Kernel integrators
affected_surfaces:
  - rust-components
  - backend
  - composition
---

# REQ-2026-0002: 交付 Provider-neutral Sandbox 生命周期核心

## 验收标准

- Provider SPI 暴露稳定 Provider Identity、Kind、Capability、Isolation Assurance、Health，以及 Allocate、Start、Stop、Destroy 异步端口。
- `TenantId`、`SandboxWorkspaceId`、`SandboxSessionId`、`SandboxId`、`SandboxRuntimeBindingId` 与 `OperationId` 使用不同不透明类型；Provider 私有 `SandboxProviderAllocationRef` 不可序列化，Debug 输出必须脱敏。
- `SandboxWorkspaceId` 与 `SandboxSessionId` 必须由 Kernel/调用方提供，Sandbox 不生成第二套 Workspace 或 Session Identity；`CreateSandboxSessionCommand` 使用 `sandbox_workspace_id`、`sandbox_session_id` 与 `sandbox_operation_id`。
- `SandboxSession` 实现 `Created -> Starting -> Running -> Stopping -> Stopped -> Destroying -> Destroyed` 以及显式失败路径；非法迁移返回 Typed Error。
- 只有 Provider Ready、Policy Enforced 与 Workspace Attached 同时成立时才能进入 `Running`。
- Provider 选择同时满足全部 Required Capability 和 Minimum Isolation Assurance；无合格 Provider 或健康 Provider 时拒绝 Start，禁止静默降级。
- 同一 `sandbox_operation_id` 的 Create、Start、Stop 或 Destroy 重试不得再次调用 Provider；同一 ID 用于不同操作时返回 Idempotency Conflict。
- Retry Start 在复用失败 Session 前先释放旧 Allocation，避免同一 Runtime Binding 产生两个活动所有者。
- `SandboxSessionRepository` 强制 Tenant Scope，并使用 Version Compare-and-swap 防止静默覆盖并发更新。
- Memory Repository Adapter 实现 Create Operation 唯一性、Tenant 隔离、Version Conflict 和 Not Found 行为，且仅用于测试和单进程开发组合。
- `SandboxProvider`、`SandboxSessionRepository` 和 `SandboxLifecycleError` 都是 Typed Error；外部错误不得暴露 Provider 私有 Reference、Host Path 或 Secret。
- Crate-level Behavior Test 覆盖成功生命周期、非法迁移、幂等重试、Capability/Assurance 拒绝、Readiness 拒绝、Tenant 隔离、Provider Failure Cleanup 与并发 Version Conflict。
- Cargo Format、Check、Test、Clippy 以及 SDKWork Component/Layering/Naming 检查通过。

## 非功能需求

| 领域 | 要求 |
| --- | --- |
| Security | 默认拒绝不满足 Capability/Assurance 的 Provider；Provider 私有引用不出现在 Debug/Error；本切片不访问 Host、Network 或 Secret。 |
| Privacy | `SandboxSessionRepository` 的所有读取和写入都带 `tenant_id`；测试只使用合成标识。 |
| Performance | 单进程 Memory Repository 的单次 Get/Save 不做无界扫描；本切片不发布吞吐或延迟 SLO。 |
| Reliability | Lifecycle Command 使用 Optimistic Version 和 Operation Idempotency；进程崩溃恢复与 Durable Reconciliation 在后续 Requirement 处理。 |

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`, `COMPONENT_SPEC.md`, `CODE_STYLE_SPEC.md`, `NAMING_SPEC.md`, `RUST_CODE_SPEC.md`, `SECURITY_SPEC.md`, `TEST_SPEC.md`.

Components: `crates/sdkwork-sandbox-provider-spi`, `crates/sdkwork-intelligence-sandbox-service`, `crates/sdkwork-intelligence-sandbox-repository-memory`.

Decision: [ADR-20260728: Sandbox Lifecycle, Provider SPI And In-memory Store](../../architecture/decisions/ADR-20260728-sandbox-lifecycle-provider-spi-and-memory-store.md).

## Verification

```bash
cargo test -p sdkwork-sandbox-provider-spi
cargo test -p sdkwork-intelligence-sandbox-service
cargo test -p sdkwork-intelligence-sandbox-repository-memory
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root . --strict
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-identity-naming.mjs --root .
node ../sdkwork-specs/tools/check-rust-backend-composition.mjs --root .
```

## Release And Review Boundary

本需求不激活应用发布面，也不创建 `sdkwork.app.config.json`。Provider SPI、`SandboxSessionLifecyclePort` 与 Memory Repository 的公共命名在人工架构评审前保持 `0.1` 候选状态；Kernel 跨仓库映射由 REQ-2026-0004 管理，真实 Local/Firecracker Provider、Production Persistence 和 API/SDK 分别由后续 Ready Requirement 与 ADR 管理，Docker Provider 当前延期。

## Current Evidence

仓库内生命周期实现与行为测试已在 2026-07-28 通过 Cargo、Clippy、Strict Component Binding、Layering、Naming、Composition 和 Repository Verification；`REQ-2026-0005` 又扩展了 PostgreSQL Repository、Lease/Fencing、Provider Timeout 与 Reconciler 测试。

公共命名（`SandboxSession`、`SandboxSessionLifecyclePort`、`SandboxProvider` 等）已在 2026-07-29 通过跨仓库集成验证，确认 `sdkwork-agents -> sdkwork-kernel -> sdkwork-sandbox` 依赖方向与现有实现一致，无 Sandbox 到 Kernel/Agents 反向边。

原始切片证据见 [REVIEW-20260728](../../engineering/reviews/REVIEW-20260728-sandbox-lifecycle-core-verification.md)，持久化证据以 `REQ-2026-0005` 为准。本需求的所有技术验收标准已满足，于 2026-07-29 更新为 `accepted`。后续生产部署需额外完成 Service Host Secret/KMS 接入与真实 Provider Conformance。
