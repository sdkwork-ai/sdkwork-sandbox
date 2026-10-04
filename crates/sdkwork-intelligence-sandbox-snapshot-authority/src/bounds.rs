//! Contract bounds for the Sandbox Snapshot/Fork authority
//! (`specs/sandbox-snapshot-fork.contract.json`).

/// Maximum length of any snapshot identifier or opaque reference.
pub const MAX_SANDBOX_SNAPSHOT_ID_LENGTH: usize = 128;

/// Maximum number of sandbox families one derivation plan may target; the
/// per-snapshot derivation count is authorized, never unbounded
/// (`fork.derivationsPerSnapshotUnboundedAuthorized` is false).
pub const MAX_SANDBOX_SNAPSHOT_DERIVATIONS_PER_PLAN: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_positive_and_finite() {
        const {
            assert!(MAX_SANDBOX_SNAPSHOT_ID_LENGTH == 128);
            assert!(MAX_SANDBOX_SNAPSHOT_DERIVATIONS_PER_PLAN == 64);
        }
    }
}
