//! Contract bounds for the Sandbox Worker launch-execution authority
//! (`specs/sandbox-worker.contract.json`).

/// Maximum length of any execution identifier or opaque reference.
pub const MAX_SANDBOX_WORKER_EXECUTION_ID_LENGTH: usize = 128;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_positive_and_finite() {
        const {
            assert!(MAX_SANDBOX_WORKER_EXECUTION_ID_LENGTH == 128);
        }
    }
}
