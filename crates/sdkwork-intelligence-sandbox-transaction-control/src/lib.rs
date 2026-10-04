#![forbid(unsafe_code)]
//! Provider-neutral control plane for the SDKWork Workspace runtime
//! transaction (`REQ-2026-0021`, `ADR-20260730`).
//!
//! The crate carries the control-plane slice REVIEW-20260730 authorized: the
//! eleven-state [`state`] machine, the fixed 21-stage [`stage`] orchestration
//! order with typed not-applicable evidence, the sealed
//! [`checkpoint::SandboxWorkspaceCheckpointCandidate`] with Agents-only
//! compare-and-swap promotion, deterministic [`compensation`] windows, and
//! the bounded [`registry`] with writer-fencing and idempotency discipline.
//!
//! What this crate deliberately does not own: real Provider/host runtime,
//! the PostgreSQL transaction authority, storage/KMS adapters, a worker, or
//! any API/SDK/transport surface — the machine contract's `x-sdkwork-no-*`
//! gates keep those closed, and the real Local/KVM/PostgreSQL evidence
//! obligations stay release-blocking. Local and Cloud share these semantics;
//! only the composition adapter and assurance differ
//! (`ADR-20260730` decision 2).

mod bounds;
mod checkpoint;
mod compensation;
mod error;
mod registry;
mod stage;
mod state;

pub use bounds::{
    SANDBOX_COMPENSATION_ATTEMPT_COUNT_MAX, SANDBOX_CONCURRENT_COMMANDS_PER_TRANSACTION_MAX,
    SANDBOX_ORCHESTRATION_STAGE_COUNT_MAX, SANDBOX_QUEUE_WAIT_MS_MAX,
    SANDBOX_RECONCILIATION_BATCH_SIZE_MAX, SANDBOX_REFERENCE_MAX_LENGTH, SANDBOX_REQUEST_MAX_BYTES,
    SANDBOX_RETRY_AFTER_MS_MAX,
};
pub use checkpoint::{SandboxCandidatePromotion, SandboxWorkspaceCheckpointCandidate};
pub use compensation::{SandboxCompensationAction, SandboxCompensationWindow};
pub use error::{SandboxWorkspaceRuntimeError, SandboxWorkspaceRuntimeResult};
pub use registry::{
    BoundedSandboxWorkspaceTransactionControl, SandboxTerminalOutcome, SandboxTransactionClock,
    SandboxTransactionHandle, SandboxWorkspaceMountMode, SandboxWorkspaceRuntimeTransactionRequest,
};
pub use stage::{
    sandbox_orchestration_stages, SandboxOrchestrationLedger, SandboxOrchestrationStage,
    SandboxStageEvidence,
};
pub use state::SandboxWorkspaceRuntimeTransactionState;

#[cfg(test)]
mod tests;
