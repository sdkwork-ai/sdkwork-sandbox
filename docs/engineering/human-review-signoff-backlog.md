# Human Review Sign-Off Backlog

Purpose: the single index of review packets that are waiting on a human decision, and of the machine
contracts that stay `implementationAuthorized: false` until those decisions are recorded.

This page is an index, not a decision record. Each packet owns its own Scope, Decision Matrix,
reviewer sign-off table, Close-Out Checklist and Exit Gate; nothing here restates them. The reviewer
roles, proposed decisions, accept/reject effects and open findings live in the linked packet.

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `QUALITY_GATE_SPEC.md`, `DOCUMENTATION_SPEC.md`.

## What This Index Is For

Every Provider-adjacent contract in `specs/` sets `implementationAuthorized: false` and requires an
approved human review before implementation. A review packet that nobody signs does not fail a test; the
contract simply stays closed and the capability is never built. This index exists so that state is
visible in one place instead of being discovered packet by packet.

Live status, counts and the full pending list come from the gate, never from this page:

```bash
node tools/check-sandbox-human-review-signoff.mjs
node tools/check-sandbox-human-review-signoff.mjs --json
node --test tests/contract/human-review-signoff.contract.test.mjs
```

The gate fails when a contract names a packet that does not exist, when a required reviewer role is not
asked to sign its packet, when a packet still marked `pending-human-review` coexists with an `Approved`
reviewer outcome or a `ready`/`accepted` requirement or an `accepted` decision, or when any contract sets
`implementationAuthorized` before every packet it names reaches a signed-off status with every reviewer
outcome `Approved`. It also checks that the summary page at
[Gate 0 Exit Readiness Package](gate-zero-exit-readiness-package.md) projects this same set rather than a
copy of it: that page may not omit a pending packet, may not report a row status or risk that disagrees
with the packet's own `Status:`/`Risk:` header, and may not carry a pending count that has gone stale.

## Contract-Gated Packets

These packets block Provider implementation. Each one must reach a signed-off status with every reviewer
outcome `Approved` before the contracts that name it may move.

| Review packet | Requirement | Decision | Gating contract(s) |
| --- | --- | --- | --- |
| [REVIEW-20260729-local-provider-architecture-security](reviews/REVIEW-20260729-local-provider-architecture-security.md) | [REQ-2026-0003](../product/requirements/REQ-2026-0003-secure-local-provider.md) | [ADR-20260728](../architecture/decisions/ADR-20260728-local-provider-assurance-and-host-boundaries.md) | `sandbox-local-provider-host-boundary.contract.json`, `sandbox-provider-delivery-gates.contract.json` |
| [REVIEW-20260729-sandbox-command-execution-architecture-security](reviews/REVIEW-20260729-sandbox-command-execution-architecture-security.md) | [REQ-2026-0007](../product/requirements/REQ-2026-0007-sandbox-command-execution-contract.md) | [ADR-20260729](../architecture/decisions/ADR-20260729-sandbox-command-execution-and-terminal-boundary.md) | `sandbox-provider-delivery-gates.contract.json` |
| [REVIEW-20260729-firecracker-provider-architecture-security](reviews/REVIEW-20260729-firecracker-provider-architecture-security.md) | [REQ-2026-0008](../product/requirements/REQ-2026-0008-firecracker-sandbox-provider.md) | [ADR-20260729](../architecture/decisions/ADR-20260729-firecracker-provider-isolation-and-node-boundaries.md) | `sandbox-provider-delivery-gates.contract.json` |
| [REVIEW-20260729-sandbox-host-isolation-broker](reviews/REVIEW-20260729-sandbox-host-isolation-broker.md) | [REQ-2026-0011](../product/requirements/REQ-2026-0011-sandbox-host-isolation-broker.md) | [ADR-20260729](../architecture/decisions/ADR-20260729-sandbox-host-isolation-broker-boundary.md) | `sandbox-provider-delivery-gates.contract.json` |
| [REVIEW-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain](reviews/REVIEW-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain.md) | [REQ-2026-0012](../product/requirements/REQ-2026-0012-sandbox-firecracker-artifact-compatibility-and-supply-chain.md) | [ADR-20260729](../architecture/decisions/ADR-20260729-sandbox-firecracker-artifact-compatibility-and-supply-chain.md) | `sandbox-provider-delivery-gates.contract.json` |
| [REVIEW-20260729-sandbox-workspace-block-device-attachment-and-sanitization](reviews/REVIEW-20260729-sandbox-workspace-block-device-attachment-and-sanitization.md) | [REQ-2026-0013](../product/requirements/REQ-2026-0013-sandbox-workspace-block-device-attachment-and-sanitization.md) | [ADR-20260729](../architecture/decisions/ADR-20260729-sandbox-workspace-block-device-attachment-and-sanitization.md) | `sandbox-provider-delivery-gates.contract.json` |
| [REVIEW-20260729-sandbox-firecracker-network-isolation](reviews/REVIEW-20260729-sandbox-firecracker-network-isolation.md) | [REQ-2026-0014](../product/requirements/REQ-2026-0014-sandbox-firecracker-network-isolation.md) | [ADR-20260729](../architecture/decisions/ADR-20260729-sandbox-firecracker-network-isolation-and-egress-policy.md) | `sandbox-firecracker-network-isolation.contract.json`, `sandbox-provider-delivery-gates.contract.json` |
| [REVIEW-20260729-sandbox-firecracker-resource-isolation](reviews/REVIEW-20260729-sandbox-firecracker-resource-isolation.md) | [REQ-2026-0015](../product/requirements/REQ-2026-0015-sandbox-firecracker-resource-isolation-and-usage.md) | [ADR-20260729](../architecture/decisions/ADR-20260729-sandbox-firecracker-resource-isolation-and-usage-facts.md) | `sandbox-firecracker-resource-isolation.contract.json`, `sandbox-provider-delivery-gates.contract.json` |
| [REVIEW-20260729-sandbox-multi-tenant-admission-scheduling-and-capacity](reviews/REVIEW-20260729-sandbox-multi-tenant-admission-scheduling-and-capacity.md) | [REQ-2026-0016](../product/requirements/REQ-2026-0016-sandbox-multi-tenant-admission-scheduling-and-capacity.md) | [ADR-20260729](../architecture/decisions/ADR-20260729-sandbox-multi-tenant-admission-scheduling-and-capacity-reservation.md) | `sandbox-multi-tenant-scheduling.contract.json`, `sandbox-provider-delivery-gates.contract.json` |
| [REVIEW-20260729-sandbox-node-trust-enrollment-attestation-and-inventory](reviews/REVIEW-20260729-sandbox-node-trust-enrollment-attestation-and-inventory.md) | [REQ-2026-0017](../product/requirements/REQ-2026-0017-sandbox-node-trust-enrollment-attestation-and-inventory.md) | [ADR-20260729](../architecture/decisions/ADR-20260729-sandbox-node-trust-enrollment-attestation-and-inventory.md) | `sandbox-node-trust-and-inventory.contract.json`, `sandbox-provider-delivery-gates.contract.json` |
| [REVIEW-20260729-sandbox-postgresql-quota-and-capacity-persistence](reviews/REVIEW-20260729-sandbox-postgresql-quota-and-capacity-persistence.md) | [REQ-2026-0018](../product/requirements/REQ-2026-0018-sandbox-postgresql-quota-and-capacity-reservation-persistence.md) | [ADR-20260729](../architecture/decisions/ADR-20260729-sandbox-postgresql-quota-and-capacity-reservation-persistence.md) | `sandbox-provider-delivery-gates.contract.json`, `sandbox-quota-and-capacity-persistence.contract.json` |
| [REVIEW-20260730-sandbox-runtime-pool-architecture-security](reviews/REVIEW-20260730-sandbox-runtime-pool-architecture-security.md) | [REQ-2026-0019](../product/requirements/REQ-2026-0019-sandbox-runtime-pool-and-fast-allocation.md) | [ADR-20260730](../architecture/decisions/ADR-20260730-sandbox-runtime-pool-claim-and-sanitization.md) | `sandbox-runtime-pool.contract.json` |
| [REVIEW-20260730-sandbox-lifecycle-history-and-idempotency-retention](reviews/REVIEW-20260730-sandbox-lifecycle-history-and-idempotency-retention.md) | [REQ-2026-0020](../product/requirements/REQ-2026-0020-sandbox-lifecycle-hot-state-and-idempotency-retention.md) | [ADR-20260730](../architecture/decisions/ADR-20260730-sandbox-lifecycle-hot-state-and-idempotency-ledger.md) | `sandbox-lifecycle-history-and-idempotency.contract.json` |
| [REVIEW-20260730-sandbox-workspace-runtime-transaction-architecture-security](reviews/REVIEW-20260730-sandbox-workspace-runtime-transaction-architecture-security.md) | [REQ-2026-0021](../product/requirements/REQ-2026-0021-sandbox-workspace-runtime-transaction-and-checkpoint.md) | [ADR-20260730](../architecture/decisions/ADR-20260730-sandbox-workspace-runtime-transaction-and-checkpoint.md) | `sandbox-workspace-runtime-transaction.contract.json` |

Additional packets are pending human review without gating a machine contract, and the command above
lists them with their requirement, decision and reviewer roles.

## Procedure

1. Run the gate and take the pending backlog from its output; do not work from a stale copy of this page.
2. For each packet, route it to the reviewer roles its own sign-off table names. A role the contract
   requires but the packet does not ask to sign is a gate failure, not a review shortcut.
3. Record each reviewer outcome as `Approved`, `Changes requested` or `Rejected`. `Approved with
   follow-up` is not available while a packet still records an open pre-review finding.
4. When every reviewer outcome on a packet is `Approved` and its close-out checklist is complete, the
   packet status may move off `pending-human-review`, the requirement may move to `ready`, the decision
   may move to `accepted`, and only then may a contract set `implementationAuthorized`.
5. Re-run the gate after each state change. It refuses a partial transition, so the order in step 4
   cannot be skipped.

## What Signing Here Does Not Authorize

A signed packet authorizes only the decisions recorded in that packet. It does not authorize a Provider,
Provider Port, API route, HTTP route, SDK, scheduler, isolation policy, secret injection, deployable
profile, runtime dependency change, host I/O, process spawn, public port, or isolation claim, and it is
never commercial or runtime evidence.
