# REVIEW-20260729: Sandbox Firecracker Artifact Compatibility And Supply Chain

Status: pending-human-review

Requirement: [REQ-2026-0012](../../product/requirements/REQ-2026-0012-sandbox-firecracker-artifact-compatibility-and-supply-chain.md)

Decision: [ADR-20260729](../../architecture/decisions/ADR-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain.md)

Owner: SDKWork Runtime Platform

Date: 2026-07-29

Risk: critical - MicroVm release inputs, executable integrity, signature trust, vulnerability response, node materialization, revocation, drain, quarantine and rollback.

## Scope

本 Review 请求人工评审 `SandboxFirecrackerArtifactManifest` 的候选命名、Artifact Roles、Architecture-specific Compatibility Tuple、Evidence、Signature/Key Authority、Runtime Materialization、TOCTOU 防护、Revocation/Advisory、Rollback、Public Redaction 和 Ownership。

本 Review 不批准真实 Artifact/Manifest、版本或 Digest、Build/Release Workflow、Signing Key、Registry、Download、Provider/Broker Runtime、Node Service、Config、Secret/KMS、Deployment Profile 或 `agent.app.config.json`。

## Candidate Contract Evidence

| Evidence | Result |
| --- | --- |
| `specs/sandbox-firecracker-artifact-compatibility.contract.json` | Draft exact-role/tuple, immutable evidence, staging, revocation, rollback, readiness and ownership boundary; `implementationAuthorized` is `false`. |
| `node --test tests/contract/sandbox-firecracker-artifact-compatibility.contract.test.mjs` | PASS (7/7); static candidate verification only, not release, signature, artifact, runtime or KVM evidence. |
| `specs/sandbox-provider-delivery-gates.contract.json` | Firecracker Gate consumes this contract before Artifact Preflight may become Ready. |
| `node --test tests/contract/*.test.mjs` | PASS (107/107) for the complete repository contract suite after Command Execution/Cancel, Workspace Block Device/Sanitization, Firecracker Network/Resource Isolation, Multi-tenant Scheduling/Capacity, Node Trust/Verified Inventory, and PostgreSQL Quota/Capacity Persistence integration. |
| Cargo workspace gates | `fmt --check`, offline `check`, offline `test`, and offline all-target Clippy with `-D warnings` PASS; 41 Rust tests pass and the one explicitly external PostgreSQL fixture remains ignored without `SDKWORK_DATABASE_TEST_POSTGRES_URL`. |
| SDKWORK repository gates | Documentation, packages layout, strict component ports, application layering, Rust composition, identity naming, docs-debt, repository baseline, and `git diff --check` PASS. |
| Real release and KVM evidence | Absent by design; no Manifest, Artifact bundle, release pipeline or Firecracker Runtime exists. |

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| ARTIFACT-01 | Public candidate name is `SandboxFirecrackerArtifactManifest`; fields use `sandbox_`. | Establishes one reviewable machine authority. | Rename requirement, ADR, contract and tests before public implementation. |
| ARTIFACT-02 | Exact Architecture Tuple contains Firecracker, Jailer, Guest Kernel, RootFS, Guest Agent and optional Initrd digests plus boot/protocol/schema compatibility. | Prevents partial and cross-architecture combinations. | Supply an equally strict compatibility authority before Provider work. |
| ARTIFACT-03 | Every Artifact requires checksum, signature, SBOM, provenance, source/build/toolchain, license and advisory evidence. | Makes Release Inputs traceable and revocable. | No Release or Provider readiness until an equivalent evidence policy is approved. |
| ARTIFACT-04 | Release Authority publishes/revokes; Provider only validates/consumes; Broker materializes only authorized sets; Service Host/Kernel/Agents cannot override Digest. | Separates publication from execution. | Update ownership ADRs and repeat threat review. |
| ARTIFACT-05 | Runtime download, arbitrary URL, `latest`, mutable alias, source checkout fallback and automatic image build are forbidden. | Removes mutable/networked release authority from Runtime. | Define and approve a stronger immutable distribution model. |
| ARTIFACT-06 | Atomic read-only regular-file materialization rejects Symlink/Hardlink and revalidates file identity after open before use. | Constrains substitution and TOCTOU. | Provide equivalent host filesystem integrity evidence. |
| ARTIFACT-07 | Revoked/Critical/Unknown advisory state blocks Allocate/Start/Recovery; Active Allocation uses reviewed Drain/Quarantine. | Makes vulnerability response fail closed. | No commercial use until an equivalent response policy exists. |
| ARTIFACT-08 | Rollback selects a previous approved, unrevoked, same-architecture exact Manifest Digest and is audited. | Keeps rollback immutable and reproducible. | Provide a stronger rollback authority before release. |
| ARTIFACT-09 | Public surfaces expose only safe Manifest identity/readiness; physical paths, URLs, credentials and signing material remain private. | Preserves Host and secret boundaries. | Provide equivalent redaction and access-control proof. |
| ARTIFACT-10 | Artifact Readiness is necessary but not sufficient for `MicroVm`; real KVM isolation evidence remains mandatory. | Prevents false assurance. | The Firecracker Provider cannot be released. |

## Pre-review Blocking Findings

1. Exact Firecracker/Jailer/Kernel/RootFS/Guest Agent/Initrd versions, digests and compatibility values are not selected.
2. Build/Release Authority, Artifact Store/Distribution, Signature Algorithm/Trust Root, Key Custody/Rotation/Revocation and Evidence Retention owners are unresolved.
3. RootFS build inputs, Guest Agent source/build, Guest Boot Contract, RootFS Schema and Protocol compatibility/migration policy are unresolved.
4. License redistribution, upstream security advisory ingestion, Critical severity policy, unknown/stale advisory TTL and emergency revocation owner are unresolved.
5. Provider-private Runtime Root, node materialization service, ACL/ownership, atomic publication, storage quota, garbage collection and file-descriptor handoff are unresolved.
6. Active Allocation drain/quarantine timing, rollback approval, Node Drain, rollback observation and incident communications are unresolved.
7. No real signature, SBOM, provenance, artifact bundle or Linux KVM boot evidence exists in the current Windows workspace.

## Required Evidence Before Ready

- Architecture/Security/Supply-chain/Release/KVM Operations/Guest/Workspace owners accept ARTIFACT-01..ARTIFACT-10.
- Exact Architecture Tuple inventory and compatibility rationale are signed off; every Artifact resolves to immutable digest and evidence.
- Key custody, signature verification, trust-root rotation, revocation, advisory ingestion, license and evidence retention policies are approved.
- Tamper, truncation, substitution, wrong architecture, partial tuple, expired/unknown evidence, Symlink/Hardlink, TOCTOU and no-runtime-network-fetch negative tests pass.
- Node materialization, restart revalidation, drain/quarantine, previous-digest rollback and audit runbooks pass fault-injection exercises.
- Real Linux KVM target boots the exact Tuple and completes authenticated Guest Readiness, common Command Conformance and cleanup without weak Provider fallback.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`. `Approved with follow-up` cannot defer immutable identity, signature/digest verification, compatibility, key custody, revocation, Critical Advisory, no runtime download, rollback or real KVM evidence.

| Reviewer role | Reviewer | Outcome | Date | Decision IDs / findings |
| --- | --- | --- | --- | --- |
| Architecture owner | pending | pending | pending | ARTIFACT-01, ARTIFACT-02, ARTIFACT-04, ARTIFACT-10 |
| Security owner | pending | pending | pending | ARTIFACT-03..ARTIFACT-10 |
| Supply-chain/release owner | pending | pending | pending | ARTIFACT-02..ARTIFACT-08 |
| Linux KVM operations owner | pending | pending | pending | ARTIFACT-05..ARTIFACT-08, real node evidence |
| Guest image/agent owner | pending | pending | pending | ARTIFACT-02, ARTIFACT-03, compatibility and vulnerability response |
| Audit/privacy owner | pending | pending | pending | ARTIFACT-07..ARTIFACT-09 |

## Implementation Gate

REQ-2026-0012 remains `draft`, ADR remains `proposed`, and this Review remains `pending-human-review`. Until every required reviewer records `Approved` and the blocking authorities are resolved, do not publish a Manifest/Artifact, create Artifact Resolver/Downloader/Builder, add runtime paths or config, create Provider/Broker code, introduce signing keys, or claim Artifact Integrity or `MicroVm` readiness.

## Close-Out Checklist (Reviewer 执行项)

Review Approved 前必须逐项核验：

- [ ] REQ-STATUS: 对应 REQ 处于 `ready` 或 `accepted`
- [ ] ADR-STATUS: 对应 ADR 处于 `accepted`
- [ ] ARCH-REVIEW: 接口契约、命名、Port 边界、L0-L6 分层符合 COMPONENT_SPEC
- [ ] SEC-REVIEW: 数据分类、红字规则、零化清理、Secret 流、并发控制符合 SECURITY_SPEC
- [ ] PERF-REVIEW: 有界 Page/Buffer、低 Cardinality Metric 符合 PERFORMANCE_SPEC
- [ ] OBS-REVIEW: Trace/Audit/Event/Outbox/Meter 符合 OBSERVABILITY_SPEC
- [ ] TEST-EVIDENCE: Unit Test 全量通过；Contract Test 通过
- [ ] DEPENDENCY-DIRECTION: cargo tree 方向正确
- [ ] EVIDENCE-SIGN-OFF: 对应 Verification Review 接受状态非 pending
- [ ] HUMAN-DECISION: Decision Matrix 每条均 Approved 或 Changes + 替代方案

## Exit Gate

1. 全部 Checklist 勾选
2. 所有 Reviewer Role 表决 Approved
3. REQ 进入 `ready`，ADR 进入 `accepted`
4. Gate 0 `implementationAuthorized` 最后一个 Review 通过后可置 true

未经上述门禁，禁止进入 V1 实现阶段。
