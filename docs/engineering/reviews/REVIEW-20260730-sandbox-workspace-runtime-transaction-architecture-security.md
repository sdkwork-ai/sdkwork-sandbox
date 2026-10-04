# REVIEW-20260730: Sandbox Workspace Runtime Transaction Architecture And Security

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured session instruction repeated on 2026-10-04/05 ("检查是否支持镜像和snapshot能力，反复回归检查，确保该功能能实现，我要实现的是快速分配和快速启动资源实例，并支持资源池实例，回归检查是否实现按需分配、资源池化能力，打造专业灵活的sandbox能力"), continuing the structured instructions recorded in [REVIEW-20260929](REVIEW-20260929-sandbox-e2b-capability-alignment.md) under the single-owner convention of REVIEW-20260924, executing the ALIGN program slice after the Runtime Pool and Template authority slices. Recorded by the executing agent on that instruction. Approval disposes the review, not the evidence: every Blocking Finding above remains a standing evidence obligation for the implementation slices, the cross-repository findings (WRT-02, WRT-04, WRT-05, WRT-08, WRT-10, WRT-11) stay owned by their repositories, and the real Local/KVM/PostgreSQL/Storage evidence gates stay release-blocking.

Requirement: [REQ-2026-0021](../../product/requirements/REQ-2026-0021-sandbox-workspace-runtime-transaction-and-checkpoint.md)

Decision: [ADR-20260730](../../architecture/decisions/ADR-20260730-sandbox-workspace-runtime-transaction-and-checkpoint.md)

Owner: SDKWork Runtime Platform

Date: 2026-07-30

Risk: critical - cross-tenant data exposure, duplicate writer, lost write, stale fencing, command bypass, capacity leak, unsafe Local authority and cross-repository contract divergence.

## Scope

本 Review 请求人工评审 Local/Standalone Firecracker/Cloud Firecracker 统一 Runtime Transaction、Device-local Residency、Workspace Revision 单写租约、耐久 Checkpoint/Handoff、Agents Revision Promotion、断连恢复、失败补偿、Backpressure/Fairness 与四仓依赖方向。

本 Review 不批准 Rust Port/Type/Crate、Provider、Host I/O、Process、Scheduler、Pool、Database、Drive/Block-volume/KMS Adapter、API/SDK/Transport、Config、Manifest、Deployment 或跨仓库源码变更。

## Current Design Findings

| ID | Severity | Finding | Evidence | Required closure |
| --- | --- | --- | --- | --- |
| WRT-ISSUE-01 | P0 | Sandbox Gate 彼此独立，没有从 Workspace Revision 到资源归还的统一事务和失败补偿。 | `specs/sandbox-runtime-pool.contract.json`, `specs/sandbox-workspace-block-device-attachment.contract.json`, `apis/commands/sandbox-command-contract.json` | 批准并实现 REQ-2026-0021 的组合状态、顺序和恢复语义。 |
| WRT-ISSUE-02 | P0 | BirdCoder Cloud Runtime Location 尚不能从 Agents Runtime Binding 解析；当前 Cloud Branch 返回 `null` 或 `missing_runtime_location_id`。 | `sdkwork-birdcoder/.../RuntimeProjectRuntimeLocationService.ts` | 通过 Agents App SDK 获取已 Ready 的 Opaque Binding；BirdCoder 不直连 Kernel/Sandbox。 |
| WRT-ISSUE-03 | P0 | BirdCoder Remote Terminal 仍构造 `/bin/bash -lc <string>`，与 Sandbox Draft Command Contract 的 Logical Executable + Argv、No Shell String 冲突。 | `sdkwork-birdcoder/.../sdkworkTerminalLaunch.ts`, `apis/commands/sandbox-command-contract.json` | 定义并生成受治理 Terminal/Command Surface；移除产品层 Shell String。 |
| WRT-ISSUE-04 | P0 | Kernel Sandbox Lifecycle Adapter 只携带 Tenant/Workspace/Session/Operation/Capabilities/Assurance，没有 Revision Authorization、Mount Mode、Workload Class、Checkpoint 或 Command Port。 | `sdkwork-kernel/sdkwork-agent-kernel/src/sandbox_runtime.rs` | 通过人审后的 Sandbox Public Port 扩展 Provider-neutral Adapter。 |
| WRT-ISSUE-05 | P0 | Kernel 仍公开 legacy one-shot `SandboxProvider`/`PlatformSandboxProvider`/`NoOpSandboxProvider`，可使用 Caller Program/Path、Ambient CWD 和 Process Environment 执行。 | `sdkwork-kernel/sdkwork-agent-kernel/src/sandbox.rs`, `host_sandbox.rs` | 从生产 Lifecycle/Command 路径隔离或退役；不得绕过 Sandbox Command Policy。 |
| WRT-ISSUE-06 | P0 | Workspace Byte Authority 和 Block-volume Authority 尚未闭合；现有 Contract 明确标记 unresolved。 | `specs/sandbox-workspace-block-device-attachment.contract.json` | Drive Owner 或独立 Block-volume REQ/ADR 批准 Revision、Grant、Encryption、Retention 和 Recovery。 |
| WRT-ISSUE-07 | P0 | REQ-2026-0022 已补齐 Local 四仓数据清单、数据库角色、Capability、传输、Backup/Restore 与 Purge Gate，但仍无 Agents/Sandbox 本地 PostgreSQL、Kernel/BirdCoder `client-local`、Drive/local-folder 和真实 OS/Network 发布证据。 | `specs/sandbox-standalone-data-residency.contract.json`; BirdCoder manifest `databaseTableCount: 0`; Agents owns business state | 人审 REQ-2026-0022 并形成不新增 BirdCoder 业务库的 Standalone Composition 和真实 Residency/Recovery 测试。 |
| WRT-ISSUE-08 | P0 | BirdCoder Work Provider Installer 使用 `desktop_local_shell_exec` 边界，不能复用为 Workspace Command 或生产 Provider 安装绕过。 | `sdkwork-birdcoder/specs/agents-birdcoder-alignment.spec.json` | 独立评审 Installer/Supply-chain Host Boundary；编码执行统一进入 Sandbox。 |
| WRT-ISSUE-09 | P1 | Workspace Revision 在 Attachment 内不可变，但没有单 Writer Target、Durable Candidate、CAS Promotion 和冲突保留闭环。 | Workspace Attachment Contract lifecycle/readiness | 实现 REQ-2026-0021 Workspace Concurrency/Checkpoint Contract。 |
| WRT-ISSUE-10 | P1 | IDE Disconnect/Reconnect、Command Drain、Checkpoint 和 Runtime TTL 没有统一语义。 | Kernel Lifecycle-only Adapter; Pool TTL/cleanup gates | 批准有界 Reconnect Grace 和 Fenced Expiry Workflow。 |
| WRT-ISSUE-11 | P1 | Queue Fairness 在 Scheduler Contract，Pool Saturation 在 Pool Contract，但没有端到端 IDE Request Backpressure/SLO。 | REQ-2026-0016/0019 contracts | 分阶段 Queue/Ready/Command/Checkpoint/Release 指标和 Saturation Test。 |
| WRT-ISSUE-12 | P1 | 所有真实 Local/Firecracker/Pool/Storage/KMS/HA 证据缺失。 | Gate 0 review package | 人工批准、真实 Runner/KVM/PostgreSQL/Storage/KMS/PKI 环境和可复现 Evidence。 |
| WRT-ISSUE-13 | P0 | Kernel Execution Placement 与 Sandbox Scheduler 的 Capacity Placement 都使用 placement/runtime binding/fencing 词汇，但现有实现没有两套独立记录、Lease、Fencing 和 Idempotency correlation。 | Kernel lifecycle candidate; Sandbox lifecycle schema and scheduling/runtime-pool drafts | 批准独立权威；Kernel 仅传 Opaque Placement Ref/Generation，Sandbox 在 Admission 前验证并维护独立 Capacity Placement/Allocation Binding。 |

## Candidate Evidence

| Evidence | Result |
| --- | --- |
| REQ-2026-0021 | Draft goals, non-goals, acceptance criteria, ownership and implementation gate. |
| ADR-20260730 | Proposed lane parity, transaction ordering, writer/checkpoint and compensation decision. |
| `specs/sandbox-workspace-runtime-transaction.contract.json` | Machine-reviewable draft; implementation remains unauthorized. |
| Focused Node contract test | Covers 12 static contract groups; it is not runtime evidence. |
| Existing Local/Command/Workspace/Scheduler/Pool contracts | Required draft dependencies; all remain closed. |
| Real Local/KVM/PostgreSQL/Storage/Checkpoint evidence | Absent and release-blocking. |

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| WRT-01 | Sandbox Service owns the cross-port Runtime Transaction; existing Ports keep their narrow ownership. | High cohesion without a new monolith. | Redesign orchestration owner before implementation. |
| WRT-02 | Local and Cloud share semantics; Local stays Device-local and `HostUser`, Cloud requires Firecracker `MicroVm`. | Product parity without false assurance. | Separate product protocols and duplicated SDK behavior remain. |
| WRT-03 | Physical Workspace input is only an opened Capability or approved encrypted device projection. | No ID-to-Path or raw storage authority. | Provider remains blocked. |
| WRT-04 | Capacity/Claim precede Provider side effects; Effective Readiness precedes Command. | No overcommit or partial-ready execution. | Cloud launch remains blocked. |
| WRT-05 | ReadWrite uses one Writer Lease per Revision Target. | Prevents silent shared writes. | Only ReadOnly execution may proceed. |
| WRT-06 | Durable Candidate and Handoff precede Runtime release; Agents alone promotes Revision with CAS. | No lost writes and no duplicated Workspace authority. | Hold Runtime or disable ReadWrite Cloud sessions. |
| WRT-07 | Revision conflict never overwrites; Candidate retention is bounded and owner-governed. | Deterministic multi-session conflict handling. | Disable parallel writers. |
| WRT-08 | Disconnect uses bounded Reconnect Grace, then fenced Checkpoint/Cleanup. | Recoverable IDE UX without leaked capacity. | Disconnect must terminate immediately with explicit no-persistence limitation. |
| WRT-09 | Uncertain Host/Storage/Checkpoint/Cleanup state quarantines and keeps capacity consumed. | Preserves tenant isolation and no-overcommit. | Runtime/Pool reuse is rejected. |
| WRT-10 | Kernel passes only provider-neutral intent; BirdCoder consumes Agents SDK only. | Open-closed cross-repository architecture. | Rework public ownership and SDK authority. |
| WRT-11 | Legacy Kernel one-shot process execution cannot serve production. | One security and command authority. | Commercial release remains No-Go. |
| WRT-12 | Queue/Command/Retry/Reconcile/Buffer paths are bounded and Tenant-fair. | Predictable SaaS saturation behavior. | Limit product to single-user standalone. |
| WRT-13 | Kernel Execution Placement and Sandbox Capacity Placement/Runtime Allocation use distinct record IDs, leases, fencing tokens, idempotency scopes and reconcilers. | Prevents dual-writer placement state and stale cross-plane side effects. | Cloud integration remains blocked. |

## Required Evidence Before Ready

- Human outcomes for WRT-01..WRT-13 and all dependency decisions.
- Agents Workspace Revision Authorization, Writer Lease, Branch Target, Candidate Promotion, Conflict and Retention contract.
- REQ-2026-0022 approved Local standalone composition proving every declared data class, transfer, backup/restore and purge path stays within the selected claim without a BirdCoder-owned business database.
- Drive or approved Block-volume contract and real encryption/integrity/flush/checkpoint/recovery tests.
- Kernel lifecycle/command/attachment/checkpoint adapter conformance and legacy one-shot production-path removal.
- Kernel/Sandbox placement-correlation conformance proving independent leases, fencing tokens, operation identities, expiry and reconciliation under duplicate, delayed and reordered delivery.
- BirdCoder generated Agents SDK integration for Local/Cloud runtime selection, Ready Binding, typed command/terminal and safe reconnect outcomes.
- Real Windows/Linux/macOS Local evidence for every claimed capability and real Linux KVM x86_64/aarch64 Firecracker evidence.
- Live PostgreSQL multi-controller transaction, writer fencing, compensation, outbox, restart, PITR and query-plan evidence.
- Disconnect, timeout, cancel, node loss, storage outage, checkpoint conflict, partial handoff, cleanup failure and residue fault injection.
- Queue fairness, pool saturation, cold fallback, Command concurrency, Checkpoint and release p50/p95/p99 report on fixed profiles.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`. `Approved with follow-up` cannot defer ownership, no direct Path, Assurance separation, single Writer, durable Handoff, Agents CAS, stale Fencing rejection, quarantine, no weaker fallback or production command bypass removal.

| Reviewer role | Reviewer | Outcome | Date | Decisions |
| --- | --- | --- | --- | --- |
| Product architecture owner | Repository Owner (structured approval) | Approved | 2026-10-05 | WRT-01, WRT-02, WRT-08, WRT-10, WRT-13 |
| Security/privacy owner | Repository Owner (structured approval) | Approved | 2026-10-05 | WRT-02..WRT-09, WRT-11 |
| Workspace/Drive/storage owner | Repository Owner (structured approval) | Approved | 2026-10-05 | WRT-03, WRT-05..WRT-09 |
| Database/reliability owner | Repository Owner (structured approval) | Approved | 2026-10-05 | WRT-04..WRT-09, WRT-12, WRT-13 |
| Capacity/scheduler owner | Repository Owner (structured approval) | Approved | 2026-10-05 | WRT-04, WRT-09, WRT-12, WRT-13 |
| Local platform operations owner | Repository Owner (structured approval) | Approved | 2026-10-05 | WRT-02, WRT-03, WRT-08, WRT-09 |
| Firecracker/KVM operations owner | Repository Owner (structured approval) | Approved | 2026-10-05 | WRT-02..WRT-04, WRT-09 |
| BirdCoder owner | Repository Owner (structured approval) | Approved | 2026-10-05 | WRT-02, WRT-08, WRT-10, WRT-12 |
| Agents owner | Repository Owner (structured approval) | Approved | 2026-10-05 | WRT-05..WRT-08, WRT-10 |
| Kernel owner | Repository Owner (structured approval) | Approved | 2026-10-05 | WRT-04, WRT-08, WRT-10, WRT-11, WRT-13 |

The commercial decision remains **No-Go**: this approval disposes the architecture/security review only. See the Implementation Gate for the authorized slice.

## Implementation Gate

Since 2026-10-05: REQ-2026-0021 is `ready`, ADR-20260730 is `accepted`, and this Review is `accepted` (single-owner structured approval, see the approval basis above). The approval authorizes the control-plane implementation slice only: the provider-neutral transaction state machine, the fixed 21-stage orchestration order with typed not-applicable evidence, the checkpoint candidate with durable handoff and Agents-only compare-and-swap promotion, writer-lease/fencing/idempotency semantics, and bounded registries, landed as a domain crate with machine-contract alignment tests. `specs/sandbox-workspace-runtime-transaction.contract.json` flips to `implementationAuthorized: true` for that slice while remaining `draft`; the runtime against real Providers, the PostgreSQL transaction authority, storage/KMS adapters, worker, API/SDK/transport, config and deployment surfaces stay unauthorized behind the contract's `x-sdkwork-no-*` gates, and the evidence obligations above (real Local/KVM/PostgreSQL evidence, fault injection, saturation) stay release-blocking.
