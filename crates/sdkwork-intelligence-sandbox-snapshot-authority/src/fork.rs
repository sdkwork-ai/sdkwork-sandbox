//! Fork derivation semantics (`contract`: `fork`).
//!
//! The consistency boundary is fixed: the source snapshot is immutable, one
//! snapshot may back N derived sandboxes running in parallel with it, every
//! derived sandbox receives fresh guest identity and independent tenant
//! grants, and no derivation path may reuse source tenant state.

use crate::bounds::MAX_SANDBOX_SNAPSHOT_DERIVATIONS_PER_PLAN;
use crate::error::{SandboxSnapshotAuthorityError, SandboxSnapshotAuthorityResult};
use crate::snapshot::SandboxSnapshot;

/// Whether the source snapshot may ever mutate because of a derivation. It
/// may not: immutability is the whole derivation contract.
pub const SANDBOX_SNAPSHOT_FORK_SOURCE_SNAPSHOT_IMMUTABLE: bool = true;

/// Whether derived sandboxes run in parallel with the source.
pub const SANDBOX_SNAPSHOT_FORK_PARALLEL_RUNNING: bool = true;

/// Whether source tenant state may be reused across a derivation. It may not.
pub const SANDBOX_SNAPSHOT_FORK_SOURCE_TENANT_STATE_REUSE_ALLOWED: bool = false;

/// Whether derived sandboxes require fresh guest identity. They do.
pub const SANDBOX_SNAPSHOT_FORK_FRESH_IDENTITY_REQUIRED: bool = true;

/// A validated fork derivation plan: N derived sandboxes from one available
/// snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxForkDerivation {
    sandbox_snapshot_id: String,
    sandbox_derivation_count: usize,
}

impl SandboxForkDerivation {
    /// Validates a fork plan against its source snapshot: the snapshot must
    /// be derivable, the derivation count bounded and at least one.
    pub fn sandbox_plan(
        snapshot: &SandboxSnapshot,
        sandbox_derivation_count: usize,
    ) -> SandboxSnapshotAuthorityResult<Self> {
        if !snapshot.sandbox_is_derivable() {
            return Err(SandboxSnapshotAuthorityError::SandboxSnapshotInvalidForkDerivation);
        }
        if sandbox_derivation_count == 0
            || sandbox_derivation_count > MAX_SANDBOX_SNAPSHOT_DERIVATIONS_PER_PLAN
        {
            return Err(SandboxSnapshotAuthorityError::SandboxSnapshotInvalidForkDerivation);
        }
        Ok(Self {
            sandbox_snapshot_id: snapshot.sandbox_snapshot_id().to_owned(),
            sandbox_derivation_count,
        })
    }

    /// The snapshot identity the plan derives from.
    #[must_use]
    pub fn sandbox_snapshot_id(&self) -> &str {
        &self.sandbox_snapshot_id
    }

    /// The bounded derivation count.
    #[must_use]
    pub const fn sandbox_derivation_count(&self) -> usize {
        self.sandbox_derivation_count
    }
}
