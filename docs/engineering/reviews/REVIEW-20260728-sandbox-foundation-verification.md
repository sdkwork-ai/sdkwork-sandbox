# REVIEW-20260728: Sandbox Foundation Verification

Status: accepted

Requirement: REQ-2026-0001

Decision: ADR-20260728-runtime-boundary-and-rust-workspace

Owner: SDKWork Runtime Platform

Date: 2026-07-28

## Scope

本 Review 保留 REQ-2026-0001/0002 当时的六组件、10 测试证据快照，不作为当前组件清单。当前仓库包含七个 Rust Component，新增 PostgreSQL Repository，并由 `REQ-2026-0005` 追踪持久化、Lease/Fencing 与 Reconciler；Local Provider、Service Host 与 CLI 仍保持未激活。Runtime Execution、API、SDK、Config、Deployment 与 Release 仍不在该基础验收范围。

## Target-level Evidence

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo check --workspace` | PASS，发现并编译六个 Workspace Member |
| `cargo test --workspace` | PASS，10 个 Lifecycle/Provider/Repository Behavior Tests 与全部 Doc-test Target |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `node ../sdkwork-specs/tools/check-repository-docs-standard.mjs --root .` | PASS，Application Profile |
| `node ../sdkwork-specs/tools/check-apps-directory-index.mjs --root .` | PASS |
| `node ../sdkwork-specs/tools/check-workspace-packages-layout.mjs --root . --mode enforce` | PASS |
| `node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root .` | PASS |
| `node ../sdkwork-specs/tools/check-application-layering.mjs --root .` | PASS |
| `node ../sdkwork-specs/tools/check-identity-naming.mjs --root .` | PASS，Consumer Mode |
| `node ../sdkwork-specs/tools/check-rust-backend-composition.mjs --root .` | PASS |
| `node ../sdkwork-specs/tools/audit-repository-baseline.mjs --root .` | PASS，Branch/L1/.sdkwork 全部通过 |
| `node ../sdkwork-specs/tools/verify-repo.mjs --root .` | PASS |

## Cross-repository Observation

`node ../sdkwork-specs/tools/sweep-verify-repo.mjs --root .` 会遍历整个 sibling checkout，而不是只验证传入 Root，因此不作为 REQ-2026-0001 的 Target-level Acceptance Evidence。当前跨仓库边界改由 Sandbox、Kernel、Agents 各自的目标测试、标准门禁与 `cargo tree` 依赖证据验证；其他 Repository 的既有失败不能替代目标仓库证据，也不改变本 Review 对 Sandbox Foundation 的范围判断。

## Review Findings

- 当前 Source 只执行 Provider-neutral Lifecycle Command，不执行 Host Command、不访问 Host、不发起 Network、不声明 Secret/Config/API/SDK，也不使用 Unsafe Rust。
- Provider SPI、Lifecycle Service 与 Memory Repository 已物化候选 Public Export/Port；Local Provider、Service Host 与 CLI 的 Component Contract 继续保持未激活边界。
- `ADR-20260728-runtime-boundary-and-rust-workspace` 及后续跨仓库/安全 ADR 保持 `proposed`，因为 Provider Isolation、Attachment Persistence 与 Public Naming 仍需 Human Review。
- `agent.app.config.json` 被刻意延后；当前没有 Registration、Package Matrix、Media Asset 或 Release Surface，提前创建会产生虚假发布声明。

## Residual Risk

该历史快照不证明 Host Runtime Function、生产安全隔离、跨平台 Provider 行为或性能目标。Durable Repository/Reconciler 已由 `REQ-2026-0005` 形成候选实现，但真实 PostgreSQL、真实 Provider Fencing、受约束 Provider、Provider Conformance、Threat Model、Config/Deployment Evidence 与人工 Kernel Integration Review 仍是当前门禁。
