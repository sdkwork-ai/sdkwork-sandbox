# SDKWork Sandbox 交付路线图

Status: draft

Owner: SDKWork Runtime Platform

Updated: 2026-09-22

Parent: [SDKWork Sandbox PRD](PRD.md)

Specs: `REQUIREMENTS_SPEC.md`, `PERFORMANCE_SPEC.md`, `TEST_SPEC.md`, `QUALITY_GATE_SPEC.md`, `ENGINEERING_WORKFLOW_SPEC.md`, `DEPLOYMENT_SPEC.md`

## Phase 0: Foundation

交付物：独立仓库、SDKWork L1/L2 基线、完整目录字典、文档 Canon，以及当前七个分层 Rust 组件边界。Provider SPI、Sandbox Service、Memory Repository 与 PostgreSQL Repository 已进入候选实现；Local Provider、Service Host 与 CLI 仍未激活运行行为。Local Host、Observability/Event/Audit/Outbox、Host Broker、Firecracker Artifact、Workspace Block Device、Network/Resource Isolation、Scheduling/Capacity、Node Trust、Quota Persistence、Runtime Pool、Lifecycle Hot State/Idempotency、Workspace Runtime Transaction 与 Standalone Data Residency/Recovery 仅形成 `draft` 机器契约与静态 Contract Test，不代表 Host execution、Checkpoint、存储迁移、KMS、网络、调度、数据驻留或发布能力。

退出门禁：Cargo Workspace Check/Test 通过；文档、Workspace Layout、Component Contract、Naming 与 Repository Baseline 检查通过；代码不声称已经具备可用 Sandbox 执行能力。

## V1: Local Runtime

当前进度：`REQ-2026-0002` 已实现 Provider-neutral `SandboxSession` Lifecycle 候选契约和 Memory Repository Adapter；`REQ-2026-0004` 已固定 `sdkwork-agents -> sdkwork-kernel -> sdkwork-sandbox`、Agents Workspace 权威与 `sandbox_*` ID 映射规则，并验证 Opaque Workspace Context 在 Allocate/Start Provider Request 中原样传递以及未附着 Readiness 关闭失败；`REQ-2026-0005` 已物化 PostgreSQL Repository、加密 `SandboxRuntimeBinding` 恢复元数据、Lease/Fencing、Provider 调用前续租、有界 Timeout 与瞬态 `SandboxSession` Reconciler，并固定 Start 恢复顺序为“稳定状态下清理旧 Allocation -> 原子保存 `Starting`/In-progress Start/无 Allocation Binding Intent -> Allocate”。故障注入已证明 Allocation 保存失败后 Reconciler 会以更高 Fencing Token 重新 Allocate 且只启动新 Allocation；Reconciler 抢 Lease 后会重读权威 Session，Renew/Save/Release 控制权故障统一为 `LeaseLost`，Memory/PostgreSQL 对 Fencing Token 上限耗尽一致关闭失败；`REQ-2026-0006` 的版本化 Key Source、Tenant-scoped Re-encryption、页目标 Protection Version 稳定性与 Tenant+Binding+Session+旧密文元数据 CAS 已通过真实 PostgreSQL 候选验证，并形成旧密钥撤销 Runbook Candidate；`REQ-2026-0007` 已形成 Provider-neutral Command/Terminal 候选契约。Local Host Boundary 已把 opened Capability、Filesystem Race、Windows Job Object、Linux delegated cgroup v2、macOS Terminal denial、空环境 allowlist、Cleanup/Quarantine 与 Supply-chain Gate 物化为静态机器权威；2026-09-24 仓库所有者以全部评审角色身份批准 Local Provider、Command Execution 与 Firecracker Provider 三个架构/安全 packet：`REQ-2026-0003`/`0007`/`0008` 进入 `ready`，`specs/sandbox-local-provider-host-boundary.contract.json` 翻转授权，真实 Local Host Execution 获准按切片实现。真实平台 Conformance、`cap-std`/Runner 依赖证据、多副本长稳/PITR/SLO 与撤销演练仍是实现切片的证据义务，尚未激活。

当前 Lifecycle Repository 仍在普通 hydrate、Operation lookup 和 Reconciliation 候选读取中加载完整有序 Operation 历史，且没有批准的最大 Operation 数、最大 Session 生命周期或终态幂等保留策略。`REQ-2026-0020`、对应 ADR/Review 与机器契约已将 bounded Hot State、point-lookup Idempotency Ledger、Late Retry、Retention Worker 和 expand/backfill/cutover Migration 建立为独立 Gate 0；精确值及行为仍待人审，当前不改变 REQ-2026-0005 的已验收候选实现。

`REQ-2026-0022` 已将 Local 商业数据承诺建立为独立 Gate 0：`standalone` 或 Local Provider 不能推导设备本地性，BirdCoder 不拥有业务表，Agents 与 Sandbox 服务权威仍使用物理位于本机的 PostgreSQL，嵌入式 SQLite 只允许声明为 `client-local` 的有界状态。完整数据清单、独立 Capability、无隐式远程副本/遥测、角色正确的备份恢复、导出清除、故障关闭和 Windows/macOS/Linux 真实证据均未实现，因此当前不能发布全部数据本地、严格本地处理或本地恢复声明。

交付顺序固定为 Local Provider -> 共享 Command/Terminal Conformance -> Firecracker Provider。Docker Provider 本阶段不实施、不作为测试替代、不作为 Capability 或 Assurance 回退。

`REQ-2026-0007` 当前已有 `apis/commands/` draft Execution/Cancel Request、Result/Error Schema、Canonical Fingerprint/Idempotency/Terminal Completion Catalog 和静态 Contract Test；它们固定 `.` Workspace Root、可移植 Path/Windows Device 与 Console Alias/UTF-8 Byte Bound、Fenced Cancel、Command Result Replay、Result-unavailable 同 Operation 重试、durable first-terminal CAS、Outcome/Exit/Truncation、Cleanup Status/Binding Quarantine 与“已启动后终态 Result、启动前/结果不可得 Error”的共享语义，2026-09-24 人审通过并授权实现（`REQ-2026-0007` 进入 `ready`、ADR 进入 `accepted`）；实现按 Provider SPI -> Service/Registry -> Common Conformance -> Kernel/Agents Dependency Chain 顺序进行。

候选需求切片：

1. Runtime Request/Response 与 Capability Negotiation 契约；实现对象固定为 `CreateSandboxSessionCommand`、`SandboxSessionLifecycleCommand`、`SandboxSession` 与 `SandboxRuntimeBinding`，不引入替代产品术语。
2. Agents-owned Workspace Identity 与 Sandbox-owned Containment/Attachment/Git Runtime Capability。
3. `SandboxSession`/`SandboxSessionState` 状态机与带 `sandbox_*` 字段的幂等生命周期 Command；共享 `OperationId` 在该边界使用 `sandbox_operation_id`。
4. Windows suspended Job Object、Linux race-free delegated cgroup v2 与 macOS Terminal denial 的 Local Provider 平台切片及公开保证限制；Process Group-only、spawn 后 attach 与字符串 Path containment 不作为保证。
5. Provider-neutral Command/Terminal、Process、Filesystem、Environment 与 Build 能力；Port、Network 与 Browser 在专项 Requirement 前不声明。
6. Resource Limit、Log/Event Streaming、本地恢复与 Operator CLI。
7. PostgreSQL `SandboxSession`/Operation/`SandboxRuntimeBinding` Authority、Tenant-scoped Lease/Fencing 与 Crash Reconciliation；Memory Repository 不作为 Server Authority。
8. 经 REQ-2026-0020 人审批准后，将 Session hydrate 收敛为 bounded Hot State + current Operation，并以独立 durable Idempotency Ledger 保留点查重放/冲突语义；禁止静默截断或猜测 TTL。
9. 经 REQ-2026-0022 人审批准后，按数据角色组合本地 PostgreSQL/`client-local` Store、Workspace/Runtime/Cache/Log/Secret/Temp Capability、无隐式传输、Backup/Restore、Export/Purge 与 Uninstall Preservation；任何缺失证据使 Local 驻留声明 Not Ready。

退出门禁：Windows/Linux 上声称的 Local Terminal/Filesystem Conformance 通过，macOS Terminal Denial/无回退 Conformance 通过；Kernel 只使用经过评审的 Provider-neutral Port 且无 Sandbox Provider 分支；安全测试覆盖 Path/Link/Mount/Identity Race、Process Cleanup、Ambient Credential Denial、Environment/Secret Redaction、Quota 与破坏性操作。未交付 Network Policy 前 Descriptor 不声明 Network/Browser/Port Capability。

## V1 Data Governance Slice: Standalone Local Residency

`REQ-2026-0022`、对应 ADR/Review 与机器契约当前分别为 `draft`/`proposed`/`pending-human-review`。该切片仅为 `sandbox_standalone_local` 增加四仓 Data Inventory、两种候选 Claim、Database Role、Runtime Directory Separation、Transfer/Sync、Backup/Restore、Export/Purge、Failure 和真实 OS Evidence Gate；它不创建 BirdCoder 业务数据库，不把 Agents/Sandbox Server Authority 改为 SQLite，也不授权 Runtime Path、Database/Migration、Backup、Telemetry、Sync、API/SDK、Installer 或跨仓库源代码变更。

## V1 Composition Governance Slice: Service Host

`REQ-2026-0009` 与对应 ADR 当前为 `draft`/`proposed`，只定义 L5 typed Service Host Composition、依赖注入、fail-closed Readiness、安全 Shutdown 与 Standalone/Cloud parity。它不激活 Local/Firecracker Provider、HTTP/API/SDK、Scheduler、Secret/KMS 实现或 Deployment Profile；实现前必须完成相关公共命名、Config/Secret/Telemetry Port、Workspace Attachment、Provider Fencing 和运维 Owner 的人工评审。

## V1/V2 Integration Governance Slice: Internal Control Plane

`REQ-2026-0023`、对应 ADR/Review 与 `sandbox-internal-control-plane.contract.json` 当前分别为 `draft`/`proposed`/`pending-human-review`/`draft`。该切片提出一个 Sandbox-owned application port，Standalone 通过 in-process adapter 组合，Cloud 通过 future Proto/RPC manifest 生成的 L3 internal-RPC client 组合；两条路径共享 lifecycle/transaction/idempotency/error/readiness conformance。Trusted context、mTLS/workload identity、independent Kernel/Sandbox lease/fencing、ambiguous-result lookup、deadline/cancellation、bounded operation-event stream、version fail-close、discovery/drain/rollback 和 real multi-process evidence 都是前置门禁。当前不创建 Rust Port、Proto、SDK、server/client、HTTP route、discovery、config、deployment 或 Kernel 源码变更。

## V1/V2 IDE Governance Slice: Interactive Terminal

`REQ-2026-0024`、对应 ADR/Review 与 `sandbox-interactive-terminal-session.contract.json` 当前分别为 `draft`/`proposed`/`pending-human-review`/`draft`。该切片纠正当前候选 `Terminal` 同时暗示非交互 Command 和 PTY 的歧义，提出 `Command`/`InteractiveTerminal` capability split、独立 Terminal Session Port、single-controller lease、at-most-once input、idempotent resize、ordered bounded output replay、disconnect/reconnect grace、first-terminal CAS、Workspace freeze/checkpoint ordering和 Windows/Linux/Firecracker exact containment；macOS 保持显式 Denied。当前不批准任何 Capability 公共命名、PTY/ConPTY、Process、Guest Agent Stream、Proto/SDK/API、Persistence、Provider、Service Host、Deployment 或跨仓库实现。

## V1/V2 Security Governance Slice: Runtime Secret Projection

`REQ-2026-0025`、对应 ADR/Review 与 `sandbox-runtime-secret-projection.contract.json` 当前分别为 `draft`/`proposed`/`pending-human-review`/`draft`。该切片保持 BirdCoder、Agents、Kernel 与 Sandbox Control Plane value-free，由 Agents/IAM/approved Secret Authority/Kernel/Sandbox 分别拥有 business intent、authorization、value/version/grant、opaque transport 与 projection lifecycle；Local 与 Cloud Authority 不同步、不跨 lane/device/region 回退。候选 target 仅为 immutable registry 中的 process handle、protected ephemeral file 和显式 environment exception；rotation/revocation/outage、Checkpoint exclusion、Secret-exposed microVM destroy、audit split 和 scoped exfiltration claim 均关闭失败。当前不批准 Secret/KMS/Keychain、value transport、Process Projection、Host Broker/Guest Agent、Proto/SDK/API、Persistence、Provider、Service Host、Deployment 或跨仓库实现。

## V2 Governance Slice: Cloud Data Residency And Recovery

`REQ-2026-0026`、对应 ADR/Review 与 `sandbox-cloud-data-residency.contract.json` 当前分别为 `draft`/`proposed`/`pending-human-review`/`draft`。该切片仅治理 Cloud lane，区分 `regionCode`、`providerRegion`、`storageRegion` 与 `availabilityZone`，并为 Workspace、Revision、Checkpoint、output、log、cache、backup/PITR、replica、export/delete 和 support data 指定唯一 authority、retention、encryption、residency 与 recovery owner。Cross-region replication/failover 必须显式授权、保留 deletion tombstone 和 legal hold，恢复顺序必须先验证 Agents/Drive data 与 Revision/Checkpoint，再创建新的 Kernel/Sandbox placement/fencing 和 Secret grant；任何未知位置、滞后副本、PITR gap、删除不确定或 cleanup uncertainty 都关闭失败。当前不批准 region/storage choice、replication、backup/restore/purge worker、API/SDK、Provider、Service Host、Deployment 或跨仓库实现。

## V2 Governance Slice: Cross-Repository Version Compatibility

`REQ-2026-0027`、对应 ADR/Review 与 `sandbox-cross-repository-version-compatibility.contract.json` 当前分别为 `draft`/`proposed`/`pending-human-review`/`draft`。该切片引入不可变 `SandboxCrossRepositoryReleaseSet`，固定 BirdCoder、Agents、Kernel、Sandbox、Workspace/Storage、RPC/Proto、generated SDK、runtime config、Local Provider、Firecracker artifact 与 evidence provenance，并按 semantic、wire、SDK、data/schema、Workspace/Checkpoint、artifact/guest protocol、residency、Secret 和 isolation assurance 维度判定兼容性。Peer preflight 必须早于 placement、mount、Secret projection、Command/Terminal attach 和 recovery；不兼容升级先停止新 placement，再 drain active transactions；rollback 只能选择已批准 immutable set，downgrade 默认拒绝，support window 到期只返回 `upgrade-required`。当前不批准 release authority、compatibility registry、SDK/proto/artifact publication、migration、rollout worker、deployment 或跨仓库实现。

## V1 Governance Slice: Observability And Events

`REQ-2026-0010`、对应 ADR 和 Review 当前分别为 `draft`/`proposed`/`pending-human-review`。`apis/async/` 已形成 Sandbox-owned AsyncAPI、`SandboxEventEnvelope`、精确 `sandbox.*` Event Catalog、Outbox Contract、`SandboxAuditRecord` 与 Observability Catalog，通过静态 Contract Test 明确 telemetry、audit-fact、billing fact、retention、redaction、ordering、replay、idempotency、低基数指标、Trace、backpressure 和 PostgreSQL Outbox 事务边界。该切片不实现 Event Worker、Exporter、Outbox migration、API/SDK、Metering、Dashboard、Secret/KMS 或 Deployment Profile；Owner、Security/Privacy、Database、Operations、Retention、Release 和跨仓库 Trace Authority 人工评审完成前不得进入 `ready`。

## V2: Isolated Cloud Runtime

当前候选入口：`REQ-2026-0008` 与对应 Firecracker ADR 已定义 Linux KVM、Jailer、Artifact Integrity、cgroup v2、Network Namespace、Workspace Block Device、Vsock、Fencing、Cleanup 与 Tenant Sanitization 门禁。`REQ-2026-0016` 已固定 Admission/Scheduler/Capacity，`REQ-2026-0017` 已固定 Node Trust/Verified Inventory，`REQ-2026-0018` 已固定 PostgreSQL Quota/Capacity Gate，`REQ-2026-0019` 已拆分 tenant-neutral `PreparedSlot` 与另行取证的 `WarmMicroVmSlot`。`REQ-2026-0021` 进一步固定 Workspace Revision -> Allocation -> Attachment -> Command -> Durable Checkpoint/Handoff -> Detach/Sanitization -> Release 的组合事务，但不批准任何 Runtime 实现。后续独立切片包括这些边界获批后的实现 Requirement、Provider Snapshot、Recovery、Internal API/SDK、Cluster Service Host 与 Application Ingress。

`REQ-2026-0011`、对应 ADR 和 Review 已把 Firecracker 的 Host Privilege 前置项收敛为 draft `SandboxHostIsolationBroker` 候选边界：固定八类 typed operation、Linux Unix Domain Socket、peer identity、短期签名 Grant、执行点 Fencing/Idempotency、最小 Privilege Profile、durable Audit 和 bounded Cleanup。当前仅有机器契约与静态测试，不创建 Broker crate/daemon/socket/service unit；Grant/KMS、Privilege、Journal、Protocol、Package/Upgrade/Rollback 和真实 KVM 证据完成人审前不得进入实现。

`REQ-2026-0012`、对应 ADR 和 Review 已把 Firecracker Artifact 前置项收敛为 draft `SandboxFirecrackerArtifactManifest` 候选边界：按 `linux-kvm-x86_64`/`linux-kvm-aarch64` 固定 Firecracker、Jailer、Guest Kernel、RootFS、Guest Agent 与可选 Initrd 的精确 Digest Tuple，要求 Signature、SBOM、Provenance、License、Advisory、只读原子 Materialization、Revocation/Drain/Quarantine 和 Previous-digest Rollback。当前不选择真实版本、不发布或下载 Artifact、不创建 Builder/Resolver/Provider Runtime；Release/Key/Advisory/Node Owner、真实签名 Evidence 与 Linux KVM Boot 完成人审前不得进入实现。

`REQ-2026-0013`、对应 ADR 和 Review 已把 Workspace Data-plane 前置项收敛为 draft `SandboxWorkspaceBlockDevicePort` 候选边界：Agents 保留 Workspace 业务权威，Drive 保留适用的文件/对象存储权威，Sandbox 只拥有一次 Session/Binding 的授权 Runtime Projection；Grant、Revision、Fencing、At-rest Encryption、Guest Device、Readiness、Detach、Ephemeral Cryptographic Erase、Residue Scan 和 Quarantine 关闭失败。当前不创建 Storage/Drive/Volume Adapter、KMS、Device、Mount 或 Sanitization Runtime；Backend/Key/Retention/Quarantine Owner 与真实 Linux KVM Evidence 完成人审前不得进入实现。

`REQ-2026-0014`、对应 ADR 和 Review 已把 Firecracker Network 前置项收敛为 draft provider-neutral `SandboxNetworkPolicyPort` 与 L4 `SandboxNetworkIsolationPort` 候选边界：默认 `DenyAll`，只接受显式 DNS/Egress Grant，永久拒绝 Cloud Metadata、Host Control Plane 与 Tenant Lateral Traffic；每个 Binding 独立 netns/Tap，Policy Revision/Fencing、Atomic Apply/Readback/Probe、Teardown/Residue/Quarantine 与 Durable Audit 关闭失败。当前不创建 Network Port/Adapter、netns、Tap、nftables/Firewall、Route、DNS Proxy 或 Runtime Config；Policy/Privilege/Network/KVM Owner 与真实 Linux KVM Evidence 完成人审前不得进入实现。

`REQ-2026-0015`、对应 ADR 和 Review 已把 Firecracker Resource 前置项收敛为 draft provider-neutral `SandboxResourcePolicyPort`、L4 `SandboxResourceIsolationPort` 与 immutable `SandboxResourceUsageFact` 候选边界：Firecracker Guest Shape 与 per-binding cgroup v2 CPU/Memory/PID/IO 双重执行，Grant/Ceiling/Node Reservation、Fencing、Effective Readback、typed Limit Outcome、Final Usage、Durable Handoff、Cleanup/Residue/Quarantine 关闭失败；Sandbox/Metric 不拥有 Price/Invoice/Payment。当前不创建 Resource Port/Adapter、Quota Engine、cgroup、Machine Config Runtime、Usage Collector/Aggregator 或 Commerce Adapter；Capacity/Policy/Commerce/KVM Owner 与真实 Evidence 完成人审前不得进入实现。

`REQ-2026-0016`、对应 ADR 和 Review 已把 SaaS 调度前置项收敛为 draft provider-neutral `SandboxAdmissionPolicyPort`、`SandboxNodeInventoryPort`、`SandboxSchedulerPort` 与 `SandboxCapacityReservationPort` 候选边界：Admission 原子预留 Tenant Quota，Scheduler 先执行 Capability/OS/Architecture/Assurance/Locality/Residency/Policy/Health/Capacity 硬过滤，PostgreSQL 原子 Reservation 在 Provider Allocate 前完成并约束 Resource Grant；Fairness、Fencing、Idempotency、Expiry/Orphan Recovery、Event/Metric 与隐私关闭失败。当前不创建 Scheduler/Admission Runtime、Database Schema、Node Agent/Enrollment、Warm Pool、Provider Placement 或 Commerce Runtime；IAM/Commerce/Node Trust/Database/Capacity/Operations/KVM Owner 与真实并发/规模/故障证据完成人审前不得进入实现。

`REQ-2026-0017`、对应 ADR 和 Review 已把 Cloud Firecracker Node Trust 前置项收敛为 draft provider-neutral `SandboxNodeEnrollmentPort`、`SandboxNodeAttestationVerificationPort`、`SandboxNodeInventoryPublicationPort` 与 `SandboxNodeLifecycleControlPort` 候选边界：Bootstrap 单次短期，Machine Identity Key-bound 且双向认证，Authentication 与 Platform Attestation 分离，Control Plane 只把 Identity/Attestation/Artifact/Policy/Health/Capacity Revision 一致且新鲜的 Verified Inventory 交给 Scheduler；Rotation/Revocation、Clone/Compromise、Drain/Quarantine、Event/Metric 与隐私关闭失败。当前不创建 Node Agent、PKI/CA/HSM、Attestation Verifier、Database Schema、Scheduler/Provider Integration 或 Deployment Profile；Security/PKI/Attestation/Database/Operations/KVM Owner 与真实 Machine Identity、Attestation、多副本、升级/故障证据完成人审前不得进入实现。

`REQ-2026-0018`、对应 ADR 和 Review 已把 REQ-2026-0016 的 PostgreSQL 前置项收敛为 draft `SandboxTenantQuotaState`、`SandboxAdmissionReservation`、`SandboxNodeCapacityState` 与 `SandboxCapacityReservation` 候选对象：State Counter 与 Reservation Fact 在同一短事务中按全局 Lock Order、Version CAS、Fencing、Database Clock 和完整事务重试更新；Confirmed/Bound 状态不确定时保留占用并 Quarantine，不能用 TTL 猜测释放。现有 `TenantId`/`tenant_id TEXT` 与 `SUBJECT_ID_SPEC.md` 正数 `BIGINT` 的偏差被列为新表实现前的预发布 Migration Blocker。当前四张 Lifecycle Active Table、Registry、Migration 与 Rust Repository 均未改变；Subject Migration、四表命名、RLS/Role、PITR/RPO/RTO 和真实 PostgreSQL/Firecracker 证据完成人审前不得实现。

退出门禁：多租户隔离与恢复 Threat Model 通过人工安全评审；Node Loss、Control Plane Restart、Pool Sanitization、Snapshot Integrity 与 Quota Contention 测试通过；Standalone 与 Cloud 保持同一契约。

## V3: Elastic Platform

候选需求切片：重新评审延期的 Docker Provider，以及 Kubernetes、gVisor、Remote VM、跨企业/私有云联邦 Node Enrollment、GPU Policy、Multi-cluster、High Availability、Region-aware Placement 和 Browser Sandbox/WASM 可行性门禁。基础 Cloud Firecracker Node Trust 已由 REQ-2026-0017 进入 V2 Gate 0，联邦 Enrollment 不能替代该基础安全门禁。

退出门禁：Provider 限制可机器发现；多集群故障与容量测试达到定义的 SLO；GPU、Browser 或 WASM 未通过工作负载证据前不得对外宣称支持。

## V4: Runtime Platform

候选结果：为 SDKWork IDE、Web IDE、Desktop、Browser、Workflow、DevOps、Automation 与 Serverless Agent 提供统一执行底座；建立第三方 Provider 治理与 Conformance 体系；实现工作负载感知调度、成本/计量优化与多区域恢复。

每项工作都必须拥有独立 Requirement、必要 ADR、Verification、Release Evidence 与 Rollback Plan。版本标签只表达产品顺序，不构成对未评审范围的交付承诺。

## 能力建设顺序与阶段映射

产品能力有一个自然的建设顺序，它与本仓交付阶段的关系如下。本表只表达建设先后，**不构成实施授权**：每一行的能力仍须先形成 `ready` 的 `REQ-*`。

| 建设顺序 | 内容 | 映射阶段 | 当前门禁 |
| --- | --- | --- | --- |
| 1. 原生执行底座 | 生命周期核心、文件系统、进程、资源限制、安全约束、网络策略、API 面 | V1 | `REQ-2026-0002`~`0007`（部分候选实现，含 `draft`） |
| 2. 池化与状态物化 | Runtime Pool、Scheduler、Template、Workspace、Snapshot | V1/V2 交界 | `REQ-2026-0016`~`0021`（全部 `draft`） |
| 3. 强隔离 | microVM 后端、Snapshot 恢复、按需内存、写时复制根文件系统 | V2 | `REQ-2026-0008`、`0012`~`0015`（全部 `draft`） |
| 4. 集群能力 | 集群 Placement、自动扩缩、高可用、故障转移、迁移 | V2/V3 | `REQ-2026-0016`、`0017`（全部 `draft`） |
| 5. Agent 集成 | Sandbox 内 Agent 运行时、MCP、Skills、Browser、Computer Use | V3/V4 | 未授权（`REQ-2026-0024`、`0025` 仅门禁） |

组件拆分与 Crate 命名必须遵守既有架构决策与 `NAMING_SPEC.md`；**不得**由能力清单直接推导出组件结构，也不得引入被禁止的通用后缀组件。详见 [TECH_ARCHITECTURE.md](../../architecture/tech/TECH_ARCHITECTURE.md) 第 3 节与 [TECH-runtime-backends-and-pools.md](../../architecture/tech/TECH-runtime-backends-and-pools.md)。

## 阶段性 MVP 定义

三个 MVP 是建设顺序上的**最小闭环**，不是交付承诺。每个 MVP 的完成都必须以其 `REQ-*` 的 Acceptance Criteria 为准。

### MVP 1：可闭环的原生执行

最小闭环要求：Agent 经 SDK 到达 Sandbox，可执行 shell、读写文件、访问网络。覆盖：Sandbox 创建与删除、进程执行与查询、文件读/写/列举、网络代理出口、CPU 与内存配额、命名空间/控制组/系统调用过滤/写时复制文件系统约束。

当前状态：生命周期核心有候选实现；文件系统、进程、网络与配额的实现全部未授权。SDK 家族不存在。

### MVP 2：池化与状态物化

最小闭环要求：创建、运行、暂停、恢复可稳定往复。在此基础上增加 Template、Snapshot、Fork、Runtime Pool 与预热容量。

当前状态：全部为 `draft` Gate 0，无实现、无真实 Snapshot 证据、无 Pool 运行时。

### MVP 3：强隔离与能力兼容

最小闭环要求：microVM 运行模式下完成 Snapshot 恢复、按需内存加载与写时复制根文件系统，达到与参考 Runtime 的能力兼容。

当前状态：仅有 Firecracker 制品、网络、资源、Workspace 设备、Broker 的 Gate 0 候选；无任何真实 KVM 运行证据。

## 验收标准

以下标准是本产品的能力验收口径。**所有数值都是工程目标，不是未经验证的承诺**；在参考硬件、Template、工作负载与统计方法被记录前，任何数值不得写入 Release Evidence 作为已达成指标。

### 功能验收

产品完成条件是对齐能力矩阵中的全部能力（见 [PRD-capabilities.md](PRD-capabilities.md) 第 11 节），且每项满足“有承载 `REQ-*` + 有验证证据”双条件：

```text
Create / Delete / Pause / Resume / Restart / Fork / Snapshot
Filesystem / Process / PTY / Shell
Network / Egress / Port
Template / Workspace / Runtime Pool
Resource Quota / Tenant Isolation
Metrics / Logs / Audit
```

### 性能验收

| 指标 | 目标（工程目标） | 前置条件 |
| --- | --- | --- |
| 命名空间沙箱创建 P95 | 小于 30 ms | 参考硬件、固定 Template、冷/热分别统计 |
| 命名空间沙箱恢复 P95 | 小于 30 ms | 同上 |
| 预热 microVM 恢复 P95 | 小于 300 ms | 同上；需真实 KVM 证据 |
| CPU 开销 | 小于 5% | 相对裸机同工作负载基线 |
| 单节点轻量执行环境数量 | 万级 | 需定义“轻量”的精确 Assurance 与资源档位 |

### 资源验收

| 指标 | 目标（工程目标） |
| --- | --- |
| 空闲轻量执行环境内存占用 | 小于 10 MB |
| 预热 Runtime 内存占用 | 尽可能小于 20 MB |
| microVM 资源 | 按 Guest 内存、vCPU、内核与运行时动态分配，不做统一声明 |

资源目标必须按运行模式分别测量与报告；不得用最轻模式的数据代表强隔离模式。

### 高并发验收

| 指标 | 目标（工程目标） |
| --- | --- |
| 单集群控制面吞吐 | 十万级请求/秒 |
| 创建、暂停、恢复、删除 | 必须能够水平扩展；吞吐不随副本数增加而劣化 |

并发能力不得通过增加线程数或增加容器数量换取（`PRD.md` 非目标）。控制面水平扩展的边界必须由压测确定，而不是由架构推断。

## Benchmark 与专项测试要求

### 基准套件

必须建立独立的基准套件，覆盖：创建、启动、暂停、恢复、Fork、删除、文件系统、网络、进程、Snapshot、Template，并在递增并发档位（百、千、万、十万）下分别测量。

### 性能测试矩阵

| 维度 | 取值 |
| --- | --- |
| CPU | 4 / 8 / 16 / 32 / 64 / 128 核 |
| 内存 | 32 / 64 / 128 / 256 GB |
| 磁盘 | SATA / SSD / NVMe |
| 网络 | 1G / 10G / 25G |

### 压测记录指标

必须记录 P50、P90、P95、P99、Max、错误率、CPU、内存、IOPS、带宽、上下文切换与页缺失。缺失分位数或缺失环境描述的测量结果不得作为门禁证据。

### Chaos 测试

必须覆盖：Node 崩溃、网络故障、存储故障、协调服务故障、元数据数据库故障、对象存储故障、Sandbox 崩溃、Agent 崩溃、OOM、磁盘写满、CPU 耗尽。

### 安全测试

必须覆盖：容器逃逸、命名空间逃逸、文件系统逃逸、权限提升、系统调用过滤绕过、网络逃逸、云 Metadata 访问、宿主访问、跨租户访问。

### 性能设计约束

高频路径禁止全局锁；调度必须在 `O(log n)` 量级完成，禁止扫描全部 Sandbox；资源不足时必须背压排队而不是无限创建；不同租户之间必须加权公平，不允许大租户饿死小租户。工程化设计原则与落地要求见 [TECH-performance-and-capacity.md](../../architecture/tech/TECH-performance-and-capacity.md)。
