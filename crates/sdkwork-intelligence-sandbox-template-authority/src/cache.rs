//! The [`SandboxTemplateCachePolicy`] authority.
//!
//! The cache semantics are a fixed authority
//! (`specs/sandbox-template-authority.contract.json`: `cachePolicy`): exactly
//! the Hot/Warm/Cold layer vocabulary, an explicit eviction policy, and
//! exact-digest matching for cross-template layer reuse. There is no storage
//! backend, eviction engine or cross-node coordination in this requirement —
//! those belong to later slices and stay unauthorized here.

use std::fmt;

use crate::bounds::MAX_SANDBOX_TEMPLATE_EVICTION_POLICY_LENGTH;
use crate::error::{SandboxTemplateAuthorityError, SandboxTemplateAuthorityResult};

/// The closed cache-layer vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxTemplateCacheLayer {
    /// In-process hot layers.
    Hot,
    /// Node-local warm layers.
    Warm,
    /// Distributed cold layers.
    Cold,
}

impl SandboxTemplateCacheLayer {
    /// The contract layer key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hot => "sandbox_hot",
            Self::Warm => "sandbox_warm",
            Self::Cold => "sandbox_cold",
        }
    }

    /// Parses a contract layer key; unknown keys are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "sandbox_hot" => Self::Hot,
            "sandbox_warm" => Self::Warm,
            "sandbox_cold" => Self::Cold,
            _ => return None,
        })
    }
}

impl fmt::Display for SandboxTemplateCacheLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The fixed cache-semantics authority: three layers, an explicit eviction
/// policy, and exact-digest reuse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxTemplateCachePolicy {
    sandbox_eviction_policy: String,
}

impl SandboxTemplateCachePolicy {
    /// The contract layer vocabulary, in order.
    #[must_use]
    pub fn sandbox_layers() -> [SandboxTemplateCacheLayer; 3] {
        [
            SandboxTemplateCacheLayer::Hot,
            SandboxTemplateCacheLayer::Warm,
            SandboxTemplateCacheLayer::Cold,
        ]
    }

    /// Declares a cache policy. The eviction policy must be explicit — a
    /// non-empty, bounded description of when entries leave the cache; an
    /// implicit or absent policy is refused.
    pub fn sandbox_new(
        sandbox_eviction_policy: impl Into<String>,
    ) -> SandboxTemplateAuthorityResult<Self> {
        let sandbox_eviction_policy = sandbox_eviction_policy.into();
        let valid = !sandbox_eviction_policy.is_empty()
            && sandbox_eviction_policy.len() <= MAX_SANDBOX_TEMPLATE_EVICTION_POLICY_LENGTH;
        if !valid {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidCachePolicy);
        }
        Ok(Self {
            sandbox_eviction_policy,
        })
    }

    /// The explicit eviction-policy description.
    #[must_use]
    pub fn sandbox_eviction_policy(&self) -> &str {
        &self.sandbox_eviction_policy
    }

    /// Cross-template layer reuse requires an exact digest match; there is no
    /// weaker reuse rule.
    #[must_use]
    pub const fn sandbox_cross_template_reuse_requires_exact_digest(&self) -> bool {
        true
    }

    /// Cache implementation stays unauthorized in this requirement slice.
    #[must_use]
    pub const fn sandbox_cache_implementation_authorized() -> bool {
        false
    }

    /// No cache storage backend is in scope in this requirement slice.
    #[must_use]
    pub const fn sandbox_cache_storage_backend_in_scope() -> bool {
        false
    }
}
