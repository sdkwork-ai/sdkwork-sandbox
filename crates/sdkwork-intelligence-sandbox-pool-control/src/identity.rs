//! Typed, validated identifiers and opaque references for the pool domain.
//!
//! Pool identity is its own domain: a slot id, a claim id and a runtime
//! binding reference are distinct records with distinct id, lease/fencing and
//! idempotency scopes (`placementAuthoritySeparation`
//! `.sandbox_kernel_and_sandbox_records_have_distinct_ids`). No identifier in
//! this module is derived from a filesystem path, and no field accepts a raw
//! host path, URL or socket address (`slot.rawHostPathOrAddressAllowed` is
//! false).

use crate::error::SandboxRuntimePoolError;

/// Maximum length of any pool identifier or opaque reference.
pub const MAX_SANDBOX_POOL_ID_LENGTH: usize = 128;

fn sandbox_validate_opaque(value: &str) -> Result<(), SandboxRuntimePoolError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_SANDBOX_POOL_ID_LENGTH
        && value
            .chars()
            .all(|c| c.is_ascii_graphic() && !c.is_whitespace())
        && !value.starts_with('/')
        && !value.contains("://");
    if valid {
        Ok(())
    } else {
        Err(SandboxRuntimePoolError::SandboxPoolInternal)
    }
}

macro_rules! sandbox_pool_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(String);

        impl $name {
            /// Builds the identifier, rejecting empty, oversized or
            /// locator-shaped values.
            pub fn new(value: impl Into<String>) -> Result<Self, SandboxRuntimePoolError> {
                let value = value.into();
                sandbox_validate_opaque(&value)?;
                Ok(Self(value))
            }

            /// The identifier value.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

use std::fmt;

sandbox_pool_id!(
    SandboxPoolSlotId,
    "Identity of one [`crate::slot::SandboxPoolSlot`]; distinct from every session, runtime binding and provider allocation identity."
);
sandbox_pool_id!(
    SandboxPoolClaimId,
    "Identity of one [`crate::claim::SandboxPoolClaim`]; assigned by the control plane, never inferred from a path."
);
sandbox_pool_id!(
    SandboxPoolTenantId,
    "Tenant identity a claim binds (`REQ-2026-0019` acceptance criterion 2)."
);
sandbox_pool_id!(
    SandboxResourceProfileId,
    "Resource profile a slot capacity target is declared for."
);
sandbox_pool_id!(
    SandboxPoolOperationId,
    "Caller operation identity; the claim idempotency key (`claim.sameOperationSameFingerprintReplays`)."
);
sandbox_pool_id!(
    SandboxPoolOpaqueRef,
    "An opaque reference (session, runtime binding, admission grant, capacity reservation, node, provider or artifact revision). Carries no locator semantics."
);
sandbox_pool_id!(
    SandboxPoolProviderKind,
    "The provider kind a slot was prepared for (for example the Firecracker provider)."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_reject_empty_oversized_and_locator_shapes() {
        assert!(SandboxPoolSlotId::new("slot-1").is_ok());
        assert!(SandboxPoolSlotId::new("").is_err());
        assert!(SandboxPoolSlotId::new("/var/lib/sandbox").is_err());
        assert!(SandboxPoolSlotId::new("socket://10.0.0.1").is_err());
        assert!(SandboxPoolSlotId::new("has space").is_err());
        assert!(SandboxPoolSlotId::new("x".repeat(MAX_SANDBOX_POOL_ID_LENGTH + 1)).is_err());
        assert!(SandboxPoolSlotId::new("x".repeat(MAX_SANDBOX_POOL_ID_LENGTH)).is_ok());
    }
}
