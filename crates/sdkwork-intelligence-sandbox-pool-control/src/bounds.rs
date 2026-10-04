//! Contract bounds for the Sandbox runtime pool control plane.
//!
//! Every constant restates one bound from
//! `specs/sandbox-runtime-pool.contract.json` (`bounds`). The values are
//! fail-closed ceilings: a request beyond a bound is rejected, never clamped.

/// Maximum claim time-to-live in seconds (`bounds.sandbox_claim_ttlSecondsMax`).
pub const SANDBOX_POOL_CLAIM_TTL_SECONDS_MAX: u64 = 60;

/// Maximum slots examined by one reconciliation pass
/// (`bounds.sandbox_reconciliationBatchSizeMax`).
pub const SANDBOX_POOL_RECONCILIATION_BATCH_SIZE_MAX: usize = 100;

/// Maximum candidate slots one bounded registry may carry
/// (`bounds.sandbox_candidateSlotCountMax`).
pub const SANDBOX_POOL_CANDIDATE_SLOT_COUNT_MAX: usize = 128;

/// Maximum failed claim attempts per operation before the operation is refused
/// (`bounds.sandbox_claimAttemptCountMax`).
pub const SANDBOX_POOL_CLAIM_ATTEMPT_COUNT_MAX: u32 = 8;

/// Maximum retry guidance the control plane may return
/// (`bounds.sandbox_retryAfterSecondsMax`).
pub const SANDBOX_POOL_RETRY_AFTER_SECONDS_MAX: u64 = 300;

/// Maximum seconds a slot may stay in a transitional state before
/// reconciliation quarantines it (`bounds.sandbox_cleanupDeadlineSecondsMax`).
pub const SANDBOX_POOL_CLEANUP_DEADLINE_SECONDS_MAX: u64 = 120;

/// Maximum refill actions one node may receive per target reconciliation
/// (`bounds.sandbox_refillOperationsPerNodeMax`).
pub const SANDBOX_POOL_REFILL_OPERATIONS_PER_NODE_MAX: usize = 16;

/// Maximum declared ready-target per resource profile
/// (`bounds.sandbox_perProfileTargetMax`).
pub const SANDBOX_POOL_PER_PROFILE_TARGET_MAX: u32 = 1000;

/// Retry guidance returned when the bounded registry is saturated. The value
/// is a policy choice inside [`SANDBOX_POOL_RETRY_AFTER_SECONDS_MAX`].
pub const SANDBOX_POOL_EXHAUSTED_RETRY_AFTER_SECONDS: u64 = 30;

/// The fencing-token ceiling; advancing past it fails closed.
pub const SANDBOX_POOL_FENCING_TOKEN_MAX: i64 = i64::MAX;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_restate_the_contract_ceilings() {
        assert_eq!(SANDBOX_POOL_CLAIM_TTL_SECONDS_MAX, 60);
        assert_eq!(SANDBOX_POOL_RECONCILIATION_BATCH_SIZE_MAX, 100);
        assert_eq!(SANDBOX_POOL_CANDIDATE_SLOT_COUNT_MAX, 128);
        assert_eq!(SANDBOX_POOL_CLAIM_ATTEMPT_COUNT_MAX, 8);
        assert_eq!(SANDBOX_POOL_RETRY_AFTER_SECONDS_MAX, 300);
        assert_eq!(SANDBOX_POOL_CLEANUP_DEADLINE_SECONDS_MAX, 120);
        assert_eq!(SANDBOX_POOL_REFILL_OPERATIONS_PER_NODE_MAX, 16);
        assert_eq!(SANDBOX_POOL_PER_PROFILE_TARGET_MAX, 1000);
        const {
            assert!(
                SANDBOX_POOL_EXHAUSTED_RETRY_AFTER_SECONDS <= SANDBOX_POOL_RETRY_AFTER_SECONDS_MAX
            );
        }
    }
}
