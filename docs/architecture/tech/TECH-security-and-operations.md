# SDKWork Sandbox 安全与运行架构

Status: draft

Owner: SDKWork Runtime Platform

Updated: 2026-07-30

Parent: [Technical Architecture](TECH_ARCHITECTURE.md)

## 1. Threat Model Boundary

Sandbox 执行不可信或部分可信代码。攻击目标可能包括 Workspace Escape、跨 Assignment Persistence、Resource Exhaustion、Host/Cloud Control Plane Access、Credential Theft、Cross-tenant Attack 与 Data Exfiltration。每个 Sandbox Provider 必须发布 Assurance Profile，产品 Policy 决定它可承载哪些 Workload。

Local Provider 无法建立强于当前 OS User 与已开放 Host Capability 的边界。当前多租户不可信 Workload 的优先候选是经过评审的 Linux KVM Firecracker MicroVm Profile；Docker Provider 延期且不能作为 Assurance 回退。未来 gVisor、Dedicated VM 或其他 Profile 必须分别提供等价评审证据。

## 2. Deny-by-default Control

- 禁止不受限 Host Filesystem；Workspace/Cache Root 只使用 Composition 已打开的 Capability Handle 与 handle-relative no-follow/file-identity verification。String canonicalization 和 check-then-open 不是安全边界。
- 默认禁止 Docker/Container Socket、Cloud Metadata Endpoint、Host Network、Host SSH Agent、Privileged Mode、Host PID Namespace。
- 禁止 Ambient Credential；Secret 通过 Reference 解析，在最小 Scope/Time 内注入，并在 Stop/Reassignment 后撤销。
- 未经评审的 Profile 禁止 Elevated Linux Capability、Device、Kernel Module 与 Unsafe Host Mount。
- Egress 默认拒绝或按 Destination、Protocol、Purpose 显式授权；DNS Resolution 不等同 Egress Grant。
- Provider 支持时 RootFS Read-only；Writable Layer、Workspace、Cache、Temp 分别限制。
- 每个 Destructive Filesystem/Workspace/Snapshot/Allocation Operation 都必须显式、授权、可审计且考虑 Idempotency。

## 3. Filesystem And Workspace Safety

Path Validation 必须按操作系统处理 `..`、Symbolic Link、Reparse Point、Mount Point、Hard Link、Case/Normalization、Device Name、Alternate Data Stream 与 TOCTOU。Provider 应优先使用已打开的 Capability-based Directory Handle，而不是反复信任 String Path。

Physical Workspace Root 与 Provider Host Path 是 Private Metadata。Public Error/Event/SDK Type 只使用 Logical Relative Path 与 Opaque `SandboxWorkspaceId`；变量名使用 `sandbox_workspace_id`，不得从该值推导路径。Workspace Snapshot 必须校验 Integrity，未经 Agents Ownership/Authorization 与 Revision Validation 不得覆盖 Active `SandboxRuntimeBinding`。

REQ-2026-0013 进一步固定 provider-neutral `SandboxWorkspaceAttachmentPort` 与其后的 L4 `SandboxWorkspaceBlockDevicePort` Gate：Agents 拥有 Workspace Identity/Authorization/Revision/Retention，Drive 拥有适用的 SDKWork 文件/对象存储生命周期，Sandbox 只拥有一次 Session/Binding 的 encrypted Runtime Projection。Firecracker 不直接挂载 Host Directory；Detach/Sanitize 不删除 Persistent Workspace，Ephemeral Projection 执行 Cryptographic Erase 和 Residue Scan，失败或未知进入 Quarantine 并阻止跨 Tenant 重用。当前只有 draft 机器契约，不存在 Storage/KMS/Device Runtime 或真实隔离证据。

REQ-2026-0022 将 Local 数据安全扩展为四仓 Release Gate。`standalone`/Local Provider 不证明设备本地性；BirdCoder 只拥有有界设备事实，Agents/Sandbox Server Authority 保持本机 PostgreSQL，SQLite 只服务独立声明的 `client-local` Kernel/BirdCoder 状态。Workspace、Service Data、Runtime Root、Cache、Log、Secret 与 Temp 使用不同 Capability，Sandbox Cleanup/Reset/Uninstall 默认保留 Workspace。`device-local-persistence` 禁止远程持久副本；`strict-device-local-processing` 还禁止源码、Prompt、Transcript、Artifact、Secret 和诊断内容外传。Sync、External Model/Tool 和 Telemetry 必须独立授权，不能由 Local Mode 推导同意。

Local Backup 默认本地 Opt-in，按 PostgreSQL/SQLite 正确机制执行，敏感内容加密并附 Integrity Manifest；Workspace 与 Secret 默认排除，且只有通过 Restore Test 才构成证据。Export/Purge 必须覆盖 Primary、Checkpoint、Log、Cache、Temp、Derived Index 和 Backup；Uninstall 不等同 Privacy Purge。Missing/Ambiguous Locality、Capability、Database、Disk Full、Corruption、Restore Failure 或 Purge Uncertainty 均关闭失败，不允许 Cloud Fallback、静默丢数据或虚假完成声明。当前仅有 draft 合同，没有真实路径、数据库、备份、清除或 OS/Network Evidence。

REQ-2026-0014 固定 provider-neutral `SandboxNetworkPolicyPort` 与其后的 L4 `SandboxNetworkIsolationPort` Gate：Policy 默认 `DenyAll`，只有显式 DNS/Egress Grant 可进入机制层，Cloud Metadata、Host Control Plane 与 Tenant Lateral Traffic 永久拒绝且不能被 Grant 覆盖。每个 Runtime Binding 独立 netns/Tap；Policy Revision/Fencing、Atomic Apply/Readback/Probe、Teardown/Residue/Quarantine 与 Durable Denial Audit 关闭失败。当前只有 draft 机器契约，不存在 Network Port/Adapter、netns、Tap、Firewall/DNS/Route Runtime 或真实 KVM 网络隔离证据。

## 4. Process、Terminal 与 Network Safety

Process Creation 执行 Runtime-Binding-scoped immutable Execution Policy、Working-directory Containment、Environment Allow/Deny、CPU/Memory/IO/PID Limit、Wall Timeout、Output Limit、Cancellation 与 Child Cleanup。`sandboxExecutable` 是逻辑标识，只能由 Provider-owned Registry Snapshot 解析；禁止 Caller Path、OS PATH Search 与 CWD Lookup，重放不得改用不同 Binary。Stop/Destroy 必须在有界策略下等待或强制结束 Descendant。

Local 平台边界由 `specs/sandbox-local-provider-host-boundary.contract.json` 固定：Windows suspended spawn 后必须先绑定 Kill-on-close Job Object 与 Completion Port 再 Resume；Linux 必须在用户代码执行前 race-free 加入每 Binding 独立的 delegated unified cgroup v2 Scope，并以 `cgroup.kill`、`cgroup.events`、PID Controller 和 `populated=0` 收敛；Process Group 或 spawn 后写 `cgroup.procs` 不足以构成保证。macOS 在 detached descendant containment 获批前不声明 Terminal，相关请求关闭失败且不回退。Environment 从空集合构造，请求不能扩展 Policy 或覆盖 PATH/PATHEXT/COMSPEC/SYSTEMROOT/WINDIR/HOME/USERPROFILE/TMP/TEMP 等 Provider 固定值，不继承 SSH Agent、Cloud/Proxy Credential、Docker Socket、Host Runtime Control 或 Secret-bearing Variable。

Terminal IO 可能包含敏感数据，必须 Bounded、Access-controlled，并与 Structured Operational Log 使用不同 Retention。Port Forward 需要显式 Lease，至少记录 Owner、Direction、Target、Expiry、Exposure Class 与 Revoke Operation。

Sandbox Lifecycle Provider Side Effect 使用 `SandboxSessionLease` 建立唯一控制权。内部身份/变量固定为 `SandboxLeaseOwnerId`/`sandbox_lease_owner_id` 与 `SandboxFencingToken`/`sandbox_fencing_token`；每次 Allocate/Start/Stop/Destroy 前必须 Renew，Provider Request 必须携带当前 Token。Acquire 发现其他控制器仍持有 Lease 时返回 `SandboxLifecycleError::LeaseUnavailable`；Acquire 成功后 Renew、Token Identity Check、Repository Save 的 `LeaseConflict` 或成功业务后的 Release 失败时返回 `SandboxLifecycleError::LeaseLost`，已有 Provider/Readiness 错误优先于并发 Release 错误。Reconciler 在 Acquire 成功后必须重新读取权威 `SandboxSession`，仅对当前仍为瞬态状态的 Session 发出 Provider Side Effect。`SandboxFencingToken` 达到 PostgreSQL `BIGINT` 上限后关闭失败为 `LeaseConflict`，不得回绕。Provider Operation Timeout 必须非零且不超过 Lease Duration 的一半，以便 Timeout/返回后仍保留提交失败状态或清理的时间预算；`LeaseLost` 后不得继续 Provider 调用或 Repository Save。真实 Provider 必须按 `SandboxRuntimeBindingId` 拒绝低于已观察值的 Token。

PostgreSQL Operation 历史使用稳定 `sandbox_operation_sequence`，避免同一事务时间戳和随机 ID 造成状态机反序。Repository Restore 必须在解密 `SandboxProviderAllocationRef` 前按该顺序重放并验证持久化组合不变量；`Running`/`Stopping`/`Stopped` 缺少受保护 Allocation、`Created`/`Destroyed` 保留 Binding、Transient State 缺少匹配 InProgress Operation、Failure 不匹配等情况统一关闭失败，且不得触发 Provider Side Effect。

上述历史 replay 是当前 REQ-2026-0005 候选行为；读取侧由 `MAX_SANDBOX_SESSION_PERSISTED_OPERATIONS` 有界化（保留界 `MAX_SANDBOX_SESSION_OPERATIONS` 加一个终态 `Destroy` 宽限行，超限的 Session 失败关闭；宽限行保证触界 Session 仍可读取并可销毁，Provider 分配不会因账本触界而泄漏）。写侧自 2026-09-29 起只持久化本次追加或解析的账本条目（快照携带持久化 frontier 与末项 outcome 指纹），不再逐条重放全量历史；历史无界增长与超限不可用的长期解仍由 REQ-2026-0020 承接，未获批前不得擅自截断或迁移。REQ-2026-0020 与候选 ADR-20260730 将目标边界固定为 bounded `SandboxSessionHotState` + separate durable `SandboxLifecycleIdempotencyRecord`：普通 hydrate 和 post-Lease reread 至多读取当前 In-progress Operation，Operation 重放/冲突通过 Tenant-scoped point lookup 处理，Audit/Event/Log 不充当幂等权威。最大 Operation 数、最大活动 Session 生命周期、终态保留和 Late Retry Outcome 未获人审前保持未定义，当前 Schema/Repository 不得擅自截断或迁移。

## 5. Secret Handling

Secret Value 禁止进入 Source `etc/`、App Manifest、Component Spec、Workspace Metadata、Event Payload、Metric Label、可避免的 Command-line Argument 或默认 Durable Terminal Replay。Sandbox Runtime 可以记录 Secret Reference、Purpose、`sandbox_provider_id`、Version、Injection Time 与 Revocation Outcome，但不能记录 Value。

Reassignment、Snapshot 与 Cache Policy 必须证明 Secret-bearing Memory/Filesystem Layer 被排除，或已加密并受访问控制。Restore 后重新解析 Secret，不能假设旧 Credential 仍有效。

Workspace Projection 的 At-rest Key 只以外部 `sandbox_*` Key Reference 表达，作用域绑定 Tenant + Workspace Revision + Projection；Raw Key 不进入公共契约、Provider 持久状态或 Telemetry/Audit Detail。REQ-2026-0013 明确保持 `x-sdkwork-no-kms-implementation: true`，实际 Algorithm、KMS、Rotation、Revocation、Unwrap 和 Memory Zeroization Evidence 未完成人工评审前不得实现。

`SandboxProviderAllocationRef` 的 Versioned Protection、Tenant-scoped Re-encryption、页目标 Protection Version 稳定性、Tenant+Binding+Session+完整旧密文元数据 CAS 与旧密钥撤销遵循 [Allocation Key Rotation ADR](../decisions/ADR-20260728-sandbox-provider-allocation-key-rotation-and-reencryption.md)。真实 PostgreSQL 候选证据见 [Allocation Key Rotation Verification](../../engineering/reviews/REVIEW-20260729-sandbox-provider-allocation-key-rotation-verification.md)，受控操作顺序见 [Allocation Key Rotation And Old-key Revocation Runbook](../../runbooks/RUNBOOK-sandbox-provider-allocation-key-rotation.md)。Runbook 当前为 `candidate`，不得替代尚未交付的 Secret/KMS Adapter、Operator Entry Point、Audit/Metric 或人工撤销审批。

`SandboxProviderAllocationKey` Key Material 使用 `Zeroizing<Vec<u8>>`，包括构造校验失败路径；派生 AES Key 使用 `Zeroizing<[u8; 32]>`，Provider-private 明文由 `SandboxProviderAllocationRef` Drop 清零。Key ID 限定为 `1..=128` bytes printable ASCII，并由 Key Carrier、Service Domain Constructor 与 PostgreSQL `CHECK` Constraint 分层拒绝不安全值；同一 Key ID/Version 在保留期内不得原地更换 Key Material。当前 Key Source 是同步 Trait，生产 Adapter 不得在 Tokio Worker 上直接进行远程 KMS 阻塞调用；必须使用经评审的短生命周期本地 Key Handle/异步刷新边界，或先批准 Async Port 演进。

Service Host Bootstrap 只消费归一化安全配置和预打开、最小权限、身份绑定的 `SandboxRuntimeDirectoryCapabilities`，不消费物理路径、Database URL/Pool、Raw Key 或 Exporter Client。外部 Database Composition 在连接预算和 Readiness 通过后只注入 `SandboxSessionRepository`；Redis 当前禁用。Secret/KMS 调用必须异步、有界、可取消并在失败/Rotation 后清零；Telemetry Adapter 只有保持有界接收、Redaction 和 Drop Accounting 时才 Ready，外部 Exporter Outage 只进入独立运维健康降级，不能替代 Audit/Outbox Authority。Bootstrap 失败按固定顺序逆向清理，任一必需依赖 `degraded`/`unknown` 都不能授权新副作用。当前这些均是 `implementationAuthorized: false` 的候选边界，不存在 `etc/`、KMS、Pool、Exporter 或 Outbox Runtime。

## 6. Resource And Quota Enforcement

Admission 在 Allocation 前按 Tenant/Platform Policy 校验 Limit。只有 `sandbox_provider_readiness` 证明 Enforcement 已生效，`SandboxSession` 才能进入 Running。限制覆盖 CPU、Memory、Disk、IO、PID、Timeout、Workspace Size、Log/Terminal Size、Network、Port Count 与 Optional GPU。

Usage 在 Provider Boundary 测量，并由 Control Plane 聚合。Sandbox 输出 Immutable Usage Fact 与 Quota Outcome；Commerce 拥有 Price、Invoice 与 Payment。超过 Hard Limit 必须产生受控 Operation/Session Outcome，不能用改变 Tool Semantics 的静默 Throttling 替代。

REQ-2026-0015 将 Firecracker Resource Gate 固定为 provider-neutral `SandboxResourcePolicyPort`、L4 `SandboxResourceIsolationPort` 与 immutable `SandboxResourceUsageFact`：Guest vCPU/Memory 精确匹配 Grant，Host 使用每 Binding 独立 cgroup v2 `cpu`/`memory`/`pids`/`io` Controller，所有 Effective Value、Process Membership、Fencing、Node Reservation 和 Prior Residue 在 Start 前读回验证。Limit Breach 使用 typed Outcome，Release 前 Final Usage 并 Durable Handoff；Metric 不是 Billing Truth，Sandbox 不拥有 Price/Invoice/Payment。当前只有 draft 契约，不存在 Quota Engine、cgroup/Machine Config、Usage/Commerce Runtime 或真实 KVM 证据。

REQ-2026-0016 将 SaaS Admission/Placement 顺序固定为 `Admission -> Trusted Node Inventory -> Hard Placement Filter -> Atomic Capacity Reservation -> Placement Decision -> Provider Allocate`。`SandboxAdmissionPolicyPort` 只消费 IAM Typed Verified Context 与批准的 Entitlement/Quota Snapshot并原子预留 Tenant 并发配额；`SandboxNodeInventoryPort` 拒绝 Stale/Draining/Quarantined/Unhealthy/Unknown Node；`SandboxSchedulerPort` 不允许弱 Assurance 回退或 Caller 指定 Provider/Node；`SandboxCapacityReservationPort` 使用 PostgreSQL 权威事务并禁止 Overcommit。Resource Grant 必须绑定 Admission Grant 与 Capacity Reservation；Priority/Fairness、Fencing、Expiry/Orphan Recovery、Node Trust、Override 和 Cross-tenant Denial 全部关闭失败。当前只有 draft Gate 0 契约，不存在 Admission/Scheduler/Reservation Database/Node Agent/Pool Runtime 或真实多副本容量证据。

REQ-2026-0018 进一步固定 PostgreSQL Quota/Capacity Authority：`SandboxTenantQuotaState` 与 `SandboxNodeCapacityState` 提供固定锁点和 Versioned Counter，`SandboxAdmissionReservation` 与 `SandboxCapacityReservation` 提供 Idempotency、Ownership、TTL、Release、Quarantine 与 Recovery Fact；Counter 和 Reservation 必须在同一事务中变化。全局锁序为 Session Lease、Session、Runtime Binding、Tenant Quota State、Admission Reservation、Node Capacity State、Capacity Reservation；远程调用不得跨锁持有，`40001`/`40P01` 只重试完整幂等事务。`prepared` 且无 Provider Side Effect 可过期释放；`confirmed`/`bound` 状态不确定时必须继续占用并 Quarantine，避免仍在运行的 Sandbox 被重复出售。Tenant Table 使用标准 `BIGINT` Subject、Tenant-leading Index、应用 Predicate 与 RLS Defense-in-depth；当前 Lifecycle 的 `tenant_id TEXT` 必须通过受评审的预发布 Migration 对齐后才允许新增表。当前只存在 draft Gate，不存在新 Table/Migration/Repository/RLS/Role/PITR 或真实并发证据。

## 7. Node Trust And Verified Inventory

REQ-2026-0017 将 Bootstrap、Machine Authentication、Platform Attestation、Inventory Publication 与 Lifecycle Control 分离。Bootstrap Credential 必须短期、单次且只传 Reference；Node 在本机生成不可导出 Key，短期 `SandboxNodeIdentity` 绑定 Key、Trust Domain 与 Audience，steady-state Channel 使用 TLS 1.3 Mutual Authentication。mTLS 只证明 Machine Identity，不能替代 Hardware/Platform Attestation；缺失、过期、重放、未知或 Policy 不匹配的 Evidence 一律不能声明 `sandbox_verified_platform_attestation`。

Node Agent 只提交 Evidence 和有界、签名、单调递增的 Inventory Publication，不能自批 Enrollment、提升 Trust Profile、扩展 Capability 或直接向 Scheduler 发布 Candidate。Control Plane 必须把 Identity、Attestation、Artifact Manifest、Network/Resource Policy、Health、Lifecycle 与 Capacity Revision 绑定为签名 `SandboxVerifiedNodeInventoryRecord`；只有 `sandbox_active` 且 Identity/Attestation/Inventory 全部新鲜的记录可经 `SandboxNodeInventoryPort` 进入 Placement。Identity Rotation/Revocation、Clone/Compromise、Drain/Quarantine、CA/Verifier Outage、Stale Revision 与 Reconciliation 均关闭失败。Node Trust 数据属于 `InternalSecuritySensitive`，Public API/Event/Metric 不暴露 Node Identity、Certificate、Raw Evidence、Host Address、Topology、Measurement 或 Capacity。

## 8. Observability Model

| Data Class | Purpose | Minimum Correlation | Sensitive-content Rule |
| --- | --- | --- | --- |
| Operational Log | 诊断 Sandbox Control/Sandbox Provider 行为 | `traceId`、Component、`sandboxOperationId`、`sandboxProviderId`；存在时附授权后的业务 ID | Structured/Redacted；禁止 Secret Value、`SandboxProviderAllocationRef` 与完整 Private Payload。 |
| Terminal Stream | 用户可见 Process IO | `sandboxSessionId`、Command、Stream Sequence | Access-controlled、Bounded、独立 Retention。 |
| Domain Event | State Transition / Integration Fact | Event ID、Type/Version、Occurred Time、Tenant、Trace、Aggregate ID | Versioned Schema，Data 最小化。 |
| Audit Event | Security/Destructive/Operator Fact | Actor、Policy、Target、Decision、Trace | Production Append-oriented、Tamper-resistant。 |
| Metric | Capacity、Latency、Error、Saturation | Provider/Deployment/Runtime Label，低 Cardinality | 禁止 Raw Path、Command、Key、User Content 与 Secret-bearing ID。 |
| Trace | End-to-end Operation Timing | W3C Trace Context 与 Operation Identity | Attribute Allowlist 与 Redaction。 |

候选 Metric Catalog 已覆盖 Node Enrollment、Identity Rotation、Attestation Verification、Inventory Publication、Node Scheduling State，以及 Admission Decision、Scheduler Placement、Queue Wait、Capacity Reservation/Saturation、Allocate/Start/Stop/Destroy Latency、Active `SandboxSession`/Sandbox、Quota Rejection、Command Duration/Exit Class、Provider Error/Health、Resource Saturation、Recovery Outcome、Event/Log Backpressure 与 Security Policy Denial。所有 Node Label 必须低基数，禁止 Node Reference、Identity Serial、Key Thumbprint、Measurement、Raw Locality/Residency/Fault Domain。Pool Hit/Miss/Sanitize Failure、Workspace Attachment/Snapshot IO 等未获批 Runtime 能力仍须在对应 Ready Requirement 中扩展，不能提前宣称可观测。

## 9. Event And Audit

对外使用前，Event 必须在 `apis/async/` 形成 Machine Authority。Standard Envelope 包含 Stable Event ID、Type、Version、Occurred Time、Tenant/Organization Context（适用时）、Trace ID、Aggregate Type/ID 与 Typed Data。At-least-once Delivery 需要 Idempotent Consumer，以及对 State-coupled Event 使用 Outbox 或等价 Durable Handoff。

Security Audit 覆盖 Sandbox Provider Selection/Assurance、Admission Decision、Placement/Capacity Reservation、Policy Change、Secret Injection/Revocation、Destructive Workspace/Snapshot、Port Exposure、Elevated Capability Grant、Quota Override、Node Enrollment/Identity Rotation/Revocation/Attestation/Drain/Quarantine、Recovery Takeover 与 Pool Sanitization Failure。Node、Placement Event 与 Metric 不得成为 Identity、Attestation、Quota、Capacity 或 Placement Authority，也不得暴露 Raw Tenant、Node、Certificate、Evidence、Topology、Entitlement 或 Capacity。

## 10. Provider Conformance

每个 Sandbox Provider 最终都必须通过共同 Lifecycle/Capability Test 和 Sandbox Provider-specific Security Test。Local 与 Firecracker 使用同一候选 `SandboxCommandExecutor` Contract；Common Conformance 包括 Idempotent Lifecycle、Invalid Transition、Workspace Containment、No-shell Argv、Provider-owned Executable Resolution、无 PATH/CWD Search、Binding Policy Snapshot 不可变、Protected Environment Override Denial、Descendant Cleanup、Network Denial、Resource Enforcement、Terminal Cancellation、Output Bound、Stale Fencing、Secret Redaction、Event Ordering、Failure Cleanup 与 Unsupported Capability Reporting。

功能测试通过不足以声明 Assurance Level。每次 Release 都要记录 Provider/Runtime Version、Host Prerequisite、Kernel/RuntimeClass Config、Known Limitation，以及该 Profile 能/不能缓解的 Threat Scenario。
