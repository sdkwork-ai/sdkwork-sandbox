//! The [`SandboxTemplateBuildInput`] boundary.
//!
//! Dockerfiles and build scripts are **build-input formats only**
//! (`specs/sandbox-template-authority.contract.json`: `buildInput`
//! `.dockerfileIsBuildInputOnly`); Docker is never the runtime dependency or
//! isolation boundary (`.dockerRuntimeBoundaryAllowed` is false). A build
//! input is an opaque reference: host paths, download URLs and embedded
//! signature or key material are rejected before anything is recorded.

use crate::error::{SandboxTemplateAuthorityError, SandboxTemplateAuthorityResult};
use crate::identity::SandboxTemplateOpaqueRef;

/// Whether the runtime may ever depend on Docker. Fixed by contract and by
/// the PRD non-goal: it may not.
pub const SANDBOX_TEMPLATE_DOCKER_RUNTIME_BOUNDARY_ALLOWED: bool = false;

/// The closed set of build-input formats.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SandboxTemplateBuildInputFormat {
    /// A Dockerfile reference, usable only as build input.
    DockerfileRef,
    /// A build-script reference, usable only as build input.
    BuildScriptRef,
}

impl SandboxTemplateBuildInputFormat {
    /// The contract format key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DockerfileRef => "sandbox_dockerfile_ref",
            Self::BuildScriptRef => "sandbox_build_script_ref",
        }
    }

    /// Parses a contract format key; unknown keys are rejected.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "sandbox_dockerfile_ref" => Self::DockerfileRef,
            "sandbox_build_script_ref" => Self::BuildScriptRef,
            _ => return None,
        })
    }
}

/// One validated build input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxTemplateBuildInput {
    sandbox_format: SandboxTemplateBuildInputFormat,
    sandbox_reference: SandboxTemplateOpaqueRef,
}

impl SandboxTemplateBuildInput {
    /// Builds one build input, enforcing the opaque-reference boundary.
    pub fn sandbox_new(
        sandbox_format: SandboxTemplateBuildInputFormat,
        sandbox_reference: SandboxTemplateOpaqueRef,
    ) -> SandboxTemplateAuthorityResult<Self> {
        let value = sandbox_reference.as_str();
        let forbidden = value.starts_with('/')
            || value.contains("://")
            || value.contains("-----BEGIN")
            || value.contains("..");
        if forbidden {
            return Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidBuildInput);
        }
        Ok(Self {
            sandbox_format,
            sandbox_reference,
        })
    }

    /// The build-input format.
    #[must_use]
    pub const fn sandbox_format(&self) -> SandboxTemplateBuildInputFormat {
        self.sandbox_format
    }

    /// The opaque build-input reference.
    #[must_use]
    pub const fn sandbox_reference(&self) -> &SandboxTemplateOpaqueRef {
        &self.sandbox_reference
    }
}
