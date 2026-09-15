# REVIEW-20260731 Sandbox Commercial Readiness Gap Audit

Status: active

Outcome: No-Go

Date: 2026-07-31

Owner: SDKWork Runtime Platform

Scope: `sdkwork-birdcoder`, `sdkwork-agents`, `sdkwork-kernel`, `sdkwork-sandbox`

Plan: [PLAN-2026-0002](../plans/PLAN-2026-0002-commercial-cloud-agent-runtime-delivery.md)

Machine gate: [`sandbox-commercial-readiness.contract.json`](../../../specs/sandbox-commercial-readiness.contract.json)

Specs: `REQUIREMENTS_SPEC.md`, `ARCHITECTURE_DECISION_SPEC.md`, `QUALITY_GATE_SPEC.md`, `SECURITY_SPEC.md`, `PRIVACY_SPEC.md`, `PERFORMANCE_SPEC.md`, `TEST_SPEC.md`

## Executive Decision

The overall ownership direction is suitable for a commercial hybrid coding product, but the product is not runnable or releasable. Current green tests prove lifecycle candidates and disabled Gate 0 contracts. They do not prove a Local IDE runtime, a Firecracker cloud runtime, tenant isolation, pool allocation, or four-repository integration.

The release decision remains **No-Go**. Runtime implementation remains constrained by the existing requirements and human-review gates. This audit does not approve a Provider, public/internal transport, API/SDK, database migration, security policy, secret mechanism, deployment profile, or production release.

## Correct Baseline To Preserve

The following boundaries should remain stable while mechanisms evolve:

| Stable concern | Authority and rule |
| --- | --- |
| Coding workbench | BirdCoder owns UI composition and execution preference UX; it uses the generated Agents SDK and never calls Kernel or Sandbox directly. |
| Business execution | Agents owns Workspace, Project, Session, Task, Revision authorization, execution intent, and Revision promotion. |
| Execution placement | Kernel owns active execution assignment, routing, cancellation, recovery, lease, and fencing. |
| Capacity placement | Sandbox owns admission, provider/node selection, capacity reservation, runtime allocation, attachment, isolation, readiness, cleanup, and quarantine. |
| Workspace bytes | Drive or an approved Workspace volume authority owns bytes, encryption, retention, backup, and deletion. Sandbox receives only bounded attachment grants. |
| Placement consistency | Kernel execution placement and Sandbox capacity placement remain different records with independent idempotency, leases, fencing generations, and reconcilers. |
| Runtime/data lifecycle | Runtime image/root is disposable. Workspace and durable Checkpoint data are attached after a fenced allocation and detached before sanitization and reuse. |
| Extensibility | Local and Firecracker implement provider-neutral Sandbox ports. Provider choice does not escape into BirdCoder, Agents, or Kernel business branches. |

## Current Findings

| Severity | Finding | Commercial effect |
| --- | --- | --- |
| P0 | BirdCoder hybrid execution, Agents orchestration, Kernel placement, and all Sandbox runtime gates are blocked, proposed, or pending human review. | There is no authorized end-to-end implementation path. |
| P0 | Local Provider contains only a test fake; Service Host and CLI are stubs; the shared command executor is not materialized. | BirdCoder cannot run a real governed Local coding Session through Sandbox. |
| P0 | The bounded command contract explicitly defers interactive PTY, stdin, resize, stream replay, reconnect, and terminal-session ownership. | A commercial IDE terminal cannot be implemented safely from the current unary command schema. |
| P0 | No reviewed authenticated and versioned Kernel-to-Sandbox cloud control-plane contract exists. REQ-0021 explicitly excludes transport/API/SDK implementation. | Kernel cannot use Sandbox across processes or nodes without inventing an authority boundary. |
| P0 | Secret injection is excluded from current delivery gates and lacks a value-free grant, projection, rotation, revocation, and cleanup contract. | Builds requiring credentials cannot run without ambient-secret leakage or an unreviewed mechanism. |
| P0 | No Firecracker Provider, Host Broker, node agent, guest agent, signed artifact tuple, KVM evidence, cgroup/network isolation, or Workspace block-device implementation exists. | No cloud isolation assurance can be claimed. |
| P0 | Workspace and Checkpoint byte authority remains a candidate choice between Drive and an approved volume owner. | ReadWrite Session durability, conflict handling, restore, retention, and deletion are unresolved. |
| P0 | Quota and capacity persistence is blocked by the current `tenant_id TEXT` to positive `BIGINT` subject migration. | Atomic tenant quota and no-overcommit cannot be implemented under the current schema. |
| P0 | Lifecycle hydration reads complete operation history, while retention and late-retry semantics remain unapproved. | Long-lived high-concurrency Session cost and idempotent retry safety are not bounded. |
| P0 | Local residency has a Gate 0 inventory but no real four-repository storage, transfer, backup/restore, export/purge, and failure evidence. | The product cannot truthfully claim all coding data remains on the device. |
| P0 | Cloud Workspace, Checkpoint, output, log, cache, backup, region, retention, export, deletion, and disaster-recovery policy is not one approved release gate. | SaaS residency and recovery claims are incomplete even if runtime isolation succeeds. |
| P0 | No compatibility contract pins BirdCoder, Agents, Kernel, Sandbox, Workspace storage, protocol, and artifact versions. | Rolling upgrade or downgrade can silently cross incompatible lifecycle semantics. |
| P1 | Pool, fairness, saturation, allocation latency, reconnect grace, retention, failover, and recovery budgets lack approved target values and reference hardware. | High-concurrency and SLO claims are unmeasured. |
| P1 | PostgreSQL 16/17 evidence is reproducible but not archived by release CI; KVM, security, load, failover, and residue evidence is absent. | Evidence cannot be tied to an immutable release revision set. |
| P1 | Node drain, compromised node, artifact rollback, pool quarantine, region/control-plane failure, and compatibility rollback runbooks are incomplete. | The service is not supportable in production. |
| P1 | Contract tests intentionally assert that implementations remain disabled, but the default test output is fully green. | Reviewers can mistake contract consistency for product readiness without a separate release gate. |

## Missing Ready Contract Boundaries

These are separate responsibilities and should not be folded into Provider crates or the Service Host:

1. **Internal control plane and transport**: define authenticated Kernel-to-Sandbox operations, request context, contract versioning, deadlines, idempotency, cancellation, streaming selection, compatibility, health, and topology parity. Decide HTTP internal-api versus RPC only under the applicable API/RPC review.
2. **Interactive terminal session**: define PTY ownership, logical executable policy, stdin, resize, output sequence/replay, reconnect grace, backpressure, cancellation, first-terminal outcome, retention, and cleanup without accepting shell command strings.
3. **Runtime Secret projection**: define opaque secret grants, least-privilege target mapping, non-persistence of values, rotation, revocation, redaction, guest delivery, cleanup, and audit with a named Secret/KMS authority.
4. **Cloud data residency and recovery**: bind region, Workspace/Checkpoint/output/log/cache lifecycle, encryption, backup/PITR, RPO/RTO, export, deletion, legal hold, failure behavior, and cross-region transfer to explicit owners.
5. **Cross-repository compatibility and release**: pin contract/protocol/artifact versions and immutable revisions, then define upgrade, drain, rollback, downgrade, and support windows.

## Composable Target

```text
BirdCoder workbench
  -> generated Agents App SDK
    -> Agents execution-intent service
      -> Kernel execution-placement port
        -> Sandbox control-plane port
          -> admission and capacity reservation
          -> provider-neutral workspace transaction
          -> Local Provider or Firecracker Provider
          -> command or interactive-terminal port
          -> checkpoint handoff
          -> cleanup, residue verification, quarantine, release
```

The Service Host composes ports and adapters; it does not implement provider mechanisms. The internal transport adapts the same control-plane port used in standalone composition. The node agent and privileged Host Broker expose fixed typed mechanisms and do not own product policy. Pooling remains an optional optimization over a correct cold allocation path.

## Delivery Order

1. Resolve human decisions and approve only the requirements needed for the Local slice, including lifecycle retention, command, terminal, Local Host boundary, Service Host, standalone data residency, and four-repository compatibility.
2. Deliver the Local standalone runtime with real Windows/Linux evidence and explicit macOS capability denial where containment is unavailable.
3. Approve and deliver the internal control plane, SQL subject migration, cold Firecracker data plane, node trust, Workspace/Checkpoint authority, Secret projection, and cloud data governance.
4. Prove cold Firecracker correctness and operations before enabling any Runtime Pool optimization.
5. Add Prepared Pool slots first; add warm microVM reuse only after snapshot compatibility, identity rotation, rebinding, and cross-tenant residue evidence.
6. Enable BirdCoder cloud capability only from live Agents/Kernel/Sandbox evidence and only on one immutable compatible release set.

## Verification Evidence

Executed from `sdkwork-sandbox` on 2026-07-31:

```text
cargo fmt --all -- --check                                      PASS
cargo check --workspace                                        PASS
cargo test --workspace                                         PASS (44 passed, 1 external PostgreSQL test ignored)
cargo clippy --workspace --all-targets -- -D warnings           PASS
node --test tests/contract/*.test.mjs                           PASS (190 tests before this audit)
check-repository-docs-standard.mjs --root .                     PASS
check-workspace-packages-layout.mjs --root . --mode enforce     PASS
check-component-port-bindings.mjs --root . --strict             PASS
audit-repository-baseline.mjs --root .                          PASS
```

The Rust command was invoked through the installed toolchain at `<home>\.cargo\bin\cargo.exe` because `cargo` was not present on the inherited PowerShell `PATH`.

## Required Human Action

Owners must review the protected decisions listed in the existing BirdCoder, Agents, Kernel, and Sandbox review packets. An approval must name accountable owners, exact target values, selected Workspace/Secret authorities, transport choice, compatibility policy, and real test environments. Changing an `implementationAuthorized` flag without that evidence is not an approval.
