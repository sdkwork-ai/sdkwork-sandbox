//! Contract bounds for the Workspace runtime transaction control plane
//! (`specs/sandbox-workspace-runtime-transaction.contract.json`: `bounds`).

/// Maximum request size in bytes (`bounds.sandbox_request_max_bytes`).
pub const SANDBOX_REQUEST_MAX_BYTES: usize = 65536;

/// Maximum orchestration stages (`bounds.sandbox_orchestration_stage_count_max`).
pub const SANDBOX_ORCHESTRATION_STAGE_COUNT_MAX: usize = 32;

/// Maximum concurrent commands per transaction
/// (`bounds.sandbox_concurrent_commands_per_transaction_max`).
pub const SANDBOX_CONCURRENT_COMMANDS_PER_TRANSACTION_MAX: usize = 16;

/// Maximum queue wait in milliseconds (`bounds.sandbox_queue_wait_ms_max`).
pub const SANDBOX_QUEUE_WAIT_MS_MAX: u64 = 300000;

/// Maximum retry guidance in milliseconds (`bounds.sandbox_retry_after_ms_max`).
pub const SANDBOX_RETRY_AFTER_MS_MAX: u64 = 300000;

/// Maximum reconciliation batch size
/// (`bounds.sandbox_reconciliation_batch_size_max`).
pub const SANDBOX_RECONCILIATION_BATCH_SIZE_MAX: usize = 100;

/// Maximum compensation attempts (`bounds.sandbox_compensation_attempt_count_max`).
pub const SANDBOX_COMPENSATION_ATTEMPT_COUNT_MAX: u32 = 16;

/// Maximum length of any opaque reference (`bounds.sandbox_reference_max_length`).
pub const SANDBOX_REFERENCE_MAX_LENGTH: usize = 512;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_restate_the_contract_ceilings() {
        const {
            assert!(SANDBOX_REQUEST_MAX_BYTES == 65536);
            assert!(SANDBOX_ORCHESTRATION_STAGE_COUNT_MAX == 32);
            assert!(SANDBOX_CONCURRENT_COMMANDS_PER_TRANSACTION_MAX == 16);
            assert!(SANDBOX_QUEUE_WAIT_MS_MAX == 300000);
            assert!(SANDBOX_RETRY_AFTER_MS_MAX == 300_000);
            assert!(SANDBOX_RECONCILIATION_BATCH_SIZE_MAX == 100);
            assert!(SANDBOX_COMPENSATION_ATTEMPT_COUNT_MAX == 16);
            assert!(SANDBOX_REFERENCE_MAX_LENGTH == 512);
        }
    }
}
