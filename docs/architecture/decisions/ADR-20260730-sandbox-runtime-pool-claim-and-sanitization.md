# ADR-20260730: Sandbox Runtime Pool Claim And Sanitization

Status: accepted

Accepted: 2026-09-29 by the repository owner via the structured session instruction recorded in REVIEW-20260929 (single-owner convention; see that packet's approval basis). POOL-01..POOL-10 approved; evidence obligations standing.

Requirement: REQ-2026-0019

Owner: SDKWork Runtime Platform

Date: 2026-07-30

## Context

Cloud Agent execution needs low allocation latency without weakening tenant isolation. A pool that reuses tenant-bound microVM state is commercially unsafe; a cold-only path is safer but cannot meet the PRD fast-allocation target. Firecracker device, snapshot, guest identity, network and cgroup state also make “warm” an assurance claim rather than a simple cache optimization.

## Decision

1. Sandbox owns the Runtime Pool. Kernel requests Capabilities and Minimum Assurance through the existing lifecycle adapter and never selects Pool, Node or concrete Provider.
2. Pool capacity has two explicit classes. `PreparedSlot` is the first commercial implementation target: trusted node eligibility, immutable artifacts and bounded Host preparation are complete, but no tenant data is present and the VMM is not yet tenant-running. `WarmMicroVmSlot` is a separate later claim that requires real KVM Snapshot/Restore and residue evidence.
3. Slot identity is distinct from `SandboxSession`, `SandboxRuntimeBinding` and Provider Allocation identity. A Slot becomes tenant-bound only through a single fenced `SandboxPoolClaim`; identities are never inferred from filesystem paths or reused across claims.
4. Allocation order is fixed: Admission Reservation -> Verified Node hard filter -> Confirmed Capacity Reservation -> Pool Claim -> fresh Workspace/Network/Resource/Guest Identity grants -> Provider Allocate/Start -> Effective Readiness -> Admission bind. No step may be reordered around an external side effect.
5. A Ready Slot contains no tenant Workspace, Secret, Credential, Network Grant, command output or Provider-private allocation metadata. Immutable content-addressed base artifacts may be shared read-only only when their release evidence is current and unrevoked.
6. Warm slots, when approved, originate from an immutable clean snapshot with no tenant state. Claim must rotate Guest Identity and rebind or recreate every tenant-specific device and policy, then verify effective state before Running. Snapshot compatibility is exact to the approved Firecracker/Jailer/Kernel/RootFS/Guest Agent tuple.
7. Release is a stateful security operation, not a cache return. It revokes grants, stops execution, detaches devices, tears down netns/cgroup/runtime state, cryptographically erases ephemeral data, scans residue and only then returns the Slot to Ready. Uncertain cleanup moves the Slot and its capacity to Quarantined.
8. PostgreSQL is the Cloud Claim and capacity authority. Node-local journals may protect Host fencing and recovery but cannot sell or release capacity independently.
9. Pool controllers use bounded reconciliation and rate-limited scale decisions. Pool minimum/target/maximum are per approved Resource Profile and failure domain; no overcommit is allowed in the first version.
10. Metrics and Usage Facts do not own billing. Commerce consumes durable, correlated facts after Sandbox lifecycle and resource usage are terminal.
11. Pool exhaustion returns a typed capacity outcome with bounded retry guidance. It never falls back to Local, Docker, a weaker Assurance level or an unverified Node.

## Alternatives

- Cold allocation only: safest initial fallback and mandatory correctness path, but insufficient as the sole commercial latency strategy.
- Reuse tenant-bound VMs: rejected because cleanup uncertainty can cross tenant boundaries.
- Preboot mutable golden VMs: rejected because mutable image drift breaks artifact and provenance authority.
- Kubernetes-owned Pod pool: deferred; it would add a second scheduler/capacity authority before the Firecracker control plane is proven.
- Kernel-owned pool: rejected because it would make Kernel branch by Provider and duplicate Sandbox lifecycle ownership.

## Consequences

- Fast allocation remains fail-closed, but `WarmMicroVmSlot` cannot ship on design evidence alone.
- The first production slice can improve latency through prepared immutable inputs and Host resources while retaining a cold VMM security boundary.
- Quarantined capacity may reduce utilization; commercial capacity planning must reserve this headroom instead of releasing uncertain slots.
- A new machine contract, PostgreSQL schema/repository design, Pool service/worker component contracts and cross-repository Kernel conformance will be required after human approval.

## Verification

- Prove ordering, CAS, fencing and no-overcommit under multi-controller PostgreSQL contention.
- Prove clean-slot construction, claim-bound identity, effective Workspace/Network/Resource readiness and cross-tenant residue absence on real KVM nodes.
- Prove cleanup failure quarantines Slot and capacity across process/node/control-plane restart.
- Benchmark Cold, Prepared and Warm paths independently on fixed x86_64/aarch64 matrices.
- Prove Kernel passes opaque identity/capability/assurance only and never branches on Pool, Node or Provider.

## Review

Required human owners: Architecture, Security, Privacy, Database, Capacity/Scheduler, Commerce/Metering, Reliability, Platform/KVM Operations, Supply Chain, Workspace/Data and `sdkwork-kernel` integration. This ADR remains `proposed` and authorizes no implementation.

## Supersedes / Superseded By

None.
