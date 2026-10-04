//! The provider-neutral pool ports.
//!
//! [`SandboxRuntimePoolPort`] is the L3 control-plane surface named by
//! `specs/sandbox-runtime-pool.contract.json` (`ports.sandbox_runtime_pool_port`):
//! the eight contract operations, no more. [`SandboxPoolHostPreparationPort`]
//! is the L4 seam toward host-side preparation and cleanup; it declares fixed
//! operations only (`ports.sandbox_pool_host_preparation_port`
//! `.fixedOperationsOnly`), never arbitrary host commands. Neither port is
//! authorized for public export (`publicExportAuthorized` is false on both).

use crate::claim::{SandboxPoolClaim, SandboxPoolClaimRequest};
use crate::error::SandboxRuntimePoolResult;
use crate::identity::{
    SandboxPoolClaimId, SandboxPoolOpaqueRef, SandboxPoolSlotId, SandboxResourceProfileId,
};
use crate::slot::{SandboxIsolationAssurance, SandboxPoolClass, SandboxPoolSlot};

/// A request to prepare one new slot on a trusted node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolPreparationRequest {
    /// Slot identity for the new slot.
    pub sandbox_pool_slot_id: SandboxPoolSlotId,
    /// The pool capacity class.
    pub sandbox_pool_class: SandboxPoolClass,
    /// The resource profile the capacity is shaped for.
    pub sandbox_resource_profile_id: SandboxResourceProfileId,
    /// Opaque verified-node reference (`allocationOrdering[1]` input).
    pub sandbox_node_reference: SandboxPoolOpaqueRef,
    /// Opaque provider identity.
    pub sandbox_provider_id: SandboxPoolOpaqueRef,
    /// The provider kind.
    pub sandbox_provider_kind: crate::identity::SandboxPoolProviderKind,
    /// The isolation assurance.
    pub sandbox_isolation_assurance: SandboxIsolationAssurance,
    /// The immutable artifact manifest revision (`REQ-2026-0012`).
    pub sandbox_artifact_manifest_revision: SandboxPoolOpaqueRef,
    /// The capacity revision the slot is admitted under.
    pub sandbox_capacity_revision: i64,
    /// The separate KVM evidence reference for
    /// [`SandboxPoolClass::WarmMicroVmSlot`]; required for that class and
    /// rejected otherwise.
    pub sandbox_warm_kvm_evidence_ref: Option<SandboxPoolOpaqueRef>,
}

/// A request to mark a `preparing` or `sanitizing` slot `ready` with fresh
/// tenant-neutral evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolReadyRequest {
    /// The slot.
    pub sandbox_pool_slot_id: SandboxPoolSlotId,
    /// The fencing token the caller observed; a stale token is rejected
    /// before any mutation.
    pub sandbox_expected_fencing_token: crate::fencing::SandboxPoolFencingToken,
    /// Fingerprint of the tenant-neutral preparation or cleanup evidence
    /// (lowercase hex SHA-256). Must differ from the previously recorded
    /// fingerprint so evidence is fresh at every `ready` entry.
    pub sandbox_preparation_evidence_fingerprint: String,
}

/// A request to start the bounded release of one bound claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolReleaseRequest {
    /// The claim to release.
    pub sandbox_pool_claim_id: SandboxPoolClaimId,
    /// The claim's fencing token; a stale token is rejected before any
    /// mutation.
    pub sandbox_expected_fencing_token: crate::fencing::SandboxPoolFencingToken,
}

/// The outcome of a release or quarantine: both sides of the bridge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolReleaseOutcome {
    /// The claim after the operation.
    pub sandbox_pool_claim: SandboxPoolClaim,
    /// The slot after the operation.
    pub sandbox_pool_slot: SandboxPoolSlot,
}

/// A request to quarantine a slot and its capacity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolQuarantineRequest {
    /// The slot.
    pub sandbox_pool_slot_id: SandboxPoolSlotId,
    /// The fencing token the caller observed.
    pub sandbox_expected_fencing_token: crate::fencing::SandboxPoolFencingToken,
    /// Why the slot is leaving service.
    pub sandbox_quarantine_reason: crate::error::SandboxPoolQuarantineReason,
}

/// A request to retire a slot permanently.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolRetireRequest {
    /// The slot.
    pub sandbox_pool_slot_id: SandboxPoolSlotId,
    /// The fencing token the caller observed.
    pub sandbox_expected_fencing_token: crate::fencing::SandboxPoolFencingToken,
}

/// One bounded reconciliation pass over slots and claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxPoolSlotReconciliationRequest {
    /// How many candidates this pass may examine, at most
    /// [`crate::bounds::SANDBOX_POOL_RECONCILIATION_BATCH_SIZE_MAX`].
    pub sandbox_batch_size: usize,
}

/// The report of one slot reconciliation pass.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct SandboxPoolSlotReconciliationReport {
    /// Candidates examined by the pass.
    pub sandbox_examined: usize,
    /// Slots the pass quarantined, with the reason.
    pub sandbox_quarantined: Vec<(SandboxPoolSlotId, crate::error::SandboxPoolQuarantineReason)>,
}

/// One declared ready-capacity target for a resource profile and failure
/// domain (`scaling.sandbox_targetScope`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolCapacityTarget {
    /// The resource profile.
    pub sandbox_resource_profile_id: SandboxResourceProfileId,
    /// The failure domain (for example one verified node) the refill applies
    /// to.
    pub sandbox_node_reference: SandboxPoolOpaqueRef,
    /// The declared ready-slot target, at most
    /// [`crate::bounds::SANDBOX_POOL_PER_PROFILE_TARGET_MAX`].
    pub sandbox_ready_target: u32,
}

/// One bounded refill action produced by target reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolRefillAction {
    /// The resource profile to refill.
    pub sandbox_resource_profile_id: SandboxResourceProfileId,
    /// The failure domain to refill on.
    pub sandbox_node_reference: SandboxPoolOpaqueRef,
    /// How many slots to prepare, at most
    /// [`crate::bounds::SANDBOX_POOL_REFILL_OPERATIONS_PER_NODE_MAX`] per
    /// node per pass.
    pub sandbox_refill_count: usize,
}

/// The report of one target reconciliation pass.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct SandboxPoolTargetReconciliationReport {
    /// Bounded refill actions, ready to drive through
    /// [`SandboxRuntimePoolPort::sandbox_prepare_pool_slot`].
    pub sandbox_refill_actions: Vec<SandboxPoolRefillAction>,
    /// Profiles the pass examined.
    pub sandbox_examined_profiles: usize,
}

/// The provider-neutral Sandbox runtime pool control plane.
///
/// Implementations own the authoritative record for slots and claims. The
/// cloud authority is PostgreSQL
/// (`persistenceConcurrencyAndRecovery.sandbox_cloud_authoritativeStore`);
/// this crate's [`crate::registry::BoundedSandboxPoolControl`] is the bounded
/// single-controller registry for composition and tests, not that authority.
pub trait SandboxRuntimePoolPort {
    /// Creates one slot in `preparing` (`operations[0]`).
    fn sandbox_prepare_pool_slot(
        &self,
        request: SandboxPoolPreparationRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlot>;

    /// Binds one fenced claim to a ready slot (`operations[1]`).
    fn sandbox_claim_pool_slot(
        &self,
        request: SandboxPoolClaimRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolClaim>;

    /// Marks a `preparing` or `sanitizing` slot `ready` with fresh
    /// tenant-neutral evidence (`operations[2]`).
    fn sandbox_mark_pool_slot_ready(
        &self,
        request: SandboxPoolReadyRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlot>;

    /// Starts the bounded release of one bound claim (`operations[3]`);
    /// idempotent.
    fn sandbox_release_pool_claim(
        &self,
        request: SandboxPoolReleaseRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolReleaseOutcome>;

    /// Quarantines a slot and its capacity (`operations[4]`).
    fn sandbox_quarantine_pool_slot(
        &self,
        request: SandboxPoolQuarantineRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlot>;

    /// Retires a slot permanently (`operations[5]`).
    fn sandbox_retire_pool_slot(
        &self,
        request: SandboxPoolRetireRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlot>;

    /// Runs one bounded slot reconciliation pass (`operations[6]`).
    fn sandbox_reconcile_pool_slots(
        &self,
        request: SandboxPoolSlotReconciliationRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolSlotReconciliationReport>;

    /// Computes one bounded refill plan for declared targets
    /// (`operations[7]`).
    fn sandbox_reconcile_pool_targets(
        &self,
        request: Vec<SandboxPoolCapacityTarget>,
    ) -> SandboxRuntimePoolResult<SandboxPoolTargetReconciliationReport>;
}

/// A host-side preparation and cleanup evidence request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolHostPreparationRequest {
    /// The slot the host work is for.
    pub sandbox_pool_slot_id: SandboxPoolSlotId,
    /// The opaque verified-node reference the work runs on.
    pub sandbox_node_reference: SandboxPoolOpaqueRef,
    /// The resource profile.
    pub sandbox_resource_profile_id: SandboxResourceProfileId,
    /// The immutable artifact manifest revision.
    pub sandbox_artifact_manifest_revision: SandboxPoolOpaqueRef,
}

/// Tenant-neutral evidence returned by one fixed host operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolHostPreparationEvidence {
    /// Fingerprint of the recorded evidence (lowercase hex SHA-256).
    pub sandbox_evidence_fingerprint: String,
    /// Whole seconds from the host's clock when the evidence was recorded.
    pub sandbox_recorded_at: u64,
}

/// The L4 seam toward host preparation and cleanup.
///
/// The operation set is fixed: exactly the steps of
/// `releaseAndSanitization.orderedSteps` a host can execute, plus the
/// readiness verification. Implementations must be idempotent and bounded;
/// an uncertain outcome must be reported as failure so the control plane
/// quarantines (`sandbox_uncertain_cleanup_quarantines_slot`).
pub trait SandboxPoolHostPreparationPort {
    /// Materializes immutable artifacts and runtime-directory identity for
    /// one slot (`orderedSteps`: bounded host preparation).
    fn sandbox_prepare_pool_host_resources(
        &self,
        request: SandboxPoolHostPreparationRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolHostPreparationEvidence>;

    /// Verifies trusted-node eligibility, artifact currency and
    /// tenant-neutral runtime-directory identity.
    fn sandbox_verify_pool_host_preparation(
        &self,
        request: SandboxPoolHostPreparationRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolHostPreparationEvidence>;

    /// Erases the ephemeral layer (`orderedSteps[5]`).
    fn sandbox_erase_pool_ephemeral_layer(
        &self,
        request: SandboxPoolHostPreparationRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolHostPreparationEvidence>;

    /// Scans for cross-tenant residue (`orderedSteps[6]`).
    fn sandbox_scan_pool_cross_tenant_residue(
        &self,
        request: SandboxPoolHostPreparationRequest,
    ) -> SandboxRuntimePoolResult<SandboxPoolHostPreparationEvidence>;
}
