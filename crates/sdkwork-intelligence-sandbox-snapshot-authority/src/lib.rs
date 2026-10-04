#![forbid(unsafe_code)]
//! Provider-neutral authority model for SDKWork Sandbox snapshots and fork
//! derivation (`REQ-2026-0031`, `ADR-20261005`).
//!
//! The crate carries the authority-model slice REVIEW-20261005 authorized:
//! the [`snapshot::SandboxSnapshot`] record with the closed lifecycle
//! [`state`] machine, the [`fork`] derivation semantics (immutable source, N
//! bounded derivations running in parallel, fresh derived identity, no
//! source-tenant-state reuse), and the [`gates`] evidence and layering
//! constants. Snapshots are immutable after creation by construction —
//! fields are private with read-only accessors and every lifecycle move is a
//! state-machine transition.
//!
//! What this crate deliberately does not own: any snapshot engine, storage
//! backend, restore pipeline, CLI, public API/SDK or deployment profile —
//! the contract's `forbidden` block keeps every one of those surfaces closed
//! until its own requirement slice lands, no E2B Snapshot/Fork
//! capability-parity claim may be made before an engine slice exists, and
//! `WarmMicroVmSlot` reuse additionally requires the `REQ-2026-0019` pool
//! evidence gate.

mod bounds;
mod error;
mod fork;
mod gates;
mod snapshot;
mod state;

pub use bounds::{MAX_SANDBOX_SNAPSHOT_DERIVATIONS_PER_PLAN, MAX_SANDBOX_SNAPSHOT_ID_LENGTH};
pub use error::{SandboxSnapshotAuthorityError, SandboxSnapshotAuthorityResult};
pub use fork::{
    SandboxForkDerivation, SANDBOX_SNAPSHOT_FORK_FRESH_IDENTITY_REQUIRED,
    SANDBOX_SNAPSHOT_FORK_PARALLEL_RUNNING, SANDBOX_SNAPSHOT_FORK_SOURCE_SNAPSHOT_IMMUTABLE,
    SANDBOX_SNAPSHOT_FORK_SOURCE_TENANT_STATE_REUSE_ALLOWED,
};
pub use gates::{
    sandbox_second_supply_chain_authority_allowed, sandbox_snapshot_owns_evidence_or_signature,
    sandbox_snapshot_references_artifact_tuple, SANDBOX_SNAPSHOT_ARTIFACT_AUTHORITY,
    SANDBOX_SNAPSHOT_CROSS_TENANT_RESIDUE_EVIDENCE_REQUIRED,
    SANDBOX_SNAPSHOT_DERIVED_IDENTITY_ROTATION_EVIDENCE_REQUIRED,
    SANDBOX_SNAPSHOT_LINE_LAYERING_CHECKPOINT_AUTHORITY,
    SANDBOX_SNAPSHOT_REAL_KVM_RESTORE_EVIDENCE_REQUIRED,
    SANDBOX_SNAPSHOT_REGISTRATION_ENABLES_WARM_SLOT,
    SANDBOX_SNAPSHOT_WARM_SLOT_REUSE_REQUIRES_POOL_EVIDENCE_GATE,
};
pub use snapshot::SandboxSnapshot;
pub use state::SandboxSnapshotState;

#[cfg(test)]
mod tests;
