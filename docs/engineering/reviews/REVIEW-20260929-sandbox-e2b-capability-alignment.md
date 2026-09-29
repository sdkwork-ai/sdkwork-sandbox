# REVIEW-20260929: Sandbox E2B Capability Alignment Program

Status: accepted

Approval basis: the repository owner issued the structured session instruction recorded verbatim below. Recorded by the executing agent on that instruction, under the same single-owner convention as REVIEW-20260924 (the repository owner exercises every listed reviewer role; no multi-party signature is claimed or implied).

Risk: high - the program decides the delivery order for closing the E2B capability parity ledger (currently 4 `mapped` / 67 `pending-gate` operations). Wrong ordering wastes authorized implementation budget on gated lanes; the mitigation is that every slice still lands behind its own requirement, machine contract, and pinned gate, and this packet authorizes no implementation by itself.

Owner: SDKWork Runtime Platform

Date: 2026-09-29

Requirement: [REQ-2026-0028](../../product/requirements/REQ-2026-0028-sandbox-e2b-compatible-api-sdk-family.md)

Decision: [ADR-20260924](../../architecture/decisions/ADR-20260924-sandbox-e2b-api-sdk-authority.md)

Review purpose: decide and record the standing, resumable program that drives the repository from the current parity state (4 `mapped` / 67 `pending-gate` operations) toward full E2B capability alignment, and fix the delivery order of the slices that close each gap class.

## Owner Instruction (verbatim)

> 持续回归对齐，确保完整兼容E2B能力，直到所有能力对齐为止

followed, in the same session, by:

> 继续

Recorded 2026-09-29. The instruction names the direction (complete E2B capability alignment), the working mode (continuous regression alignment until done), and does not waive any gate: each slice keeps its own requirement record, machine contract, evidence obligations, and pinned contract tests.

## Current Parity State (2026-09-29, machine-checked)

- Parity ledger `apis/internal-api/intelligence/sandbox-e2b-parity-ledger.json`: 71 E2B reference operations = 4 `mapped` + 67 `pending-gate`; the mapping is generated from the pinned baseline and verified by contract tests.
- Capability matrix: 78 baseline rows / 34 product rows; the residual typed gaps live in `TECH-e2b-capability-parity.md` section 3.2.
- Zero-carrier capability classes (section 3.4): Template definition/build/cache, Snapshot/Fork, Auto-pause/resume, port exposure, MCP surface, filesystem execution face.
- `draft` requirements that carry aligned-but-unimplemented capabilities: Pool (REQ-2026-0019), checkpoint/transaction (REQ-2026-0021), internal control plane (REQ-2026-0023), terminal session (REQ-2026-0024), secret projection (REQ-2026-0025).
- Evidence registry: 125 of 127 evidence ids still require real-platform or human-reviewed evidence; the working environment has no Linux KVM node, Docker, or PostgreSQL.

## Decision Matrix

| ID | Decision proposed | Rationale | Reviewer action required |
| --- | --- | --- | --- |
| ALIGN-01 | Alignment proceeds in slice order: (1) Template authority REQ + contract (closes the largest zero-carrier class); (2) REQ-2026-0019 Pool promotion + control-plane slice; (3) REQ-2026-0021 checkpoint slice (workspace-level pause/resume semantics); (4) REQ-2026-0028 authority v1 lifecycle-face expansion with SDK regeneration; (5) Warm-slot and KVM evidence slices last. | Every earlier slice is control-plane and verifiable in this environment; the KVM-evidence slices cannot complete here and must not be faked. | Approve the order, or amend it. |
| ALIGN-02 | Each slice flips exactly the gates its own requirement names: requirement status, review packet status, machine-contract `implementationAuthorized`, contract-test pins, parity ledger/census, and the exit-readiness package move together in one commit. | The tree stays fully green after every slice; a slice that cannot move all its surfaces is not landed. | Approve the all-or-nothing landing rule. |
| ALIGN-03 | Capabilities that need a real Linux KVM node, external PostgreSQL, or Docker remain honestly `pending-gate`/`draft` until the environment exists; no simulator, mock, or WSL result may be recorded as platform evidence. | REQ-2026-0008 and the evidence registry already forbid functional simulation as assurance; alignment does not override that. | Confirm the no-simulation boundary. |
| ALIGN-04 | The Template authority REQ is authored as `draft` with a machine contract at `implementationAuthorized: false`; it carries the capability class (definition, build input, tags/aliases, start command, cache) without authorizing any builder, registry, or runtime. | Section 3.4 row 1 turns red the moment the record exists; the row and its citations move in the same commit as the record. | Approve the carrier-first sequencing. |
| ALIGN-05 | This packet authorizes no implementation, no contract-status flip, and no requirement-status flip by itself; it fixes the program, the order, and the landing rule only. | Human review per slice stays intact; this packet is the program-level decision those slice packets cite. | Confirm the packet's own scope. |

## Human Outcome

| Reviewer role | Reviewer | Outcome | Date | Decision IDs / findings |
| --- | --- | --- | --- | --- |
| Platform Owner | Repository Owner (structured approval) | Approved | 2026-09-29 | ALIGN-01, ALIGN-03, ALIGN-05 |
| Product Owner | Repository Owner (structured approval) | Approved | 2026-09-29 | ALIGN-01, ALIGN-04 |
| Security Owner | Repository Owner (structured approval) | Approved | 2026-09-29 | ALIGN-03, ALIGN-05 |
| Architecture Owner | Repository Owner (structured approval) | Approved | 2026-09-29 | ALIGN-01, ALIGN-02, ALIGN-04 |
| Engineering/Verification Owner | Repository Owner (structured approval) | Approved | 2026-09-29 | ALIGN-02, ALIGN-03 |

## Standing Regression Loop

Every alignment slice runs the same loop, recorded here so any session can resume it:

1. Read the next slice in ALIGN-01 order and its requirement/contract readiness blockers.
2. Land the slice with every surface it names (requirement, packet, contract, code, tests, pins, ledger, docs, changelog).
3. Run the full battery: 20 node gates, `node --test tests/contract/*.test.mjs`, `cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`.
4. Commit only when every gate is green; record the new parity reading in the changelog and the memory index.
