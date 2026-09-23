---
id: REQ-2026-0008
title: Deliver the Firecracker Sandbox Provider
owner: SDKWork Runtime Platform
status: ready
priority: critical
source: security
problem: Untrusted multi-tenant Agent workloads need a reviewed microVM execution boundary, while Local HostUser assurance cannot safely satisfy the cloud isolation requirement.
goals:
  - Deliver a Linux KVM Firecracker Sandbox Provider that truthfully reports MicroVm assurance.
  - Enforce jailer, boot artifact integrity, cgroup, network namespace, Workspace attachment, fencing, cleanup, and tenant sanitization boundaries.
  - Run the same provider-neutral lifecycle and command conformance used by the Local Provider without Kernel branches.
non_goals:
  - Support Windows-native, macOS-native, WSL, Docker, gVisor, Kubernetes scheduling, GPU, nested virtualization, or DedicatedVm assurance.
  - Implement Snapshot/Restore, Warm Pool, live migration, public API/SDK, Scheduler, Node Enrollment, KMS, billing, or automatic image building in the first Provider slice.
  - Expose Firecracker API sockets, host paths, tap names, cgroup paths, guest credentials, or microVM ids in public Sandbox contracts.
users:
  - SDKWork SaaS runtime operators
  - Security and compliance reviewers
  - Sandbox Provider maintainers
  - SDKWork Kernel integrators
affected_surfaces:
  - rust-components
  - composition
  - security
  - deployment
  - observability
  - operations
---

# REQ-2026-0008: 交付 Firecracker Sandbox Provider

## Readiness Blockers

- 人工接受 Provider SPI、Agents Workspace Attachment、Command Execution 与本需求对应 ADR 的公共命名、安全边界和数据所有权。
- `REQ-2026-0012` 已定义 draft `SandboxFirecrackerArtifactManifest`、精确 Architecture Tuple、Evidence、Materialization、Revocation 与 Rollback 边界；仍需人工接受并填充受支持的 Firecracker/Jailer、Linux Kernel、RootFS/Guest Agent 真实版本、Digest、Signature/Key/Release/Advisory Authority，禁止 `latest` 或运行时未校验下载。
- 提供真实 Linux KVM x86_64 和/或 aarch64 节点测试环境；Windows、macOS、WSL 和无 `/dev/kvm` 环境不能作为 MicroVm Assurance 证据。
- `REQ-2026-0013` 已定义 draft Workspace Block Device/Sanitization，`REQ-2026-0014` 已定义 draft Network Policy/Isolation，`REQ-2026-0015` 已定义 draft Resource Policy/Isolation/Usage，`REQ-2026-0016` 已定义 draft Admission/Scheduler/Placement/Capacity Reservation，`REQ-2026-0017` 已定义 draft Node Trust/Enrollment/Attestation/Verified Inventory 边界；仍需与最小特权 Host Isolation Broker、Jailer 目标 UID/GID、`/dev/kvm` 最小权限、Runtime Data Root 一并完成人工架构、安全、隐私、PKI/Attestation、Drive/Storage、KMS、网络、容量/配额、Commerce 与运维所有权评审。
- 定义 Provider Node 故障、残留 Allocation、镜像撤销、安全公告与 Release Rollback 流程。

## Candidate Acceptance Criteria

- 候选 Component 名称为 `sdkwork-sandbox-provider-firecracker`；Provider Descriptor 固定 Kind `firecracker`、Assurance `MicroVm`，且只声明通过当前 Node Preflight 和 Conformance 的 Capability。
- Preflight 只有在 Linux、受支持 Architecture、KVM 可用、Firecracker/Jailer 版本与 Digest 匹配、cgroup v2 可写、Runtime Data Root 安全、Network/Workspace 前置能力满足时才报告 Ready；任一条件缺失时报告 Degraded/Unavailable，不降低 Assurance。
- Firecracker 与 Jailer 作为固定版本的外部 Release Artifact 使用；Adapter 不复制 Firecracker 源码、不手写 KVM/VMM，也不在本 Crate 使用 `unsafe` 绕过宿主机边界。
- 每个 `SandboxRuntimeBindingId` 使用独立 Jailer Root、API Socket、cgroup、Network Namespace、Tap、Ephemeral RootFS Layer 与 Provider-private Allocation Metadata。Host Path 和这些 Identity 不进入公共 Result、Error、Log、Metric 或 SDK Type。
- 普通 Adapter 不持有任意 Root/Sudo/Command 权限。需要 Host 特权的 Jailer Root、cgroup、Network Namespace/Tap 与 Device 准备通过经过审计的最小 Host Isolation Broker 完成；Broker 使用固定操作和验证后的 Opaque Identity，不接受任意 Shell/Path。准备完成后 Firecracker VMM 必须在专用非特权 UID/GID、Chroot、Seccomp 与 Resource Limit 下运行，并只获得最小 `/dev/kvm` 访问；禁止 Root Firecracker、Ambient Linux Capability、Host PID/Network Namespace、Docker Socket、Cloud Credential 与不受限 Device。
- Firecracker、Jailer、Kernel、RootFS、Guest Agent 和可选 Initrd 遵循 `REQ-2026-0012` 的不可变 Compatibility Tuple；在 Allocate/Start 前校验 Manifest Signature、Artifact Digest/Evidence、Revocation/Advisory、Architecture 与 Runtime File Identity。RootFS 默认只读；Workspace、Temp 与 Cache 使用不同、限额明确的 Guest Block Device 或 Ephemeral Layer。
- Authorized Workspace Attachment 遵循 REQ-2026-0013：Service Host 只依赖 provider-neutral `SandboxWorkspaceAttachmentPort`，Firecracker L4 机制通过已评审 Adapter/Broker 提供 Provider-private Reference 并映射为 encrypted Guest Block Device；不得根据 `sandbox_workspace_id` 推导 Host Path，也不得把宿主机目录直接暴露给 Guest。
- Host 与 Guest Command Control 使用私有、一次性启动身份绑定的 Vsock/等价 Channel；第一版只声明 Authenticated Guest Readiness，不声称 Guest Hardware/Remote Attestation。该 Guest Channel 不替代 REQ-2026-0017 的 Host Node Machine Identity 与 Platform Attestation；Firecracker API Socket 只存在于 Provider-private Runtime Directory，并使用最小文件权限，不能绑定 TCP 或进入 Workspace。
- 每个 Allocation 的网络遵循 REQ-2026-0014：provider-neutral `SandboxNetworkPolicyPort` 签发绑定 Revision/Fencing 的 Grant，Firecracker L4 `SandboxNetworkIsolationPort` 只执行机制；每个 Binding 独立 Network Namespace/Tap，默认 `DenyAll`，显式 DNS/Egress Grant 也不能覆盖 Cloud Metadata、Host Control Plane 与 Tenant Lateral Traffic 永久拒绝。只有实际 Atomic Apply/Readback/Probe 与 Residue Clear 成功后才声明 Network Ready。
- vCPU/Memory 与 Host Resource Enforcement 遵循 REQ-2026-0015：provider-neutral `SandboxResourcePolicyPort` 签发 finite、capacity-backed Grant，Firecracker L4 `SandboxResourceIsolationPort` 执行精确 Machine Config + per-binding cgroup v2 CPU/Memory/PID/IO；Effective Value、Process Membership、Fencing、Final Usage 与 Residue 无法验证时 Provider 不进入 Ready，Metric 不作为 Billing Truth。
- Cloud Firecracker Placement 同时遵循 REQ-2026-0016 与 REQ-2026-0017：Scheduler 只能消费 Control Plane 签发且 Identity/Attestation/Artifact/Policy/Health/Capacity Revision 一致、新鲜、`sandbox_active` 的 Verified Node Projection；`SandboxAdmissionGrant` 与 PostgreSQL `SandboxCapacityReservation` 必须在 Provider Allocate 前有效且已确认，`SandboxPlacementDecision` 绑定 Opaque Node、Provider、Runtime Binding、Fencing 与 Resource Vector。Firecracker Provider、Host Broker 和 Node Agent 不拥有 Enrollment、Attestation Approval、Inventory Verification、Admission、Scheduler、Placement、Quota 或 Priority，也不能在 Trust/Reservation 失败时回退 Local/Docker。
- Provider-private Allocation State 持久记录当前 `sandbox_runtime_binding_id` 与最高已观察 `sandbox_fencing_token`；所有 Mutating Operation 在副作用前拒绝较低 Token，并以原子写入/恢复测试证明 Node 进程重启后仍成立。
- Stop 先执行 Guest 有界 Shutdown，再升级为 VMM Termination；Destroy 必须幂等清理 VMM/Jailer Process、cgroup、Network Namespace、Tap、API Socket、Ephemeral Disk、Secret/Control Channel 和临时 Metadata，但不得删除 Agents-owned Workspace。
- Start Readiness 同时证明 VMM Running、Guest Agent Authenticated/Ready、Policy Enforced、Workspace Attached、Resource Limit Active 与 Fencing Current；缺一项不能返回 `sandbox_provider_ready=true`。
- Snapshot/Restore/Warm Pool 在第一版 Descriptor 中保持未声明。Pool 后续边界由 REQ-2026-0019、ADR-20260730 和 `specs/sandbox-runtime-pool.contract.json` 管理；启用前必须证明 Firecracker/Kernel/RootFS/CPU 兼容矩阵、Secret 排除、Tenant Sanitization、Integrity 与回滚策略。
- 实际 Linux KVM Conformance 覆盖冷启动、Command Execution、Timeout/Cancel/Output Bound、Stale Fencing、Guest Escape Negative Cases、Metadata/Egress Denial、Resource Exhaustion、VMM Crash、Node Process Restart、Destroy Cleanup 与跨 Tenant Residue Scan。

## Candidate Non-functional Requirements

| 领域 | 要求 |
| --- | --- |
| Security | MicroVm 声明必须由真实 KVM + Jailer + Seccomp + cgroup + Network/Workspace Policy 证据支持；功能模拟器、Mock、WSL 或无 KVM CI 不构成 Assurance。 |
| Privacy | Guest Disk、Terminal Output、Vsock Payload、Crash Log 与 Allocation Metadata 按 Tenant/Session 分类；销毁与重分配前证明无上一 Tenant 残留。 |
| Performance | 冷启动、Guest Ready、Command Start、Stop/Destroy 分别记录 p50/p95/p99；目标只在固定硬件、镜像、版本与样本量的基准记录后成为 Release Gate。 |
| Reliability | VMM/Jailer Crash、Node Agent Restart 和 Control-plane Retry 不产生双重活动 Binding；残留资源可检测、隔离并幂等回收。 |
| Observability | 提供低基数 Allocate/Start/Stop/Destroy/Command Duration、Failure Kind、Active microVM、cgroup Saturation、Cleanup Failure 与 Policy Denial Metric；不记录 Host/Guest 私有标识。 |
| Operations | 发布物固定 Firecracker/Jailer/Kernel/RootFS/Guest Agent Digest，包含 SBOM、Provenance、Checksum、漏洞响应、Rollout/Rollback 与 Node Drain Runbook。 |

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `APPLICATION_LAYERED_ARCHITECTURE_SPEC.md`, `COMPONENT_SPEC.md`, `SECURITY_SPEC.md`, `DEPLOYMENT_SPEC.md`, `RUNTIME_DIRECTORY_SPEC.md`, `OBSERVABILITY_SPEC.md`, `PERFORMANCE_SPEC.md`, `SUPPLY_CHAIN_SECURITY_SPEC.md`, `RUST_CODE_SPEC.md`, `TEST_SPEC.md`.

Components: proposed `crates/sdkwork-sandbox-provider-firecracker`, `crates/sdkwork-sandbox-provider-spi`, `crates/sdkwork-intelligence-sandbox-service`, future reviewed Workspace Attachment Adapter, and future Service Host composition.

Decisions: [ADR-20260729: Firecracker Provider Isolation And Node Boundaries](../../architecture/decisions/ADR-20260729-firecracker-provider-isolation-and-node-boundaries.md), [ADR-20260729: Sandbox Firecracker Artifact Compatibility And Supply Chain](../../architecture/decisions/ADR-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain.md), [ADR-20260729: Sandbox Workspace Block Device Attachment And Sanitization](../../architecture/decisions/ADR-20260729-sandbox-workspace-block-device-attachment-and-sanitization.md), [ADR-20260729: Sandbox Firecracker Network Isolation And Egress Policy](../../architecture/decisions/ADR-20260729-sandbox-firecracker-network-isolation-and-egress-policy.md), [ADR-20260729: Sandbox Firecracker Resource Isolation And Usage Facts](../../architecture/decisions/ADR-20260729-sandbox-firecracker-resource-isolation-and-usage-facts.md), [ADR-20260729: Sandbox Multi-tenant Admission, Scheduling And Capacity Reservation](../../architecture/decisions/ADR-20260729-sandbox-multi-tenant-admission-scheduling-and-capacity-reservation.md), [ADR-20260729: Sandbox Node Trust, Enrollment, Attestation And Inventory](../../architecture/decisions/ADR-20260729-sandbox-node-trust-enrollment-attestation-and-inventory.md), [ADR-20260729: Sandbox PostgreSQL Quota And Capacity Reservation Persistence](../../architecture/decisions/ADR-20260729-sandbox-postgresql-quota-and-capacity-reservation-persistence.md), [ADR-20260729: Sandbox Host Isolation Broker Boundary](../../architecture/decisions/ADR-20260729-sandbox-host-isolation-broker-boundary.md), [ADR-20260729: Sandbox Command Execution And Terminal Boundary](../../architecture/decisions/ADR-20260729-sandbox-command-execution-and-terminal-boundary.md), and [ADR-20260728: Agents Workspace And Sandbox Attachment Ownership](../../architecture/decisions/ADR-20260728-agents-workspace-and-sandbox-attachment-ownership.md).

## Verification Plan

Repository verification will include Cargo Format/Check/Test/Clippy, strict Component Port Binding, Layering, Naming, Documentation, Packages Layout, Baseline and Supply-chain checks. Release evidence additionally requires a real Linux KVM matrix that boots the pinned image and runs Provider Common Conformance plus Firecracker-specific isolation, cleanup, fencing, resource, network and tenant-sanitization tests.

Gate 0 candidate evidence includes `specs/sandbox-provider-delivery-gates.contract.json`, which fixes the proposed Firecracker Kind `firecracker`, Assurance `MicroVm`, Linux KVM x86_64/aarch64 targets, fail-closed preflight, Jailer/cgroup/Workspace/Fencing/Policy evidence, deferred first-version capabilities, forbidden public metadata and forbidden Local/Docker fallback. It consumes the Command、Host Broker、Artifact、Workspace、Network、Resource、Scheduling、Node Trust and PostgreSQL Quota/Capacity machine contracts under `apis/commands/` and `specs/` as mandatory draft preflight dependencies. All contracts remain `draft` with `implementationAuthorized: false`; they do not create the Firecracker crate, publish artifacts, attach devices, implement networking/resources/usage/admission/scheduling/capacity/node trust/persistence, or provide KVM assurance.

## Release Boundary

本需求只交付候选 Firecracker Provider 与真实 KVM 证据。没有已批准且已物化的 REQ-2026-0016 Admission/Scheduler/PostgreSQL Capacity Reservation、REQ-2026-0017 Node Trust/Enrollment/Attestation/Verified Inventory、Secret/KMS、Production Deployment、Monitoring/Alerting、Incident Runbook、Artifact Release Supply Chain 与人工安全评审时，不得标记 `accepted` 或宣称 SaaS Commercial Ready。

## Implementation Authorization

`ready` since 2026-09-24: the repository owner approved the corresponding architecture/security packet for every listed reviewer role via the structured implementation-gate decision of that date, promoting this requirement to `ready` and its decision record to `accepted`. The packet's evidence obligations (real-platform conformance, dependency review items, and runner ownership where named) remain standing evidence requirements for the implementation slices; approval disposes the review, not the evidence.
