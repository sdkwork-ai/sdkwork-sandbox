//! Typed failures for the Sandbox runtime pool control plane.
//!
//! The wire-visible error codes are exactly the eight codes declared in
//! `specs/sandbox-runtime-pool.contract.json` (`errors.sandbox_errorCodes`).
//! Display strings stay generic on purpose: host, node, capacity and tenant
//! detail must never surface through an error message
//! (`errors.sandbox_hostNodeCapacityOrTenantDetailAllowed` is false).

use std::fmt;
use std::time::Duration;

use crate::bounds::SANDBOX_POOL_RETRY_AFTER_SECONDS_MAX;

/// Bounded retry guidance attached to a retryable pool failure.
///
/// Construction fails closed: a guidance above
/// [`SANDBOX_POOL_RETRY_AFTER_SECONDS_MAX`] cannot exist.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SandboxPoolRetryAfter(u64);

impl SandboxPoolRetryAfter {
    /// Builds retry guidance, rejecting values beyond the contract bound.
    #[must_use]
    pub fn new(seconds: u64) -> Option<Self> {
        if seconds <= SANDBOX_POOL_RETRY_AFTER_SECONDS_MAX {
            Some(Self(seconds))
        } else {
            None
        }
    }

    /// The bounded guidance in whole seconds.
    #[must_use]
    pub fn as_seconds(self) -> u64 {
        self.0
    }

    /// The bounded guidance as a [`Duration`].
    #[must_use]
    pub fn as_duration(self) -> Duration {
        Duration::from_secs(self.0)
    }
}

impl fmt::Display for SandboxPoolRetryAfter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}s", self.0)
    }
}

/// A pool dependency the claim ordering requires but the request did not
/// confirm (`allocationOrdering` steps one to three).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxPoolDependency {
    /// No confirmed admission reservation accompanied the claim.
    AdmissionReservation,
    /// No verified node inventory evidence backs the slot's node reference.
    VerifiedNodeInventory,
    /// No confirmed capacity reservation accompanied the claim.
    CapacityReservation,
    /// No preparation evidence fingerprint was recorded for the slot.
    PreparationEvidence,
    /// A warm slot was prepared without the separate KVM evidence reference.
    WarmKvmEvidence,
}

impl fmt::Display for SandboxPoolDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::AdmissionReservation => "admission reservation",
            Self::VerifiedNodeInventory => "verified node inventory",
            Self::CapacityReservation => "capacity reservation",
            Self::PreparationEvidence => "preparation evidence",
            Self::WarmKvmEvidence => "warm KVM evidence",
        };
        f.write_str(name)
    }
}

/// Why a claim request conflicts with the recorded pool state.
///
/// Every variant names a shape of contention, never a tenant, node or host
/// detail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxPoolClaimConflictKind {
    /// The operation was replayed with a different immutable fingerprint.
    FingerprintMismatch,
    /// The slot already carries one active claim
    /// (`claim.singleActiveClaimPerSlot`).
    ClaimAlreadyActive,
    /// The runtime binding already carries one active claim
    /// (`claim.singleActiveClaimPerRuntimeBinding`).
    RuntimeBindingAlreadyClaimed,
    /// The claim's capacity revision does not match the slot's.
    CapacityRevisionMismatch,
    /// The operation exhausted its bounded failed-attempt budget
    /// (`bounds.sandbox_claimAttemptCountMax`).
    ClaimAttemptBudgetExhausted,
}

impl fmt::Display for SandboxPoolClaimConflictKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::FingerprintMismatch => "request fingerprint mismatch",
            Self::ClaimAlreadyActive => "slot already carries an active claim",
            Self::RuntimeBindingAlreadyClaimed => "runtime binding already carries an active claim",
            Self::CapacityRevisionMismatch => "capacity revision mismatch",
            Self::ClaimAttemptBudgetExhausted => "claim attempt budget exhausted",
        };
        f.write_str(name)
    }
}

/// Why a slot moved to `quarantined`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxPoolQuarantineReason {
    /// Cleanup reported an uncertain outcome
    /// (`releaseAndSanitization.sandbox_uncertain_cleanup_quarantines_slot`).
    CleanupUncertain,
    /// Reconciliation found a claim past its time-to-live; the slot is
    /// quarantined instead of guessed back to `ready`
    /// (`releaseAndSanitization.sandbox_ttl_aloneMayReturnSlotToReady`).
    ClaimExpired,
    /// Reconciliation found a slot stuck in a transitional state past the
    /// cleanup deadline.
    TransitionalStuck,
    /// An explicit operator or controller decision.
    OperatorDecision,
}

impl fmt::Display for SandboxPoolQuarantineReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::CleanupUncertain => "cleanup uncertain",
            Self::ClaimExpired => "claim expired",
            Self::TransitionalStuck => "transitional state stuck",
            Self::OperatorDecision => "operator decision",
        };
        f.write_str(name)
    }
}

/// Typed failures of the Sandbox runtime pool control plane.
///
/// The variant set maps one-to-one onto the contract's eight error codes; see
/// [`SandboxRuntimePoolError::sandbox_error_code`].
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SandboxRuntimePoolError {
    /// The slot or claim is not in the state an operation requires.
    #[error("sandbox pool resource is not in the required state")]
    SandboxPoolNotReady {
        /// The state the operation requires, as a contract state name.
        sandbox_expected_state: String,
        /// The observed state, when a resource exists to observe.
        sandbox_actual_state: Option<String>,
    },
    /// The bounded registry or a profile is saturated; capacity is not
    /// overcommitted (`persistenceConcurrencyAndRecovery`
    /// `.sandbox_capacityOvercommitAllowed` is false).
    #[error("sandbox pool capacity is exhausted")]
    SandboxPoolExhausted {
        /// Bounded retry guidance.
        sandbox_retry_after: SandboxPoolRetryAfter,
    },
    /// The claim request conflicts with recorded pool state.
    #[error("sandbox pool claim conflict: {sandbox_conflict_kind}")]
    SandboxPoolClaimConflict {
        /// The contention shape.
        sandbox_conflict_kind: SandboxPoolClaimConflictKind,
    },
    /// A fencing token older than the persisted highest token was presented
    /// before any side effect (`claim.staleFencingRejectedBeforeSideEffect`).
    #[error("sandbox pool fencing token is stale")]
    SandboxPoolStaleFencing,
    /// A fixed-order dependency was not confirmed.
    #[error("sandbox pool dependency unavailable: {sandbox_dependency}")]
    SandboxPoolDependencyUnavailable {
        /// The missing dependency.
        sandbox_dependency: SandboxPoolDependency,
    },
    /// Cleanup reported failure; the slot must quarantine
    /// (`releaseAndSanitization.sandbox_cleanup_failure_visible`).
    #[error("sandbox pool cleanup failed")]
    SandboxPoolCleanupFailed,
    /// The slot is quarantined and cannot serve the requested operation
    /// (`releaseAndSanitization.sandbox_quarantineMayBeBypassedForAvailability`
    /// is false).
    #[error("sandbox pool slot is quarantined")]
    SandboxPoolSlotQuarantined,
    /// A request or an internal invariant failed validation; nothing was
    /// mutated.
    #[error("sandbox pool internal failure")]
    SandboxPoolInternal,
}

/// The result type every pool control-plane entrypoint returns.
pub type SandboxRuntimePoolResult<T> = Result<T, SandboxRuntimePoolError>;

impl SandboxRuntimePoolError {
    /// The exact contract error code for this failure
    /// (`errors.sandbox_errorCodes`).
    #[must_use]
    pub fn sandbox_error_code(&self) -> &'static str {
        match self {
            Self::SandboxPoolNotReady { .. } => "sandbox_pool_not_ready",
            Self::SandboxPoolExhausted { .. } => "sandbox_pool_exhausted",
            Self::SandboxPoolClaimConflict { .. } => "sandbox_pool_claim_conflict",
            Self::SandboxPoolStaleFencing => "sandbox_pool_stale_fencing",
            Self::SandboxPoolDependencyUnavailable { .. } => "sandbox_pool_dependency_unavailable",
            Self::SandboxPoolCleanupFailed { .. } => "sandbox_pool_cleanup_failed",
            Self::SandboxPoolSlotQuarantined => "sandbox_pool_slot_quarantined",
            Self::SandboxPoolInternal => "sandbox_pool_internal_failure",
        }
    }

    /// Whether retrying the same request can succeed. The answer is explicit
    /// for every variant (`errors.sandbox_retryabilityExplicit`).
    #[must_use]
    pub fn sandbox_retryable(&self) -> bool {
        matches!(
            self,
            Self::SandboxPoolExhausted { .. } | Self::SandboxPoolDependencyUnavailable { .. }
        )
    }

    /// Bounded retry guidance, present only when the control plane can state
    /// one (`errors.sandbox_retryAfterBounded`).
    #[must_use]
    pub fn sandbox_retry_after(&self) -> Option<SandboxPoolRetryAfter> {
        match self {
            Self::SandboxPoolExhausted {
                sandbox_retry_after,
            } => Some(*sandbox_retry_after),
            _ => None,
        }
    }
}
