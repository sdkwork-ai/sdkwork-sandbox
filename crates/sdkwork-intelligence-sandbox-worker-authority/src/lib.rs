#![forbid(unsafe_code)]
//! Provider-neutral authority model for SDKWork Sandbox worker launch
//! executions (`REQ-2026-0034`, `ADR-20261006`).
//!
//! The crate carries the authority-model slice REVIEW-20261006 authorized:
//! the [`execution::SandboxWorkerExecution`] record with the closed lifecycle
//! [`state`] machine (`accepted` is the only initial state; `started` is
//! reachable only from `starting`, so a completion claim must be earned
//! through the executor; `started`/`failed`/`quarantined` are terminal), the
//! earned-reference rules (provisioning earns the allocation reference,
//! starting earns the start-command reference), and the [`gates`] evidence
//! and layering constants. Records are private-fielded with read-only
//! accessors, so every lifecycle move is a state-machine transition.
//!
//! What this crate deliberately does not own: the local-lane execution
//! adapter, the Firecracker VMM runtime, warm-slot consumption, any CLI,
//! public API/SDK or deployment profile — the contract's `forbidden` block
//! keeps every one of those surfaces closed until its own requirement slice
//! lands, no E2B fast-create capability-parity claim may be made before the
//! local-lane adapter exists and proves real execution, and warm-slot
//! consumption additionally requires the `REQ-2026-0019` pool evidence gate.
//! Launch plans stay owned by `REQ-2026-0033` (acceptance is the only plan
//! handoff; the worker never writes plan state), command execution by
//! `REQ-2026-0007` (consumed through the executor port, never
//! reimplemented).

mod bounds;
mod error;
mod execution;
mod gates;
mod state;

pub use bounds::MAX_SANDBOX_WORKER_EXECUTION_ID_LENGTH;
pub use error::{SandboxWorkerAuthorityError, SandboxWorkerAuthorityResult};
pub use execution::SandboxWorkerExecution;
pub use gates::{
    sandbox_firecracker_lane_execution_authorized, sandbox_local_lane_execution_slice_authorized,
    sandbox_worker_authority_model_slice_authorized, SANDBOX_WORKER_COMMAND_EXECUTION_AUTHORITY,
    SANDBOX_WORKER_COMPLETION_MUST_BE_EARNED, SANDBOX_WORKER_FIRECRACKER_PROVIDER_AUTHORITY,
    SANDBOX_WORKER_FIRST_COMMAND_ZERO_WAIT_EVIDENCE_REQUIRED,
    SANDBOX_WORKER_KVM_LANE_EVIDENCE_REQUIRED, SANDBOX_WORKER_LAUNCH_PLAN_AUTHORITY,
    SANDBOX_WORKER_PROVIDER_SPI_BOUNDARY, SANDBOX_WORKER_REAL_EXECUTION_EVIDENCE_REQUIRED,
    SANDBOX_WORKER_REGISTRATION_DOES_NOT_TOUCH_THE_VMM,
    SANDBOX_WORKER_REGISTRATION_ENABLES_WARM_SLOT, SANDBOX_WORKER_WARM_SLOT_GATE,
};
pub use state::SandboxWorkerExecutionState;

#[cfg(test)]
mod tests;
