//! The [`SandboxTemplateDefinition`] record.
//!
//! A definition is the declarative environment description
//! (`specs/sandbox-template-authority.contract.json`: `templateDefinition`):
//! base environment reference, file layers, environment variables and the
//! start command. Fields are private with read-only accessors, so
//! `immutableAfterPublication` is enforced by construction — there is no
//! mutation API to misuse, and every change is a new definition version.

use std::collections::BTreeMap;

use crate::bounds::{
    MAX_SANDBOX_TEMPLATE_ENV_COUNT, MAX_SANDBOX_TEMPLATE_ENV_KEY_LENGTH,
    MAX_SANDBOX_TEMPLATE_ENV_VALUE_LENGTH, MAX_SANDBOX_TEMPLATE_FILE_LAYERS,
    MAX_SANDBOX_TEMPLATE_START_COMMAND_LENGTH, MAX_SANDBOX_TEMPLATE_VERSION_LENGTH,
};
use crate::error::{SandboxTemplateAuthorityError, SandboxTemplateAuthorityResult};
use crate::identity::{SandboxTemplateDefinitionId, SandboxTemplateOpaqueRef};

/// One immutable file layer of a definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxTemplateFileLayer {
    sandbox_layer_name: String,
    sandbox_layer_content_ref: SandboxTemplateOpaqueRef,
}

impl SandboxTemplateFileLayer {
    /// Builds one file layer from its name and opaque content reference.
    pub fn sandbox_new(
        sandbox_layer_name: impl Into<String>,
        sandbox_layer_content_ref: SandboxTemplateOpaqueRef,
    ) -> SandboxTemplateAuthorityResult<Self> {
        let sandbox_layer_name = sandbox_layer_name.into();
        let valid = !sandbox_layer_name.is_empty()
            && sandbox_layer_name.len() <= crate::bounds::MAX_SANDBOX_TEMPLATE_NAME_LENGTH
            && sandbox_layer_name
                .chars()
                .all(|c| c.is_ascii_graphic() && !c.is_whitespace())
            && !sandbox_layer_name.starts_with('/')
            && !sandbox_layer_name.contains('\\')
            && !sandbox_layer_name.contains("..");
        if !valid {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition);
        }
        Ok(Self {
            sandbox_layer_name,
            sandbox_layer_content_ref,
        })
    }

    /// The layer name.
    #[must_use]
    pub fn sandbox_layer_name(&self) -> &str {
        &self.sandbox_layer_name
    }

    /// The opaque content reference.
    #[must_use]
    pub const fn sandbox_layer_content_ref(&self) -> &SandboxTemplateOpaqueRef {
        &self.sandbox_layer_content_ref
    }
}

/// One published, immutable template definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxTemplateDefinition {
    sandbox_template_definition_id: SandboxTemplateDefinitionId,
    sandbox_template_definition_version: String,
    sandbox_base_environment_ref: SandboxTemplateOpaqueRef,
    sandbox_file_layers: Vec<SandboxTemplateFileLayer>,
    sandbox_set_envs: BTreeMap<String, String>,
    sandbox_set_start_cmd: String,
    sandbox_published_at: u64,
}

impl SandboxTemplateDefinition {
    /// Publishes one immutable definition. Every field is validated
    /// fail-closed; a rejected field aborts the whole publication.
    #[allow(clippy::too_many_arguments)]
    pub fn sandbox_publish(
        sandbox_template_definition_id: SandboxTemplateDefinitionId,
        sandbox_template_definition_version: impl Into<String>,
        sandbox_base_environment_ref: SandboxTemplateOpaqueRef,
        sandbox_file_layers: Vec<SandboxTemplateFileLayer>,
        sandbox_set_envs: BTreeMap<String, String>,
        sandbox_set_start_cmd: impl Into<String>,
        sandbox_published_at: u64,
    ) -> SandboxTemplateAuthorityResult<Self> {
        let sandbox_template_definition_version = sandbox_template_definition_version.into();
        let version_valid = !sandbox_template_definition_version.is_empty()
            && sandbox_template_definition_version.len() <= MAX_SANDBOX_TEMPLATE_VERSION_LENGTH
            && sandbox_template_definition_version
                .chars()
                .all(|c| c.is_ascii_graphic() && !c.is_whitespace());
        if !version_valid {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition);
        }
        if sandbox_file_layers.len() > MAX_SANDBOX_TEMPLATE_FILE_LAYERS {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition);
        }
        if sandbox_set_envs.len() > MAX_SANDBOX_TEMPLATE_ENV_COUNT {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition);
        }
        for (key, value) in &sandbox_set_envs {
            let key_valid = !key.is_empty()
                && key.len() <= MAX_SANDBOX_TEMPLATE_ENV_KEY_LENGTH
                && key
                    .chars()
                    .all(|c| c.is_ascii_graphic() && !c.is_whitespace());
            let value_valid = value.len() <= MAX_SANDBOX_TEMPLATE_ENV_VALUE_LENGTH;
            if !key_valid || !value_valid {
                return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition);
            }
        }
        let sandbox_set_start_cmd = sandbox_set_start_cmd.into();
        let start_valid = !sandbox_set_start_cmd.is_empty()
            && sandbox_set_start_cmd.len() <= MAX_SANDBOX_TEMPLATE_START_COMMAND_LENGTH;
        if !start_valid {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition);
        }
        Ok(Self {
            sandbox_template_definition_id,
            sandbox_template_definition_version,
            sandbox_base_environment_ref,
            sandbox_file_layers,
            sandbox_set_envs,
            sandbox_set_start_cmd,
            sandbox_published_at,
        })
    }

    /// The definition identity.
    #[must_use]
    pub const fn sandbox_template_definition_id(&self) -> &SandboxTemplateDefinitionId {
        &self.sandbox_template_definition_id
    }

    /// The definition version string.
    #[must_use]
    pub fn sandbox_template_definition_version(&self) -> &str {
        &self.sandbox_template_definition_version
    }

    /// The opaque base-environment reference.
    #[must_use]
    pub const fn sandbox_base_environment_ref(&self) -> &SandboxTemplateOpaqueRef {
        &self.sandbox_base_environment_ref
    }

    /// The immutable file layers, in declaration order.
    #[must_use]
    pub fn sandbox_file_layers(&self) -> &[SandboxTemplateFileLayer] {
        &self.sandbox_file_layers
    }

    /// The declared environment variables.
    #[must_use]
    pub fn sandbox_set_envs(&self) -> &BTreeMap<String, String> {
        &self.sandbox_set_envs
    }

    /// The declared start command.
    #[must_use]
    pub fn sandbox_set_start_cmd(&self) -> &str {
        &self.sandbox_set_start_cmd
    }

    /// The publication timestamp (whole seconds from the caller's clock).
    #[must_use]
    pub const fn sandbox_published_at(&self) -> u64 {
        self.sandbox_published_at
    }
}
