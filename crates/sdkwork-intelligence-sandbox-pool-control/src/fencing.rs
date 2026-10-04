//! Monotonic fencing tokens for the Sandbox runtime pool.
//!
//! Pool fencing is its own domain: a pool token must never be derived from or
//! compared against a Kernel execution-placement lease or token
//! (`kernelBoundary`
//! `.sandbox_kernelExecutionPlacementLeaseOrFenceMayBeReusedAsPoolClaimLeaseOrFence`
//! is false). The registry persists only the highest issued token
//! (`claim.highestFencingTokenPersisted`); an operation presenting an older
//! token fails before any state mutation.

use std::fmt;

/// The token the first slot in a registry starts from. Zero is never handed
/// out, so a caller presenting zero is always stale.
pub const SANDBOX_POOL_FENCING_TOKEN_INITIAL: i64 = 0;

/// A monotonic pool fencing token.
///
/// Ordering is the whole contract: a token strictly below the registry's
/// persisted highest token is stale.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SandboxPoolFencingToken(i64);

impl SandboxPoolFencingToken {
    /// The seed token a fresh slot or registry starts from.
    #[must_use]
    pub const fn sandbox_initial() -> Self {
        Self(SANDBOX_POOL_FENCING_TOKEN_INITIAL)
    }

    /// The next monotonic token, or `None` at the `i64` ceiling; the caller
    /// must fail closed instead of wrapping.
    #[must_use]
    pub const fn sandbox_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(next) => Some(Self(next)),
            None => None,
        }
    }

    /// Whether this token is older than `highest` and must therefore be
    /// rejected before any side effect.
    #[must_use]
    pub const fn sandbox_is_stale_compared_to(self, highest: Self) -> bool {
        self.0 < highest.0
    }

    /// The wire value (`int64`-shaped).
    #[must_use]
    pub const fn sandbox_as_i64(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for SandboxPoolFencingToken {
    type Error = ();

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        if value < SANDBOX_POOL_FENCING_TOKEN_INITIAL {
            Err(())
        } else {
            Ok(Self(value))
        }
    }
}

impl std::str::FromStr for SandboxPoolFencingToken {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value
            .parse::<i64>()
            .ok()
            .and_then(|parsed| Self::try_from(parsed).ok())
            .ok_or(())
    }
}

impl Default for SandboxPoolFencingToken {
    fn default() -> Self {
        Self::sandbox_initial()
    }
}

impl fmt::Display for SandboxPoolFencingToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sandbox_pool_fencing_token({})", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_monotonic_and_never_wrap() {
        let first = SandboxPoolFencingToken::sandbox_initial()
            .sandbox_next()
            .expect("initial token has a successor");
        assert_eq!(first.sandbox_as_i64(), 1);
        assert!(first.sandbox_is_stale_compared_to(first.sandbox_next().expect("successor")));
        assert_eq!(
            SandboxPoolFencingToken::try_from(i64::MAX)
                .expect("max is a valid token")
                .sandbox_next(),
            None,
            "the ceiling fails closed instead of wrapping",
        );
    }

    #[test]
    fn negative_tokens_are_rejected() {
        assert!(SandboxPoolFencingToken::try_from(-1).is_err());
        assert!(
            SandboxPoolFencingToken::try_from(crate::bounds::SANDBOX_POOL_FENCING_TOKEN_MAX)
                .is_ok(),
        );
    }

    #[test]
    fn parse_rejects_non_numeric_text() {
        assert!("zero".parse::<SandboxPoolFencingToken>().is_err());
        assert_eq!(
            "3".parse::<SandboxPoolFencingToken>()
                .map(|token| token.sandbox_as_i64()),
            Ok(3),
        );
    }
}
