//! Contract bounds for the Sandbox Template authority.
//!
//! These are authority-model ceilings for the in-memory record shapes; wire
//! and storage bounds are finalized by their own later slices. Every constant
//! is fail-closed: a record beyond a bound is rejected, never clamped.

/// Maximum length of any template identifier, name or opaque reference.
pub const MAX_SANDBOX_TEMPLATE_ID_LENGTH: usize = 128;

/// Maximum length of a template definition version string.
pub const MAX_SANDBOX_TEMPLATE_VERSION_LENGTH: usize = 64;

/// Maximum number of file layers in one definition.
pub const MAX_SANDBOX_TEMPLATE_FILE_LAYERS: usize = 64;

/// Maximum number of environment variables in one definition.
pub const MAX_SANDBOX_TEMPLATE_ENV_COUNT: usize = 128;

/// Maximum length of an environment variable key.
pub const MAX_SANDBOX_TEMPLATE_ENV_KEY_LENGTH: usize = 128;

/// Maximum length of an environment variable value.
pub const MAX_SANDBOX_TEMPLATE_ENV_VALUE_LENGTH: usize = 4096;

/// Maximum length of the start command.
pub const MAX_SANDBOX_TEMPLATE_START_COMMAND_LENGTH: usize = 8192;

/// Maximum number of tags or aliases on one version.
pub const MAX_SANDBOX_TEMPLATE_NAME_COUNT: usize = 32;

/// Maximum length of a tag or alias.
pub const MAX_SANDBOX_TEMPLATE_NAME_LENGTH: usize = 64;

/// Maximum length of the explicit eviction-policy description.
pub const MAX_SANDBOX_TEMPLATE_EVICTION_POLICY_LENGTH: usize = 256;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_positive_and_finite() {
        const {
            assert!(MAX_SANDBOX_TEMPLATE_ID_LENGTH > 0);
            assert!(MAX_SANDBOX_TEMPLATE_VERSION_LENGTH > 0);
            assert!(MAX_SANDBOX_TEMPLATE_FILE_LAYERS > 0);
            assert!(MAX_SANDBOX_TEMPLATE_ENV_COUNT > 0);
            assert!(MAX_SANDBOX_TEMPLATE_ENV_KEY_LENGTH > 0);
            assert!(MAX_SANDBOX_TEMPLATE_ENV_VALUE_LENGTH > 0);
            assert!(MAX_SANDBOX_TEMPLATE_START_COMMAND_LENGTH > 0);
            assert!(MAX_SANDBOX_TEMPLATE_NAME_COUNT > 0);
            assert!(MAX_SANDBOX_TEMPLATE_NAME_LENGTH > 0);
            assert!(MAX_SANDBOX_TEMPLATE_EVICTION_POLICY_LENGTH > 0);
        }
    }
}
