//! The [`SandboxTemplateVersion`] record.
//!
//! A version is the versioned naming plane
//! (`specs/sandbox-template-authority.contract.json`: `versioning`): tags and
//! aliases are versioned names, immutable once published, and every version
//! binds exactly one artifact-tuple reference owned by `REQ-2026-0012`.

use std::collections::BTreeSet;

use crate::bounds::MAX_SANDBOX_TEMPLATE_NAME_COUNT;
use crate::error::{SandboxTemplateAuthorityError, SandboxTemplateAuthorityResult};
use crate::identity::{
    SandboxTemplateDefinitionId, SandboxTemplateName, SandboxTemplateOpaqueRef,
    SandboxTemplateVersionId,
};

/// One published, immutable template version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxTemplateVersion {
    sandbox_template_version_id: SandboxTemplateVersionId,
    sandbox_template_definition_id: SandboxTemplateDefinitionId,
    sandbox_tags: BTreeSet<SandboxTemplateName>,
    sandbox_aliases: BTreeSet<SandboxTemplateName>,
    sandbox_artifact_tuple_ref: SandboxTemplateOpaqueRef,
}

impl SandboxTemplateVersion {
    /// Publishes one immutable version binding tags, aliases and exactly one
    /// artifact-tuple reference.
    pub fn sandbox_publish(
        sandbox_template_version_id: SandboxTemplateVersionId,
        sandbox_template_definition_id: SandboxTemplateDefinitionId,
        sandbox_tags: BTreeSet<SandboxTemplateName>,
        sandbox_aliases: BTreeSet<SandboxTemplateName>,
        sandbox_artifact_tuple_ref: SandboxTemplateOpaqueRef,
    ) -> SandboxTemplateAuthorityResult<Self> {
        if sandbox_tags.len() > MAX_SANDBOX_TEMPLATE_NAME_COUNT
            || sandbox_aliases.len() > MAX_SANDBOX_TEMPLATE_NAME_COUNT
        {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidVersion);
        }
        Ok(Self {
            sandbox_template_version_id,
            sandbox_template_definition_id,
            sandbox_tags,
            sandbox_aliases,
            sandbox_artifact_tuple_ref,
        })
    }

    /// The version identity.
    #[must_use]
    pub const fn sandbox_template_version_id(&self) -> &SandboxTemplateVersionId {
        &self.sandbox_template_version_id
    }

    /// The definition this version instantiates.
    #[must_use]
    pub const fn sandbox_template_definition_id(&self) -> &SandboxTemplateDefinitionId {
        &self.sandbox_template_definition_id
    }

    /// The immutable tag set.
    #[must_use]
    pub const fn sandbox_tags(&self) -> &BTreeSet<SandboxTemplateName> {
        &self.sandbox_tags
    }

    /// The immutable alias set.
    #[must_use]
    pub const fn sandbox_aliases(&self) -> &BTreeSet<SandboxTemplateName> {
        &self.sandbox_aliases
    }

    /// The exact `REQ-2026-0012` artifact-tuple reference this version runs
    /// on. The authority owns the reference, never the tuple's evidence.
    #[must_use]
    pub const fn sandbox_artifact_tuple_ref(&self) -> &SandboxTemplateOpaqueRef {
        &self.sandbox_artifact_tuple_ref
    }
}
