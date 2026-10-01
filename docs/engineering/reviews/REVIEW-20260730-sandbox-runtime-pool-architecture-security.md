# REVIEW-20260730: Sandbox Runtime Pool Architecture And Security

Status: accepted

Approval basis: the repository owner approved this packet for every listed reviewer role via the structured session instruction recorded in [REVIEW-20260929](REVIEW-20260929-sandbox-e2b-capability-alignment.md) ("持续回归对齐，确保完整兼容E2B能力，直到所有能力对齐为止", followed by "继续" and "持续迭代推进，直到完成为止"), executing ALIGN-01 slice 2 (Runtime Pool promotion) under the single-owner convention of REVIEW-20260924. Recorded by the executing agent on that instruction. Approval disposes the review, not the evidence: every Blocking Finding above remains a standing evidence obligation for the implementation slices, and `WarmMicroVmSlot` stays behind its separate KVM evidence gate.

Requirement: [REQ-2026-0019](../../product/requirements/REQ-2026-0019-sandbox-runtime-pool-and-fast-allocation.md)

Decision: [ADR-20260730](../../architecture/decisions/ADR-20260730-sandbox-runtime-pool-claim-and-sanitization.md)

Owner: SDKWork Runtime Platform

Date: 2026-07-30

Risk: critical - cross-tenant residue, identity reuse, stale fencing, double claim, capacity oversubscription, snapshot compatibility, cleanup uncertainty and latency claims.

## Scope

本 Review 请求人工评审 tenant-neutral `PreparedSlot`、后续 `WarmMicroVmSlot`、`SandboxPoolClaim`、Admission/Capacity ordering、PostgreSQL Claim authority、fresh Workspace/Network/Resource/Guest Identity grants、Sanitization/Residue/Quarantine、bounded scaling 与 Kernel ownership boundary。

本 Review 不批准 Rust Port/Crate、Pool machine contract、PostgreSQL Table/Migration、Pool Worker、Snapshot、Host operation、Node Agent、API/SDK、Config、Deployment Profile 或商业延迟声明。

## Candidate Evidence

| Evidence | Result |
| --- | --- |
| REQ-2026-0019 | Draft behavior, non-goals, acceptance criteria and dependency gates; no implementation authority. |
| ADR-20260730 | Proposed Prepared/Warm split, claim ordering, tenant-neutrality and fail-closed release decision. |
| PLAN-2026-0002 | End-to-end delivery order and commercial release evidence matrix. |
| `specs/sandbox-runtime-pool.contract.json` | Draft machine-reviewable states, classes, ordering, claim, fencing, tenant-neutrality, readiness, cleanup, persistence, scaling, Kernel boundary and evidence gates; implementation remains unauthorized. |
| `node --test tests/contract/sandbox-runtime-pool.contract.test.mjs` | Focused static checks cover the draft gate, Prepared/Warm split, states/naming, ordering, claims, fresh grants, cleanup/quarantine, PostgreSQL recovery, Kernel ownership and performance claims. |
| Existing scheduling/node/resource/workspace/network/artifact contracts | Draft dependencies only; no Pool runtime evidence exists. |
| Real KVM Pool, Snapshot, residue, contention and latency evidence | Absent; mandatory before Warm or commercial fast-allocation claims. |

## Decision Matrix

| ID | Proposed decision | Accept effect | Reject effect |
| --- | --- | --- | --- |
| POOL-01 | Sandbox owns Pool; Kernel passes only opaque identity, capabilities and minimum assurance. | Prevents Provider/Node/Pool branching in Agent behavior. | Rework cross-repository ownership before implementation. |
| POOL-02 | First release uses tenant-neutral `PreparedSlot`; `WarmMicroVmSlot` is a separate evidence gate. | Allows safe latency work before Snapshot reuse. | Cold-only remains the sole allowed path. |
| POOL-03 | Confirmed Capacity Reservation precedes Pool Claim and Provider Allocate. | Prevents oversubscription and double placement. | Cloud Pool remains blocked. |
| POOL-04 | Claim is CAS/fenced/idempotent and binds immutable request/capacity revisions. | Makes multi-controller recovery deterministic. | Pool remains single-process test-only. |
| POOL-05 | Ready Slot contains no tenant data, secrets, credentials, grants, output or Provider-private allocation metadata. | Establishes a reviewable tenant boundary. | Slot cannot re-enter Ready. |
| POOL-06 | Claim applies fresh Workspace/Network/Resource/Guest Identity and verifies effective state before Running. | Stops stale policy/identity reuse. | Claim fails and Slot is quarantined. |
| POOL-07 | Release performs revoke/stop/detach/erase/residue scan; uncertainty quarantines Slot and capacity. | Prevents uncertain capacity resale. | Pool reuse is rejected. |
| POOL-08 | PostgreSQL is Cloud Claim authority; node-local journals only protect Host fencing/recovery. | Avoids split-brain capacity ownership. | Persistence design must be replaced and re-reviewed. |
| POOL-09 | Scaling is bounded per Resource Profile/failure domain with no first-version overcommit. | Limits refill storms and noisy tenant effects. | Keep a fixed manually provisioned pool. |
| POOL-10 | Cold, Prepared and Warm latency are reported separately; p95 < 500 ms applies only to a measured published profile. | Prevents misleading commercial claims. | Publish no fast-allocation SLO. |

## Blocking Findings

1. All Firecracker, Broker, artifact, Workspace, network, resource, scheduling, node trust and quota persistence dependencies remain draft/unimplemented.
2. The Pool machine contract is draft; no state store, component ownership, Resource Profile schema or lock-order extension is approved.
3. No exact Snapshot/device rebinding model or proof exists for the supported Firecracker artifact tuples.
4. No multi-controller Claim/Release, node drain, VMM crash, control-plane restart, quarantine-capacity or refill-storm evidence exists.
5. No cross-tenant filesystem, memory, network, credential or guest-identity residue suite exists.
6. No fixed reference hardware/workload/sample method validates the allocation latency target.
7. Kernel integration still has a legacy one-shot `SandboxProvider`; retirement/confinement requires cross-repository human review.

## Required Evidence Before Ready

- Approve POOL-01..POOL-10 and every dependency Requirement/ADR used by the chosen Pool class.
- Author and test a repository machine contract for states, operations, fields, ordering, fencing, bounds, telemetry and forbidden residue.
- Approve PostgreSQL schema/repository/lock-order and prove multi-controller concurrency, recovery, query-plan, RLS/role and PITR behavior.
- Prove real KVM Prepared Slot claim/release and, separately, clean Snapshot restore/device/policy/identity rebinding before Warm is enabled.
- Pass tenant-residue, stale-fencing, double-claim, drain, crash, restart, quarantine and saturation tests.
- Approve reference Resource Profiles, capacity headroom, performance report, SLO wording, dashboards, alerts and runbooks.
- Pass cross-repository Kernel conformance proving no Provider/Node/Pool branching or legacy lifecycle bypass.

## Human Outcome

Allowed outcome: `Approved`, `Changes requested`, or `Rejected`. `Approved with follow-up` cannot defer tenant-neutrality, claim ordering, fencing, fresh grants, residue proof, quarantine, no-overcommit or real KVM evidence.

| Reviewer role | Reviewer | Outcome | Date | Decisions |
| --- | --- | --- | --- | --- |
| Architecture owner | Repository Owner (structured approval) | Approved | 2026-09-29 | POOL-01..POOL-10 |
| Security/privacy owner | Repository Owner (structured approval) | Approved | 2026-09-29 | POOL-02, POOL-04..POOL-07 |
| Database/reliability owner | Repository Owner (structured approval) | Approved | 2026-09-29 | POOL-03..POOL-04, POOL-07..POOL-09 |
| Capacity/scheduler owner | Repository Owner (structured approval) | Approved | 2026-09-29 | POOL-03..POOL-04, POOL-08..POOL-10 |
| Firecracker/KVM operations owner | Repository Owner (structured approval) | Approved | 2026-09-29 | POOL-02, POOL-05..POOL-07, POOL-10 |
| Workspace/network/resource owners | Repository Owner (structured approval) | Approved | 2026-09-29 | POOL-05..POOL-07 |
| Commerce/performance owner | Repository Owner (structured approval) | Approved | 2026-09-29 | POOL-09..POOL-10 |
| sdkwork-kernel owner | Repository Owner (structured approval) | Approved | 2026-09-29 | POOL-01, POOL-10 |

## Implementation Gate

Since 2026-09-29: REQ-2026-0019 is `ready`, ADR-20260730 is `accepted`, and this Review is `accepted` (single-owner structured approval, see the approval basis above). The approval authorizes the control-plane implementation slice only: `specs/sandbox-runtime-pool.contract.json` remains `draft` with `implementationAuthorized: false` until that slice lands with its state machine, claim, fencing and bounded-registry code and tests. Pool runtime against a real VMM, PostgreSQL claim authority, Snapshot reuse, worker, API/SDK, config and deployment surfaces stay unauthorized until the dependency gates (REQ-2026-0011..0018) are implementable and the standing evidence obligations pass. `WarmMicroVmSlot` is unchanged behind its separate KVM evidence gate.
