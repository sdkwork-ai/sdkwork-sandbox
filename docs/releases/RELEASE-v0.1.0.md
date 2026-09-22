# RELEASE-v0.1.0: Phase 0 Foundation Candidate

Status: draft
Owner: SDKWork Runtime Platform
Date: 2026-07-29

本 Release 标志 `sdkwork-sandbox` 仓库完成 Phase 0 Foundation 候选状态。仓库具备独立工作区、文档 Canon、分层机器契约与静态 Contract Test，但不声称已经具备可用 Sandbox 执行、真实 Host/KVM、SaaS 调度或商业运营能力。

## 1. Scope

Phase 0 交付物包含七个分层 Rust 组件边界、Provider Delivery Gate、18 Requirements（全部 draft/in-progress）、17 ADRs（全部 proposed）、11 份人工评审 Review Packet（全部 pending-human-review）、107 个静态 Contract Test（全部通过）和 Cargo Workspace Check/Test/Format/Clippy 全通过。

Component 边界：
- L0 Contract: `apis/commands/` Provider-neutral Command Execution/Cancel/Result Schema，精确 Fingerprint、Idempotency 与 Terminal Completion Catalog。
- L3 Port: `sdkwork-sandbox-provider-spi` Lifecycle Port、HostUser/MicroVm Assurance 目录。
- L2 Service: `sdkwork-intelligence-sandbox-service` Provider-neutral `SandboxSession` Lifecycle 候选契约。
- L4 Repository Adapter: `sdkwork-intelligence-sandbox-repository-memory`（候选状态）与 `sdkwork-intelligence-sandbox-repository-sqlx`（PostgreSQL Lease/Fencing/Key Rotation）。
- L4 Provider Adapter: `sdkwork-sandbox-provider-local`（HostUser，未激活运行行为）、`specs/` 下 Firecracker/KVM/Artifact/Network/Resource/Quota/Capacity/Node Trust/Scheduling 门禁契约。
- L5 Composition: `sdkwork-sandbox-service-host` L5 typed Service Host Composition、依赖注入、fail-closed Readiness 边界（draft）。
- L6 Delivery: `sdkwork-sandbox-sandbox-cli` Operator CLI 框架（Gate 0 范围锁定）。

## 2. Gate 0 Exit Preconditions

未经本门禁不得进入 V1 Implementation 阶段：

- 所有 107 个 Contract Test 通过。
- `cargo check --workspace` 通过。
- `cargo test --workspace` 通过。
- `cargo fmt --all -- --check` 通过。
- `cargo clippy --workspace -- -D warnings` 通过。
- Component Port Checker、Documentation Checker、Packages Layout Checker、Repository Baseline Audit 全通过。
- 11 份 Review Packet 全部状态为 `pending-human-review`：每份已记录 Close-Out Checklist 与 Exit Gate；任何一份 Reviewer Role 未 Approved 则 Gate 0 不得退出。
- 18 Requirements 状态保持 `draft` 或 `in-progress`，只有人工评审通过后才进入 `ready`。
- 17 ADRs 状态保持 `proposed`，Human Decision Approved 后才进入 `accepted`。
- `specs/sandbox-provider-delivery-gates.contract.json` 保持 `implementationAuthorized: false`。

## 3. Next V1 Slice Preview

进入 V1 后按以下顺序交付，每项均需独立 Review 与 Gate 通过：

1. **Local Provider (REQ-2026-0003)**：Windows/macOS/Linux HostUser 本地进程执行与有界 Descendant Cleanup 证据。
2. **Shared Command/Terminal Conformance (REQ-2026-0007)**：公共 `SandboxCommandExecutor` 命名与 Local/Firecracker 共用契约。
3. **Service Host (REQ-2026-0009)**：L5 Composition 运行时与 Readiness。
4. **Observability/Event/Audit/Outbox (REQ-2026-0010)**：PostgreSQL Outbox 与 Event Worker。
5. **Firecracker Provider (REQ-2026-0008)** 及 Artifact/Network/Resource/Host Isolation Broker 证据。
6. **V2 Isolated Cloud Runtime**：Admission/Scheduler/Capacity、Node Trust、PostgreSQL Quota/Capacity Reservation 真实实现。

Docker Provider 不在本路线图、不作为 Capability Fallback、不作为测试替代、不作为 Release Claim。

## 4. Known Constraints

- Local Provider 实现激活未完成，platform-specific Process Supervision 无发布能力。
- Firecracker/KVM/Jailer/Network Namespace/Secret Injection 所有组件仅停留在 Machine Contract 与静态 Test，不形成运行时。
- Scheduler/Admission/Node Agent/PKI/CA/HSM/Attestation/Warm Pool/Billing/Commerce Runtime 未建立。
- PostgreSQL Outbox Worker、Event Exporter、Quota Metering、Dashboard 未建立。
- KMS、Secret、Deployment Profile、Public API、SDK Family 未建立。
- 无生产 Deployment、无高可用证明、无 SLO/SLA、无 PITR 演练、无撤销演练、无 Evidence Authority。

## 5. Rollback Plan

- 保持 Machine Contract / Fake Host Test / Component 边界后退：`git restore specs/` + `git restore crates/` 即可回到 Gate 0 静态 Draft 状态。
- Database Migration 单独回滚：通过 `database/migrations/postgres/*` 各自独立 forward-fix migration 避免不可逆破坏。
- Release Document 与 Review Packet 保留 pending-human-review 状态，误批准可通过 git revert 还原为 pending。

## 6. Sign-Off Table（待评审）

| 负责人角色 | 签章 | 日期 |
| --- | --- | --- |
| Architecture Reviewer | —— | —— |
| Security Reviewer | —— | —— |
| Performance Reviewer | —— | —— |
| Operations Reviewer | —— | —— |
| Product Owner | —— | —— |
| Database Owner | —— | —— |
| Evidence Authority | —— | —— |

## 7. Evidence Checklist

- [x] `cargo check --workspace` 通过
- [x] `cargo test --workspace` 通过
- [x] `cargo fmt --all -- --check` 通过
- [x] `cargo clippy --workspace -- -D warnings` 通过
- [x] 107 Contract Test 全部通过
- [x] 7 个 `component.spec.json` 符合 COMPONENT_SPEC v1.1
- [x] 18 Requirements 维持 draft 状态
- [x] 17 ADRs 维持 proposed 状态
- [x] 14 Contract JSON（1 主门禁 + 13 组件/模块）都是 draft 状态且 `implementationAuthorized: false`
- [ ] 真实 Windows/macOS/Linux Local Provider 运行证据（V1）
- [ ] 真实 Linux KVM Firecracker Boot 证据（V2）
- [ ] 并发/规模/故障注入生产证据（V2+）
- [ ] PITR/RPO/RTO 演练记录（V2+）
- [ ] Secret/KMS 撤销演练记录（V2+）
- [ ] SaaS 多租户调度放置证据（V2+）

## 8. References

- [PRD](../product/prd/PRD.md)
- [Tech Architecture](../architecture/tech/TECH_ARCHITECTURE.md)
- [PRD Roadmap](../product/prd/PRD-roadmap.md)
- [Provider Delivery Plan](../engineering/plans/PLAN-2026-0001-local-and-firecracker-provider-delivery.md)
- [Provider Delivery Gates](../../specs/sandbox-provider-delivery-gates.contract.json)
- [INDEX](../INDEX.yaml)