//! The Firecracker artifact compatibility tuple (`REQ-2026-0012`).
//!
//! The provider never builds, downloads, or resolves artifacts at runtime
//! (`specs/sandbox-firecracker-artifact-compatibility.contract.json`:
//! `runtimeConsumption.runtimeCompatibilityOverrideAllowed` is false and no
//! download URL exists in the model). Composition hands over one immutable
//! [`SandboxFirecrackerArtifactManifest`] published by the release authority,
//! and this module validates its exact compatibility tuple before any
//! preflight or allocation may consume it. Validation is pure data: no host
//! I/O, no digest computation, no network.

use std::collections::BTreeSet;
use std::fmt;

/// The host architectures a Firecracker provider may ever report
/// (`specs/sandbox-provider-delivery-gates.contract.json`:
/// `sandbox_supported_host_platforms`).
pub const SANDBOX_FIRECRACKER_SUPPORTED_ARCHITECTURES: [&str; 2] =
    ["linux-kvm-x86_64", "linux-kvm-aarch64"];

/// The artifact roles the manifest must carry. `sandbox_initrd` is optional;
/// every other role is required and must appear exactly once.
const SANDBOX_REQUIRED_ARTIFACT_ROLES: [&str; 5] = [
    "sandbox_firecracker",
    "sandbox_jailer",
    "sandbox_guest_kernel",
    "sandbox_rootfs",
    "sandbox_guest_agent",
];

/// Why a release-published artifact manifest cannot back a Firecracker
/// provider. Every variant is fail-closed: the caller may only replace the
/// manifest, never relax the check that produced one of these values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SandboxFirecrackerArtifactError {
    /// A declared role is missing from the manifest's artifact set.
    RequiredArtifactRoleMissing { sandbox_artifact_role: String },
    /// An artifact digest is not 64 lowercase hex characters (sha256).
    ArtifactDigestInvalid { sandbox_artifact_role: String },
    /// The Firecracker and Jailer digests differ; the contract requires the
    /// two releases to match exactly.
    FirecrackerJailerReleaseMismatch,
    /// The manifest's host architecture is outside the supported tuple set.
    HostArchitectureUnsupported {
        sandbox_supported_host_architecture: String,
    },
    /// A compatibility-tuple digest reference does not match the artifact it
    /// names: a partial or stale tuple fails closed.
    CompatibilityTupleDigestMismatch { sandbox_artifact_role: String },
    /// The manifest references a release by a mutable or locator-shaped value
    /// instead of an opaque release reference.
    ReleaseReferenceInvalid,
}

impl fmt::Display for SandboxFirecrackerArtifactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiredArtifactRoleMissing {
                sandbox_artifact_role,
            } => write!(f, "required sandbox artifact role is missing: {sandbox_artifact_role}"),
            Self::ArtifactDigestInvalid {
                sandbox_artifact_role,
            } => write!(f, "sandbox artifact digest is not a lowercase sha256 hex digest: {sandbox_artifact_role}"),
            Self::FirecrackerJailerReleaseMismatch => f.write_str(
                "the sandbox firecracker and jailer artifact digests must match one release",
            ),
            Self::HostArchitectureUnsupported {
                sandbox_supported_host_architecture,
            } => write!(
                f,
                "sandbox host architecture is unsupported: {sandbox_supported_host_architecture}"
            ),
            Self::CompatibilityTupleDigestMismatch {
                sandbox_artifact_role,
            } => write!(
                f,
                "sandbox compatibility tuple digest does not match its artifact: {sandbox_artifact_role}"
            ),
            Self::ReleaseReferenceInvalid => {
                f.write_str("the sandbox artifact release reference is not opaque")
            }
        }
    }
}

impl std::error::Error for SandboxFirecrackerArtifactError {}

/// One release-published artifact inside the manifest. The descriptor carries
/// the contract's required evidence references only: no host path, no download
/// URL, no embedded signature or key material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerArtifactDescriptor {
    /// The contract's artifact role name (`sandbox_firecracker`,
    /// `sandbox_jailer`, `sandbox_guest_kernel`, `sandbox_rootfs`,
    /// `sandbox_guest_agent`, `sandbox_initrd`).
    pub sandbox_artifact_role: String,
    /// The pinned upstream version string for this artifact.
    pub sandbox_artifact_version: String,
    /// sha256 digest, 64 lowercase hex characters.
    pub sandbox_artifact_sha256: String,
    /// Opaque release reference resolved by the release authority.
    pub sandbox_artifact_release_ref: String,
    /// Opaque reference to the artifact's signature record.
    pub sandbox_artifact_signature_ref: String,
    /// Opaque reference to the artifact's SBOM record.
    pub sandbox_artifact_sbom_ref: String,
    /// Opaque reference to the artifact's provenance record.
    pub sandbox_artifact_provenance_ref: String,
}

impl SandboxFirecrackerArtifactDescriptor {
    /// Whether the digest is a well-formed sha256 lowercase hex digest.
    #[must_use]
    pub fn sandbox_artifact_digest_is_valid(&self) -> bool {
        is_sandbox_sha256_digest(&self.sandbox_artifact_sha256)
    }
}

/// The exact compatibility tuple the contract pins. Every field is required;
/// cross-architecture reuse and partial tuples are rejected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerCompatibilityTuple {
    /// Host architecture this tuple boots on.
    pub sandbox_supported_host_architecture: String,
    /// Minimum host kernel version the tuple was verified against.
    pub sandbox_minimum_host_kernel_version: String,
    /// The KVM API requirement the tuple declares.
    pub sandbox_kvm_api_requirement: String,
    /// Digest reference of the Firecracker binary, matching its artifact.
    pub sandbox_firecracker_artifact_sha256: String,
    /// Digest reference of the Jailer binary, matching its artifact.
    pub sandbox_jailer_artifact_sha256: String,
    /// Digest reference of the guest kernel, matching its artifact.
    pub sandbox_guest_kernel_artifact_sha256: String,
    /// Digest reference of the rootfs, matching its artifact.
    pub sandbox_rootfs_artifact_sha256: String,
    /// Digest reference of the guest agent, matching its artifact.
    pub sandbox_guest_agent_artifact_sha256: String,
    /// Guest boot contract version the rootfs and agent agree on.
    pub sandbox_guest_boot_contract_version: String,
    /// Rootfs schema version the provider prepares devices for.
    pub sandbox_rootfs_schema_version: String,
    /// Guest agent protocol version the vsock channel speaks.
    pub sandbox_guest_agent_protocol_version: String,
}

/// The release-published manifest. One manifest backs one architecture and is
/// immutable after publication; composition loads it from the release
/// authority and hands the validated value to the provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerArtifactManifest {
    /// Stable manifest identifier.
    pub sandbox_artifact_manifest_id: String,
    /// Manifest revision within the identifier.
    pub sandbox_artifact_manifest_version: String,
    /// sha256 digest of the published manifest document.
    pub sandbox_artifact_manifest_digest: String,
    /// The release version this manifest was published for.
    pub sandbox_release_version: String,
    /// Host architecture the manifest's tuple targets.
    pub sandbox_supported_host_architecture: String,
    /// The exact compatibility tuple.
    pub sandbox_compatibility_tuple: SandboxFirecrackerCompatibilityTuple,
    /// One descriptor per artifact role; extra roles are ignored by
    /// validation but preserved for the release authority's evidence chain.
    pub sandbox_artifacts: Vec<SandboxFirecrackerArtifactDescriptor>,
    /// Opaque evidence bundle reference (`REQ-2026-0012` evidence block).
    pub sandbox_evidence_bundle_ref: String,
    /// Opaque signature policy reference.
    pub sandbox_signature_policy_ref: String,
    /// Opaque revocation policy reference.
    pub sandbox_revocation_policy_ref: String,
    /// Publication timestamp (RFC 3339) fixed by the release authority.
    pub sandbox_published_at: String,
}

impl SandboxFirecrackerArtifactManifest {
    /// The artifact descriptor for one role, when the manifest carries it.
    #[must_use]
    pub fn sandbox_artifact(
        &self,
        sandbox_artifact_role: &str,
    ) -> Option<&SandboxFirecrackerArtifactDescriptor> {
        self.sandbox_artifacts
            .iter()
            .find(|descriptor| descriptor.sandbox_artifact_role == sandbox_artifact_role)
    }

    /// Validates the manifest against the exact-tuple contract. A successful
    /// return authorizes preflight consumption only; boot-time verification of
    /// materialized bytes stays with the host isolation broker slice.
    ///
    /// # Errors
    ///
    /// Returns the first failed contract rule: a missing required role, a
    /// malformed digest, a Firecracker/Jailer release mismatch, an unsupported
    /// host architecture, or a tuple digest that does not match its artifact.
    pub fn validate(&self) -> Result<(), SandboxFirecrackerArtifactError> {
        if !is_sandbox_sha256_digest(&self.sandbox_artifact_manifest_digest) {
            return Err(SandboxFirecrackerArtifactError::ArtifactDigestInvalid {
                sandbox_artifact_role: "sandbox_artifact_manifest".to_owned(),
            });
        }
        if SANDBOX_FIRECRACKER_SUPPORTED_ARCHITECTURES
            .iter()
            .all(|supported| *supported != self.sandbox_supported_host_architecture)
        {
            return Err(
                SandboxFirecrackerArtifactError::HostArchitectureUnsupported {
                    sandbox_supported_host_architecture: self
                        .sandbox_supported_host_architecture
                        .clone(),
                },
            );
        }
        if self.sandbox_supported_host_architecture
            != self
                .sandbox_compatibility_tuple
                .sandbox_supported_host_architecture
        {
            return Err(
                SandboxFirecrackerArtifactError::HostArchitectureUnsupported {
                    sandbox_supported_host_architecture: self
                        .sandbox_compatibility_tuple
                        .sandbox_supported_host_architecture
                        .clone(),
                },
            );
        }

        let sandbox_roles: BTreeSet<&str> = self
            .sandbox_artifacts
            .iter()
            .map(|descriptor| descriptor.sandbox_artifact_role.as_str())
            .collect();
        for sandbox_required_role in SANDBOX_REQUIRED_ARTIFACT_ROLES {
            if !sandbox_roles.contains(sandbox_required_role) {
                return Err(
                    SandboxFirecrackerArtifactError::RequiredArtifactRoleMissing {
                        sandbox_artifact_role: sandbox_required_role.to_owned(),
                    },
                );
            }
        }
        for sandbox_descriptor in &self.sandbox_artifacts {
            if !sandbox_descriptor.sandbox_artifact_digest_is_valid() {
                return Err(SandboxFirecrackerArtifactError::ArtifactDigestInvalid {
                    sandbox_artifact_role: sandbox_descriptor.sandbox_artifact_role.clone(),
                });
            }
        }
        for sandbox_reference in [
            &self.sandbox_evidence_bundle_ref,
            &self.sandbox_signature_policy_ref,
            &self.sandbox_revocation_policy_ref,
        ] {
            if sandbox_reference.is_empty() {
                return Err(SandboxFirecrackerArtifactError::ReleaseReferenceInvalid);
            }
        }

        let sandbox_tuple = &self.sandbox_compatibility_tuple;
        if sandbox_tuple.sandbox_firecracker_artifact_sha256
            != sandbox_tuple.sandbox_jailer_artifact_sha256
        {
            return Err(SandboxFirecrackerArtifactError::FirecrackerJailerReleaseMismatch);
        }
        let sandbox_tuple_digest_roles = [
            (
                "sandbox_firecracker",
                &sandbox_tuple.sandbox_firecracker_artifact_sha256,
            ),
            (
                "sandbox_jailer",
                &sandbox_tuple.sandbox_jailer_artifact_sha256,
            ),
            (
                "sandbox_guest_kernel",
                &sandbox_tuple.sandbox_guest_kernel_artifact_sha256,
            ),
            (
                "sandbox_rootfs",
                &sandbox_tuple.sandbox_rootfs_artifact_sha256,
            ),
            (
                "sandbox_guest_agent",
                &sandbox_tuple.sandbox_guest_agent_artifact_sha256,
            ),
        ];
        for (sandbox_artifact_role, sandbox_tuple_digest) in sandbox_tuple_digest_roles {
            let Some(sandbox_descriptor) = self.sandbox_artifact(sandbox_artifact_role) else {
                return Err(
                    SandboxFirecrackerArtifactError::RequiredArtifactRoleMissing {
                        sandbox_artifact_role: sandbox_artifact_role.to_owned(),
                    },
                );
            };
            if sandbox_descriptor.sandbox_artifact_sha256 != *sandbox_tuple_digest {
                return Err(
                    SandboxFirecrackerArtifactError::CompatibilityTupleDigestMismatch {
                        sandbox_artifact_role: sandbox_artifact_role.to_owned(),
                    },
                );
            }
        }
        Ok(())
    }
}

/// Whether the value is exactly 64 lowercase hex characters (sha256).
#[must_use]
pub fn is_sandbox_sha256_digest(sandbox_value: &str) -> bool {
    sandbox_value.len() == 64
        && sandbox_value.bytes().all(|sandbox_byte| {
            sandbox_byte.is_ascii_digit() || (b'a'..=b'f').contains(&sandbox_byte)
        })
}

#[cfg(test)]
mod tests {
    use super::{
        SandboxFirecrackerArtifactDescriptor, SandboxFirecrackerArtifactError,
        SandboxFirecrackerArtifactManifest, SandboxFirecrackerCompatibilityTuple,
        SANDBOX_FIRECRACKER_SUPPORTED_ARCHITECTURES,
    };

    const SANDBOX_DIGEST_A: &str =
        "aaaa000000000000000000000000000000000000000000000000000000000000";
    const SANDBOX_DIGEST_B: &str =
        "bbbb000000000000000000000000000000000000000000000000000000000000";

    fn sandbox_descriptor(
        sandbox_artifact_role: &str,
        digest: &str,
    ) -> SandboxFirecrackerArtifactDescriptor {
        SandboxFirecrackerArtifactDescriptor {
            sandbox_artifact_role: sandbox_artifact_role.to_owned(),
            sandbox_artifact_version: "1.0.0".to_owned(),
            sandbox_artifact_sha256: digest.to_owned(),
            sandbox_artifact_release_ref: "release-ref".to_owned(),
            sandbox_artifact_signature_ref: "signature-ref".to_owned(),
            sandbox_artifact_sbom_ref: "sbom-ref".to_owned(),
            sandbox_artifact_provenance_ref: "provenance-ref".to_owned(),
        }
    }

    fn sandbox_manifest() -> SandboxFirecrackerArtifactManifest {
        SandboxFirecrackerArtifactManifest {
            sandbox_artifact_manifest_id: "manifest-id".to_owned(),
            sandbox_artifact_manifest_version: "1".to_owned(),
            sandbox_artifact_manifest_digest: SANDBOX_DIGEST_A.to_owned(),
            sandbox_release_version: "2026.09".to_owned(),
            sandbox_supported_host_architecture: "linux-kvm-x86_64".to_owned(),
            sandbox_compatibility_tuple: SandboxFirecrackerCompatibilityTuple {
                sandbox_supported_host_architecture: "linux-kvm-x86_64".to_owned(),
                sandbox_minimum_host_kernel_version: "6.1".to_owned(),
                sandbox_kvm_api_requirement: "kvm-v5".to_owned(),
                sandbox_firecracker_artifact_sha256: SANDBOX_DIGEST_A.to_owned(),
                sandbox_jailer_artifact_sha256: SANDBOX_DIGEST_A.to_owned(),
                sandbox_guest_kernel_artifact_sha256: SANDBOX_DIGEST_A.to_owned(),
                sandbox_rootfs_artifact_sha256: SANDBOX_DIGEST_A.to_owned(),
                sandbox_guest_agent_artifact_sha256: SANDBOX_DIGEST_B.to_owned(),
                sandbox_guest_boot_contract_version: "1".to_owned(),
                sandbox_rootfs_schema_version: "1".to_owned(),
                sandbox_guest_agent_protocol_version: "1".to_owned(),
            },
            sandbox_artifacts: vec![
                sandbox_descriptor("sandbox_firecracker", SANDBOX_DIGEST_A),
                sandbox_descriptor("sandbox_jailer", SANDBOX_DIGEST_A),
                sandbox_descriptor("sandbox_guest_kernel", SANDBOX_DIGEST_A),
                sandbox_descriptor("sandbox_rootfs", SANDBOX_DIGEST_A),
                sandbox_descriptor("sandbox_guest_agent", SANDBOX_DIGEST_B),
            ],
            sandbox_evidence_bundle_ref: "evidence-ref".to_owned(),
            sandbox_signature_policy_ref: "signature-policy-ref".to_owned(),
            sandbox_revocation_policy_ref: "revocation-policy-ref".to_owned(),
            sandbox_published_at: "2026-09-24T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn complete_tuple_manifest_validates() {
        assert_eq!(Ok(()), sandbox_manifest().validate());
    }

    #[test]
    fn missing_required_role_fails_closed() {
        let mut sandbox_manifest = sandbox_manifest();
        sandbox_manifest
            .sandbox_artifacts
            .retain(|descriptor| descriptor.sandbox_artifact_role != "sandbox_jailer");
        assert_eq!(
            Err(
                SandboxFirecrackerArtifactError::RequiredArtifactRoleMissing {
                    sandbox_artifact_role: "sandbox_jailer".to_owned(),
                }
            ),
            sandbox_manifest.validate()
        );
    }

    #[test]
    fn firecracker_jailer_release_mismatch_fails_closed() {
        let mut sandbox_manifest = sandbox_manifest();
        sandbox_manifest
            .sandbox_compatibility_tuple
            .sandbox_jailer_artifact_sha256 = SANDBOX_DIGEST_B.to_owned();
        assert_eq!(
            Err(SandboxFirecrackerArtifactError::FirecrackerJailerReleaseMismatch),
            sandbox_manifest.validate()
        );
    }

    #[test]
    fn tuple_digest_mismatch_fails_closed() {
        let mut sandbox_manifest = sandbox_manifest();
        sandbox_manifest
            .sandbox_compatibility_tuple
            .sandbox_rootfs_artifact_sha256 = SANDBOX_DIGEST_B.to_owned();
        assert_eq!(
            Err(
                SandboxFirecrackerArtifactError::CompatibilityTupleDigestMismatch {
                    sandbox_artifact_role: "sandbox_rootfs".to_owned(),
                }
            ),
            sandbox_manifest.validate()
        );
    }

    #[test]
    fn unsupported_and_cross_architecture_tuples_fail_closed() {
        let mut sandbox_broken = sandbox_manifest();
        sandbox_broken.sandbox_supported_host_architecture = "windows-hyperv-x86_64".to_owned();
        assert!(matches!(
            sandbox_broken.validate(),
            Err(SandboxFirecrackerArtifactError::HostArchitectureUnsupported { .. })
        ));

        let mut sandbox_cross = sandbox_manifest();
        sandbox_cross
            .sandbox_compatibility_tuple
            .sandbox_supported_host_architecture = "linux-kvm-aarch64".to_owned();
        assert!(matches!(
            sandbox_cross.validate(),
            Err(SandboxFirecrackerArtifactError::HostArchitectureUnsupported { .. })
        ));
    }

    #[test]
    fn malformed_digests_fail_closed() {
        let mut sandbox_broken = sandbox_manifest();
        sandbox_broken.sandbox_artifact_manifest_digest = "not-a-digest".to_owned();
        assert!(matches!(
            sandbox_broken.validate(),
            Err(SandboxFirecrackerArtifactError::ArtifactDigestInvalid { .. })
        ));

        let mut sandbox_uppercase = sandbox_manifest();
        sandbox_uppercase.sandbox_artifacts[0].sandbox_artifact_sha256 =
            SANDBOX_DIGEST_A.to_ascii_uppercase();
        assert!(matches!(
            sandbox_uppercase.validate(),
            Err(SandboxFirecrackerArtifactError::ArtifactDigestInvalid { .. })
        ));
    }

    #[test]
    fn supported_architecture_vocabulary_is_the_gate_contract_vocabulary() {
        assert_eq!(
            ["linux-kvm-x86_64", "linux-kvm-aarch64"],
            SANDBOX_FIRECRACKER_SUPPORTED_ARCHITECTURES
        );
    }
}
