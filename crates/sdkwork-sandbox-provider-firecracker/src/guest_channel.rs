//! The authenticated guest command channel seam (`REQ-2026-0008`).
//!
//! Host-to-guest command control runs over a private, one-time-per-boot
//! authenticated channel (the vsock/first-class guest agent lane). This module
//! is the provider-side port: one admitted command in, one bounded terminal
//! outcome out, plus the cancellation handle the executor's fenced cancel
//! path signals. The real vsock transport and the guest agent protocol land
//! with the guest-agent slice; the fake host proves the executor semantics.

use std::fmt;

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{SandboxCommandExecutionError, SandboxCommandOutcome};
use tokio::sync::Notify;

/// One admitted command handed to the guest channel after the admission
/// sequence passed. The channel never sees the request envelope again: it
/// receives exactly the provider-validated execution shape.
#[derive(Clone, Debug)]
pub struct SandboxFirecrackerAdmittedCommand {
    /// Allowlisted bare executable name, resolved inside the pinned rootfs.
    pub sandbox_executable: String,
    /// Argument vector, order preserved.
    pub sandbox_arguments: Vec<String>,
    /// Logical working directory relative to the Workspace root.
    pub sandbox_working_directory: String,
    /// Environment additions that passed the boundary rules.
    pub sandbox_environment: std::collections::BTreeMap<String, String>,
    /// The hard bounds the guest must enforce.
    pub sandbox_command_limits: sdkwork_sandbox_provider_spi::SandboxCommandLimits,
}

/// Why a guest channel operation failed at the channel level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxFirecrackerGuestChannelError {
    /// The channel is temporarily unable to carry the execution.
    Unavailable,
    /// The guest failed on an internal invariant (protocol violation, agent
    /// crash).
    InternalFailure,
    /// The channel could not verify the guest's authenticated identity.
    GuestAuthenticationMissing,
}

impl fmt::Display for SandboxFirecrackerGuestChannelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sandbox_message = match self {
            Self::Unavailable => "the sandbox guest channel is unavailable",
            Self::InternalFailure => "the sandbox guest channel hit an internal failure",
            Self::GuestAuthenticationMissing => {
                "the sandbox guest channel could not authenticate the guest"
            }
        };
        f.write_str(sandbox_message)
    }
}

impl std::error::Error for SandboxFirecrackerGuestChannelError {}

/// The cancellation handle published in the live registry. The executor's
/// fenced cancel path signals it; the channel implementation observes it and
/// terminates the guest-side execution (the real slice maps the signal to the
/// guest agent's cancellation message and descendant cleanup).
#[derive(Clone, Default)]
pub struct SandboxFirecrackerCancellationHandle {
    sandbox_notify: std::sync::Arc<Notify>,
}

impl SandboxFirecrackerCancellationHandle {
    /// Builds a fresh, unsignalled handle.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Signals cancellation; idempotent and safe after completion. The
    /// notification stores a permit, so a cancellation arriving before the
    /// channel first polls its wait is still observed there.
    pub fn sandbox_cancel(&self) {
        self.sandbox_notify.notify_one();
    }

    /// Waits until the handle is signalled.
    pub async fn sandbox_wait(&self) {
        self.sandbox_notify.notified().await;
    }
}

impl fmt::Debug for SandboxFirecrackerCancellationHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SandboxFirecrackerCancellationHandle")
    }
}

/// The guest command channel port. The channel owns the guest transport and
/// the guest-side bounded-output capture; it reports the terminal outcome or
/// the typed channel failure.
#[async_trait]
pub trait SandboxFirecrackerGuestCommandChannel: Send + Sync {
    /// Runs one admitted command inside the guest to a terminal outcome under
    /// the declared hard bounds, observing the cancellation handle.
    ///
    /// # Errors
    ///
    /// Returns the typed channel error for transport-level failures; the
    /// executor maps them onto the contract's error families.
    async fn sandbox_execute_admitted(
        &self,
        sandbox_command: &SandboxFirecrackerAdmittedCommand,
        sandbox_cancellation: &SandboxFirecrackerCancellationHandle,
    ) -> Result<SandboxCommandOutcome, SandboxFirecrackerGuestChannelError>;
}

/// Maps a channel failure onto the contract's typed execution error family.
/// The mapping is total and loses no channel variant; the executor uses it so
/// no channel-specific error can leak past the port.
#[must_use]
pub const fn sandbox_map_guest_channel_error(
    sandbox_error: &SandboxFirecrackerGuestChannelError,
) -> SandboxCommandExecutionError {
    match sandbox_error {
        SandboxFirecrackerGuestChannelError::Unavailable => {
            SandboxCommandExecutionError::ProviderUnavailable
        }
        SandboxFirecrackerGuestChannelError::InternalFailure => {
            SandboxCommandExecutionError::InternalFailure
        }
        SandboxFirecrackerGuestChannelError::GuestAuthenticationMissing => {
            SandboxCommandExecutionError::ProviderUnavailable
        }
    }
}
