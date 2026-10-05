//! The [`SandboxTemplateBuild`] record (`contract`: `build`).
//!
//! Every contract required field appears verbatim; references are opaque and
//! fields are private with read-only accessors, so a terminal build is
//! immutable by construction — the terminal states have no outbound edges in
//! [`crate::state`], and every lifecycle move is a state-machine transition.
//! A successful build must bind a `REQ-2026-0012` artifact tuple
//! (`artifactBoundary.successRequiresArtifactTupleRef`); a failed or
//! quarantined build must never bind one — a failed build that claims an
//! artifact is a lie the authority refuses to record.

use crate::bounds::MAX_SANDBOX_TEMPLATE_BUILD_ID_LENGTH;
use crate::error::{SandboxTemplateBuildAuthorityError, SandboxTemplateBuildAuthorityResult};
use crate::state::SandboxTemplateBuildState;

fn sandbox_validated_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_SANDBOX_TEMPLATE_BUILD_ID_LENGTH
        && value
            .chars()
            .all(|c| c.is_ascii_graphic() && !c.is_whitespace())
        && !value.starts_with('/')
        && !value.contains("://")
}

/// One created, lifecycle-tracked build of a template version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxTemplateBuild {
    sandbox_template_build_id: String,
    sandbox_template_definition_id: String,
    sandbox_template_version_id: String,
    sandbox_template_build_input_ref: String,
    sandbox_template_build_state: SandboxTemplateBuildState,
    sandbox_artifact_tuple_ref: Option<String>,
    sandbox_requested_at: u64,
    sandbox_updated_at: u64,
}

impl SandboxTemplateBuild {
    /// Creates one build record in `requested`, validating every field shape
    /// fail-closed. No builder has run; binding no artifact is correct here.
    pub fn sandbox_new(
        sandbox_template_build_id: &str,
        sandbox_template_definition_id: &str,
        sandbox_template_version_id: &str,
        sandbox_template_build_input_ref: &str,
        sandbox_requested_at: u64,
    ) -> SandboxTemplateBuildAuthorityResult<Self> {
        let references_valid = [
            sandbox_template_build_id,
            sandbox_template_definition_id,
            sandbox_template_version_id,
            sandbox_template_build_input_ref,
        ]
        .iter()
        .all(|reference| sandbox_validated_reference(reference));
        if !references_valid {
            return Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildInvalidBuild);
        }
        Ok(Self {
            sandbox_template_build_id: sandbox_template_build_id.to_owned(),
            sandbox_template_definition_id: sandbox_template_definition_id.to_owned(),
            sandbox_template_version_id: sandbox_template_version_id.to_owned(),
            sandbox_template_build_input_ref: sandbox_template_build_input_ref.to_owned(),
            sandbox_template_build_state: SandboxTemplateBuildState::Requested,
            sandbox_artifact_tuple_ref: None,
            sandbox_requested_at,
            sandbox_updated_at: sandbox_requested_at,
        })
    }

    fn sandbox_advance_timestamp(
        &self,
        sandbox_updated_at: u64,
    ) -> SandboxTemplateBuildAuthorityResult<()> {
        if sandbox_updated_at < self.sandbox_updated_at {
            return Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildInvalidBuild);
        }
        Ok(())
    }

    /// Records that a builder started executing. Only `requested -> building`
    /// is legal.
    pub fn sandbox_record_building(
        &mut self,
        sandbox_updated_at: u64,
    ) -> SandboxTemplateBuildAuthorityResult<()> {
        self.sandbox_advance_timestamp(sandbox_updated_at)?;
        if !self
            .sandbox_template_build_state
            .sandbox_can_transition_to(SandboxTemplateBuildState::Building)
        {
            return Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildIllegalTransition);
        }
        self.sandbox_template_build_state = SandboxTemplateBuildState::Building;
        self.sandbox_updated_at = sandbox_updated_at;
        Ok(())
    }

    /// Records the deterministic outcome. Only `building -> succeeded|failed`
    /// is legal. A successful build must bind exactly one artifact tuple; a
    /// failed build must bind none.
    pub fn sandbox_record_outcome(
        &mut self,
        sandbox_succeeded: bool,
        sandbox_artifact_tuple_ref: Option<&str>,
        sandbox_updated_at: u64,
    ) -> SandboxTemplateBuildAuthorityResult<()> {
        self.sandbox_advance_timestamp(sandbox_updated_at)?;
        let next = if sandbox_succeeded {
            SandboxTemplateBuildState::Succeeded
        } else {
            SandboxTemplateBuildState::Failed
        };
        if !self
            .sandbox_template_build_state
            .sandbox_can_transition_to(next)
        {
            return Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildIllegalTransition);
        }
        match (sandbox_succeeded, sandbox_artifact_tuple_ref) {
            (true, Some(tuple)) if sandbox_validated_reference(tuple) => {
                self.sandbox_artifact_tuple_ref = Some(tuple.to_owned());
            }
            (true, _) => {
                return Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildInvalidBuild);
            }
            (false, Some(_)) => {
                return Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildInvalidBuild);
            }
            (false, None) => {}
        }
        self.sandbox_template_build_state = next;
        self.sandbox_updated_at = sandbox_updated_at;
        Ok(())
    }

    /// Records an uncertain outcome (timeout, lost builder, unverifiable
    /// result); terminal, and the build binds no artifact. The closed table
    /// gives the terminal states no outbound edge, so a deterministic
    /// success can never be rewritten here.
    pub fn sandbox_quarantine(
        &mut self,
        sandbox_updated_at: u64,
    ) -> SandboxTemplateBuildAuthorityResult<()> {
        self.sandbox_advance_timestamp(sandbox_updated_at)?;
        if !self
            .sandbox_template_build_state
            .sandbox_can_transition_to(SandboxTemplateBuildState::Quarantined)
        {
            return Err(SandboxTemplateBuildAuthorityError::SandboxTemplateBuildIllegalTransition);
        }
        self.sandbox_artifact_tuple_ref = None;
        self.sandbox_template_build_state = SandboxTemplateBuildState::Quarantined;
        self.sandbox_updated_at = sandbox_updated_at;
        Ok(())
    }

    /// The build identity.
    #[must_use]
    pub fn sandbox_template_build_id(&self) -> &str {
        &self.sandbox_template_build_id
    }

    /// The template definition the build consumes (authority:
    /// `REQ-2026-0029`).
    #[must_use]
    pub fn sandbox_template_definition_id(&self) -> &str {
        &self.sandbox_template_definition_id
    }

    /// The template version the build consumes (authority:
    /// `REQ-2026-0029`).
    #[must_use]
    pub fn sandbox_template_version_id(&self) -> &str {
        &self.sandbox_template_version_id
    }

    /// The opaque build-input reference (semantics owned by
    /// `REQ-2026-0029`).
    #[must_use]
    pub fn sandbox_template_build_input_ref(&self) -> &str {
        &self.sandbox_template_build_input_ref
    }

    /// The current lifecycle state.
    #[must_use]
    pub const fn sandbox_template_build_state(&self) -> SandboxTemplateBuildState {
        self.sandbox_template_build_state
    }

    /// The bound `REQ-2026-0012` artifact tuple; present only on a
    /// deterministic success.
    #[must_use]
    pub fn sandbox_artifact_tuple_ref(&self) -> Option<&str> {
        self.sandbox_artifact_tuple_ref.as_deref()
    }

    /// The request timestamp (whole seconds from the caller's clock).
    #[must_use]
    pub const fn sandbox_requested_at(&self) -> u64 {
        self.sandbox_requested_at
    }

    /// The last transition timestamp (whole seconds from the caller's
    /// clock).
    #[must_use]
    pub const fn sandbox_updated_at(&self) -> u64 {
        self.sandbox_updated_at
    }
}
