//! Contract bounds for the Sandbox Template Build authority
//! (`specs/sandbox-template-build.contract.json`).

/// Maximum length of any build identifier or opaque reference.
pub const MAX_SANDBOX_TEMPLATE_BUILD_ID_LENGTH: usize = 128;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_positive_and_finite() {
        const {
            assert!(MAX_SANDBOX_TEMPLATE_BUILD_ID_LENGTH == 128);
        }
    }
}
