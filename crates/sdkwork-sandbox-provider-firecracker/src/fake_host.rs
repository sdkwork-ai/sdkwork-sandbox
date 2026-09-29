//! Test-only fake host: the fake broker, fake guest channel, and static facts
//! source the conformance and lifecycle tests drive. No real process, socket,
//! or device exists behind any of these seams.

use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Mutex,
};

use async_trait::async_trait;

use crate::broker::{
    SandboxFirecrackerBindingPrepared, SandboxFirecrackerBootVmmRequest,
    SandboxFirecrackerBrokerError, SandboxFirecrackerBrokerErrorKind,
    SandboxFirecrackerCleanupBindingRequest, SandboxFirecrackerHostIsolationBroker,
    SandboxFirecrackerPrepareBindingRequest, SandboxFirecrackerShutdownGuestRequest,
    SandboxFirecrackerTerminateVmmRequest, SandboxFirecrackerVmmBootOutcome,
};
use crate::guest_channel::{
    SandboxFirecrackerAdmittedCommand, SandboxFirecrackerCancellationHandle,
    SandboxFirecrackerGuestChannelError, SandboxFirecrackerGuestCommandChannel,
};
use crate::lifecycle::SandboxFirecrackerHostFactsSource;
use crate::preflight::SandboxFirecrackerHostFacts;
use sdkwork_sandbox_provider_spi::SandboxCommandOutcome;

/// A fake broker that verifies the boot sequence's resource claims and
/// records operation counts. Cleanup failures are injectable so the
/// quarantine path is testable.
#[derive(Default)]
pub struct SandboxFakeBroker {
    sandbox_prepare_calls: AtomicUsize,
    sandbox_boot_calls: AtomicUsize,
    sandbox_shutdown_calls: AtomicUsize,
    sandbox_terminate_calls: AtomicUsize,
    sandbox_cleanup_calls: AtomicUsize,
    /// When true, cleanup reports a quarantined failure instead of success.
    pub sandbox_cleanup_fails: AtomicBool,
    /// When false, the broker's resource/workspace enforcement claims come
    /// back false so the readiness gate must refuse to claim them.
    pub sandbox_enforcement_holds: AtomicBool,
    /// When false, the guest agent's authenticated readiness comes back
    /// false so `sandbox_provider_ready` must stay false.
    pub sandbox_guest_authenticates: AtomicBool,
    /// The last prepared tenant/binding pair, for sequence assertions.
    pub sandbox_last_binding: Mutex<Option<(String, String)>>,
}

impl SandboxFakeBroker {
    fn sandbox_record_prepare(&self, sandbox_request: &SandboxFirecrackerPrepareBindingRequest) {
        self.sandbox_prepare_calls.fetch_add(1, Ordering::SeqCst);
        let mut sandbox_last = self
            .sandbox_last_binding
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *sandbox_last = Some((
            sandbox_request.sandbox_tenant_id.clone(),
            sandbox_request.sandbox_runtime_binding_id.clone(),
        ));
    }

    /// How many prepare operations ran.
    pub fn sandbox_prepare_count(&self) -> usize {
        self.sandbox_prepare_calls.load(Ordering::SeqCst)
    }

    /// How many boot operations ran.
    pub fn sandbox_boot_count(&self) -> usize {
        self.sandbox_boot_calls.load(Ordering::SeqCst)
    }

    /// How many shutdown operations ran.
    pub fn sandbox_shutdown_count(&self) -> usize {
        self.sandbox_shutdown_calls.load(Ordering::SeqCst)
    }

    /// How many terminate operations ran.
    pub fn sandbox_terminate_count(&self) -> usize {
        self.sandbox_terminate_calls.load(Ordering::SeqCst)
    }

    /// How many cleanup operations ran.
    pub fn sandbox_cleanup_count(&self) -> usize {
        self.sandbox_cleanup_calls.load(Ordering::SeqCst)
    }
}

/// Builds the typed broker error for one operation.
fn sandbox_broker_error_for(
    sandbox_operation: crate::broker::SandboxFirecrackerBrokerOperation,
    sandbox_kind: SandboxFirecrackerBrokerErrorKind,
) -> SandboxFirecrackerBrokerError {
    SandboxFirecrackerBrokerError {
        sandbox_broker_operation: sandbox_operation,
        sandbox_broker_error_kind: sandbox_kind,
    }
}

#[async_trait]
impl SandboxFirecrackerHostIsolationBroker for SandboxFakeBroker {
    async fn sandbox_prepare_binding(
        &self,
        sandbox_request: &SandboxFirecrackerPrepareBindingRequest,
    ) -> Result<SandboxFirecrackerBindingPrepared, SandboxFirecrackerBrokerError> {
        self.sandbox_record_prepare(sandbox_request);
        let sandbox_enforcement = self.sandbox_enforcement_holds.load(Ordering::SeqCst);
        Ok(SandboxFirecrackerBindingPrepared {
            sandbox_binding_reference: SandboxProviderRefHelper::sandbox_ref("binding"),
            sandbox_network_enforced: sandbox_enforcement,
            sandbox_resource_enforced: sandbox_enforcement,
            sandbox_workspace_attached: sandbox_enforcement,
        })
    }

    async fn sandbox_boot_vmm(
        &self,
        _sandbox_request: &SandboxFirecrackerBootVmmRequest,
    ) -> Result<SandboxFirecrackerVmmBootOutcome, SandboxFirecrackerBrokerError> {
        self.sandbox_boot_calls.fetch_add(1, Ordering::SeqCst);
        Ok(SandboxFirecrackerVmmBootOutcome {
            sandbox_vmm_reference: SandboxProviderRefHelper::sandbox_ref("vmm"),
            sandbox_guest_agent_authenticated: self
                .sandbox_guest_authenticates
                .load(Ordering::SeqCst),
        })
    }

    async fn sandbox_shutdown_guest(
        &self,
        _sandbox_request: &SandboxFirecrackerShutdownGuestRequest,
    ) -> Result<(), SandboxFirecrackerBrokerError> {
        self.sandbox_shutdown_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn sandbox_terminate_vmm(
        &self,
        _sandbox_request: &SandboxFirecrackerTerminateVmmRequest,
    ) -> Result<(), SandboxFirecrackerBrokerError> {
        self.sandbox_terminate_calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn sandbox_cleanup_binding(
        &self,
        _sandbox_request: &SandboxFirecrackerCleanupBindingRequest,
    ) -> Result<(), SandboxFirecrackerBrokerError> {
        self.sandbox_cleanup_calls.fetch_add(1, Ordering::SeqCst);
        if self.sandbox_cleanup_fails.load(Ordering::SeqCst) {
            return Err(sandbox_broker_error_for(
                crate::broker::SandboxFirecrackerBrokerOperation::CleanupBinding,
                SandboxFirecrackerBrokerErrorKind::Quarantined,
            ));
        }
        Ok(())
    }
}

/// Builds opaque provider-private references for the fake seams.
pub struct SandboxProviderRefHelper;

impl SandboxProviderRefHelper {
    /// Builds an opaque reference with the given label; never a host path.
    ///
    /// # Panics
    ///
    /// Never: the label is a fixed literal shape in tests.
    pub fn sandbox_ref(
        sandbox_label: &str,
    ) -> sdkwork_sandbox_provider_spi::SandboxProviderAllocationRef {
        sdkwork_sandbox_provider_spi::SandboxProviderAllocationRef::new(format!(
            "fake:{sandbox_label}:{}",
            uuid::Uuid::new_v4()
        ))
        .expect("fake reference shape is fixed and valid")
    }
}

/// A fake guest channel: the allowlisted fast executables answer immediately
/// with the argv joined by spaces; the slow executable runs until cancelled
/// or timed out, so the timeout and cancellation scenarios exercise the
/// executor's bounds the way a real long-running guest command would.
pub struct SandboxFakeGuestChannel {
    sandbox_slow_executable: String,
}

impl SandboxFakeGuestChannel {
    /// Builds the channel with the one executable name that never settles on
    /// its own.
    pub fn new(sandbox_slow_executable: &str) -> Self {
        Self {
            sandbox_slow_executable: sandbox_slow_executable.to_owned(),
        }
    }
}

#[async_trait]
impl SandboxFirecrackerGuestCommandChannel for SandboxFakeGuestChannel {
    async fn sandbox_execute_admitted(
        &self,
        sandbox_command: &SandboxFirecrackerAdmittedCommand,
        sandbox_cancellation: &SandboxFirecrackerCancellationHandle,
    ) -> Result<SandboxCommandOutcome, SandboxFirecrackerGuestChannelError> {
        if sandbox_command.sandbox_executable == self.sandbox_slow_executable {
            sandbox_cancellation.sandbox_wait().await;
            return Ok(SandboxCommandOutcome {
                sandbox_exit_code: None,
                sandbox_stdout: Vec::new(),
                sandbox_stderr: Vec::new(),
                sandbox_stdout_truncated: false,
                sandbox_stderr_truncated: false,
            });
        }
        let sandbox_stdout = sandbox_command.sandbox_arguments.join(" ");
        Ok(SandboxCommandOutcome {
            sandbox_exit_code: Some(0),
            sandbox_stdout: sandbox_stdout.into_bytes(),
            sandbox_stderr: Vec::new(),
            sandbox_stdout_truncated: false,
            sandbox_stderr_truncated: false,
        })
    }
}

/// A static facts source over one snapshot.
pub struct SandboxStaticFactsSource {
    sandbox_facts: SandboxFirecrackerHostFacts,
}

impl SandboxStaticFactsSource {
    /// Wraps one facts snapshot.
    pub fn new(sandbox_facts: SandboxFirecrackerHostFacts) -> Self {
        Self { sandbox_facts }
    }
}

#[async_trait]
impl SandboxFirecrackerHostFactsSource for SandboxStaticFactsSource {
    async fn sandbox_host_facts(&self) -> SandboxFirecrackerHostFacts {
        self.sandbox_facts.clone()
    }
}

/// The ready-facts template the tests specialize.
pub fn sandbox_ready_facts() -> SandboxFirecrackerHostFacts {
    SandboxFirecrackerHostFacts {
        sandbox_host_platform: crate::preflight::SandboxFirecrackerHostPlatform::LinuxKvmX86_64,
        sandbox_kvm_device_available: true,
        sandbox_cgroup_v2_writable: true,
        sandbox_jailer_artifact_verified: true,
        sandbox_runtime_data_root_secured: true,
        sandbox_host_isolation_broker_available: true,
        sandbox_workspace_attachment_available: true,
        sandbox_network_policy_available: true,
        sandbox_resource_policy_available: true,
        sandbox_guest_channel_available: true,
        sandbox_artifact_manifest: None,
    }
}
