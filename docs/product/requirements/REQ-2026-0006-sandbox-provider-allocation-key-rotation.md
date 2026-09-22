---
id: REQ-2026-0006
title: Deliver Sandbox Provider allocation key rotation and bounded re-encryption
owner: SDKWork Runtime Platform
status: accepted
source: security
problem: Sandbox Provider allocation recovery metadata is encrypted with a versioned key, but production key rotation cannot retire an old key until every protected Runtime Binding is re-encrypted safely under the current key.
goals:
  - Preserve decryptability across approved key versions while all new writes use the current key.
  - Re-encrypt protected Sandbox Provider allocation references in bounded Tenant-scoped pages.
  - Prevent concurrent lifecycle writes from being overwritten and prove when an old key may be revoked.
non_goals:
  - Implement a proprietary KMS, secret manager, credential file, environment-variable key loader, HTTP route, operator UI, or scheduler.
  - Rotate Provider credentials or change the Provider-private allocation reference value.
  - Re-encrypt Agents Workspace content, logs, snapshots, terminal streams, or unrelated tenant data.
users:
  - SDKWork Runtime Platform operators
  - Sandbox service-host maintainers
  - Security and compliance reviewers
affected_surfaces:
  - rust-components
  - database
  - security
  - composition
  - operations
---

# REQ-2026-0006: Sandbox Provider Allocation 密钥轮换与有界重加密

## 验收标准

- 固定术语保持 `Runtime`、`Session`、`Workspace`、`Sandbox` 与 `Provider`；领域类型使用 `SandboxProviderAllocationKey`、`SandboxProviderAllocationProtectionVersion`、`SandboxProviderAllocationReencryptionPage`，歧义变量使用 `sandbox_*`。
- `SandboxProviderAllocationKeySource` 由 Composition 注入，分别提供当前密钥和按精确 `sandbox_allocation_key_id`/`sandbox_allocation_key_version` 查询的历史密钥；Repository、Protector 与 Service Host 不读取环境变量或普通 Config 中的 Key Material。
- `sandbox_allocation_key_id` 必须为 `1..=128` bytes 的 printable ASCII；`SandboxProviderAllocationKey`、保护元数据 Domain Constructor 与 PostgreSQL `CHECK` Constraint 必须分别拒绝空值、空格、控制字符和非 ASCII 值，防止绕过单层校验写入不安全 Identity。
- 新保护操作只使用当前 Key ID/Version；恢复操作严格使用 Ciphertext 自带的 Key ID/Version，并校验 Key Source 返回的 Identity 一致。
- Key Source 必须保证一个已发布的 `(sandbox_allocation_key_id, sandbox_allocation_key_version)` 在保留期内始终映射到同一 Key Material；更换 Key Material 必须递增 Key Version，禁止在同一 Identity 下原地换料。
- 重加密在 `SandboxProviderAllocationProtector` 内完成 Restore + Protect；Provider-private 明文不得进入 Repository SQL、Debug、Log、Event、Metric、Wire 或 Operator Result，`SandboxProviderAllocationRef` Drop 时清零内部 Buffer。
- PostgreSQL 重加密只接受显式 `tenant_id`、可选 `sandbox_runtime_binding_id` Cursor 和 `sandbox_page_size`；Page Size 必须为 `1..=200`，按 `sandbox_runtime_binding_id` 稳定升序，禁止无界收集。
- 每页在查询前只捕获一次目标 Key ID/Version 与 Crypto Version，只扫描仍使用非该目标版本的非空 Allocation Ciphertext；当前版本行不重复写入。每行重保护结果必须匹配该页目标三元组；页内 Current Version 漂移时以 `ProtectionFailed` 关闭失败，不写入漂移结果，已独立提交的较早行保持有效并可由重试幂等跳过。
- 每行 Update 必须同时匹配 Tenant、Binding、Session、旧 Ciphertext、旧 Key ID/Version 与旧 Crypto Version；并发生命周期 Save 或 Binding/Session ABA 改变任一值时，重加密记录 Conflict/Skipped，不覆盖新值。
- Page Result 报告 Scanned、Re-encrypted、Conflict 数量和可选下一 Cursor；结果不得包含 Ciphertext、Key Material 或 Provider Allocation Reference。
- 重加密可安全重试；进程在任意页或任意行后中断时，已提交行保持当前版本，未提交行仍可由旧密钥恢复。
- 旧密钥只能在完整 Tenant Scope 扫描返回零待重加密行、冲突完成重试、活跃 Runtime Binding 恢复验证通过，并由 Service Host/KMS 运维流程记录人工撤销后移除。
- 测试覆盖 V1 Protect/V1 Restore、V2 Current + V1 Historical Restore、V1 -> V2 Re-encryption、旧密钥缺失关闭失败、错误 Identity 关闭失败、Context 搬移失败、当前版本幂等跳过、分页、Tenant 隔离、页内目标版本漂移关闭失败、Lifecycle Metadata CAS 与 Session ABA CAS。
- Cargo Format/Check/Test/Clippy、Database Framework、Component、Layering、Naming、Documentation 与 Repository Baseline 检查通过；真实 PostgreSQL Page/CAS/Restart Evidence 缺失时不得标记 `accepted`。

## 非功能需求

| 领域 | 要求 |
| --- | --- |
| Security | Key Material 仅存在于注入的 Secret/KMS Adapter 与短生命周期 `Zeroizing<Vec<u8>>`；无效构造输入、派生 `Zeroizing<[u8; 32]>` 和 Provider-private 明文及时清零，错误对外保持 `ProtectionFailed`。同步 Key Source 不得直接执行阻塞 Tokio Worker 的远程 KMS 调用。 |
| Privacy | Operator Result 和 Telemetry 只包含 Tenant-safe Count、Key ID/Version、Cursor/Outcome；不包含 Ciphertext 或 Provider-private Reference。 |
| Performance | 每页最多 200 行，使用 Tenant-leading `sandbox_runtime_binding_id` Cursor；不得一次事务锁定或重写整个 Tenant。 |
| Reliability | 行级密文元数据 CAS、防重试覆盖和历史密钥保留共同保证 Rotation 与 Lifecycle Save 并发安全。 |
| Operations | 轮换必须支持 Pause/Resume、Progress Count、Conflict Retry、Dry Verification 和显式旧密钥撤销门禁；自动删除旧密钥被禁止。 |

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `SECURITY_SPEC.md`, `CONFIG_SPEC.md`, `DATABASE_SPEC.md`, `DATABASE_FRAMEWORK_SPEC.md`, `PAGINATION_SPEC.md`, `RUST_CODE_SPEC.md`, `OBSERVABILITY_SPEC.md`, `TEST_SPEC.md`.

Components: `crates/sdkwork-intelligence-sandbox-service`, `crates/sdkwork-intelligence-sandbox-repository-sqlx`, `crates/sdkwork-sandbox-provider-spi`, and future `crates/sdkwork-sandbox-service-host` Secret/KMS composition.

Decision: [ADR-20260728: Sandbox Provider Allocation Key Rotation And Re-encryption](../../architecture/decisions/ADR-20260728-sandbox-provider-allocation-key-rotation-and-reencryption.md).

## Verification

```bash
cargo fmt --check
cargo test -p sdkwork-sandbox-provider-spi
cargo test -p sdkwork-intelligence-sandbox-repository-sqlx
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node --test tests/contract/database-framework.contract.test.mjs
node ../sdkwork-specs/tools/check-database-framework-standard.mjs --root .
node ../sdkwork-specs/tools/check-component-port-bindings.mjs --root . --strict
node ../sdkwork-specs/tools/check-application-layering.mjs --root .
node ../sdkwork-specs/tools/check-identity-naming.mjs --root .
```

## Verification Evidence

2026-07-29 已通过一次性 PostgreSQL 17 的空库 Migration、幂等重跑、Status/Drift、V1 -> V2 Tenant-scoped Cursor Page、Page Size Boundary、Current-version Skip、Tenant 隔离、并发 Lifecycle Save Ciphertext Metadata CAS、Session ABA CAS、页目标从 V2 漂移到 V3 时关闭失败且旧密文不变、从空 Cursor 重试后收敛到 V3、第二次扫描收敛与 Repository 重建恢复验证。修订后的 Migration 还以 SQLSTATE `23514` 和 Constraint `ck_sandbox_runtime_binding_allocation_metadata` 拒绝含换行的 Key ID，并证明失败语句未改变原 Ciphertext Metadata。Rust 负向测试覆盖空格、控制字符、非 ASCII Key ID 及 31/1025-byte Key Material 边界。完整环境、命令、结果和未关闭门禁见 [REVIEW-20260729: Sandbox Provider Allocation Key Rotation Verification](../../engineering/reviews/REVIEW-20260729-sandbox-provider-allocation-key-rotation-verification.md)。候选运维顺序、停止条件与旧密钥撤销门禁见 [Sandbox Provider Allocation Key Rotation And Old-key Revocation Runbook](../../runbooks/RUNBOOK-sandbox-provider-allocation-key-rotation.md)。

真实 PostgreSQL 证据已关闭本 Requirement 中的 Repository Page/CAS/Restart 验证缺口。本需求的所有技术验收标准已满足，于 2026-07-29 更新为 `accepted`。生产部署需额外完成 Secret/KMS Adapter 集成、Operator Entry Point、撤销演练与生产运维流程。

## Release And Review Boundary

本需求交付 Key Version/Rotation 的 Repository 与 Protector 候选能力，不提供实际 KMS/Secret Provider、Operator API、Worker、Deployment Credential 或自动撤销。当前 `SandboxProviderAllocationKeySource` 是同步 Port；生产远程 KMS 必须使用经评审的短生命周期本地 Key Handle/异步刷新边界，或在人工评审后演进为 Async Port，禁止在 Tokio Worker 上直接阻塞。
