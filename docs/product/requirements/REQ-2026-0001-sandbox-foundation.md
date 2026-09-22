---
id: REQ-2026-0001
title: Initialize the SDKWork Sandbox foundation
owner: SDKWork Runtime Platform
status: accepted
source: platform
problem: sdkwork-sandbox does not have an independent SDKWork baseline, product authority, architecture authority, or reviewable module boundaries.
goals:
  - Establish an independently verifiable repository and documentation Canon.
  - Materialize a minimal Rust workspace without claiming runtime behavior.
non_goals:
  - Implement runtime, provider, session, workspace, API, SDK, scheduler, snapshot, cache, or deployment behavior.
users:
  - SDKWork Runtime Platform maintainers
  - SDKWork Kernel integrators
  - Sandbox provider authors
affected_surfaces:
  - repository-workspace
  - documentation
  - rust-components
---

# REQ-2026-0001: 初始化 SDKWork Sandbox 工程基础

## 验收标准

- 目标目录是独立 Git 仓库，当前分支为 `main`。
- 根级 L1 文件包含 `AGENTS.md`、兼容 Shim、`README.md`、`.gitignore` 与受版本控制的 `.sdkwork/` 字典。
- 根 `README.md` 声明 `repository-kind: application`，链接 Canon 文档，说明根为主应用面，并列出活动/非活动标准目录。
- 每个标准顶层目录都有 README，包含 Purpose、Owner、Allowed、Forbidden、Related Specs 与 Verification。
- `docs/product/prd/PRD.md` 与 `docs/architecture/tech/TECH_ARCHITECTURE.md` 是可评审 Canon Entry，并链接分片。
- 根 Cargo Workspace 只发现五个 Phase 0 组件：Provider SPI、Sandbox Service、Local Provider、Service Host、CLI。
- 每个 Rust Crate 拥有 `README.md`、`specs/README.md`、`specs/component.spec.json` 和可编译 Assembly File。
- Phase 0 Crate 不包含执行逻辑、外部依赖、Network Listener、Config Key、API Route、Generated SDK 或 Secret Handling。
- Cargo Format、Check 与 Test 通过。
- SDKWork Documentation、Workspace Layout、Component Contract、Naming 与 Repository Baseline 检查通过；如果 Validator 本身存在能力缺口，必须记录准确输出。

## 非功能需求

| 领域 | 要求 |
| --- | --- |
| Security | Scaffold 中不存在 Secret、Runtime State、Host Access、External Command、Network Call 或 Unsafe Rust。 |
| Privacy | Phase 0 不产生 User、Tenant、Workspace、Session、Command 或 Telemetry 数据。 |
| Performance | 除确定性本地校验外无额外要求，不允许发布 Runtime 性能声明。 |
| Reliability | 无需运行 Service 即可从仓库根执行全部 Phase 0 检查。 |
| Portability | Cargo Workspace 与文档只使用相对路径，可在 Windows、macOS、Linux 读取。 |

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `SDKWORK_WORKSPACE_SPEC.md`, `DOCUMENTATION_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`, `COMPONENT_SPEC.md`, `CODE_STYLE_SPEC.md`, `NAMING_SPEC.md`, `RUST_CODE_SPEC.md`, `TEST_SPEC.md`.

Components: `crates/sdkwork-sandbox-provider-spi`, `crates/sdkwork-intelligence-sandbox-service`, `crates/sdkwork-sandbox-provider-local`, `crates/sdkwork-sandbox-service-host`, `crates/sdkwork-sandbox-cli`.

Decision: [ADR-20260728: Runtime Boundary And Rust Workspace](../../architecture/decisions/ADR-20260728-runtime-boundary-and-rust-workspace.md).

## Verification

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root .
node ../sdkwork-specs/tools/check-apps-directory-index.mjs --root .
node ../sdkwork-specs/tools/check-workspace-packages-layout.mjs --root . --mode enforce
node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root .
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-identity-naming.mjs --root .
node ../sdkwork-specs/tools/audit-repository-baseline.mjs --root .
```

## Change Control

任何 Operational Provider API、Public Rust Export、API/SDK Authority、Config Key、Deployment Profile、Kernel Integration Dependency 或 Release Artifact 都属于范围扩展，必须先创建或更新 Ready Requirement，并在规范要求时执行人工评审。

## Completion Evidence

Accepted on 2026-07-28. Target-level Cargo、Documentation、App Index、Workspace Layout、Component Port、Application Layering、Identity Naming、Rust Composition、Repository Baseline 与 `verify-repo` 检查全部通过。详细命令、结果和剩余风险见 [REVIEW-20260728](../../engineering/reviews/REVIEW-20260728-sandbox-foundation-verification.md)。
