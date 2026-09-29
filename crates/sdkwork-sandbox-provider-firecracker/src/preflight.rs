//! Node preflight for the Firecracker provider (`REQ-2026-0008`).
//!
//! Preflight is the fail-closed gate between "a Firecracker provider object
//! exists" and "this node may boot a microVM": Linux with KVM, a writable
//! cgroup v2 hierarchy, a verified artifact tuple, a secured runtime data
//! root, and every policy/broker integration the boot sequence depends on.
//! The facts come from the composition layer through
//! [`SandboxFirecrackerHostFacts`]; this module performs no host I/O, so the
//! fail-closed decisions are unit-testable on every platform the crate
//! compiles for. A missing condition reports Degraded or Unavailable and never
//! downgrades the MicroVm assurance.

use std::fmt;
use std::sync::Arc;

use crate::artifact::SandboxFirecrackerArtifactManifest;

/// The host platform facts as the composition layer observed them. The
/// vocabulary matches the delivery-gates contract's
/// `sandbox_supported_host_platforms` for the Firecracker provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerHostFacts {
    /// The observed host platform; `Unsupported` covers every non-Linux-KVM
    /// host, including Windows, macOS, WSL, and Linux without `/dev/kvm`.
    pub sandbox_host_platform: SandboxFirecrackerHostPlatform,
    /// `/dev/kvm` is available to the provider's unprivileged runtime user.
    pub sandbox_kvm_device_available: bool,
    /// A writable cgroup v2 hierarchy exists for per-binding enforcement
    /// (`REQ-2026-0015`).
    pub sandbox_cgroup_v2_writable: bool,
    /// The materialized Firecracker/Jailer binaries matched the manifest
    /// digests and signature policy (`REQ-2026-0012`).
    pub sandbox_jailer_artifact_verified: bool,
    /// The provider-private runtime data root is secured (permissions,
    /// source-checkout separation) per the runtime directory rules.
    pub sandbox_runtime_data_root_secured: bool,
    /// The minimal-privilege host isolation broker is enrolled and callable
    /// (`REQ-2026-0011`).
    pub sandbox_host_isolation_broker_available: bool,
    /// The workspace block-device attachment port is available
    /// (`REQ-2026-0013`).
    pub sandbox_workspace_attachment_available: bool,
    /// The network isolation policy/mechanism pair is available
    /// (`REQ-2026-0014`).
    pub sandbox_network_policy_available: bool,
    /// The resource isolation policy/mechanism pair is available
    /// (`REQ-2026-0015`).
    pub sandbox_resource_policy_available: bool,
    /// The authenticated guest vsock channel is available for command and
    /// readiness traffic (`REQ-2026-0007`/`REQ-2026-0008`).
    pub sandbox_guest_channel_available: bool,
    /// The release-published artifact manifest this node consumes.
    pub sandbox_artifact_manifest: Option<Arc<SandboxFirecrackerArtifactManifest>>,
}

/// The host platform a Firecracker node may report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxFirecrackerHostPlatform {
    /// Linux KVM on x86_64.
    LinuxKvmX86_64,
    /// Linux KVM on aarch64.
    LinuxKvmAarch64,
    /// Any host that cannot back a microVM. The provider reports Unavailable
    /// and never falls back to a weaker provider.
    Unsupported,
}

impl SandboxFirecrackerHostPlatform {
    /// The delivery-gates vocabulary for this platform.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinuxKvmX86_64 => "linux-kvm-x86_64",
            Self::LinuxKvmAarch64 => "linux-kvm-aarch64",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Why a preflight condition failed, in the delivery-gates vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxFirecrackerPreflightReason {
    /// The host platform cannot back a microVM.
    HostPlatformUnsupported,
    /// `/dev/kvm` is missing or inaccessible.
    KvmDeviceUnavailable,
    /// No writable cgroup v2 hierarchy.
    CgroupV2Unavailable,
    /// The Firecracker/Jailer materialization failed verification.
    JailerArtifactUnverified,
    /// The runtime data root is missing or insecure.
    RuntimeDataRootInsecure,
    /// The host isolation broker is not enrolled.
    HostIsolationBrokerUnavailable,
    /// The workspace attachment port is not available.
    WorkspaceAttachmentUnavailable,
    /// The network policy/mechanism pair is not available.
    NetworkPolicyUnavailable,
    /// The resource policy/mechanism pair is not available.
    ResourcePolicyUnavailable,
    /// The guest channel is not available.
    GuestChannelUnavailable,
    /// No artifact manifest was handed over, or the manifest failed the
    /// exact-tuple validation.
    ArtifactManifestInvalid,
}

impl fmt::Display for SandboxFirecrackerPreflightReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sandbox_message = match self {
            Self::HostPlatformUnsupported => "host platform cannot back a microVM",
            Self::KvmDeviceUnavailable => "the kvm device is unavailable",
            Self::CgroupV2Unavailable => "a writable cgroup v2 hierarchy is unavailable",
            Self::JailerArtifactUnverified => "the jailer artifact failed verification",
            Self::RuntimeDataRootInsecure => "the runtime data root is missing or insecure",
            Self::HostIsolationBrokerUnavailable => "the host isolation broker is not enrolled",
            Self::WorkspaceAttachmentUnavailable => "the workspace attachment port is unavailable",
            Self::NetworkPolicyUnavailable => "the network isolation policy pair is unavailable",
            Self::ResourcePolicyUnavailable => "the resource isolation policy pair is unavailable",
            Self::GuestChannelUnavailable => "the guest channel is unavailable",
            Self::ArtifactManifestInvalid => {
                "the artifact manifest is missing or fails the exact-tuple contract"
            }
        };
        f.write_str(sandbox_message)
    }
}

/// The preflight verdict. `missing_preflight_is_ready` is false by contract:
/// only [`SandboxFirecrackerPreflightStatus::Ready`] allows a start sequence,
/// and neither verdict downgrades the MicroVm assurance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxFirecrackerPreflightStatus {
    /// Every condition holds; the node may boot microVMs.
    Ready,
    /// The platform core holds but one or more policy integrations are
    /// missing; the node may not claim the corresponding readiness fields.
    Degraded,
    /// A platform-core condition failed; the node may not boot microVMs.
    Unavailable,
}

impl SandboxFirecrackerPreflightStatus {
    /// Whether a start sequence may proceed.
    #[must_use]
    pub const fn sandbox_preflight_allows_start(self) -> bool {
        matches!(self, Self::Ready)
    }
}

/// One preflight run's report: the verdict plus every reason that contributed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerPreflightReport {
    sandbox_status: SandboxFirecrackerPreflightStatus,
    sandbox_reasons: Vec<SandboxFirecrackerPreflightReason>,
}

impl SandboxFirecrackerPreflightReport {
    /// The verdict.
    #[must_use]
    pub const fn sandbox_status(&self) -> SandboxFirecrackerPreflightStatus {
        self.sandbox_status
    }

    /// Every failed condition, in declaration order.
    #[must_use]
    pub fn sandbox_reasons(&self) -> &[SandboxFirecrackerPreflightReason] {
        &self.sandbox_reasons
    }

    /// Whether the node may boot microVMs right now.
    #[must_use]
    pub const fn sandbox_preflight_allows_start(&self) -> bool {
        self.sandbox_status.sandbox_preflight_allows_start()
    }
}

/// Runs the fail-closed preflight over one facts snapshot. The report lists
/// every failed condition (not just the first), so a degraded node's operator
/// sees the whole repair list at once.
#[must_use]
pub fn sandbox_run_firecracker_preflight(
    sandbox_facts: &SandboxFirecrackerHostFacts,
) -> SandboxFirecrackerPreflightReport {
    let mut sandbox_reasons = Vec::new();
    if matches!(
        sandbox_facts.sandbox_host_platform,
        SandboxFirecrackerHostPlatform::Unsupported
    ) {
        sandbox_reasons.push(SandboxFirecrackerPreflightReason::HostPlatformUnsupported);
    }
    if !sandbox_facts.sandbox_kvm_device_available {
        sandbox_reasons.push(SandboxFirecrackerPreflightReason::KvmDeviceUnavailable);
    }
    if !sandbox_facts.sandbox_cgroup_v2_writable {
        sandbox_reasons.push(SandboxFirecrackerPreflightReason::CgroupV2Unavailable);
    }
    if !sandbox_facts.sandbox_jailer_artifact_verified {
        sandbox_reasons.push(SandboxFirecrackerPreflightReason::JailerArtifactUnverified);
    }
    if !sandbox_facts.sandbox_runtime_data_root_secured {
        sandbox_reasons.push(SandboxFirecrackerPreflightReason::RuntimeDataRootInsecure);
    }
    if !sandbox_facts.sandbox_host_isolation_broker_available {
        sandbox_reasons.push(SandboxFirecrackerPreflightReason::HostIsolationBrokerUnavailable);
    }
    let sandbox_manifest_valid = sandbox_facts
        .sandbox_artifact_manifest
        .as_ref()
        .is_some_and(|sandbox_manifest| sandbox_manifest.validate().is_ok());
    if !sandbox_manifest_valid {
        sandbox_reasons.push(SandboxFirecrackerPreflightReason::ArtifactManifestInvalid);
    }

    if !sandbox_reasons.is_empty() {
        return SandboxFirecrackerPreflightReport {
            sandbox_status: SandboxFirecrackerPreflightStatus::Unavailable,
            sandbox_reasons,
        };
    }

    // The platform core holds; policy integrations degrade instead of
    // blocking, and each missing port maps to one readiness field the start
    // sequence must refuse to claim.
    let sandbox_degraded_conditions = [
        (
            sandbox_facts.sandbox_workspace_attachment_available,
            SandboxFirecrackerPreflightReason::WorkspaceAttachmentUnavailable,
        ),
        (
            sandbox_facts.sandbox_network_policy_available,
            SandboxFirecrackerPreflightReason::NetworkPolicyUnavailable,
        ),
        (
            sandbox_facts.sandbox_resource_policy_available,
            SandboxFirecrackerPreflightReason::ResourcePolicyUnavailable,
        ),
        (
            sandbox_facts.sandbox_guest_channel_available,
            SandboxFirecrackerPreflightReason::GuestChannelUnavailable,
        ),
    ];
    for (sandbox_available, sandbox_reason) in sandbox_degraded_conditions {
        if !sandbox_available {
            sandbox_reasons.push(sandbox_reason);
        }
    }
    let sandbox_status = if sandbox_reasons.is_empty() {
        SandboxFirecrackerPreflightStatus::Ready
    } else {
        SandboxFirecrackerPreflightStatus::Degraded
    };
    SandboxFirecrackerPreflightReport {
        sandbox_status,
        sandbox_reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        sandbox_run_firecracker_preflight, SandboxFirecrackerHostFacts,
        SandboxFirecrackerHostPlatform, SandboxFirecrackerPreflightReason,
        SandboxFirecrackerPreflightStatus,
    };
    use crate::artifact::{
        SandboxFirecrackerArtifactDescriptor, SandboxFirecrackerArtifactManifest,
        SandboxFirecrackerCompatibilityTuple,
    };

    const SANDBOX_DIGEST_A: &str =
        "aaaa000000000000000000000000000000000000000000000000000000000000";
    const SANDBOX_DIGEST_B: &str =
        "bbbb000000000000000000000000000000000000000000000000000000000000";

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
                ["sandbox_firecracker", SANDBOX_DIGEST_A],
                ["sandbox_jailer", SANDBOX_DIGEST_A],
                ["sandbox_guest_kernel", SANDBOX_DIGEST_A],
                ["sandbox_rootfs", SANDBOX_DIGEST_A],
                ["sandbox_guest_agent", SANDBOX_DIGEST_B],
            ]
            .into_iter()
            .map(
                |[sandbox_role, sandbox_digest]| SandboxFirecrackerArtifactDescriptor {
                    sandbox_artifact_role: sandbox_role.to_owned(),
                    sandbox_artifact_version: "1.0.0".to_owned(),
                    sandbox_artifact_sha256: sandbox_digest.to_owned(),
                    sandbox_artifact_release_ref: "release-ref".to_owned(),
                    sandbox_artifact_signature_ref: "signature-ref".to_owned(),
                    sandbox_artifact_sbom_ref: "sbom-ref".to_owned(),
                    sandbox_artifact_provenance_ref: "provenance-ref".to_owned(),
                },
            )
            .collect(),
            sandbox_evidence_bundle_ref: "evidence-ref".to_owned(),
            sandbox_signature_policy_ref: "signature-policy-ref".to_owned(),
            sandbox_revocation_policy_ref: "revocation-policy-ref".to_owned(),
            sandbox_published_at: "2026-09-24T00:00:00Z".to_owned(),
        }
    }

    fn sandbox_ready_facts() -> SandboxFirecrackerHostFacts {
        SandboxFirecrackerHostFacts {
            sandbox_host_platform: SandboxFirecrackerHostPlatform::LinuxKvmX86_64,
            sandbox_kvm_device_available: true,
            sandbox_cgroup_v2_writable: true,
            sandbox_jailer_artifact_verified: true,
            sandbox_runtime_data_root_secured: true,
            sandbox_host_isolation_broker_available: true,
            sandbox_workspace_attachment_available: true,
            sandbox_network_policy_available: true,
            sandbox_resource_policy_available: true,
            sandbox_guest_channel_available: true,
            sandbox_artifact_manifest: Some(std::sync::Arc::new(sandbox_manifest())),
        }
    }

    #[test]
    fn complete_facts_report_ready() {
        let sandbox_report = sandbox_run_firecracker_preflight(&sandbox_ready_facts());
        assert_eq!(
            SandboxFirecrackerPreflightStatus::Ready,
            sandbox_report.sandbox_status()
        );
        assert!(sandbox_report.sandbox_reasons().is_empty());
        assert!(sandbox_report.sandbox_preflight_allows_start());
    }

    #[test]
    fn a_windows_dev_host_reports_unavailable_with_every_core_reason() {
        let mut sandbox_facts = sandbox_ready_facts();
        sandbox_facts.sandbox_host_platform = SandboxFirecrackerHostPlatform::Unsupported;
        sandbox_facts.sandbox_kvm_device_available = false;
        sandbox_facts.sandbox_cgroup_v2_writable = false;
        sandbox_facts.sandbox_jailer_artifact_verified = false;
        sandbox_facts.sandbox_runtime_data_root_secured = false;
        sandbox_facts.sandbox_host_isolation_broker_available = false;
        sandbox_facts.sandbox_artifact_manifest = None;
        let sandbox_report = sandbox_run_firecracker_preflight(&sandbox_facts);
        assert_eq!(
            SandboxFirecrackerPreflightStatus::Unavailable,
            sandbox_report.sandbox_status()
        );
        // The report lists every core failure, not just the first, so the
        // repair list is complete.
        assert_eq!(7, sandbox_report.sandbox_reasons().len());
        assert!(!sandbox_report.sandbox_preflight_allows_start());
    }

    #[test]
    fn missing_policy_integrations_degrade_without_touching_the_core() {
        let mut sandbox_facts = sandbox_ready_facts();
        sandbox_facts.sandbox_workspace_attachment_available = false;
        sandbox_facts.sandbox_network_policy_available = false;
        sandbox_facts.sandbox_resource_policy_available = false;
        sandbox_facts.sandbox_guest_channel_available = false;
        let sandbox_report = sandbox_run_firecracker_preflight(&sandbox_facts);
        assert_eq!(
            SandboxFirecrackerPreflightStatus::Degraded,
            sandbox_report.sandbox_status()
        );
        assert_eq!(
            vec![
                SandboxFirecrackerPreflightReason::WorkspaceAttachmentUnavailable,
                SandboxFirecrackerPreflightReason::NetworkPolicyUnavailable,
                SandboxFirecrackerPreflightReason::ResourcePolicyUnavailable,
                SandboxFirecrackerPreflightReason::GuestChannelUnavailable,
            ],
            sandbox_report.sandbox_reasons()
        );
    }

    #[test]
    fn an_invalid_manifest_is_a_core_failure() {
        let mut sandbox_facts = sandbox_ready_facts();
        let mut sandbox_broken_manifest = sandbox_manifest();
        sandbox_broken_manifest
            .sandbox_compatibility_tuple
            .sandbox_jailer_artifact_sha256 = SANDBOX_DIGEST_B.to_owned();
        sandbox_facts.sandbox_artifact_manifest =
            Some(std::sync::Arc::new(sandbox_broken_manifest));
        let sandbox_report = sandbox_run_firecracker_preflight(&sandbox_facts);
        assert_eq!(
            SandboxFirecrackerPreflightStatus::Unavailable,
            sandbox_report.sandbox_status()
        );
        assert!(sandbox_report
            .sandbox_reasons()
            .contains(&SandboxFirecrackerPreflightReason::ArtifactManifestInvalid));
    }
}
