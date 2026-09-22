# Developer Quick Start

Status: active

## Prerequisites

- Rust 1.92+ (via rustup)
- Node.js 18+ (for contract tests)
- PostgreSQL 17+ (optional, for repository tests)

## Build

```bash
git clone <sdkwork-sandbox-url>
cd sdkwork-sandbox

# Format check
cargo fmt --check

# Compile
cargo check --workspace

# Run all tests
cargo test --workspace

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Contract tests
node --test tests/contract/*.test.mjs
```

## Project Structure

```
sdkwork-sandbox/
  crates/
    sdkwork-sandbox-provider-spi/      # L3 Provider Port (SandboxProvider)
    sdkwork-intelligence-sandbox-service/  # L2 Lifecycle Service
    sdkwork-intelligence-sandbox-repository-memory/  # L4 Memory Repository
    sdkwork-intelligence-sandbox-repository-sqlx/    # L4 PostgreSQL Repository
    sdkwork-sandbox-provider-local/    # L4 Local Provider (Gate 0 only)
    sdkwork-sandbox-service-host/      # L5 Composition (inactive)
    sdkwork-sandbox-cli/               # L6 CLI (inactive)
  apis/
    commands/                          # Command contract schemas
    async/                            # Event/Outbox schemas
  specs/                               # Machine contracts
  database/ddl/baseline/postgres/      # Immutable bootstrap baseline (0001_sandbox_baseline.sql)
  database/migrations/postgres/        # Post-baseline ordered migrations (empty at initialization)
```

## Architecture Overview

SDKWork Sandbox 采用 L0-L6 分层架构：

| 层级 | 路径 | 职责 |
| --- | --- | --- |
| L0 | apis/ | 契约定义 |
| L3 | provider-spi | Provider-neutral Port |
| L2 | service | 业务逻辑 |
| L4 | repository-*, provider-* | 适配实现 |
| L5 | service-host | 组合编排 |
| L6 | cli | 交付入口 |

## Key Types

```rust
// Identity
SandboxId, SandboxSessionId, SandboxWorkspaceId, SandboxProviderId, TenantId

// Lifecycle
SandboxProvider, SandboxProviderDescriptor, SandboxProviderAllocation
SandboxSession, SandboxSessionState, SandboxSessionLease

// Security
SandboxFencingToken, SandboxProviderAllocationRef (zeroized, redacted)
```

## Testing Strategy

1. **Unit Tests**: 每个 crate 内部 `#[cfg(test)]` 模块
2. **Contract Tests**: `tests/contract/*.test.mjs` (Node.js)
3. **Integration Tests**: PostgreSQL 真实连接；需按 [PostgreSQL Repository 验证说明](../../../crates/sdkwork-intelligence-sandbox-repository-sqlx/README.md#verification) 预置并初始化规范测试数据库，再将 `SDKWORK_DATABASE_URL` 与 `SDKWORK_DATABASE_TEST_POSTGRES_URL` 设置为完全相同的测试 URL

## Contribution Workflow

1. 创建 Git 分支
2. 修改代码 + 测试
3. 运行 full verification suite
4. 提交 PR
5. 人工评审 + Merge

## Adding a New Component

1. 创建 `crates/sdkwork-sandbox-<name>/`
2. 添加 `Cargo.toml` 与 `specs/component.spec.json`
3. 在根 `Cargo.toml` 注册 workspace 成员
4. 实现并测试
5. 更新 INDEX.yaml 与 TECH_ARCHITECTURE.md

## Code Style Rules

- 禁止 `#![forbid(unsafe_code)]` 之外的 unsafe 代码
- 所有 `sandbox_*` 字段/变量使用 snake_case
- 所有 `Sandbox*` 类型使用 PascalCase
- 公共错误类型使用 `Sandbox*Error` 命名
- Debug 输出必须 redact secret/private 字段

## Common Commands

```bash
# Build specific crate
cargo build -p sdkwork-sandbox-provider-spi

# Test specific crate
cargo test -p sdkwork-intelligence-sandbox-service

# Check specific crate
cargo clippy -p sdkwork-sandbox-provider-local --all-targets -- -D warnings

# Run contract tests only
node --test tests/contract/*.test.mjs

# Format the workspace members
cargo fmt
```
