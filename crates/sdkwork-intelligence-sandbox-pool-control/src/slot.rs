//! The [`SandboxPoolSlot`] record and its pool classes.
//!
//! A slot is the provider-neutral unit of pooled runtime capacity
//! (`specs/sandbox-runtime-pool.contract.json`: `slot`). Every contract
//! required field appears verbatim as a Rust field. A slot never carries
//! tenant workspace, secrets, credentials, network grants, command output or
//! provider-private metadata (`slot.providerPrivateMetadataAllowed` is false):
//! the fields below are enumerable, tenant-neutral facts about trusted-node
//! eligibility, immutable artifacts and runtime-directory identity.

use crate::error::SandboxRuntimePoolError;
use crate::fencing::SandboxPoolFencingToken;
use crate::identity::{
    SandboxPoolOpaqueRef, SandboxPoolProviderKind, SandboxPoolSlotId, SandboxResourceProfileId,
};
use crate::state::SandboxPoolSlotState;

/// The pool capacity classes (`poolClasses`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxPoolClass {
    /// The first commercial implementation target: trusted node, immutable
    /// artifacts and bounded host preparation are complete, the VMM is not
    /// tenant-running and no tenant state is present
    /// (`poolClasses.sandbox_prepared_slot`).
    PreparedSlot,
    /// Reserved for the separately approved KVM snapshot-evidence slice;
    /// construction is fail-closed without a warm evidence reference
    /// (`poolClasses.sandbox_warm_microvm_slot`).
    WarmMicroVmSlot,
    /// The mandatory correctness fallback; it never weakens assurance
    /// (`poolClasses.sandbox_cold_allocation`).
    ColdAllocation,
}

impl SandboxPoolClass {
    /// The contract class key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreparedSlot => "sandbox_prepared_slot",
            Self::WarmMicroVmSlot => "sandbox_warm_microvm_slot",
            Self::ColdAllocation => "sandbox_cold_allocation",
        }
    }

    /// Parses a contract class key; unknown keys are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "sandbox_prepared_slot" => Self::PreparedSlot,
            "sandbox_warm_microvm_slot" => Self::WarmMicroVmSlot,
            "sandbox_cold_allocation" => Self::ColdAllocation,
            _ => return None,
        })
    }
}

impl std::fmt::Display for SandboxPoolClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The isolation assurance a slot was prepared for.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxIsolationAssurance {
    /// A hardware-virtualized microVM.
    MicroVm,
    /// A container boundary; never a Firecracker capacity fallback
    /// (`deploymentScope.sandbox_localDockerOrWeakAssuranceFallbackAllowed`
    /// is false).
    Container,
    /// A process boundary.
    Process,
}

impl SandboxIsolationAssurance {
    /// The contract assurance name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MicroVm => "micro_vm",
            Self::Container => "container",
            Self::Process => "process",
        }
    }

    /// Parses an assurance name; unknown names are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "micro_vm" => Self::MicroVm,
            "container" => Self::Container,
            "process" => Self::Process,
            _ => return None,
        })
    }
}

impl std::fmt::Display for SandboxIsolationAssurance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One pooled runtime capacity unit.
///
/// Mutation is owned by the control plane ([`crate::registry::
/// BoundedSandboxPoolControl`]); the state machine transitions in
/// [`crate::state`] are the only sanctioned path between states.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolSlot {
    /// Slot identity.
    pub sandbox_pool_slot_id: SandboxPoolSlotId,
    /// The pool capacity class.
    pub sandbox_pool_class: SandboxPoolClass,
    /// The resource profile the capacity is shaped for.
    pub sandbox_resource_profile_id: SandboxResourceProfileId,
    /// Opaque verified-node reference; never a host path or address.
    pub sandbox_node_reference: SandboxPoolOpaqueRef,
    /// Opaque provider identity.
    pub sandbox_provider_id: SandboxPoolOpaqueRef,
    /// The provider kind.
    pub sandbox_provider_kind: SandboxPoolProviderKind,
    /// The isolation assurance the slot was prepared for.
    pub sandbox_isolation_assurance: SandboxIsolationAssurance,
    /// The immutable artifact manifest revision this slot was materialized
    /// from (`REQ-2026-0012` exact-tuple discipline).
    pub sandbox_artifact_manifest_revision: SandboxPoolOpaqueRef,
    /// The capacity reservation revision the slot was admitted under.
    pub sandbox_capacity_revision: i64,
    /// Current lifecycle state.
    pub sandbox_slot_state: SandboxPoolSlotState,
    /// Highest fencing token persisted for the slot.
    pub sandbox_fencing_token: SandboxPoolFencingToken,
    /// Compare-and-swap version; every transition bumps it by one.
    pub sandbox_version: i64,
    /// Fingerprint of the tenant-neutral preparation (or cleanup) evidence
    /// that put the slot into `ready`; `None` outside `ready`.
    pub sandbox_preparation_evidence_fingerprint: Option<String>,
    /// Creation time, in whole seconds from the injected clock.
    pub sandbox_created_at: u64,
    /// Last transition time, in whole seconds from the injected clock.
    pub sandbox_updated_at: u64,
}

impl SandboxPoolSlot {
    /// Builds a fresh slot in `preparing` with no tenant state and no
    /// evidence fingerprint yet.
    ///
    /// `sandbox_now` is whole seconds from the injected clock; the cloud
    /// authority must supply the database clock
    /// (`persistenceConcurrencyAndRecovery.sandbox_databaseClockRequired`).
    #[allow(clippy::too_many_arguments)]
    pub fn sandbox_new(
        sandbox_pool_slot_id: SandboxPoolSlotId,
        sandbox_pool_class: SandboxPoolClass,
        sandbox_resource_profile_id: SandboxResourceProfileId,
        sandbox_node_reference: SandboxPoolOpaqueRef,
        sandbox_provider_id: SandboxPoolOpaqueRef,
        sandbox_provider_kind: SandboxPoolProviderKind,
        sandbox_isolation_assurance: SandboxIsolationAssurance,
        sandbox_artifact_manifest_revision: SandboxPoolOpaqueRef,
        sandbox_capacity_revision: i64,
        sandbox_now: u64,
    ) -> Result<Self, SandboxRuntimePoolError> {
        if sandbox_capacity_revision < 0 {
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        Ok(Self {
            sandbox_pool_slot_id,
            sandbox_pool_class,
            sandbox_resource_profile_id,
            sandbox_node_reference,
            sandbox_provider_id,
            sandbox_provider_kind,
            sandbox_isolation_assurance,
            sandbox_artifact_manifest_revision,
            sandbox_capacity_revision,
            sandbox_slot_state: SandboxPoolSlotState::Preparing,
            sandbox_fencing_token: SandboxPoolFencingToken::sandbox_initial(),
            sandbox_version: 0,
            sandbox_preparation_evidence_fingerprint: None,
            sandbox_created_at: sandbox_now,
            sandbox_updated_at: sandbox_now,
        })
    }

    /// Whether the slot currently carries any tenant-derived state. By
    /// construction it never does: the record has no tenant fields, so a
    /// `ready` slot is tenant-neutral exactly when this returns false.
    #[must_use]
    pub const fn sandbox_is_tenant_neutral(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_and_assurance_names_round_trip_and_reject_unknowns() {
        for class in [
            SandboxPoolClass::PreparedSlot,
            SandboxPoolClass::WarmMicroVmSlot,
            SandboxPoolClass::ColdAllocation,
        ] {
            assert_eq!(SandboxPoolClass::parse(class.as_str()), Some(class));
        }
        assert!(SandboxPoolClass::parse("sandbox_golden_vm").is_none());
        for assurance in [
            SandboxIsolationAssurance::MicroVm,
            SandboxIsolationAssurance::Container,
            SandboxIsolationAssurance::Process,
        ] {
            assert_eq!(
                SandboxIsolationAssurance::parse(assurance.as_str()),
                Some(assurance),
            );
        }
        assert!(SandboxIsolationAssurance::parse("shared_kernel").is_none());
    }

    #[test]
    fn fresh_slots_start_tenant_neutral_in_preparing() {
        let slot = SandboxPoolSlot::sandbox_new(
            SandboxPoolSlotId::new("slot-1").expect("valid id"),
            SandboxPoolClass::PreparedSlot,
            SandboxResourceProfileId::new("profile-1").expect("valid profile"),
            SandboxPoolOpaqueRef::new("node-ref").expect("valid ref"),
            SandboxPoolOpaqueRef::new("provider-1").expect("valid ref"),
            SandboxPoolProviderKind::new("firecracker").expect("valid kind"),
            SandboxIsolationAssurance::MicroVm,
            SandboxPoolOpaqueRef::new("manifest-r1").expect("valid ref"),
            7,
            100,
        )
        .expect("valid slot");
        assert_eq!(slot.sandbox_slot_state, SandboxPoolSlotState::Preparing);
        assert!(slot.sandbox_is_tenant_neutral());
        assert_eq!(slot.sandbox_preparation_evidence_fingerprint, None);
        assert_eq!(slot.sandbox_fencing_token.sandbox_as_i64(), 0);
        assert_eq!(slot.sandbox_version, 0);
    }

    #[test]
    fn negative_capacity_revisions_are_rejected() {
        assert!(SandboxPoolSlot::sandbox_new(
            SandboxPoolSlotId::new("slot-1").expect("valid id"),
            SandboxPoolClass::PreparedSlot,
            SandboxResourceProfileId::new("profile-1").expect("valid profile"),
            SandboxPoolOpaqueRef::new("node-ref").expect("valid ref"),
            SandboxPoolOpaqueRef::new("provider-1").expect("valid ref"),
            SandboxPoolProviderKind::new("firecracker").expect("valid kind"),
            SandboxIsolationAssurance::MicroVm,
            SandboxPoolOpaqueRef::new("manifest-r1").expect("valid ref"),
            -1,
            100,
        )
        .is_err());
    }
}
