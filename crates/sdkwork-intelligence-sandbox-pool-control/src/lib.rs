#![forbid(unsafe_code)]
//! Provider-neutral control plane for the SDKWork Sandbox runtime pool
//! (`REQ-2026-0019`, `ADR-20260730`).
//!
//! The crate carries the control-plane slice the accepted review packet
//! (`REVIEW-20260730-sandbox-runtime-pool-architecture-security`) authorized:
//! the [`state`] machine of [`slot::SandboxPoolSlot`] and
//! [`claim::SandboxPoolClaim`], single-owner fenced idempotent claims, and
//! the bounded [`registry`] that implements the eight contract operations of
//! `specs/sandbox-runtime-pool.contract.json`.
//!
//! What this crate deliberately does not own: any Provider, Node, VMM or
//! snapshot runtime (the L4 [`port::SandboxPoolHostPreparationPort`] is a
//! declared seam with no in-repo implementation), any database persistence
//! (PostgreSQL stays the future cloud claim authority behind `REQ-2026-0018`),
//! any API, SDK, config or deployment surface, and `WarmMicroVmSlot` runtime
//! evidence (a separate KVM evidence gate). A `ready` slot carries no tenant
//! state by construction, and cleanup uncertainty can only quarantine —
//! capacity is never overcommitted and a time-to-live never returns a slot
//! to `ready`.

mod bounds;
mod claim;
mod error;
mod fencing;
mod identity;
mod port;
mod registry;
mod slot;
mod state;

pub use bounds::{
    SANDBOX_POOL_CANDIDATE_SLOT_COUNT_MAX, SANDBOX_POOL_CLAIM_ATTEMPT_COUNT_MAX,
    SANDBOX_POOL_CLAIM_TTL_SECONDS_MAX, SANDBOX_POOL_CLEANUP_DEADLINE_SECONDS_MAX,
    SANDBOX_POOL_EXHAUSTED_RETRY_AFTER_SECONDS, SANDBOX_POOL_FENCING_TOKEN_MAX,
    SANDBOX_POOL_PER_PROFILE_TARGET_MAX, SANDBOX_POOL_RECONCILIATION_BATCH_SIZE_MAX,
    SANDBOX_POOL_REFILL_OPERATIONS_PER_NODE_MAX, SANDBOX_POOL_RETRY_AFTER_SECONDS_MAX,
};
pub use claim::{SandboxPoolClaim, SandboxPoolClaimRequest, SANDBOX_POOL_FINGERPRINT_LENGTH};
pub use error::{
    SandboxPoolClaimConflictKind, SandboxPoolDependency, SandboxPoolQuarantineReason,
    SandboxPoolRetryAfter, SandboxRuntimePoolError, SandboxRuntimePoolResult,
};
pub use fencing::{SandboxPoolFencingToken, SANDBOX_POOL_FENCING_TOKEN_INITIAL};
pub use identity::{
    SandboxPoolClaimId, SandboxPoolOpaqueRef, SandboxPoolOperationId, SandboxPoolProviderKind,
    SandboxPoolSlotId, SandboxPoolTenantId, SandboxResourceProfileId, MAX_SANDBOX_POOL_ID_LENGTH,
};
pub use port::{
    SandboxPoolCapacityTarget, SandboxPoolHostPreparationEvidence, SandboxPoolHostPreparationPort,
    SandboxPoolHostPreparationRequest, SandboxPoolPreparationRequest, SandboxPoolQuarantineRequest,
    SandboxPoolReadyRequest, SandboxPoolRefillAction, SandboxPoolReleaseOutcome,
    SandboxPoolReleaseRequest, SandboxPoolRetireRequest, SandboxPoolSlotReconciliationReport,
    SandboxPoolSlotReconciliationRequest, SandboxPoolTargetReconciliationReport,
    SandboxRuntimePoolPort,
};
pub use registry::{sandbox_system_clock, BoundedSandboxPoolControl, SandboxPoolClock};
pub use slot::{SandboxIsolationAssurance, SandboxPoolClass, SandboxPoolSlot};
pub use state::{SandboxPoolClaimState, SandboxPoolSlotState};

#[cfg(test)]
mod tests;
