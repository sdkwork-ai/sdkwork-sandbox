# ADR-20260730: Sandbox Workspace Runtime Transaction And Checkpoint

Status: accepted

Accepted: 2026-10-05 by the repository owner via the structured session instruction recorded in [REVIEW-20260730](../../engineering/reviews/REVIEW-20260730-sandbox-workspace-runtime-transaction-architecture-security.md) (single-owner convention). The approval authorizes the control-plane implementation slice only; runtime, PostgreSQL authority, storage/KMS, worker and API/SDK/transport stay behind the machine contract's gates.

Requirement: REQ-2026-0021

Owner: SDKWork Runtime Platform

Date: 2026-07-30

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `DEPLOYMENT_SPEC.md`, `APP_RUNTIME_TOPOLOGY_SPEC.md`, `DRIVE_SPEC.md`, `SECURITY_SPEC.md`, `PERFORMANCE_SPEC.md`, `TEST_SPEC.md`

## Context

The current design has correct component-level boundaries but no transaction spanning them. Pool Claim can be fenced and Workspace Attachment can be encrypted, yet a Controller could still release capacity before a write Checkpoint is durable, two IDE sessions could target the same Workspace Revision, or a disconnected Client could leave commands and mounts active without a governed recovery decision. Local and Cloud also risk diverging if Local treats a Host Path as authority while Cloud treats a Runtime Binding as authority.

BirdCoder must remain a stateless Workbench, Agents must remain the Workspace/Project/Session/Revision business authority, Kernel must remain Provider-neutral, and Sandbox must not become a file business database. Kernel Execution Placement and Sandbox Capacity Placement are also separate control-plane facts: the first owns agent-execution intent, delivery lease, cancellation and recovery; the second owns eligible infrastructure, reservation and allocation. Sharing one record, lease, fencing token or idempotency key would create a dual-writer boundary. The missing responsibility is orchestration: a Sandbox-owned, recoverable transaction that composes existing ports and returns a safe outcome.

## Decision

1. Introduce `SandboxWorkspaceRuntimeTransaction` as a Sandbox Service orchestration contract, not a new storage, scheduler, provider, command or business Workspace authority.
2. Support three explicit lanes: `sandbox_standalone_local`, `sandbox_standalone_firecracker`, and `sandbox_cloud_firecracker`. They share request, Revision, Command, Checkpoint, error and compensation semantics. Adapter mechanisms and Assurance differ only at Composition.
3. Local Workspace input is an already-opened Capability selected by an authorized local product composition. Sandbox never accepts a Host Root string or derives a Path from Workspace ID. Runtime Root and Workspace Capability are distinct. Workspace upload is denied by default; the broader Local data-residency/processing claim, optional synchronization, backup, restore and purge are governed separately by REQ-2026-0022 and Agents/Drive/user authorization.
4. Firecracker Workspace input is an immutable Revision projected as an encrypted Guest Block Device. Cloud bytes are owned by Drive when applicable or by a Block-volume authority approved through an independent Ready Requirement. RootFS, Workspace, Cache, Temp and Log are separate resources.
5. Fix the orchestration order from Authorization through Admission/Capacity/Binding, Pool-or-Cold selection, fresh grants, Provider/Attachment/Effective Readiness, Command, Freeze/Drain, Flush, durable Checkpoint/Handoff, Stop/Detach/Sanitize/Residue and final Release. A Local no-op for Cloud-only stages is explicit durable evidence, not an omitted step.
6. A ReadWrite Attachment uses one Writer Lease per Agents-approved Revision Target. The source Revision is immutable during attachment. Parallel writers require distinct Revision Targets; shared writable devices across active Bindings are forbidden.
7. Sandbox seals a durable `SandboxWorkspaceCheckpointCandidate` through the approved Workspace Storage Adapter and persists an outbox/equivalent handoff before releasing Runtime resources. Only Agents can promote the Candidate with Compare-and-swap against the expected source Revision.
8. Revision conflicts are product outcomes, not storage overwrites. The newer Agents Revision remains authoritative and the losing Candidate is retained only under a bounded owner-approved recovery policy.
9. Client disconnect retains the Runtime only under a bounded Reconnect Lease/Grace. Expiry acquires fresh Fencing, freezes new commands, drains/cancels active commands, checkpoints ReadWrite state and performs cleanup. TTL alone never proves Checkpoint, Detach or Sanitization.
10. Every failure window has deterministic idempotent compensation. Any uncertain Host, Storage, Checkpoint or Cleanup side effect quarantines the affected Binding/Attachment/Slot/Node/Capacity as applicable; uncertain capacity remains consumed.
11. Kernel may pass opaque Workspace Revision Authorization, provider-neutral Workload Class, Capability, Minimum Assurance, Mount Mode, Deadline and cancellation. It cannot select Provider, Node, Pool, Slot, physical Storage or Resource values above the entitled Sandbox policy ceiling.
12. Kernel passes an opaque Execution Placement reference and generation that Sandbox verifies before Admission. Kernel Execution Placement, Sandbox Capacity Placement and Sandbox Runtime Allocation Binding retain distinct identities, leases, fencing domains, operation/idempotency scopes and reconcilers. Sandbox cannot replace or advance the Kernel placement record, and Kernel cannot select Sandbox infrastructure. Expiry or revocation of the upstream placement triggers a fenced cancel/compensation path instead of token reuse.
13. The future split-cloud transport is authored as Sandbox `internal-api` and generated SDK only after separate Ready Requirement and human API/SDK authority review. BirdCoder continues to call Agents SDK; it never calls Kernel or Sandbox directly.

## Alternatives

- Let Pool own the full workflow: rejected because Cold and Local paths still need identical Checkpoint/compensation semantics, and Pool must stay an optional acceleration mechanism.
- Let Workspace Attachment own Checkpoint and Revision promotion: rejected because Attachment owns runtime projection, while Agents owns business Revision and Sandbox Service owns cross-port orchestration.
- Let Kernel coordinate Scheduler, Storage and Provider calls: rejected because this creates Provider/Node/Pool branching in Agent orchestration and duplicates Sandbox recovery authority.
- Bind Local directly to a caller-supplied Host Path: rejected because Path strings are ambient authority and cannot provide race-free containment or opaque identity.
- Release resources after local filesystem flush only: rejected because Cloud durability and Agents Revision promotion would be unproven, causing lost writes on failover.
- Hold the VM until Agents promotion succeeds: rejected as the default because a durable Candidate plus durable handoff allows expensive Runtime capacity to be sanitized and released independently; uncertainty before durable handoff still quarantines.

## Consequences

- The design becomes composable: Agents business state, Sandbox transaction state, Workspace bytes, Provider resources and Pool slots keep separate authorities.
- Cloud command latency includes explicit Attachment and Effective Readiness; fast Pool claims cannot bypass those gates.
- Write completion has two observable stages: durable Candidate/Handoff and Agents Revision Promotion. Product UI must not present the latter before Agents confirms it.
- Local mode can keep data on-device without creating BirdCoder-owned business tables, but REQ-2026-0022 separately requires role-correct Agents/Kernel/Sandbox persistence, Drive/local-folder authority, transfer, backup/restore, purge and real OS/network evidence before any all-data claim.
- Quarantine and Candidate retention consume capacity/storage during uncertainty; capacity planning and operations must budget that headroom.
- Kernel and BirdCoder need later reviewed contract changes because their current runtime-location and command surfaces do not carry the complete transaction inputs.
- Kernel and Sandbox persistence need separate placement/allocation correlations and reconcilers; correlation never turns either record into the other authority.

## Verification

- Static contract tests prove lane separation, request closure, ordering, writer fencing, durable Checkpoint/Handoff, failure compensation, bounded backpressure and safe metadata.
- Cross-contract tests prove Capacity precedes Pool Claim, Environment Ready precedes Command, and Cleanup/Residue precedes Capacity Release.
- Future real tests prove Local device residency, Windows/Linux/macOS claimed behavior, Firecracker block-device isolation, multi-controller fencing, revision conflict, disconnect recovery, no lost writes and cross-tenant residue absence.
- Cross-repository tests prove `BirdCoder -> Agents -> Kernel -> Sandbox`, generated SDK consumption, no direct/raw transport, no Kernel Provider branching and no legacy one-shot process execution.

## Review

This decision affects multiple repositories, data ownership, security posture and future API/SDK authority. Product Architecture, Security/Privacy, Workspace/Drive/Storage, Database/Reliability, Capacity/Scheduler, Local Platform, Firecracker/KVM, BirdCoder, Agents and Kernel owners must approve it before implementation.

## Supersedes / Superseded By

None. This decision composes, but does not supersede, the existing Lifecycle, Command, Workspace Attachment, Scheduling and Runtime Pool decisions.
