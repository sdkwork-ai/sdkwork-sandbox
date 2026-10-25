---
id: REQ-2026-0012
title: Define the Sandbox Firecracker artifact compatibility and supply-chain boundary
owner: SDKWork Runtime Platform
status: draft
priority: critical
source: security
problem: Firecracker cannot truthfully claim MicroVm assurance unless every VMM, jailer, guest boot, filesystem, and control-plane artifact is an immutable, compatible, verified release input with an enforceable revocation and rollback policy.
goals:
  - Define one signed SandboxFirecrackerArtifactManifest candidate for an exact architecture-specific compatibility tuple.
  - Require digest, signature, SBOM, provenance, license, advisory, revocation, staging, and rollback evidence before Allocate or Start.
  - Keep release publication separate from Provider validation and forbid runtime downloads, mutable aliases, path overrides, or image building.
non_goals:
  - Select real Firecracker, Jailer, Kernel, RootFS, Guest Agent, Initrd, signature algorithm, signing key, registry, build pipeline, or vulnerability scanner.
  - Build, download, publish, install, cache, mirror, deploy, or execute any artifact.
  - Create a Firecracker Provider, Host Isolation Broker, image builder, public API/SDK, deployment profile, Node Enrollment, Scheduler, Secret/KMS implementation, Snapshot, Restore, or Warm Pool.
users:
  - SDKWork runtime release maintainers
  - Sandbox Provider maintainers
  - Security and supply-chain reviewers
  - Linux KVM platform operators
affected_surfaces:
  - security
  - supply-chain
  - deployment
  - operations
  - composition
---

# REQ-2026-0012: Sandbox Firecracker Artifact 兼容与供应链边界

## Readiness Blockers

- 人工确认 Artifact Release Authority、Artifact Registry/Distribution、Signature Trust Root/Algorithm、Key Custody、Revocation Feed、Advisory/Vulnerability Response、SBOM/Provenance 与 Rollback Owner。
- 选择并评审至少一个真实 `linux-kvm-x86_64` 或 `linux-kvm-aarch64` 的 Firecracker/Jailer/Guest Kernel/RootFS/Guest Agent 版本和 Digest Tuple；可选 Initrd 必须进入同一 Tuple。
- 确定 RootFS 构建来源、Guest Agent Protocol、Guest Boot Contract、RootFS Schema、Host Kernel/KVM Compatibility 和许可证分发策略。
- 确定不可变 Artifact Materialization 的 Runtime Directory、ACL、原子发布、只读文件、TOCTOU 防护、容量预算和清理 Owner。
- 确定撤销、Critical Advisory、Active Allocation Drain/Quarantine、Node Drain、Previous-known-good Rollback 与 Incident Response 流程。

## Candidate Acceptance Criteria

- 候选机器权威为 `SandboxFirecrackerArtifactManifest`；所有 Sandbox-owned 字段使用 `sandbox_` 前缀，未知字段关闭失败。Manifest 发布后不可变并由 Release Authority 签名，不由 Provider、Kernel、Agents、Service Host 或 Host Isolation Broker 修改。
- 每个 Architecture Tuple 精确绑定 Firecracker、Jailer、Guest Kernel、RootFS 与 Guest Agent 的 Version 和 SHA-256 Digest；Initrd 启用时同样绑定。禁止 Partial Tuple、跨 Architecture 复用和 Runtime Compatibility Override。
- Firecracker 与 Jailer 来自同一受支持 Release；Tuple 还固定 Host Architecture、最低 Host Kernel/KVM 要求、Guest Boot Contract、RootFS Schema 与 Guest Agent Protocol Version。
- 每个 Artifact 都有不可变 Release Reference、Checksum、Signature Reference、CycloneDX/SPDX SBOM、SLSA/in-toto-compatible Provenance、Source Revision、Build Workflow/Toolchain、License Record 和 Advisory Snapshot；Evidence 必须与 Release Version、Architecture 和 Digest 一致。
- Signature Private Key、Build Credential、Guest Credential、Host Path、Download URL、API Socket 与 Provider-private Runtime Root 不进入 Manifest、公用 Result、Log、Metric、Event、Audit Detail 或 SDK Type。
- Provider 只验证并消费由 Composition 注入的已批准 Manifest Reference。Provider 不构建、发布或下载 Artifact；Host Isolation Broker 不选择或构建 Artifact；Service Host 不覆盖 Digest；Kernel/Agents 不选择 Provider-private Artifact。
- Runtime 禁止网络下载、任意 URL、`latest`、Mutable Alias、未校验本地文件和 Source Checkout Fallback。Artifact 只能原子物化到 Provider-private Runtime Root，必须是权限收紧的只读 Regular File；拒绝 Symlink、Hardlink、文件身份变化和校验后替换。
- Allocate 前选择并验证 Manifest；Start 前重新验证 Manifest Signature、Artifact Digest、Evidence、Revocation/Advisory、Architecture Compatibility 与 Runtime File Identity。任一缺失、未知或变化都返回 Not Ready，不能启动或降低 Assurance。
- Revoked Tuple、Critical Advisory 或未知 Advisory State 阻止新 Allocate/Start，也不能用于 Recovery。Active Allocation 必须进入经评审的 Drain 或 Quarantine 流程，不能静默继续分配。
- Rollback 只能指向此前批准且未撤销的精确 Manifest Digest；禁止通过重建、Mutable Alias 或跨 Architecture 替代执行回滚。选择、批准、执行和结果必须形成审计事实。
- Artifact Readiness 仅证明供应链输入满足候选边界，不单独构成 `IsolationAssurance::MicroVm`；真实 KVM、Jailer、Seccomp、cgroup、Network、Workspace、Fencing、Guest Readiness 和 Cleanup 证据仍由 Firecracker Provider Gate 共同证明。

## Candidate Non-functional Requirements

| 领域 | 要求 |
| --- | --- |
| Security | Tamper、Truncation、Substitution、Wrong Architecture、Revocation、Expired/Unknown Evidence、Symlink/Hardlink、TOCTOU 与 Runtime Fetch 必须关闭失败。 |
| Privacy | Manifest/Evidence 不包含 Secret、Credential、Tenant Data、Host Path、Guest Payload 或 Provider-private Physical Identity。 |
| Performance | Digest/Signature/Advisory 校验必须有固定 Artifact Size、Node Storage 与样本量基准；无测量前不设虚假 Release SLO。 |
| Reliability | 原子 Materialization、进程重启重验、撤销、Drain/Quarantine 与 Previous-known-good Rollback 不得产生 Partial Tuple 或静默降级。 |
| Operations | Release、Key Custody、Advisory、Node Drain、Quarantine、Rollback、Evidence Retention 与 Incident Owner 必须明确。 |

## Trace

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `ENGINEERING_WORKFLOW_SPEC.md`, `QUALITY_GATE_SPEC.md`, `SECURITY_SPEC.md`, `DEPLOYMENT_SPEC.md`, `RUNTIME_DIRECTORY_SPEC.md`, `SUPPLY_CHAIN_SECURITY_SPEC.md`, `OBSERVABILITY_SPEC.md`, `PERFORMANCE_SPEC.md`, `TEST_SPEC.md`.

Components: future reviewed `sdkwork-sandbox-provider-firecracker`, future reviewed Artifact Release Authority/Resolver, future reviewed `sdkwork-sandbox-host-isolation-broker`, and future Sandbox Service Host composition.

Decisions: [ADR-20260729: Sandbox Firecracker Artifact Compatibility And Supply Chain](../../architecture/decisions/ADR-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain.md), [ADR-20260729: Firecracker Provider Isolation And Node Boundaries](../../architecture/decisions/ADR-20260729-firecracker-provider-isolation-and-node-boundaries.md), and [ADR-20260729: Sandbox Host Isolation Broker Boundary](../../architecture/decisions/ADR-20260729-sandbox-host-isolation-broker-boundary.md).

## Verification Plan

- `tests/contract/sandbox-firecracker-artifact-compatibility.contract.test.mjs` 验证 Draft Gate、Sandbox 命名、精确 Artifact Role、Evidence、不可变 Runtime Consumption、Revocation、Rollback、Readiness 与 Ownership。
- 实现阶段必须增加真实 Manifest Schema/Parser、Signature/Digest、Wrong Architecture、Tamper/Substitution、Revocation/Advisory、Symlink/Hardlink/TOCTOU、Restart、Rollback 与无 Runtime Network Fetch 测试。
- Release 阶段必须提供真实 Linux KVM Boot/Command/Cleanup Evidence，并将精确 Manifest Digest、Artifact Evidence、SBOM、Provenance、Signature、Checksum、Advisory Snapshot 和 Rollback Target 绑定到同一 Release Record。
- Cargo、Contract、Documentation、Layering、Naming、Packages Layout、Baseline 与适用 Supply-chain Validator 全部通过。

## Release Boundary

本需求只定义 Gate 0 候选边界。它不发布真实 Manifest/Artifact，不创建 Runtime、Config、Secret、Registry、Workflow、Deployment Profile 或 `agent.app.config.json`。在人工评审、真实 Tuple、Key/Release/Advisory Owner 和 Linux KVM Evidence 完成前，REQ 保持 `draft`，不得将静态契约测试解释为 Artifact Integrity 或 `MicroVm` Assurance。
