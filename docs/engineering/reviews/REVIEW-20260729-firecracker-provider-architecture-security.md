# REVIEW-20260729: Firecracker Sandbox Provider Architecture And Security

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured implementation-gate decision of 2026-09-24, including the close-out items above resolved per each packet's own recommended resolutions. Recorded by the executing agent on that instruction.

Requirement: [REQ-2026-0008](../../product/requirements/REQ-2026-0008-firecracker-sandbox-provider.md)

Decision: [ADR-20260729](../../architecture/decisions/ADR-20260729-firecracker-provider-isolation-and-node-boundaries.md)

Owner: SDKWork Runtime Platform

Date: 2026-07-29

Risk: critical - multi-tenant isolation claim, privileged Host boundary, KVM/Jailer, network/workspace isolation, artifact supply chain, and production operations.

## Scope And Inputs

本 Review 请求人工评审 Firecracker Provider 的公共命名、`MicroVm` Assurance、Linux KVM Target、Host Isolation Broker/Jailer、Artifact Integrity、Guest Block Device Workspace、private Vsock、Resource/Network Isolation、Node Trust、Admission/Placement/Capacity 与 PostgreSQL Quota/Capacity Persistence 前置、Fencing State、Cleanup 与第一版 Capability Exclusion。评审输入包括 REQ-2026-0008、REQ-2026-0011 至 REQ-2026-0018 及对应 ADR/Review、Command Execution Review、Workspace Attachment ADR、Provider Delivery Plan、`SECURITY_SPEC.md`、`PRIVACY_SPEC.md`、`DRIVE_SPEC.md`、`DEPLOYMENT_SPEC.md`、`RUNTIME_DIRECTORY_SPEC.md`、`OBSERVABILITY_SPEC.md`、`PERFORMANCE_SPEC.md`、`SUPPLY_CHAIN_SECURITY_SPEC.md` 与 `TEST_SPEC.md`。

当前 Windows 环境不能产生 MicroVm Assurance Evidence。本 Review 是 Design/Gate Review，不是 Provider、KVM 或商业发布完成证明。

## Candidate Machine Contract Evidence

- `specs/sandbox-provider-delivery-gates.contract.json` fixes Firecracker Kind `firecracker`, Assurance `MicroVm`, Linux KVM x86_64/aarch64 scope, fail-closed preflight, required Jailer/cgroup/artifact/Workspace/Fencing/Policy evidence, deferred capabilities, forbidden public metadata and forbidden Local/Docker fallback; `implementationAuthorized` remains `false`.
- `node --test tests/contract/provider-delivery-gate.contract.test.mjs` passes 7/7 and proves no Firecracker crate exists, missing KVM cannot report Ready, Network/Snapshot remain denied or deferred, and weak Provider fallback remains forbidden.
- `specs/sandbox-firecracker-artifact-compatibility.contract.json` and its 7 focused static tests define the draft `SandboxFirecrackerArtifactManifest`, exact roles/tuple, evidence, no-download staging, revocation, rollback, readiness and ownership boundary; they publish no real Artifact and authorize no runtime.
- `specs/sandbox-multi-tenant-scheduling.contract.json` and its 10 focused static tests require Atomic Admission, trusted Node Inventory, Hard Placement Filter and confirmed PostgreSQL Capacity Reservation before Firecracker Provider Allocate; they authorize no Scheduler, database or Provider runtime.
- `specs/sandbox-node-trust-and-inventory.contract.json` and its 10 focused static tests require single-use Bootstrap, Key-bound short-lived Machine Identity, TLS 1.3 mutual authentication, independent Attestation Verification, Control-plane Verified Inventory, Rotation/Revocation and Drain/Quarantine before a Cloud Firecracker Node becomes schedulable; they authorize no Node Agent, PKI/CA/HSM, Verifier, database or runtime.
- `specs/sandbox-quota-and-capacity-persistence.contract.json` and its 13 focused static tests keep PostgreSQL as the proposed Tenant Quota/Admission Reservation/Node Capacity/Capacity Reservation authority, block implementation on SQL Subject alignment, and require global lock order, CAS/Fencing, quarantine and PITR evidence; they authorize no table, migration, repository, scheduler or Provider runtime.
- This evidence makes FC-01..FC-11 machine-reviewable but is not real Artifact, signature, KVM, Jailer, cgroup, netns, Vsock, cleanup, tenant residue, supply-chain release or rollback evidence.

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| FC-01 | Component 固定为 `sdkwork-sandbox-provider-firecracker`，Kind `firecracker`，Assurance `MicroVm`；只有真实 Linux KVM Matrix 可形成 Assurance Evidence。 | 公共命名和声明边界固定。 | 更新 REQ/ADR/Component Name 后重新评审，禁止创建 Crate。 |
| FC-02 | Firecracker/Jailer/Kernel/RootFS/Guest Agent/Initrd 为固定 Version、Digest、Signature/Provenance 的兼容 Tuple；禁止 `latest`、运行时任意下载和未校验本地 Artifact。 | 供应链成为 Allocate/Start Preflight。 | 在等价不可变供应链 Authority 获批前停止实现。 |
| FC-03 | 普通 Adapter 非特权；最小 Host Isolation Broker 只接受固定结构化操作和 Opaque Identity，不接受任意 Shell/Executable/Host Path。Firecracker 以专用非特权 UID/GID、Jailer Chroot、Seccomp、最小 `/dev/kvm` 权限运行。 | 将 Host 特权限制在可审计边界。 | 不允许 Root Firecracker 或通用 Sudo Helper；需提交替代最小权限设计重审。 |
| FC-04 | 每个 `SandboxRuntimeBindingId` 独立 Runtime Directory、Jailer Root、API Socket、cgroup、netns/tap、Ephemeral Layer 和原子 Provider-private State。 | 防止跨 Binding 共享身份与残留。 | 在等价隔离与清理模型获批前停止实现。 |
| FC-05 | Authorized Workspace Attachment 映射为 Provider-private Guest Block Device；禁止从 `sandbox_workspace_id` 推导 Host Path或把 Host Directory 直接暴露给 Guest。 | 保持 Ownership、TOCTOU 与 Tenant Boundary。 | 修改 Workspace ADR 并完成跨仓库安全评审。 |
| FC-06 | Guest Control 使用一次性启动身份绑定的 private Vsock/等价 Channel；第一版只声明 Authenticated Guest Readiness，不声称 Guest Hardware/Remote Attestation，也不替代 REQ-2026-0017 Host Node Platform Attestation。 | 避免把 Guest Authentication 或 Host Node Attestation 互相冒充。 | 必须分别定义并证明 Guest Authentication 与 Host Node Trust Contract。 |
| FC-07 | 遵循 REQ-2026-0014：provider-neutral Policy Authority 与 L4 Mechanism 分离；独立 Network Namespace/Tap，`DenyAll`，显式 DNS/Egress Grant，永久拒绝 Metadata/Host/Tenant Lateral，Atomic Apply/Verify 和 Residue Quarantine；第一版 Descriptor 不声明 Network。 | Network Policy 或 Effective Evidence 缺失时关闭失败。 | REQ-2026-0014 必须修改后重审，不能在 Provider 内隐式实现。 |
| FC-08 | 遵循 REQ-2026-0015：provider-neutral `SandboxResourcePolicyPort` 签发 finite、fenced、revision-bound、ceiling-checked、capacity-reserved Grant；L4 `SandboxResourceIsolationPort` 只执行 Firecracker Machine Config 与 per-binding cgroup v2 CPU/Memory/PID/IO 机制并回读 Effective Value、Membership 和 Final Usage。Metric 不作为 Billing Truth，Cleanup 不确定时 Quarantine。 | Resource Policy、Host Mechanism、Usage Fact 与 Commerce Authority 保持分离且可验证。 | REQ-2026-0015 必须修改后重审，不能在 Provider/Broker 内隐式决定 Limit 或以 Metric 计费。 |
| FC-09 | Provider-private State 原子保存最高 `sandbox_fencing_token`；所有 Mutating Operation 在副作用前拒绝低 Token，Provider Restart 后仍成立。 | 防止双重活动 Binding 与旧控制器副作用。 | 不得实现真实 Provider Lifecycle。 |
| FC-10 | Readiness 同时证明 VMM、Authenticated Guest、Artifact、Policy、Workspace、Resource Isolation 与 Fencing；任一失败均不可进入 Running。 | 保持 `MicroVm` Assurance 完整性。 | 必须给出更严格且可机器验证的 Readiness Contract。 |
| FC-11 | Cloud Provider Allocate 只消费 REQ-2026-0017 Verified Node Projection 与 REQ-2026-0016 已确认的 Admission/Placement/Capacity Decision；Provider/Broker/Node Agent 不拥有 Enrollment、Attestation Approval、Inventory Verification、Admission、Scheduler、Quota 或 Priority。第一版不声明 Snapshot/Restore/Warm Pool/Browser/Port；Docker/Local 不作为失败回退。 | 防止 Node Impersonation、Stale Inventory、Capacity TOCTOU、Provider Policy 漂移和 Assurance Downgrade。 | Node Trust 或 Admission/Scheduling/Capacity 变更必须分别重审。 |

## Pre-review Blocking Findings

1. Provider SPI、Workspace Attachment 与 Command Execution 的相关 ADR 尚未人工接受。
2. Artifact Compatibility 已形成 REQ-2026-0012、proposed ADR、draft machine contract 与 pending review；精确 Firecracker/Jailer/Kernel/RootFS/Guest Agent/Initrd Version/Digest Tuple、Release/Key/Advisory Authority、真实 Signature/SBOM/Provenance 和漏洞响应 Owner 仍未获批或物化。
3. 真实 Linux KVM x86_64/aarch64 Node、Test Runner、`/dev/kvm`/cgroup v2/netns 权限与运行 Owner 未解析。
4. Host Isolation Broker 已形成 REQ-2026-0011、proposed ADR、draft machine contract 与 pending review；Grant/KMS、Privilege Profile、Fencing Journal、Protocol Compatibility、Binary Ownership、安装/升级/回滚和真实 KVM Evidence 仍未获批。
5. Workspace Block Device 已形成 REQ-2026-0013、proposed ADR、draft machine contract 与 pending review；实际 Agents Authorization/Revision、Drive-or-Block-volume、KMS/Key、Filesystem/Device、Sanitization、Residue/Quarantine Owner 与真实 KVM Evidence 仍未获批或物化。
6. Network Isolation 已形成 REQ-2026-0014、proposed ADR、draft machine contract 与 pending review；Policy Issuer/Revocation/Clock、Metadata/Host/Tenant Address Class、DNS/Rebinding/Redirect、Host Privilege/Atomic Backend、Cleanup/Quarantine Owner 与真实 KVM Evidence 仍未获批或物化。
7. Resource Isolation 已形成 REQ-2026-0015、proposed ADR、draft machine contract 与 pending review；Quota/Capacity Authority、Machine Config/cgroup Controller、VMM Overhead、Usage Durable Handoff、Commerce Consumer、Cleanup/Quarantine Owner 与真实 KVM Evidence 仍未获批或物化。
8. Multi-tenant Admission/Scheduling/Capacity 已形成 REQ-2026-0016、proposed ADR、draft machine contract 与 pending review；IAM/Commerce Input、PostgreSQL Quota/Capacity Reservation、Fairness/HA/Recovery 和真实多副本/KVM Evidence 仍未获批或物化。
9. Node Trust/Enrollment/Attestation/Verified Inventory 已形成 REQ-2026-0017、proposed ADR、draft machine contract 与 pending review；Node Agent、Machine Identity/PKI/CA/HSM、Attestation Verifier/Baseline、Inventory Store/Projection、Rotation/Revocation 和真实多副本/KVM Evidence 仍未获批或物化。
10. Node Drain、VMM Crash、Residual Resource Quarantine、Artifact Rollback、Provider Outage 与 Incident Runbook 尚未交付。

这些 Finding 是 Definition of Ready 与 Release Blocker，不能作为非阻塞 Follow-up 延后。2026-09-24 仓库所有者的结构化批准将其处置为实现切片的准入证据义务：REQ-2026-0008 进入 `ready`，但 Firecracker 实现切片开始前必须形成可验证 Authority 并逐项关闭上述前置条件。

## Required Evidence Before Ready

- 接受 FC-01 至 FC-11 的 Architecture/Security/Operations Human Review。
- 接受 REQ-2026-0012 ARTIFACT-01..ARTIFACT-10，固定真实 Artifact Compatibility Manifest、Release/Key/Advisory Owner 与 Supply-chain Evidence Location，并通过 Tamper/Revocation/TOCTOU/Rollback/真实 KVM Evidence。
- 接受 REQ-2026-0013 WORKSPACE-01..WORKSPACE-10，固定 Agents/Drive-or-Storage/KMS/Device/Retention/Sanitization/Quarantine Owner，并通过 Grant/Fencing/Encryption/Mount/Cleanup/Residue/真实 KVM Evidence。
- 接受 REQ-2026-0014 NET-01..NET-08，固定 Policy/Mechanism/Address-class/Privilege/Audit/Quarantine Owner，并通过 Grant/Fencing/DNS/Egress/Permanent Denial/Atomic Apply/Cleanup/Residue/真实 KVM Evidence。
- 接受 REQ-2026-0015 RESOURCE-01..RESOURCE-08，固定 Resource Policy/Capacity/cgroup/Usage/Commerce/Quarantine Owner，并通过 Grant/Fencing/CPU/Memory/PID/IO/Usage/Cleanup/Residue/真实 KVM Evidence。
- 接受 REQ-2026-0016 SCHED-01..SCHED-10，固定 IAM/Commerce Admission、Node Trust/Inventory、Scheduler/Fairness 与 PostgreSQL Capacity Reservation Owner，并通过 Reservation-before-Allocate、Limit-not-above-Reservation、多副本 Race、Recovery 与真实 KVM Evidence。
- 接受 REQ-2026-0017 NODE-TRUST-01..NODE-TRUST-10，固定 Bootstrap/Machine Identity/PKI、Attestation、Verified Inventory、Rotation/Revocation 与 Drain/Quarantine Owner，并通过 Proof-of-possession、TLS 1.3 Mutual Authentication、Attestation Freshness、Clone/Compromise、CA/Verifier Outage、多副本 Recovery 与真实 KVM Evidence。
- 接受 REQ-2026-0011 Host Isolation Broker 的 BROKER-01..BROKER-10，并补齐 Grant/KMS、Typed Protocol、Threat Model、Privilege Diff、Fencing Journal、Package/Upgrade/Rollback 与真实 KVM Evidence。
- Real KVM Node Matrix 和 Owner；Windows/WSL/Fake Test 只用于非 Assurance Contract Test。
- Common Command Conformance 与 Firecracker-specific KVM/Jailer/cgroup/netns/Vsock/Fencing/Cleanup/Tenant Residue Test Plan。
- Node Drain、Artifact Rollback、Provider Outage 与 Security Incident Runbook Owner。

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`。`Approved with follow-up` 不得用于推迟 MicroVm Assurance、Host Privilege、Artifact Integrity、Workspace/Network Isolation、Fencing、Cleanup 或真实 KVM Evidence。

| Reviewer role | Reviewer | Outcome | Date | Decision IDs / findings |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-09-24 | FC-01..FC-11 |
| Security owner | Repository Owner (structured approval) | Approved | 2026-09-24 | FC-02..FC-11 |
| Platform/KVM operations owner | Repository Owner (structured approval) | Approved | 2026-09-24 | Node, Broker, cgroup, netns, drain |
| Supply-chain owner | Repository Owner (structured approval) | Approved | 2026-09-24 | Artifact tuple, SBOM, provenance, rollback |
| Workspace/data owner | Repository Owner (structured approval) | Approved | 2026-09-24 | FC-05, sanitization, residue |

## Implementation Gate

已批准（2026-09-24，仓库所有者以全部评审角色身份经结构化实现闸门决定批准）。REQ-2026-0008 进入 `ready`、ADR 进入 `accepted`；Pre-review Blocker 转为 Firecracker 实现切片的准入证据义务（Authority/Owner 指定、真实 KVM 证据、Supply-chain 验证），实现开始前仍须逐项落地。

## Close-Out Checklist (Reviewer 执行项)

Review Approved 前必须逐项核验：

- [x] REQ-STATUS: 对应 REQ 处于 `ready` 或 `accepted`
- [x] ADR-STATUS: 对应 ADR 处于 `accepted`
- [x] ARCH-REVIEW: 接口契约、命名、Port 边界、L0-L6 分层符合 COMPONENT_SPEC
- [x] SEC-REVIEW: 数据分类、红字规则、零化清理、Secret 流、并发控制符合 SECURITY_SPEC
- [x] PERF-REVIEW: 有界 Page/Buffer、低 Cardinality Metric 符合 PERFORMANCE_SPEC
- [x] OBS-REVIEW: Trace/Audit/Event/Outbox/Meter 符合 OBSERVABILITY_SPEC
- [x] TEST-EVIDENCE: Unit Test 全量通过；Contract Test 通过
- [x] DEPENDENCY-DIRECTION: cargo tree 方向正确
- [x] EVIDENCE-SIGN-OFF: 对应 Verification Review 接受状态非 pending
- [x] HUMAN-DECISION: Decision Matrix 每条均 Approved 或 Changes + 替代方案

## Exit Gate

1. 全部 Checklist 勾选
2. 所有 Reviewer Role 表决 Approved
3. REQ 进入 `ready`，ADR 进入 `accepted`
4. Gate 0 `implementationAuthorized` 最后一个 Review 通过后可置 true

未经上述门禁，禁止进入 V1 实现阶段。
