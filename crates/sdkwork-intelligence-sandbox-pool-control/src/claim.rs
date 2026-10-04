//! The [`SandboxPoolClaim`] record and its request shape.
//!
//! A claim is the single fenced bridge between tenant identity and a pooled
//! slot (`ADR-20260730` decision 3). Claim requests carry the confirmed
//! inputs of the fixed allocation order
//! (`specs/sandbox-runtime-pool.contract.json`: `allocationOrdering` steps one
//! to three); the control plane rejects any request that skips them.

use crate::bounds::SANDBOX_POOL_CLAIM_TTL_SECONDS_MAX;
use crate::error::SandboxRuntimePoolError;
use crate::fencing::SandboxPoolFencingToken;
use crate::identity::{
    SandboxPoolClaimId, SandboxPoolOpaqueRef, SandboxPoolOperationId, SandboxPoolSlotId,
    SandboxPoolTenantId, SandboxResourceProfileId,
};
use crate::state::SandboxPoolClaimState;

/// Length of a lowercase hex SHA-256 fingerprint.
pub const SANDBOX_POOL_FINGERPRINT_LENGTH: usize = 64;

/// Validates the immutable request fingerprint shape (lowercase hex SHA-256).
pub(crate) fn sandbox_validate_fingerprint(value: &str) -> Result<(), SandboxRuntimePoolError> {
    let valid = value.len() == SANDBOX_POOL_FINGERPRINT_LENGTH
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if valid {
        Ok(())
    } else {
        Err(SandboxRuntimePoolError::SandboxPoolInternal)
    }
}

/// A durable claim binding one tenant runtime identity to one slot.
///
/// Every contract required field appears verbatim as a Rust field; the tenant
/// binding required by `REQ-2026-0019` acceptance criterion 2 travels as
/// [`Self::sandbox_tenant_id`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolClaim {
    /// Tenant the claim binds; capacity accounting stays tenant-fair through
    /// it (`persistenceConcurrencyAndRecovery`
    /// `.sandbox_reconciliationTenantAwareAndBounded`).
    pub sandbox_tenant_id: SandboxPoolTenantId,
    /// Claim identity; assigned by the control plane, opaque downstream.
    pub sandbox_pool_claim_id: SandboxPoolClaimId,
    /// The claimed slot.
    pub sandbox_pool_slot_id: SandboxPoolSlotId,
    /// The confirmed admission reservation grant.
    pub sandbox_admission_grant_id: SandboxPoolOpaqueRef,
    /// The confirmed capacity reservation.
    pub sandbox_capacity_reservation_id: SandboxPoolOpaqueRef,
    /// The capacity revision that was confirmed; equals the slot's.
    pub sandbox_capacity_revision: i64,
    /// The opaque sandbox session identity receiving the slot.
    pub sandbox_session_id: SandboxPoolOpaqueRef,
    /// The opaque runtime binding identity receiving the slot.
    pub sandbox_runtime_binding_id: SandboxPoolOpaqueRef,
    /// The caller operation identity; the idempotency key.
    pub sandbox_operation_id: SandboxPoolOperationId,
    /// The immutable request fingerprint.
    pub sandbox_request_fingerprint: String,
    /// The fencing token this claim was bound under.
    pub sandbox_fencing_token: SandboxPoolFencingToken,
    /// Current claim state.
    pub sandbox_claim_state: SandboxPoolClaimState,
    /// Compare-and-swap version; every transition bumps it by one.
    pub sandbox_version: i64,
    /// Bind time, in whole seconds from the injected clock.
    pub sandbox_claimed_at: u64,
    /// Expiry time; past it reconciliation quarantines instead of guessing.
    pub sandbox_expires_at: u64,
}

impl SandboxPoolClaim {
    /// Whether the claim is past its time-to-live at `sandbox_now`.
    #[must_use]
    pub const fn sandbox_is_expired_at(&self, sandbox_now: u64) -> bool {
        sandbox_now >= self.sandbox_expires_at
    }
}

/// A validated claim request: the tenant side of the fixed allocation order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxPoolClaimRequest {
    /// Tenant the resulting claim binds.
    pub sandbox_tenant_id: SandboxPoolTenantId,
    /// Slot to claim; never chosen by the Kernel
    /// (`claim.callerSelectedSlotNodeOrProviderAllowed` is false: callers
    /// select a slot only from the control plane's own ready inventory).
    pub sandbox_pool_slot_id: SandboxPoolSlotId,
    /// Opaque sandbox session identity.
    pub sandbox_session_id: SandboxPoolOpaqueRef,
    /// Opaque runtime binding identity.
    pub sandbox_runtime_binding_id: SandboxPoolOpaqueRef,
    /// Caller operation identity.
    pub sandbox_operation_id: SandboxPoolOperationId,
    /// Immutable request fingerprint (lowercase hex SHA-256 over the caller's
    /// canonical request).
    pub sandbox_request_fingerprint: String,
    /// Confirmed admission reservation grant
    /// (`allocationOrdering[0]`); `None` means the step never happened and
    /// the claim is refused.
    pub sandbox_admission_grant_id: Option<SandboxPoolOpaqueRef>,
    /// Confirmed capacity reservation (`allocationOrdering[2]`); `None`
    /// means the step never happened and the claim is refused.
    pub sandbox_capacity_reservation_id: Option<SandboxPoolOpaqueRef>,
    /// The confirmed capacity revision; must equal the slot's.
    pub sandbox_capacity_revision: i64,
    /// The slot fencing token the caller observed; a stale token is rejected
    /// before any mutation.
    pub sandbox_expected_fencing_token: SandboxPoolFencingToken,
    /// Claim time-to-live in seconds, at most
    /// [`SANDBOX_POOL_CLAIM_TTL_SECONDS_MAX`].
    pub sandbox_ttl_seconds: u64,
    /// The resource profile the slot was prepared for; a mismatch fails
    /// closed.
    pub sandbox_resource_profile_id: SandboxResourceProfileId,
}

impl SandboxPoolClaimRequest {
    /// Validates every field shape; nothing about the pool state is read.
    pub fn sandbox_validated(self) -> Result<Self, SandboxRuntimePoolError> {
        sandbox_validate_fingerprint(&self.sandbox_request_fingerprint)?;
        if self.sandbox_ttl_seconds == 0
            || self.sandbox_ttl_seconds > SANDBOX_POOL_CLAIM_TTL_SECONDS_MAX
        {
            return Err(SandboxRuntimePoolError::SandboxPoolInternal);
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_must_be_lowercase_hex_sha256() {
        assert!(sandbox_validate_fingerprint(&"a".repeat(64)).is_ok());
        assert!(sandbox_validate_fingerprint(&"A".repeat(64)).is_err());
        assert!(sandbox_validate_fingerprint(&"a".repeat(63)).is_err());
        assert!(sandbox_validate_fingerprint(&"g".repeat(64)).is_err());
        assert!(sandbox_validate_fingerprint("").is_err());
    }

    #[test]
    fn claim_requests_reject_missing_or_oversized_ttl() {
        let base = || SandboxPoolClaimRequest {
            sandbox_tenant_id: SandboxPoolTenantId::new("tenant").expect("valid tenant"),
            sandbox_pool_slot_id: SandboxPoolSlotId::new("slot").expect("valid slot"),
            sandbox_session_id: SandboxPoolOpaqueRef::new("session").expect("valid ref"),
            sandbox_runtime_binding_id: SandboxPoolOpaqueRef::new("binding").expect("valid ref"),
            sandbox_operation_id: SandboxPoolOperationId::new("operation").expect("valid op"),
            sandbox_request_fingerprint: "a".repeat(64),
            sandbox_admission_grant_id: Some(
                SandboxPoolOpaqueRef::new("grant").expect("valid ref"),
            ),
            sandbox_capacity_reservation_id: Some(
                SandboxPoolOpaqueRef::new("capacity").expect("valid ref"),
            ),
            sandbox_capacity_revision: 1,
            sandbox_expected_fencing_token: SandboxPoolFencingToken::sandbox_initial(),
            sandbox_ttl_seconds: 30,
            sandbox_resource_profile_id: SandboxResourceProfileId::new("profile")
                .expect("valid profile"),
        };
        assert!(base().sandbox_validated().is_ok());
        assert!(SandboxPoolClaimRequest {
            sandbox_ttl_seconds: SANDBOX_POOL_CLAIM_TTL_SECONDS_MAX + 1,
            ..base()
        }
        .sandbox_validated()
        .is_err());
        assert!(SandboxPoolClaimRequest {
            sandbox_ttl_seconds: 0,
            ..base()
        }
        .sandbox_validated()
        .is_err());
    }
}
