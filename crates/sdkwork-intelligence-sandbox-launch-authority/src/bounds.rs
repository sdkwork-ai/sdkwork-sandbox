//! Contract bounds for the Sandbox Fast-Start Launch authority
//! (`specs/sandbox-instance-fast-start.contract.json`).

/// Maximum length of any launch plan identifier or opaque reference.
pub const MAX_SANDBOX_LAUNCH_PLAN_ID_LENGTH: usize = 128;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_positive_and_finite() {
        const {
            assert!(MAX_SANDBOX_LAUNCH_PLAN_ID_LENGTH == 128);
        }
    }
}
