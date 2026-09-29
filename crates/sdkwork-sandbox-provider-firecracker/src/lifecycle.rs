//! The Firecracker provider lifecycle (`REQ-2026-0008`).
//!
//! [`SandboxFirecrackerProvider`] implements the provider-neutral
//! [`SandboxProvider`] port over the fencing store and the host isolation
//! broker seam. The sequence per operation is always: fenced authority check
//! against the provider-private store first (a stale token never reaches a
//! side effect), then the bounded per-binding registry, then the broker.
//! Readiness is earned, never defaulted: `sandbox_provider_ready` requires
//! VMM running plus guest-agent authenticated readiness, and the policy and
//! workspace fields come from the broker's verified enforcement claims.
//! Snapshot/Restore/Warm-Pool stay unclaimed in the first-version descriptor.

use std::collections::{BTreeSet, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    IsolationAssurance, RuntimeCapability, SandboxProvider, SandboxProviderAllocation,
    SandboxProviderAllocationRef, SandboxProviderAllocationRequest, SandboxProviderDescriptor,
    SandboxProviderDestroyRequest, SandboxProviderError, SandboxProviderErrorKind,
    SandboxProviderHealth, SandboxProviderHealthStatus, SandboxProviderId, SandboxProviderKind,
    SandboxProviderOperation, SandboxProviderReadiness, SandboxProviderResult,
    SandboxProviderStartRequest, SandboxProviderStopRequest,
};
use tokio::time::timeout;
use uuid::Uuid;

use crate::broker::{
    SandboxFirecrackerBootVmmRequest, SandboxFirecrackerBrokerError,
    SandboxFirecrackerBrokerErrorKind, SandboxFirecrackerCleanupBindingRequest,
    SandboxFirecrackerHostIsolationBroker, SandboxFirecrackerPrepareBindingRequest,
    SandboxFirecrackerShutdownGuestRequest, SandboxFirecrackerTerminateVmmRequest,
};
use crate::fencing::{
    sandbox_check_fencing_authority, SandboxFirecrackerFencingAuthorityError,
    SandboxFirecrackerFencingStore,
};
use crate::preflight::{
    sandbox_run_firecracker_preflight, SandboxFirecrackerHostFacts,
    SandboxFirecrackerPreflightStatus,
};

/// Upper bound on concurrently tracked allocations node-wide. At capacity the
/// provider refuses new allocations as Unavailable instead of growing the
/// registry without limit; destroy retires allocations and frees slots.
const MAX_SANDBOX_ACTIVE_ALLOCATIONS: usize = 1024;

/// The lifecycle budgets the provider applies around every external broker
/// await (every external await has a timeout). The defaults are the Gate 0
/// values; the real-node deployment slice re-baselines them from measured
/// cold-boot evidence before any release claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerLifecycleBudgets {
    /// Hard bound for one broker prepare-binding operation.
    pub sandbox_prepare_binding_ms: u64,
    /// Hard bound for one VMM boot including guest-agent readiness.
    pub sandbox_boot_vmm_ms: u64,
    /// Hard bound for the bounded guest shutdown.
    pub sandbox_shutdown_guest_ms: u64,
    /// Hard bound for the VMM termination.
    pub sandbox_terminate_vmm_ms: u64,
    /// Hard bound for the idempotent binding cleanup.
    pub sandbox_cleanup_binding_ms: u64,
}

impl Default for SandboxFirecrackerLifecycleBudgets {
    fn default() -> Self {
        Self {
            sandbox_prepare_binding_ms: 30_000,
            sandbox_boot_vmm_ms: 60_000,
            sandbox_shutdown_guest_ms: 30_000,
            sandbox_terminate_vmm_ms: 10_000,
            sandbox_cleanup_binding_ms: 60_000,
        }
    }
}

/// The capability evidence a composition may hand over once the corresponding
/// real-platform evidence exists. Every flag maps one contract capability
/// policy to a claimable capability; anything without a flag here stays
/// unclaimed by construction.
///
/// - `sandbox_filesystem_evidence`: guest block-device attachment evidence
///   (`REQ-2026-0013`).
/// - `sandbox_terminal_evidence`: authenticated guest readiness evidence for
///   terminal supervision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxFirecrackerCapabilityEvidence {
    /// The guest block-device attachment evidence landed.
    pub sandbox_filesystem_evidence: bool,
    /// The authenticated guest readiness evidence landed.
    pub sandbox_terminal_evidence: bool,
}

impl Default for SandboxFirecrackerCapabilityEvidence {
    fn default() -> Self {
        // Fail closed: no evidence, no claim.
        Self {
            sandbox_filesystem_evidence: false,
            sandbox_terminal_evidence: false,
        }
    }
}

impl SandboxFirecrackerCapabilityEvidence {
    fn sandbox_claimed_capabilities(self) -> BTreeSet<RuntimeCapability> {
        let mut sandbox_capabilities = BTreeSet::new();
        if self.sandbox_filesystem_evidence {
            sandbox_capabilities.insert(RuntimeCapability::Filesystem);
        }
        if self.sandbox_terminal_evidence {
            sandbox_capabilities.insert(RuntimeCapability::Terminal);
        }
        sandbox_capabilities
    }
}

/// Why a provider object could not be constructed.
#[derive(Debug, thiserror::Error)]
pub enum SandboxFirecrackerProviderConstructionError {
    /// The provider identity vocabulary rejected the fixed provider id or
    /// kind.
    #[error("sandbox firecracker provider identity is invalid")]
    IdentityInvalid(#[from] sdkwork_sandbox_provider_spi::SandboxIdentifierError),
}

/// The composition-owned source of the current host facts snapshot. Probe
/// failures belong to the probe: a fact it could not observe is reported as
/// unavailable, so preflight stays a pure function over the snapshot.
#[async_trait]
pub trait SandboxFirecrackerHostFactsSource: Send + Sync {
    /// Returns the current host facts snapshot.
    async fn sandbox_host_facts(&self) -> SandboxFirecrackerHostFacts;
}

/// The lifecycle phase of one binding's allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SandboxFirecrackerAllocationPhase {
    /// The allocation is registered, but the boot has not completed.
    Allocated,
    /// The boot completed and readiness was fully claimed.
    Running,
    /// The VMM terminated cleanly and the binding awaits destroy.
    Stopped,
    /// Cleanup ended in an unverifiable state; the binding stays visible
    /// until a later cleanup succeeds.
    Quarantined,
}

/// One binding's provider-private lifecycle state. The fencing maximum lives
/// in the fencing store so it survives a node restart; the references here
/// are this node's in-memory view of one allocation.
struct SandboxFirecrackerRegistryEntry {
    /// The provider-side allocation reference handed to the caller.
    sandbox_allocation_reference: SandboxProviderAllocationRef,
    /// The broker-side reference for the prepared binding, once prepare
    /// succeeded.
    sandbox_binding_reference: Option<SandboxProviderAllocationRef>,
    /// The broker-side VMM reference, once boot succeeded.
    sandbox_vmm_reference: Option<SandboxProviderAllocationRef>,
    sandbox_phase: SandboxFirecrackerAllocationPhase,
}

/// The Firecracker provider: descriptor, fencing store, broker seam, host
/// facts source, bounded registry, and lifecycle budgets.
pub struct SandboxFirecrackerProvider {
    sandbox_provider_id: SandboxProviderId,
    sandbox_descriptor: SandboxProviderDescriptor,
    sandbox_fencing_store: Arc<dyn SandboxFirecrackerFencingStore>,
    sandbox_broker: Arc<dyn SandboxFirecrackerHostIsolationBroker>,
    sandbox_facts_source: Arc<dyn SandboxFirecrackerHostFactsSource>,
    sandbox_budgets: SandboxFirecrackerLifecycleBudgets,
    sandbox_registry: Mutex<HashMap<String, SandboxFirecrackerRegistryEntry>>,
}

impl fmt::Debug for SandboxFirecrackerProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The registry holds provider-private allocation references; the debug
        // shape names only the identity.
        f.debug_struct("SandboxFirecrackerProvider")
            .field("sandbox_provider_id", &self.sandbox_provider_id)
            .finish_non_exhaustive()
    }
}

impl SandboxFirecrackerProvider {
    /// Builds the provider over its fixed identity, the fencing store, the
    /// broker seam, the host facts source, the capability evidence, and the
    /// lifecycle budgets.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxFirecrackerProviderConstructionError`] when the fixed
    /// identity strings fail the identifier vocabulary.
    pub fn new(
        sandbox_fencing_store: Arc<dyn SandboxFirecrackerFencingStore>,
        sandbox_broker: Arc<dyn SandboxFirecrackerHostIsolationBroker>,
        sandbox_facts_source: Arc<dyn SandboxFirecrackerHostFactsSource>,
        sandbox_capability_evidence: SandboxFirecrackerCapabilityEvidence,
        sandbox_budgets: SandboxFirecrackerLifecycleBudgets,
    ) -> Result<Self, SandboxFirecrackerProviderConstructionError> {
        let sandbox_provider_id = SandboxProviderId::parse("firecracker")?;
        let sandbox_provider_kind = SandboxProviderKind::parse("firecracker")?;
        let sandbox_descriptor = SandboxProviderDescriptor::new(
            sandbox_provider_id.clone(),
            sandbox_provider_kind,
            sandbox_capability_evidence.sandbox_claimed_capabilities(),
            IsolationAssurance::MicroVm,
        );
        Ok(Self {
            sandbox_provider_id,
            sandbox_descriptor,
            sandbox_fencing_store,
            sandbox_broker,
            sandbox_facts_source,
            sandbox_budgets,
            sandbox_registry: Mutex::new(HashMap::new()),
        })
    }

    fn sandbox_lock_registry(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<String, SandboxFirecrackerRegistryEntry>> {
        self.sandbox_registry
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn sandbox_error(
        &self,
        sandbox_operation: SandboxProviderOperation,
        sandbox_error_kind: SandboxProviderErrorKind,
    ) -> SandboxProviderError {
        SandboxProviderError::new(
            self.sandbox_provider_id.clone(),
            sandbox_operation,
            sandbox_error_kind,
        )
    }

    /// Maps a fenced authority failure onto the provider error vocabulary: a
    /// stale token is a policy rejection; a store that cannot prove authority
    /// is an unavailable provider.
    fn sandbox_map_fencing_error(
        &self,
        sandbox_operation: SandboxProviderOperation,
        sandbox_fencing_error: &SandboxFirecrackerFencingAuthorityError,
    ) -> SandboxProviderError {
        match sandbox_fencing_error {
            SandboxFirecrackerFencingAuthorityError::StaleFencing => {
                self.sandbox_error(sandbox_operation, SandboxProviderErrorKind::Rejected)
            }
            SandboxFirecrackerFencingAuthorityError::StoreUnavailable(sandbox_store_error) => {
                tracing::warn!(
                    sandbox_operation = ?sandbox_operation,
                    "sandbox fencing store could not prove authority: {sandbox_store_error}"
                );
                self.sandbox_error(sandbox_operation, SandboxProviderErrorKind::Unavailable)
            }
        }
    }

    /// Maps a broker failure onto the provider error vocabulary: a quarantined
    /// broker operation leaves the binding unverifiable, so the provider is
    /// unavailable for that binding until cleanup succeeds.
    fn sandbox_map_broker_error(
        sandbox_broker_error: &SandboxFirecrackerBrokerError,
    ) -> SandboxProviderErrorKind {
        match sandbox_broker_error.sandbox_broker_error_kind {
            SandboxFirecrackerBrokerErrorKind::Rejected => SandboxProviderErrorKind::Rejected,
            SandboxFirecrackerBrokerErrorKind::Unavailable
            | SandboxFirecrackerBrokerErrorKind::Quarantined => {
                SandboxProviderErrorKind::Unavailable
            }
        }
    }

    async fn sandbox_run_broker<T>(
        &self,
        sandbox_operation: SandboxProviderOperation,
        sandbox_budget_ms: u64,
        sandbox_future: impl std::future::Future<Output = Result<T, SandboxFirecrackerBrokerError>>,
    ) -> SandboxProviderResult<T> {
        match timeout(Duration::from_millis(sandbox_budget_ms), sandbox_future).await {
            Ok(Ok(sandbox_value)) => Ok(sandbox_value),
            Ok(Err(sandbox_broker_error)) => {
                tracing::warn!(
                    sandbox_operation = ?sandbox_operation,
                    broker_error = %sandbox_broker_error,
                    "sandbox broker operation failed"
                );
                Err(self.sandbox_error(
                    sandbox_operation,
                    Self::sandbox_map_broker_error(&sandbox_broker_error),
                ))
            }
            Err(_sandbox_elapsed) => {
                Err(self.sandbox_error(sandbox_operation, SandboxProviderErrorKind::Timeout))
            }
        }
    }
}

#[async_trait]
impl SandboxProvider for SandboxFirecrackerProvider {
    fn sandbox_provider_descriptor(&self) -> &SandboxProviderDescriptor {
        &self.sandbox_descriptor
    }

    async fn sandbox_provider_health(&self) -> SandboxProviderResult<SandboxProviderHealth> {
        let sandbox_facts = self.sandbox_facts_source.sandbox_host_facts().await;
        let sandbox_report = sandbox_run_firecracker_preflight(&sandbox_facts);
        let sandbox_health_status = match sandbox_report.sandbox_status() {
            SandboxFirecrackerPreflightStatus::Ready => SandboxProviderHealthStatus::Ready,
            SandboxFirecrackerPreflightStatus::Degraded => SandboxProviderHealthStatus::Degraded,
            SandboxFirecrackerPreflightStatus::Unavailable => {
                SandboxProviderHealthStatus::Unavailable
            }
        };
        Ok(SandboxProviderHealth {
            sandbox_provider_health_status: sandbox_health_status,
        })
    }

    async fn allocate(
        &self,
        sandbox_request: SandboxProviderAllocationRequest,
    ) -> SandboxProviderResult<SandboxProviderAllocation> {
        // Stale fencing is rejected before any side effect, per REQ-2026-0008.
        if let Err(sandbox_fencing_error) = sandbox_check_fencing_authority(
            self.sandbox_fencing_store.as_ref(),
            sandbox_request.sandbox_runtime_binding_id.as_str(),
            sandbox_request.sandbox_fencing_token.value(),
        )
        .await
        {
            return Err(self.sandbox_map_fencing_error(
                SandboxProviderOperation::Allocate,
                &sandbox_fencing_error,
            ));
        }

        let mut sandbox_registry = self.sandbox_lock_registry();
        if sandbox_registry.contains_key(sandbox_request.sandbox_runtime_binding_id.as_str()) {
            return Err(self.sandbox_error(
                SandboxProviderOperation::Allocate,
                SandboxProviderErrorKind::Conflict,
            ));
        }
        if sandbox_registry.len() >= MAX_SANDBOX_ACTIVE_ALLOCATIONS {
            return Err(self.sandbox_error(
                SandboxProviderOperation::Allocate,
                SandboxProviderErrorKind::Unavailable,
            ));
        }
        let sandbox_allocation_reference =
            SandboxProviderAllocationRef::new(format!("fcalloc:{}", Uuid::new_v4())).map_err(
                |_| {
                    self.sandbox_error(
                        SandboxProviderOperation::Allocate,
                        SandboxProviderErrorKind::Internal,
                    )
                },
            )?;
        sandbox_registry.insert(
            sandbox_request
                .sandbox_runtime_binding_id
                .as_str()
                .to_owned(),
            SandboxFirecrackerRegistryEntry {
                sandbox_allocation_reference: sandbox_allocation_reference.clone(),
                sandbox_binding_reference: None,
                sandbox_vmm_reference: None,
                sandbox_phase: SandboxFirecrackerAllocationPhase::Allocated,
            },
        );
        Ok(SandboxProviderAllocation {
            sandbox_allocation_reference,
        })
    }

    async fn start(
        &self,
        sandbox_request: SandboxProviderStartRequest,
    ) -> SandboxProviderResult<SandboxProviderReadiness> {
        if let Err(sandbox_fencing_error) = sandbox_check_fencing_authority(
            self.sandbox_fencing_store.as_ref(),
            sandbox_request.sandbox_runtime_binding_id.as_str(),
            sandbox_request.sandbox_fencing_token.value(),
        )
        .await
        {
            return Err(self.sandbox_map_fencing_error(
                SandboxProviderOperation::Start,
                &sandbox_fencing_error,
            ));
        }

        let sandbox_allocation_reference = {
            let sandbox_registry = self.sandbox_lock_registry();
            match sandbox_registry.get(sandbox_request.sandbox_runtime_binding_id.as_str()) {
                Some(sandbox_entry)
                    if matches!(
                        sandbox_entry.sandbox_phase,
                        SandboxFirecrackerAllocationPhase::Allocated
                            | SandboxFirecrackerAllocationPhase::Stopped
                    ) =>
                {
                    sandbox_entry.sandbox_allocation_reference.clone()
                }
                // A running binding cannot boot twice; a quarantined binding
                // is not startable at all; an unknown binding never allocated.
                Some(_) | None => {
                    return Err(self.sandbox_error(
                        SandboxProviderOperation::Start,
                        SandboxProviderErrorKind::Conflict,
                    ))
                }
            }
        };

        let sandbox_prepared = self
            .sandbox_run_broker(
                SandboxProviderOperation::Start,
                self.sandbox_budgets.sandbox_prepare_binding_ms,
                self.sandbox_broker.sandbox_prepare_binding(
                    &SandboxFirecrackerPrepareBindingRequest {
                        sandbox_tenant_id: sandbox_request.tenant_id.as_str().to_owned(),
                        sandbox_runtime_binding_id: sandbox_request
                            .sandbox_runtime_binding_id
                            .as_str()
                            .to_owned(),
                        sandbox_allocation_reference: sandbox_allocation_reference.clone(),
                        sandbox_workspace_id: sandbox_request
                            .sandbox_workspace_id
                            .as_str()
                            .to_owned(),
                    },
                ),
            )
            .await?;
        let sandbox_boot = self
            .sandbox_run_broker(
                SandboxProviderOperation::Start,
                self.sandbox_budgets.sandbox_boot_vmm_ms,
                self.sandbox_broker
                    .sandbox_boot_vmm(&SandboxFirecrackerBootVmmRequest {
                        sandbox_binding_reference: sandbox_prepared
                            .sandbox_binding_reference
                            .clone(),
                        sandbox_tenant_id: sandbox_request.tenant_id.as_str().to_owned(),
                        sandbox_runtime_binding_id: sandbox_request
                            .sandbox_runtime_binding_id
                            .as_str()
                            .to_owned(),
                    }),
            )
            .await?;

        // Readiness is earned field by field from the broker's verified
        // claims; no field defaults to true.
        let sandbox_readiness = SandboxProviderReadiness {
            sandbox_provider_ready: sandbox_boot.sandbox_guest_agent_authenticated,
            sandbox_policy_enforced: sandbox_prepared.sandbox_network_enforced
                && sandbox_prepared.sandbox_resource_enforced,
            sandbox_workspace_attached: sandbox_prepared.sandbox_workspace_attached,
        };
        {
            let mut sandbox_registry = self.sandbox_lock_registry();
            if let Some(sandbox_entry) =
                sandbox_registry.get_mut(sandbox_request.sandbox_runtime_binding_id.as_str())
            {
                sandbox_entry.sandbox_binding_reference =
                    Some(sandbox_prepared.sandbox_binding_reference.clone());
                sandbox_entry.sandbox_vmm_reference = Some(sandbox_boot.sandbox_vmm_reference);
                if sandbox_readiness.is_sandbox_running_ready() {
                    sandbox_entry.sandbox_phase = SandboxFirecrackerAllocationPhase::Running;
                }
            }
        }
        if !sandbox_readiness.is_sandbox_running_ready() {
            tracing::warn!(
                sandbox_binding = %sandbox_request.sandbox_runtime_binding_id,
                "sandbox firecracker start did not earn full readiness; the binding stays allocated"
            );
        }
        Ok(sandbox_readiness)
    }

    async fn stop(&self, sandbox_request: SandboxProviderStopRequest) -> SandboxProviderResult<()> {
        if let Err(sandbox_fencing_error) = sandbox_check_fencing_authority(
            self.sandbox_fencing_store.as_ref(),
            sandbox_request.sandbox_runtime_binding_id.as_str(),
            sandbox_request.sandbox_fencing_token.value(),
        )
        .await
        {
            return Err(self.sandbox_map_fencing_error(
                SandboxProviderOperation::Stop,
                &sandbox_fencing_error,
            ));
        }

        let (sandbox_allocation_reference, sandbox_vmm_reference, sandbox_already_stopped) = {
            let sandbox_registry = self.sandbox_lock_registry();
            match sandbox_registry.get(sandbox_request.sandbox_runtime_binding_id.as_str()) {
                Some(sandbox_entry)
                    if matches!(
                        sandbox_entry.sandbox_phase,
                        SandboxFirecrackerAllocationPhase::Running
                            | SandboxFirecrackerAllocationPhase::Allocated
                    ) =>
                {
                    (
                        sandbox_entry.sandbox_allocation_reference.clone(),
                        sandbox_entry.sandbox_vmm_reference.clone(),
                        false,
                    )
                }
                // Stop on an already-stopped binding is idempotent.
                Some(sandbox_entry)
                    if matches!(
                        sandbox_entry.sandbox_phase,
                        SandboxFirecrackerAllocationPhase::Stopped
                    ) =>
                {
                    (
                        sandbox_entry.sandbox_allocation_reference.clone(),
                        None,
                        true,
                    )
                }
                // A quarantined binding may only be destroyed, not stopped.
                Some(_) | None => {
                    return Err(self.sandbox_error(
                        SandboxProviderOperation::Stop,
                        SandboxProviderErrorKind::Conflict,
                    ))
                }
            }
        };
        if sandbox_already_stopped {
            return Ok(());
        }

        // Stop upgrades in the REQ's order: bounded guest shutdown first, VMM
        // termination second. A shutdown transport failure is not fatal - the
        // bounded shutdown is best effort; termination is the enforcement.
        if let Err(sandbox_shutdown_error) = self
            .sandbox_run_broker(
                SandboxProviderOperation::Stop,
                self.sandbox_budgets.sandbox_shutdown_guest_ms,
                self.sandbox_broker.sandbox_shutdown_guest(
                    &SandboxFirecrackerShutdownGuestRequest {
                        sandbox_binding_reference: sandbox_allocation_reference.clone(),
                        sandbox_vmm_reference: sandbox_vmm_reference.clone(),
                    },
                ),
            )
            .await
        {
            tracing::warn!(
                sandbox_operation = ?SandboxProviderOperation::Stop,
                shutdown_error = %sandbox_shutdown_error,
                "the bounded guest shutdown failed; proceeding to VMM termination"
            );
        }
        self.sandbox_run_broker(
            SandboxProviderOperation::Stop,
            self.sandbox_budgets.sandbox_terminate_vmm_ms,
            self.sandbox_broker
                .sandbox_terminate_vmm(&SandboxFirecrackerTerminateVmmRequest {
                    sandbox_binding_reference: sandbox_allocation_reference.clone(),
                    sandbox_vmm_reference: sandbox_vmm_reference.clone(),
                }),
        )
        .await?;

        {
            let mut sandbox_registry = self.sandbox_lock_registry();
            if let Some(sandbox_entry) =
                sandbox_registry.get_mut(sandbox_request.sandbox_runtime_binding_id.as_str())
            {
                sandbox_entry.sandbox_phase = SandboxFirecrackerAllocationPhase::Stopped;
            }
        }
        Ok(())
    }

    async fn destroy(
        &self,
        sandbox_request: SandboxProviderDestroyRequest,
    ) -> SandboxProviderResult<()> {
        if let Err(sandbox_fencing_error) = sandbox_check_fencing_authority(
            self.sandbox_fencing_store.as_ref(),
            sandbox_request.sandbox_runtime_binding_id.as_str(),
            sandbox_request.sandbox_fencing_token.value(),
        )
        .await
        {
            return Err(self.sandbox_map_fencing_error(
                SandboxProviderOperation::Destroy,
                &sandbox_fencing_error,
            ));
        }

        let sandbox_allocation_reference = {
            let sandbox_registry = self.sandbox_lock_registry();
            match sandbox_registry.get(sandbox_request.sandbox_runtime_binding_id.as_str()) {
                Some(sandbox_entry) => Some(sandbox_entry.sandbox_allocation_reference.clone()),
                // Destroy is idempotent: an unknown binding has nothing to
                // clean, so the destroy succeeds without broker work.
                None => None,
            }
        };
        let Some(sandbox_allocation_reference) = sandbox_allocation_reference else {
            return Ok(());
        };
        if let Some(sandbox_declared) = &sandbox_request.sandbox_allocation_reference {
            // A destroy that declares its allocation must declare the current
            // one: a stale caller cannot clean a binding it does not own.
            if sandbox_declared.expose_to_provider()
                != sandbox_allocation_reference.expose_to_provider()
            {
                return Err(self.sandbox_error(
                    SandboxProviderOperation::Destroy,
                    SandboxProviderErrorKind::Rejected,
                ));
            }
        }

        if let Err(sandbox_cleanup_error) = self
            .sandbox_run_broker(
                SandboxProviderOperation::Destroy,
                self.sandbox_budgets.sandbox_cleanup_binding_ms,
                self.sandbox_broker.sandbox_cleanup_binding(
                    &SandboxFirecrackerCleanupBindingRequest {
                        sandbox_binding_reference: sandbox_allocation_reference.clone(),
                        sandbox_vmm_reference: None,
                    },
                ),
            )
            .await
        {
            // Cleanup ended unverifiable: quarantine the binding so the
            // composition layer sees it, and surface the failure.
            {
                let mut sandbox_registry = self.sandbox_lock_registry();
                if let Some(sandbox_entry) =
                    sandbox_registry.get_mut(sandbox_request.sandbox_runtime_binding_id.as_str())
                {
                    sandbox_entry.sandbox_phase = SandboxFirecrackerAllocationPhase::Quarantined;
                }
            }
            return Err(sandbox_cleanup_error);
        }

        self.sandbox_lock_registry()
            .remove(sandbox_request.sandbox_runtime_binding_id.as_str());
        Ok(())
    }
}
