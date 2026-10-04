//! The [`SandboxSnapshot`] record (`contract`: `snapshot`).
//!
//! Every contract required field appears verbatim; references are opaque and
//! fields are private with read-only accessors, so
//! `immutableAfterCreation` is enforced by construction — a created snapshot
//! has no mutation API, and every lifecycle move is a state-machine
//! transition in [`crate::state`].

use crate::bounds::MAX_SANDBOX_SNAPSHOT_ID_LENGTH;
use crate::error::{SandboxSnapshotAuthorityError, SandboxSnapshotAuthorityResult};
use crate::state::SandboxSnapshotState;

fn sandbox_validated_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SANDBOX_SNAPSHOT_ID_LENGTH
        && value
            .chars()
            .all(|c| c.is_ascii_graphic() && !c.is_whitespace())
        && !value.starts_with('/')
        && !value.contains("://")
}

const SANDBOX_SNAPSHOT_FINGERPRINT_LENGTH: usize = 64;

/// One created, immutable snapshot of a sandbox runtime's full state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxSnapshot {
    sandbox_snapshot_id: String,
    sandbox_source_session_ref: String,
    sandbox_memory_included: bool,
    sandbox_filesystem_fingerprint: String,
    sandbox_artifact_tuple_ref: String,
    sandbox_snapshot_state: SandboxSnapshotState,
    sandbox_created_at: u64,
}

impl SandboxSnapshot {
    /// Creates one snapshot record in `creating`, validating every field
    /// shape fail-closed.
    #[allow(clippy::too_many_arguments)]
    pub fn sandbox_new(
        sandbox_snapshot_id: &str,
        sandbox_source_session_ref: &str,
        sandbox_memory_included: bool,
        sandbox_filesystem_fingerprint: &str,
        sandbox_artifact_tuple_ref: &str,
        sandbox_created_at: u64,
    ) -> SandboxSnapshotAuthorityResult<Self> {
        let references_valid = [
            sandbox_snapshot_id,
            sandbox_source_session_ref,
            sandbox_artifact_tuple_ref,
        ]
        .iter()
        .all(|reference| sandbox_validated_reference(reference));
        let fingerprint_valid = sandbox_filesystem_fingerprint.len()
            == SANDBOX_SNAPSHOT_FINGERPRINT_LENGTH
            && sandbox_filesystem_fingerprint
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if !references_valid || !fingerprint_valid {
            return Err(SandboxSnapshotAuthorityError::SandboxSnapshotInvalidSnapshot);
        }
        Ok(Self {
            sandbox_snapshot_id: sandbox_snapshot_id.to_owned(),
            sandbox_source_session_ref: sandbox_source_session_ref.to_owned(),
            sandbox_memory_included,
            sandbox_filesystem_fingerprint: sandbox_filesystem_fingerprint.to_owned(),
            sandbox_artifact_tuple_ref: sandbox_artifact_tuple_ref.to_owned(),
            sandbox_snapshot_state: SandboxSnapshotState::Creating,
            sandbox_created_at,
        })
    }

    /// Marks the materialization complete; the snapshot becomes immutable and
    /// derivable. Only `creating -> available` is legal.
    pub fn sandbox_mark_available(&mut self) -> SandboxSnapshotAuthorityResult<()> {
        if !self
            .sandbox_snapshot_state
            .sandbox_can_transition_to(SandboxSnapshotState::Available)
        {
            return Err(SandboxSnapshotAuthorityError::SandboxSnapshotIllegalTransition);
        }
        self.sandbox_snapshot_state = SandboxSnapshotState::Available;
        Ok(())
    }

    /// Marks a restore/derivation read; content never changes.
    pub fn sandbox_mark_restoring(&mut self) -> SandboxSnapshotAuthorityResult<()> {
        if !self
            .sandbox_snapshot_state
            .sandbox_can_transition_to(SandboxSnapshotState::Restoring)
        {
            return Err(SandboxSnapshotAuthorityError::SandboxSnapshotIllegalTransition);
        }
        self.sandbox_snapshot_state = SandboxSnapshotState::Restoring;
        Ok(())
    }

    /// Ends the restore/derivation read.
    pub fn sandbox_mark_restore_finished(&mut self) -> SandboxSnapshotAuthorityResult<()> {
        if !self
            .sandbox_snapshot_state
            .sandbox_can_transition_to(SandboxSnapshotState::Available)
        {
            return Err(SandboxSnapshotAuthorityError::SandboxSnapshotIllegalTransition);
        }
        self.sandbox_snapshot_state = SandboxSnapshotState::Available;
        Ok(())
    }

    /// Records a deterministic deletion; terminal.
    pub fn sandbox_mark_deleted(&mut self) -> SandboxSnapshotAuthorityResult<()> {
        if !self
            .sandbox_snapshot_state
            .sandbox_can_transition_to(SandboxSnapshotState::Deleted)
        {
            return Err(SandboxSnapshotAuthorityError::SandboxSnapshotIllegalTransition);
        }
        self.sandbox_snapshot_state = SandboxSnapshotState::Deleted;
        Ok(())
    }

    /// Records an uncertain deletion or verification; terminal, and the
    /// snapshot can no longer serve derivations.
    pub fn sandbox_mark_quarantined(&mut self) -> SandboxSnapshotAuthorityResult<()> {
        if !self
            .sandbox_snapshot_state
            .sandbox_can_transition_to(SandboxSnapshotState::Quarantined)
        {
            return Err(SandboxSnapshotAuthorityError::SandboxSnapshotIllegalTransition);
        }
        self.sandbox_snapshot_state = SandboxSnapshotState::Quarantined;
        Ok(())
    }

    /// The snapshot identity.
    #[must_use]
    pub fn sandbox_snapshot_id(&self) -> &str {
        &self.sandbox_snapshot_id
    }

    /// The opaque source-session reference.
    #[must_use]
    pub fn sandbox_source_session_ref(&self) -> &str {
        &self.sandbox_source_session_ref
    }

    /// Whether the snapshot includes guest memory.
    #[must_use]
    pub const fn sandbox_memory_included(&self) -> bool {
        self.sandbox_memory_included
    }

    /// The filesystem content fingerprint (lowercase hex SHA-256).
    #[must_use]
    pub fn sandbox_filesystem_fingerprint(&self) -> &str {
        &self.sandbox_filesystem_fingerprint
    }

    /// The exact `REQ-2026-0012` artifact-tuple reference.
    #[must_use]
    pub fn sandbox_artifact_tuple_ref(&self) -> &str {
        &self.sandbox_artifact_tuple_ref
    }

    /// The current lifecycle state.
    #[must_use]
    pub const fn sandbox_snapshot_state(&self) -> SandboxSnapshotState {
        self.sandbox_snapshot_state
    }

    /// The creation timestamp (whole seconds from the caller's clock).
    #[must_use]
    pub const fn sandbox_created_at(&self) -> u64 {
        self.sandbox_created_at
    }

    /// Whether the snapshot may serve derivations or restores: only
    /// `available` (and its transient `restoring`) qualify; `quarantined` and
    /// `deleted` never do.
    #[must_use]
    pub const fn sandbox_is_derivable(&self) -> bool {
        matches!(
            self.sandbox_snapshot_state,
            SandboxSnapshotState::Available | SandboxSnapshotState::Restoring
        )
    }
}
