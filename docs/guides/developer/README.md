# Developer Guide

Local setup, verification, and contribution workflow.

See `DOCUMENTATION_SPEC.md` section 2.

## Quick Start

见 [quick-start.md](quick-start.md)，涵盖：

- Prerequisites & Build
- Project Structure (L0-L6 组件布局)
- Key Types 概要
- Testing Strategy (Unit / Contract / Integration)
- Contribution Workflow
- Code Style Rules

## 仓库约定

### 组件命名

| 后缀 | 职责 | 示例 |
| --- | --- | --- |
| `provider-spi` | L3 Provider-neutral Port | `sdkwork-sandbox-provider-spi` |
| `service` | L2 业务逻辑 | `sdkwork-intelligence-sandbox-service` |
| `repository-*` | L4 仓储 | `sdkwork-intelligence-sandbox-repository-memory` |
| `provider-*` | L4 Provider 实现 | `sdkwork-sandbox-provider-local` |
| `service-host` | L5 组合编排 | `sdkwork-sandbox-service-host` |

### Gate 0 工作边界

在 `implementationAuthorized: false` 期间，开发者**不得**：

- 新增公共 Runtime Port (如 CommandExecutor)
- 新增 Node Agent 或 WebSocket/RPC/PKI/CA/HSM 组件
- 实现真正的 SandboxProvider 资源调度
- 修改契约状态为 `in-progress` / `ready` 除非对应 REQ 已获得批准

**Gate 0 期间鼓励工作**：

- 文档与架构视图
- 契约 Schema 评审
- 测试用例准备
- 代码风格 / 命名规范化

### 命名规范

| 规则 | 说明 | 示例 |
| --- | --- | --- |
| snake_case | 字段、变量、函数 | `sandbox_id`, `fencing_token` |
| PascalCase | 枚举、结构体、类型别名 | `SandboxProvider`, `SandboxSessionState` |
| CamelCase | ❌ 禁止 | — |
| `Sandbox*Error` | 公共错误类型 | `SandboxHostError`, `SandboxProviderError` |

### 安全约束

- `#![forbid(unsafe_code)]` 在所有 crate lib 根启用
- Secret/PII 字段使用 `SecretCarrier` + `zeroize`
- `Debug` 实现必须 redact 敏感字段 (标记 `#[redact("secret")]`)
- 网络数据与持久化字段必须 validation

## 核心开发流程

### 1. 本地迭代

```bash
# 每次修改后运行
node tools/check-sandbox-cargo-path-dependencies.mjs
node tools/check-sandbox-workspace-dependency-inheritance.mjs
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`cargo fmt --check` 而不是 `cargo fmt --all -- --check`：`--all` 的语义包含本地路径依赖，会把
`sdkwork-web-framework`、`sdkwork-utils`、`sdkwork-database` 等兄弟仓库的格式偏差算到本仓门禁上，
并要求改写本仓不得修改的文件。

### 2. 契约测试

```bash
node --test tests/contract/*.test.mjs
```

### 3. 全量验证

```bash
# 路径依赖与依赖声明位置（必须先跑：清单里多余的 .. 会让 cargo metadata 失败）
node tools/check-sandbox-cargo-path-dependencies.mjs
node tools/check-sandbox-workspace-dependency-inheritance.mjs

# 文档链接与命令处方
node tools/check-sandbox-doc-integrity.mjs

# 文档规范
node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root .

# 组件端口绑定
node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root . --strict

# 包布局
node ../sdkwork-specs/tools/check-workspace-packages-layout.mjs --root . --mode enforce

# 仓库存量基线审计
node ../sdkwork-specs/tools/audit-repository-baseline.mjs --root .

# Gate 0 状态
node tools/check-sandbox-commercial-readiness.mjs
node tools/check-sandbox-evidence-traceability.mjs
node tools/check-sandbox-human-review-signoff.mjs
```

### 4. PR 提交

1. 填写 PR 描述：关联 REQ / ADR / 组件
2. 标注破坏性 / 非破坏性
3. 运行 full verification 并传结果截图
4. 等待人工评审
5. Merge 后 rebase 主分支

## 新增组件检查清单

```text
- [ ] 创建 crate 目录 `crates/sdkwork-sandbox-<name>/`
- [ ] 添加 `Cargo.toml` (包含 `#![forbid(unsafe_code)]` 的 lib target)
- [ ] 创建 `specs/component.spec.json`
- [ ] 修改根 `Cargo.toml` 注册 workspace 成员
- [ ] 添加 Unit / Integration 测试
- [ ] 添加 Contract test (如涉及公共 API)
- [ ] 更新 `docs/INDEX.yaml`
- [ ] 更新 `docs/architecture/tech/TECH_ARCHITECTURE.md`
- [ ] 更新 `docs/architecture/views/` 相关视图
- [ ] 人工架构评审并通过
```

## 常见陷阱

| 陷阱 | 原因 | 修复 |
| --- | --- | --- |
| 公共 Port 未批准 | Gate 0 限制 | 仅在 provider-spi 允许范围内扩展 |
| unsafe 代码违禁 | 默认 forbid | 使用 safe 替代；必要时提交 RFC |
| contract test 失败 | schema 不一致 | 重新检查 JSON Schema 语法 |
| tokio test 缺依赖 | dev-dependencies | 添加 `tokio.workspace = true` |

## 参考

- [技术架构概述](../../architecture/tech/TECH_ARCHITECTURE.md)
- [Traceability Map](../../architecture/views/traceability-map.md)
- [Gate Zero Current State](../../architecture/views/gate-zero-current-state.md)
- [`CODE_STYLE_SPEC`](../../../../sdkwork-specs/CODE_STYLE_SPEC.md)
- [`COMPONENT_SPEC`](../../../../sdkwork-specs/COMPONENT_SPEC.md)
