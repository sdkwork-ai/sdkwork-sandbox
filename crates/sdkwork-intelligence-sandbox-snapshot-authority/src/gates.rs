//! Evidence gates and the artifact boundary (`contract`: `evidenceGates` and
//! `artifactBoundary`).
//!
//! The gates are fail-closed documentation: they state, as constants, what
//! must be true before any engine/restore capability is authorized. The
//! artifact boundary pins `REQ-2026-0012` as the single supply-chain
//! authority.

/// The single supply-chain authority every snapshot references.
pub const SANDBOX_SNAPSHOT_ARTIFACT_AUTHORITY: &str = "REQ-2026-0012";

/// Real Linux KVM restore evidence is release-blocking.
pub const SANDBOX_SNAPSHOT_REAL_KVM_RESTORE_EVIDENCE_REQUIRED: bool = true;

/// Cross-tenant residue evidence is release-blocking.
pub const SANDBOX_SNAPSHOT_CROSS_TENANT_RESIDUE_EVIDENCE_REQUIRED: bool = true;

/// Derived-identity rotation evidence is release-blocking.
pub const SANDBOX_SNAPSHOT_DERIVED_IDENTITY_ROTATION_EVIDENCE_REQUIRED: bool = true;

/// `WarmMicroVmSlot` reuse of a snapshot additionally requires the
/// REQ-2026-0019 pool evidence gate; this registration does not move it.
pub const SANDBOX_SNAPSHOT_WARM_SLOT_REUSE_REQUIRES_POOL_EVIDENCE_GATE: bool = true;

/// Whether snapshots reference (rather than embed) artifact tuples. They do.
#[must_use]
pub const fn sandbox_snapshot_references_artifact_tuple() -> bool {
    true
}

/// Whether the snapshot authority owns artifact evidence. It does not.
#[must_use]
pub const fn sandbox_snapshot_owns_evidence_or_signature() -> bool {
    false
}

/// Whether a second supply-chain authority may exist next to
/// `REQ-2026-0012`. It may not.
#[must_use]
pub const fn sandbox_second_supply_chain_authority_allowed() -> bool {
    false
}

/// Whether the checkpoint line (`REQ-2026-0021`) and this snapshot line share
/// a persistence authority. They do not: a snapshot is sandbox runtime full
/// state, a checkpoint is a workspace transaction durability candidate.
pub const SANDBOX_SNAPSHOT_LINE_LAYERING_CHECKPOINT_AUTHORITY: &str = "REQ-2026-0021";

/// Whether registering this capability enables `WarmMicroVmSlot`. It does
/// not.
pub const SANDBOX_SNAPSHOT_REGISTRATION_ENABLES_WARM_SLOT: bool = false;
