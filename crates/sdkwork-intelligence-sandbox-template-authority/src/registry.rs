//! The bounded in-process template registry (`REVIEW-20261006-bounded-registry`).
//!
//! The registry composes the 0029 authority records directly — publish
//! definition, publish version, resolve a start command — with no second
//! record type (`REG-01`). It is bounded and in-process: capacity beyond
//! [`SANDBOX_TEMPLATE_REGISTRY_CAPACITY_MAX`] is refused and storage stays
//! behind the contract's `x-sdkwork-no-storage-backend` gate (`REG-02`).
//! The resolve surface exposes exactly one thing: the published start
//! command for a version reference, through the authority accessors
//! version -> definition -> `sandbox_set_start_cmd()` (`REG-03`).

use std::collections::BTreeMap;

use crate::definition::SandboxTemplateDefinition;
use crate::error::{SandboxTemplateAuthorityError, SandboxTemplateAuthorityResult};
use crate::version::SandboxTemplateVersion;

/// Maximum published definitions and versions the bounded registry holds.
pub const SANDBOX_TEMPLATE_REGISTRY_CAPACITY_MAX: usize = 1024;

/// The bounded in-process registry over published 0029 authority records.
#[derive(Default)]
pub struct BoundedSandboxTemplateRegistry {
    sandbox_definitions: BTreeMap<String, SandboxTemplateDefinition>,
    sandbox_versions: BTreeMap<String, SandboxTemplateVersion>,
}

impl BoundedSandboxTemplateRegistry {
    /// Creates an empty bounded registry.
    #[must_use]
    pub fn sandbox_new() -> Self {
        Self::default()
    }

    /// Publishes one definition. Capacity beyond the bound is refused.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition`]
    /// when the registry is at capacity, or the authority's own validation
    /// error for a malformed record.
    pub fn sandbox_publish_definition(
        &mut self,
        definition: SandboxTemplateDefinition,
    ) -> SandboxTemplateAuthorityResult<()> {
        if self.sandbox_definitions.len() >= SANDBOX_TEMPLATE_REGISTRY_CAPACITY_MAX {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition);
        }
        let key = definition
            .sandbox_template_definition_id()
            .as_str()
            .to_owned();
        if self.sandbox_definitions.contains_key(&key) {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition);
        }
        self.sandbox_definitions.insert(key, definition);
        Ok(())
    }

    /// Publishes one version. The referenced definition must already be
    /// published; capacity beyond the bound is refused.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`SandboxTemplateAuthorityError::SandboxTemplateInvalidVersion`] when
    /// the registry is at capacity, the referenced definition is unknown, or
    /// the version identity is already published.
    pub fn sandbox_publish_version(
        &mut self,
        version: SandboxTemplateVersion,
    ) -> SandboxTemplateAuthorityResult<()> {
        if self.sandbox_versions.len() >= SANDBOX_TEMPLATE_REGISTRY_CAPACITY_MAX {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidVersion);
        }
        let key = version.sandbox_template_version_id().as_str().to_owned();
        if self.sandbox_versions.contains_key(&key) {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidVersion);
        }
        if !self
            .sandbox_definitions
            .contains_key(version.sandbox_template_definition_id().as_str())
        {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidVersion);
        }
        self.sandbox_versions.insert(key, version);
        Ok(())
    }

    /// Resolves the published start command for one version reference:
    /// version -> definition -> `sandbox_set_start_cmd()` (`REG-03`). This
    /// is the registry's entire resolve surface.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`SandboxTemplateAuthorityError::SandboxTemplateInvalidVersion`] when
    /// the version reference is unknown.
    pub fn sandbox_resolve_start_command(
        &self,
        sandbox_template_version_ref: &str,
    ) -> SandboxTemplateAuthorityResult<String> {
        let version = self
            .sandbox_versions
            .get(sandbox_template_version_ref)
            .ok_or(SandboxTemplateAuthorityError::SandboxTemplateInvalidVersion)?;
        let definition = self
            .sandbox_definitions
            .get(version.sandbox_template_definition_id().as_str())
            .ok_or(SandboxTemplateAuthorityError::SandboxTemplateInvalidVersion)?;
        Ok(definition.sandbox_set_start_cmd().to_owned())
    }

    /// How many definitions the registry holds.
    #[must_use]
    pub fn sandbox_definition_count(&self) -> usize {
        self.sandbox_definitions.len()
    }

    /// How many versions the registry holds.
    #[must_use]
    pub fn sandbox_version_count(&self) -> usize {
        self.sandbox_versions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::SandboxTemplateFileLayer;
    use crate::identity::{
        SandboxTemplateDefinitionId, SandboxTemplateName, SandboxTemplateOpaqueRef,
        SandboxTemplateVersionId,
    };
    use std::collections::{BTreeMap, BTreeSet};

    fn published_registry() -> (BoundedSandboxTemplateRegistry, String) {
        let mut registry = BoundedSandboxTemplateRegistry::sandbox_new();
        let definition_id = SandboxTemplateDefinitionId::new("definition-1").expect("valid id");
        registry
            .sandbox_publish_definition(
                SandboxTemplateDefinition::sandbox_publish(
                    definition_id.clone(),
                    "1",
                    SandboxTemplateOpaqueRef::new("base-env-1").expect("valid ref"),
                    vec![SandboxTemplateFileLayer::sandbox_new(
                        "layer-1",
                        SandboxTemplateOpaqueRef::new("content-1").expect("valid ref"),
                    )
                    .expect("valid layer")],
                    BTreeMap::new(),
                    "echo fast-start",
                    900,
                )
                .expect("valid definition"),
            )
            .expect("definition published");
        let version_id = SandboxTemplateVersionId::new("version-1").expect("valid id");
        registry
            .sandbox_publish_version(
                SandboxTemplateVersion::sandbox_publish(
                    version_id.clone(),
                    definition_id,
                    BTreeSet::from([SandboxTemplateName::new("latest").expect("valid name")]),
                    BTreeSet::new(),
                    SandboxTemplateOpaqueRef::new(
                        "req-0012-rootfs-sha256-0000000000000000000000000000000000000000000000000000000000000000",
                    )
                    .expect("valid tuple ref"),
                )
                .expect("valid version"),
            )
            .expect("version published");
        (registry, version_id.as_str().to_owned())
    }

    #[test]
    fn resolution_follows_version_to_definition_to_the_published_start_command() {
        let (registry, version_ref) = published_registry();
        assert_eq!(
            registry
                .sandbox_resolve_start_command(&version_ref)
                .expect("resolved"),
            "echo fast-start"
        );
        assert_eq!(registry.sandbox_definition_count(), 1);
        assert_eq!(registry.sandbox_version_count(), 1);
    }

    #[test]
    fn unknown_versions_and_duplicate_identities_are_refused() {
        let (registry, version_ref) = published_registry();
        assert!(registry
            .sandbox_resolve_start_command("version-unknown")
            .is_err());
        let mut registry = registry;
        // Duplicate version identity is refused.
        let duplicate = SandboxTemplateVersion::sandbox_publish(
            SandboxTemplateVersionId::new(&version_ref).expect("valid id"),
            SandboxTemplateDefinitionId::new("definition-1").expect("valid id"),
            BTreeSet::new(),
            BTreeSet::new(),
            SandboxTemplateOpaqueRef::new(
                "req-0012-rootfs-sha256-0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("valid tuple ref"),
        )
        .expect("valid version");
        assert!(registry.sandbox_publish_version(duplicate).is_err());
    }

    #[test]
    fn a_version_without_a_published_definition_is_refused() {
        let mut registry = BoundedSandboxTemplateRegistry::sandbox_new();
        let orphan = SandboxTemplateVersion::sandbox_publish(
            SandboxTemplateVersionId::new("version-orphan").expect("valid id"),
            SandboxTemplateDefinitionId::new("definition-unknown").expect("valid id"),
            BTreeSet::new(),
            BTreeSet::new(),
            SandboxTemplateOpaqueRef::new(
                "req-0012-rootfs-sha256-0000000000000000000000000000000000000000000000000000000000000000",
            )
            .expect("valid tuple ref"),
        )
        .expect("valid version");
        assert!(registry.sandbox_publish_version(orphan).is_err());
    }
}
