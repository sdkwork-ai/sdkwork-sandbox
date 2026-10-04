//! Typed, validated identifiers and names for the Template authority.
//!
//! Every reference is opaque: no host path, no URL, no embedded signature or
//! key material (`specs/sandbox-template-authority.contract.json`:
//! `buildInput`).

use std::fmt;

use crate::bounds::MAX_SANDBOX_TEMPLATE_ID_LENGTH;
use crate::error::SandboxTemplateAuthorityError;

fn sandbox_validate_opaque(value: &str) -> Result<(), SandboxTemplateAuthorityError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_SANDBOX_TEMPLATE_ID_LENGTH
        && value
            .chars()
            .all(|c| c.is_ascii_graphic() && !c.is_whitespace())
        && !value.starts_with('/')
        && !value.contains("://")
        && !value.contains("-----BEGIN");
    if valid {
        Ok(())
    } else {
        Err(SandboxTemplateAuthorityError::SandboxTemplateInvalidDefinition)
    }
}

macro_rules! sandbox_template_id {
    ($name:ident, $error:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(String);

        impl $name {
            /// Builds the identifier, rejecting empty, oversized or
            /// locator-shaped values.
            pub fn new(value: impl Into<String>) -> Result<Self, SandboxTemplateAuthorityError> {
                let value = value.into();
                sandbox_validate_opaque(&value)
                    .map_err(|_| SandboxTemplateAuthorityError::$error)?;
                Ok(Self(value))
            }

            /// The identifier value.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

sandbox_template_id!(
    SandboxTemplateDefinitionId,
    SandboxTemplateInvalidDefinition,
    "Identity of one template definition."
);
sandbox_template_id!(
    SandboxTemplateVersionId,
    SandboxTemplateInvalidVersion,
    "Identity of one template version."
);
sandbox_template_id!(
    SandboxTemplateOpaqueRef,
    SandboxTemplateInvalidDefinition,
    "An opaque reference (base environment, file-layer content, artifact tuple). Carries no locator semantics."
);
sandbox_template_id!(
    SandboxTemplateName,
    SandboxTemplateInvalidVersion,
    "A versioned template name: one tag or one alias on a published version."
);
