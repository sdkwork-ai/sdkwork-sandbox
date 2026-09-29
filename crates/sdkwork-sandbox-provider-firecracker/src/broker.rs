//! The host isolation broker seam (`REQ-2026-0011`).
//!
//! The provider adapter itself never holds root, sudo, or arbitrary-command
//! authority: the jailer root, cgroup, network namespace, tap, and workspace
//! block-device preparation that need host privilege run through the audited
//! minimal host isolation broker, which accepts fixed operations over opaque
//! references only - never a shell command or a caller-chosen path. This
//! module is the provider side of that boundary: the typed operations the
//! boot sequence needs and the port the broker adapter implements. The broker
//! contract (`specs/sandbox-host-isolation-broker.contract.json`) is still
//! draft, so no real broker ships in this slice; the lifecycle drives the
//! port and the fake host proves the sequence.

use std::fmt;

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::SandboxProviderAllocationRef;

/// The fixed operations the boot sequence drives through the broker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxFirecrackerBrokerOperation {
    /// Prepare one binding's jailer root, cgroup, network namespace, tap, and
    /// workspace block device.
    PrepareBinding,
    /// Boot the microVM under the prepared jailer target.
    BootVmm,
    /// Bounded guest shutdown.
    ShutdownGuest,
    /// Terminate the VMM process.
    TerminateVmm,
    /// Idempotent cleanup of every prepared resource for the binding.
    CleanupBinding,
}

impl fmt::Display for SandboxFirecrackerBrokerOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sandbox_message = match self {
            Self::PrepareBinding => "prepare-binding",
            Self::BootVmm => "boot-vmm",
            Self::ShutdownGuest => "shutdown-guest",
            Self::TerminateVmm => "terminate-vmm",
            Self::CleanupBinding => "cleanup-binding",
        };
        f.write_str(sandbox_message)
    }
}

/// How a broker operation failed. `Quarantined` marks an operation whose
/// cleanup state is unknown or partial: the binding must stay visible to the
/// composition layer until a later cleanup succeeds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxFirecrackerBrokerErrorKind {
    /// The broker or the host resource is temporarily unavailable.
    Unavailable,
    /// The operation ended in an unverifiable state; the binding quarantines.
    Quarantined,
    /// The broker refused the request as malformed or unauthorized.
    Rejected,
}

/// A broker operation failure: operation kind plus failure kind, with no
/// host path, socket, or identity payload (the safe-error redaction rule).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerBrokerError {
    /// The operation that failed.
    pub sandbox_broker_operation: SandboxFirecrackerBrokerOperation,
    /// How it failed.
    pub sandbox_broker_error_kind: SandboxFirecrackerBrokerErrorKind,
}

impl fmt::Display for SandboxFirecrackerBrokerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "sandbox broker operation {} failed with {:?}",
            self.sandbox_broker_operation, self.sandbox_broker_error_kind
        )
    }
}

impl std::error::Error for SandboxFirecrackerBrokerError {}

/// The request to prepare one binding's host-side isolation resources. Every
/// field is an opaque provider-side identifier; the broker resolves its own
/// verified internal identities from its enrollment, never from request data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerPrepareBindingRequest {
    /// The tenant the binding belongs to (for broker-side accounting only).
    pub sandbox_tenant_id: String,
    /// The runtime binding the resources are prepared for.
    pub sandbox_runtime_binding_id: String,
    /// The provider-private allocation reference binding the resources to one
    /// allocation.
    pub sandbox_allocation_reference: SandboxProviderAllocationRef,
    /// The workspace the binding runs inside; the broker maps it to an
    /// encrypted guest block device through the attachment adapter
    /// (`REQ-2026-0013`), never to a host directory mount.
    pub sandbox_workspace_id: String,
}

/// The broker's report for one prepared binding. Each boolean is the broker's
/// verified enforcement claim; the start sequence refuses to claim the
/// corresponding readiness field when it is false.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerBindingPrepared {
    /// The opaque broker-side reference for the prepared binding; every later
    /// operation for this binding carries it.
    pub sandbox_binding_reference: SandboxProviderAllocationRef,
    /// Network namespace/tap enforcement verified (`REQ-2026-0014`).
    pub sandbox_network_enforced: bool,
    /// Machine config plus per-binding cgroup v2 enforcement verified
    /// (`REQ-2026-0015`).
    pub sandbox_resource_enforced: bool,
    /// Workspace block device attached and verified (`REQ-2026-0013`).
    pub sandbox_workspace_attached: bool,
}

/// The request to boot the VMM for one prepared binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerBootVmmRequest {
    /// The prepared binding reference from [`SandboxFirecrackerBindingPrepared`].
    pub sandbox_binding_reference: SandboxProviderAllocationRef,
    /// The tenant the boot is scoped to.
    pub sandbox_tenant_id: String,
    /// The runtime binding the boot belongs to.
    pub sandbox_runtime_binding_id: String,
}

/// The broker's report for one booted VMM.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerVmmBootOutcome {
    /// The opaque broker-side VMM reference.
    pub sandbox_vmm_reference: SandboxProviderAllocationRef,
    /// The guest agent completed its one-time authenticated readiness handshake
    /// over the private channel. This is authenticated readiness only; it is
    /// not guest hardware attestation (`REQ-2026-0017` owns node attestation).
    pub sandbox_guest_agent_authenticated: bool,
}

/// The request to shut the guest down in a bounded way before termination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerShutdownGuestRequest {
    /// The prepared binding reference.
    pub sandbox_binding_reference: SandboxProviderAllocationRef,
    /// The VMM reference from the boot outcome, when the boot reached that
    /// stage; a stop before boot has none.
    pub sandbox_vmm_reference: Option<SandboxProviderAllocationRef>,
}

/// The request to terminate the VMM process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerTerminateVmmRequest {
    /// The prepared binding reference.
    pub sandbox_binding_reference: SandboxProviderAllocationRef,
    /// The VMM reference from the boot outcome, when the boot reached that
    /// stage; a stop before boot has none.
    pub sandbox_vmm_reference: Option<SandboxProviderAllocationRef>,
}

/// The request to clean one binding's host-side resources. Cleanup must be
/// idempotent: repeating a completed cleanup succeeds. Agents-owned workspace
/// content is never cleanup territory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerCleanupBindingRequest {
    /// The prepared binding reference.
    pub sandbox_binding_reference: SandboxProviderAllocationRef,
    /// The VMM reference, when the boot reached that stage.
    pub sandbox_vmm_reference: Option<SandboxProviderAllocationRef>,
}

/// The minimal host isolation broker port (`REQ-2026-0011`). The real broker
/// adapter is its own reviewed component; the provider only drives this port
/// and never performs the privileged operations itself.
#[async_trait]
pub trait SandboxFirecrackerHostIsolationBroker: Send + Sync {
    /// Prepares one binding's host-side isolation resources.
    ///
    /// # Errors
    ///
    /// Returns the typed broker error for unavailable, quarantined, or
    /// rejected outcomes.
    async fn sandbox_prepare_binding(
        &self,
        sandbox_request: &SandboxFirecrackerPrepareBindingRequest,
    ) -> Result<SandboxFirecrackerBindingPrepared, SandboxFirecrackerBrokerError>;

    /// Boots the VMM for one prepared binding.
    ///
    /// # Errors
    ///
    /// Returns the typed broker error for unavailable, quarantined, or
    /// rejected outcomes.
    async fn sandbox_boot_vmm(
        &self,
        sandbox_request: &SandboxFirecrackerBootVmmRequest,
    ) -> Result<SandboxFirecrackerVmmBootOutcome, SandboxFirecrackerBrokerError>;

    /// Shuts the guest down within a bounded budget. A guest that misses the
    /// budget is not an error here; termination is the next operation.
    ///
    /// # Errors
    ///
    /// Returns the typed broker error for unavailable or rejected outcomes.
    async fn sandbox_shutdown_guest(
        &self,
        sandbox_request: &SandboxFirecrackerShutdownGuestRequest,
    ) -> Result<(), SandboxFirecrackerBrokerError>;

    /// Terminates the VMM process.
    ///
    /// # Errors
    ///
    /// Returns the typed broker error for unavailable, quarantined, or
    /// rejected outcomes.
    async fn sandbox_terminate_vmm(
        &self,
        sandbox_request: &SandboxFirecrackerTerminateVmmRequest,
    ) -> Result<(), SandboxFirecrackerBrokerError>;

    /// Idempotently cleans every host-side resource prepared for the binding.
    ///
    /// # Errors
    ///
    /// Returns a [`Quarantined`](SandboxFirecrackerBrokerErrorKind::Quarantined)
    /// error when the cleanup state cannot be verified; the binding quarantines
    /// until a later cleanup succeeds.
    async fn sandbox_cleanup_binding(
        &self,
        sandbox_request: &SandboxFirecrackerCleanupBindingRequest,
    ) -> Result<(), SandboxFirecrackerBrokerError>;
}
