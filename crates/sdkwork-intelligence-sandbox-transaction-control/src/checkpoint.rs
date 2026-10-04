//! The [`SandboxWorkspaceCheckpointCandidate`] and its Agents-only
//! compare-and-swap promotion (`contract`: `checkpoint`).
//!
//! A candidate is sealed before handoff, its storage reference is opaque,
//! handoff durability precedes runtime release, and only Agents promotes a
//! candidate to a workspace revision. Promotion is a compare-and-swap on the
//! expected source revision: the same candidate with the same expected
//! revision replays idempotently, a different expected revision is a
//! non-destructive conflict that preserves the bounded candidate
//! (`sandbox_checkpoint_conflict_may_overwrite_newer_revision` is false).

use crate::error::{SandboxWorkspaceRuntimeError, SandboxWorkspaceRuntimeResult};

/// The promotion outcome of one candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxCandidatePromotion {
    /// The candidate advanced the workspace revision.
    Promoted,
    /// The candidate was already promoted with the same expected revision;
    /// the replay is idempotent.
    AlreadyPromoted,
}

/// One durable checkpoint candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxWorkspaceCheckpointCandidate {
    sandbox_workspace_checkpoint_candidate_id: String,
    sandbox_workspace_id: String,
    sandbox_source_workspace_revision_ref: String,
    sandbox_candidate_workspace_revision_ref: String,
    sandbox_workspace_runtime_transaction_id: String,
    sandbox_session_id: String,
    sandbox_runtime_binding_id: String,
    sandbox_fencing_token: i64,
    sandbox_content_digest: String,
    sandbox_content_size_bytes: u64,
    sandbox_storage_authority_ref: String,
    sandbox_created_at: u64,
    sandbox_trace_id: String,
    sandbox_sealed: bool,
    sandbox_promoted_expected_source_revision_ref: Option<String>,
}

const SANDBOX_CHECKPOINT_FINGERPRINT_LENGTH: usize = 64;

fn sandbox_validated_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= crate::bounds::SANDBOX_REFERENCE_MAX_LENGTH
        && value
            .chars()
            .all(|c| c.is_ascii_graphic() && !c.is_whitespace())
        && !value.starts_with('/')
        && !value.contains("://")
}

impl SandboxWorkspaceCheckpointCandidate {
    /// Builds an unsealed candidate, validating every field shape.
    #[allow(clippy::too_many_arguments)]
    pub fn sandbox_new(
        sandbox_workspace_checkpoint_candidate_id: &str,
        sandbox_workspace_id: &str,
        sandbox_source_workspace_revision_ref: &str,
        sandbox_candidate_workspace_revision_ref: &str,
        sandbox_workspace_runtime_transaction_id: &str,
        sandbox_session_id: &str,
        sandbox_runtime_binding_id: &str,
        sandbox_fencing_token: i64,
        sandbox_content_digest: &str,
        sandbox_content_size_bytes: u64,
        sandbox_storage_authority_ref: &str,
        sandbox_created_at: u64,
        sandbox_trace_id: &str,
    ) -> SandboxWorkspaceRuntimeResult<Self> {
        let references_valid = [
            sandbox_workspace_checkpoint_candidate_id,
            sandbox_workspace_id,
            sandbox_source_workspace_revision_ref,
            sandbox_candidate_workspace_revision_ref,
            sandbox_workspace_runtime_transaction_id,
            sandbox_session_id,
            sandbox_runtime_binding_id,
            sandbox_storage_authority_ref,
            sandbox_trace_id,
        ]
        .iter()
        .all(|reference| sandbox_validated_reference(reference));
        let digest_valid = sandbox_content_digest.len() == SANDBOX_CHECKPOINT_FINGERPRINT_LENGTH
            && sandbox_content_digest
                .bytes()
                .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if !references_valid || !digest_valid || sandbox_fencing_token < 0 {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeInvalidRequest);
        }
        Ok(Self {
            sandbox_workspace_checkpoint_candidate_id: sandbox_workspace_checkpoint_candidate_id
                .to_owned(),
            sandbox_workspace_id: sandbox_workspace_id.to_owned(),
            sandbox_source_workspace_revision_ref: sandbox_source_workspace_revision_ref.to_owned(),
            sandbox_candidate_workspace_revision_ref: sandbox_candidate_workspace_revision_ref
                .to_owned(),
            sandbox_workspace_runtime_transaction_id: sandbox_workspace_runtime_transaction_id
                .to_owned(),
            sandbox_session_id: sandbox_session_id.to_owned(),
            sandbox_runtime_binding_id: sandbox_runtime_binding_id.to_owned(),
            sandbox_fencing_token,
            sandbox_content_digest: sandbox_content_digest.to_owned(),
            sandbox_content_size_bytes,
            sandbox_storage_authority_ref: sandbox_storage_authority_ref.to_owned(),
            sandbox_created_at,
            sandbox_trace_id: sandbox_trace_id.to_owned(),
            sandbox_sealed: false,
            sandbox_promoted_expected_source_revision_ref: None,
        })
    }

    /// Seals the candidate; a sealed candidate is content-frozen and only
    /// then may enter the durable handoff (`sandbox_candidate_sealed_before_
    /// handoff`). Sealing is idempotent.
    pub fn sandbox_seal(&mut self) {
        self.sandbox_sealed = true;
    }

    /// Whether the candidate is sealed.
    #[must_use]
    pub const fn sandbox_is_sealed(&self) -> bool {
        self.sandbox_sealed
    }

    /// Agents-only compare-and-swap promotion: the caller is the Agents
    /// authority presenting the expected source revision; a mismatch is a
    /// non-destructive conflict that preserves the candidate.
    pub fn sandbox_promote(
        &mut self,
        sandbox_expected_source_workspace_revision_ref: &str,
    ) -> SandboxWorkspaceRuntimeResult<SandboxCandidatePromotion> {
        if !self.sandbox_sealed {
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeCheckpointFailed);
        }
        if let Some(promoted) = &self.sandbox_promoted_expected_source_revision_ref {
            return if promoted == sandbox_expected_source_workspace_revision_ref {
                Ok(SandboxCandidatePromotion::AlreadyPromoted)
            } else {
                Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeRevisionConflict)
            };
        }
        if self.sandbox_source_workspace_revision_ref
            != sandbox_expected_source_workspace_revision_ref
        {
            // Non-destructive: the candidate stays bounded and preserved for
            // Agents resolution; the newer revision is never overwritten.
            return Err(SandboxWorkspaceRuntimeError::SandboxWorkspaceRuntimeRevisionConflict);
        }
        self.sandbox_promoted_expected_source_revision_ref =
            Some(sandbox_expected_source_workspace_revision_ref.to_owned());
        Ok(SandboxCandidatePromotion::Promoted)
    }

    /// The candidate identity.
    #[must_use]
    pub fn sandbox_workspace_checkpoint_candidate_id(&self) -> &str {
        &self.sandbox_workspace_checkpoint_candidate_id
    }

    /// The source workspace revision the candidate was cut from.
    #[must_use]
    pub fn sandbox_source_workspace_revision_ref(&self) -> &str {
        &self.sandbox_source_workspace_revision_ref
    }

    /// The candidate revision Agents would promote.
    #[must_use]
    pub fn sandbox_candidate_workspace_revision_ref(&self) -> &str {
        &self.sandbox_candidate_workspace_revision_ref
    }

    /// The opaque storage-authority reference.
    #[must_use]
    pub fn sandbox_storage_authority_ref(&self) -> &str {
        &self.sandbox_storage_authority_ref
    }

    /// The content digest (lowercase hex SHA-256).
    #[must_use]
    pub fn sandbox_content_digest(&self) -> &str {
        &self.sandbox_content_digest
    }

    /// The content size in bytes.
    #[must_use]
    pub const fn sandbox_content_size_bytes(&self) -> u64 {
        self.sandbox_content_size_bytes
    }
}
