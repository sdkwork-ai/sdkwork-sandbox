//! Lifecycle tests for the Firecracker provider: descriptor truthfulness,
//! fenced authority ordering, the boot sequence's readiness claims, the
//! bounded registry, and destroy idempotency with quarantine.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::fake_host::{sandbox_ready_facts, SandboxFakeBroker, SandboxStaticFactsSource};
use crate::fencing::{SandboxFirecrackerFileFencingStore, SandboxFirecrackerInMemoryFencingStore};
use crate::lifecycle::{
    SandboxFirecrackerCapabilityEvidence, SandboxFirecrackerLifecycleBudgets,
    SandboxFirecrackerProvider,
};
use sdkwork_sandbox_provider_spi::{
    IsolationAssurance, RuntimeCapability, SandboxFencingToken, SandboxProvider,
    SandboxProviderAllocationRef, SandboxProviderAllocationRequest, SandboxProviderDestroyRequest,
    SandboxProviderErrorKind, SandboxProviderOperation, SandboxProviderStartRequest,
    SandboxProviderStopRequest, SandboxRuntimeBindingId, SandboxSessionId, SandboxWorkspaceId,
    TenantId,
};

fn sandbox_fencing_token(sandbox_value: u64) -> SandboxFencingToken {
    SandboxFencingToken::new(sandbox_value)
        .unwrap_or_else(|error| panic!("test fencing token {sandbox_value} must be valid: {error}"))
}

fn sandbox_allocation_request(
    sandbox_binding: &str,
    sandbox_token: u64,
) -> SandboxProviderAllocationRequest {
    SandboxProviderAllocationRequest {
        tenant_id: TenantId::parse("tenant-a")
            .unwrap_or_else(|error| panic!("test tenant id must parse: {error}")),
        sandbox_workspace_id: SandboxWorkspaceId::parse("workspace-a")
            .unwrap_or_else(|error| panic!("test workspace id must parse: {error}")),
        sandbox_session_id: SandboxSessionId::parse("session-a")
            .unwrap_or_else(|error| panic!("test session id must parse: {error}")),
        sandbox_id: sdkwork_sandbox_provider_spi::SandboxId::generate(),
        sandbox_runtime_binding_id: SandboxRuntimeBindingId::parse(sandbox_binding)
            .unwrap_or_else(|error| panic!("test binding id must parse: {error}")),
        sandbox_fencing_token: sandbox_fencing_token(sandbox_token),
        sandbox_required_capabilities: [].into_iter().collect(),
        sandbox_minimum_assurance: IsolationAssurance::MicroVm,
    }
}

fn sandbox_start_request(sandbox_binding: &str, sandbox_token: u64) -> SandboxProviderStartRequest {
    SandboxProviderStartRequest {
        tenant_id: TenantId::parse("tenant-a")
            .unwrap_or_else(|error| panic!("test tenant id must parse: {error}")),
        sandbox_workspace_id: SandboxWorkspaceId::parse("workspace-a")
            .unwrap_or_else(|error| panic!("test workspace id must parse: {error}")),
        sandbox_session_id: SandboxSessionId::parse("session-a")
            .unwrap_or_else(|error| panic!("test session id must parse: {error}")),
        sandbox_id: sdkwork_sandbox_provider_spi::SandboxId::generate(),
        sandbox_runtime_binding_id: SandboxRuntimeBindingId::parse(sandbox_binding)
            .unwrap_or_else(|error| panic!("test binding id must parse: {error}")),
        sandbox_fencing_token: sandbox_fencing_token(sandbox_token),
        sandbox_allocation_reference: SandboxProviderAllocationRef::new("fake:allocation")
            .unwrap_or_else(|error| panic!("test allocation ref must parse: {error}")),
    }
}

fn sandbox_stop_request(sandbox_binding: &str, sandbox_token: u64) -> SandboxProviderStopRequest {
    SandboxProviderStopRequest {
        tenant_id: TenantId::parse("tenant-a")
            .unwrap_or_else(|error| panic!("test tenant id must parse: {error}")),
        sandbox_session_id: SandboxSessionId::parse("session-a")
            .unwrap_or_else(|error| panic!("test session id must parse: {error}")),
        sandbox_id: sdkwork_sandbox_provider_spi::SandboxId::generate(),
        sandbox_runtime_binding_id: SandboxRuntimeBindingId::parse(sandbox_binding)
            .unwrap_or_else(|error| panic!("test binding id must parse: {error}")),
        sandbox_fencing_token: sandbox_fencing_token(sandbox_token),
        sandbox_allocation_reference: SandboxProviderAllocationRef::new("fake:allocation")
            .unwrap_or_else(|error| panic!("test allocation ref must parse: {error}")),
    }
}

fn sandbox_destroy_request(
    sandbox_binding: &str,
    sandbox_token: u64,
) -> SandboxProviderDestroyRequest {
    SandboxProviderDestroyRequest {
        tenant_id: TenantId::parse("tenant-a")
            .unwrap_or_else(|error| panic!("test tenant id must parse: {error}")),
        sandbox_session_id: SandboxSessionId::parse("session-a")
            .unwrap_or_else(|error| panic!("test session id must parse: {error}")),
        sandbox_id: sdkwork_sandbox_provider_spi::SandboxId::generate(),
        sandbox_runtime_binding_id: SandboxRuntimeBindingId::parse(sandbox_binding)
            .unwrap_or_else(|error| panic!("test binding id must parse: {error}")),
        sandbox_fencing_token: sandbox_fencing_token(sandbox_token),
        sandbox_allocation_reference: None,
    }
}

fn sandbox_provider() -> (SandboxFirecrackerProvider, Arc<SandboxFakeBroker>) {
    let sandbox_broker = Arc::new(SandboxFakeBroker::default());
    let sandbox_provider = SandboxFirecrackerProvider::new(
        Arc::new(SandboxFirecrackerInMemoryFencingStore::default()),
        sandbox_broker.clone(),
        Arc::new(SandboxStaticFactsSource::new(sandbox_ready_facts())),
        SandboxFirecrackerCapabilityEvidence::default(),
        SandboxFirecrackerLifecycleBudgets::default(),
    )
    .unwrap_or_else(|error| panic!("provider construction must hold: {error}"));
    (sandbox_provider, sandbox_broker)
}

#[tokio::test]
async fn descriptor_is_microvm_assurance_and_claims_no_capability_without_evidence() {
    let (sandbox_provider, _sandbox_broker) = sandbox_provider();
    let sandbox_descriptor = sandbox_provider.sandbox_provider_descriptor();
    assert_eq!(
        "firecracker",
        sandbox_descriptor.sandbox_provider_kind().as_str()
    );
    assert_eq!(
        "firecracker",
        sandbox_descriptor.sandbox_provider_id().as_str()
    );
    assert_eq!(
        IsolationAssurance::MicroVm,
        sandbox_descriptor.sandbox_isolation_assurance()
    );
    assert!(sandbox_descriptor.sandbox_runtime_capabilities().is_empty(),);
}

#[tokio::test]
async fn descriptor_claims_exactly_the_supplied_evidence_capabilities() {
    let sandbox_broker = Arc::new(SandboxFakeBroker::default());
    let sandbox_provider = SandboxFirecrackerProvider::new(
        Arc::new(SandboxFirecrackerInMemoryFencingStore::default()),
        sandbox_broker.clone(),
        Arc::new(SandboxStaticFactsSource::new(sandbox_ready_facts())),
        SandboxFirecrackerCapabilityEvidence {
            sandbox_filesystem_evidence: true,
            sandbox_terminal_evidence: true,
        },
        SandboxFirecrackerLifecycleBudgets::default(),
    )
    .unwrap_or_else(|error| panic!("provider construction must hold: {error}"));
    let sandbox_capabilities = sandbox_provider.sandbox_provider_descriptor();
    let sandbox_claimed = sandbox_capabilities.sandbox_runtime_capabilities();
    assert!(sandbox_claimed.contains(&RuntimeCapability::Filesystem));
    assert!(sandbox_claimed.contains(&RuntimeCapability::Terminal));
    assert_eq!(2, sandbox_claimed.len());
}

#[tokio::test]
async fn health_reports_unavailable_on_a_host_without_kvm_facts() {
    let sandbox_broker = Arc::new(SandboxFakeBroker::default());
    let mut sandbox_facts = sandbox_ready_facts();
    sandbox_facts.sandbox_host_platform =
        crate::preflight::SandboxFirecrackerHostPlatform::Unsupported;
    sandbox_facts.sandbox_kvm_device_available = false;
    let sandbox_provider = SandboxFirecrackerProvider::new(
        Arc::new(SandboxFirecrackerInMemoryFencingStore::default()),
        sandbox_broker.clone(),
        Arc::new(SandboxStaticFactsSource::new(sandbox_facts)),
        SandboxFirecrackerCapabilityEvidence::default(),
        SandboxFirecrackerLifecycleBudgets::default(),
    )
    .unwrap_or_else(|error| panic!("provider construction must hold: {error}"));
    let sandbox_health = sandbox_provider
        .sandbox_provider_health()
        .await
        .unwrap_or_else(|error| panic!("health must not error: {error}"));
    assert!(matches!(
        sandbox_health.sandbox_provider_health_status,
        sdkwork_sandbox_provider_spi::SandboxProviderHealthStatus::Unavailable
    ));
}

#[tokio::test]
async fn start_sequence_earns_full_readiness_and_records_the_boot() {
    let (sandbox_provider, sandbox_broker) = sandbox_provider();
    sandbox_broker
        .sandbox_enforcement_holds
        .store(true, Ordering::SeqCst);
    sandbox_broker
        .sandbox_guest_authenticates
        .store(true, Ordering::SeqCst);
    sandbox_provider
        .allocate(sandbox_allocation_request("binding-boot", 5))
        .await
        .unwrap_or_else(|error| panic!("allocate must hold: {error}"));
    let sandbox_readiness = sandbox_provider
        .start(sandbox_start_request("binding-boot", 5))
        .await
        .unwrap_or_else(|error| panic!("start must hold: {error}"));
    assert!(sandbox_readiness.is_sandbox_running_ready());
    assert_eq!(1, sandbox_broker.sandbox_prepare_count());
    assert_eq!(1, sandbox_broker.sandbox_boot_count());
}

#[tokio::test]
async fn start_refuses_to_claim_readiness_when_the_broker_cannot_verify_enforcement() {
    let (sandbox_provider, sandbox_broker) = sandbox_provider();
    // Enforcement defaults to false in the fake; the guest authenticates.
    sandbox_broker
        .sandbox_guest_authenticates
        .store(true, Ordering::SeqCst);
    sandbox_provider
        .allocate(sandbox_allocation_request("binding-partial", 5))
        .await
        .unwrap_or_else(|error| panic!("allocate must hold: {error}"));
    let sandbox_readiness = sandbox_provider
        .start(sandbox_start_request("binding-partial", 5))
        .await
        .unwrap_or_else(|error| panic!("start must hold: {error}"));
    assert!(!sandbox_readiness.is_sandbox_running_ready());
    assert!(sandbox_readiness.sandbox_provider_ready);
    assert!(!sandbox_readiness.sandbox_policy_enforced);
    assert!(!sandbox_readiness.sandbox_workspace_attached);
}

#[tokio::test]
async fn stale_fencing_is_rejected_before_any_allocation_side_effect() {
    let (sandbox_provider, sandbox_broker) = sandbox_provider();
    sandbox_provider
        .allocate(sandbox_allocation_request("binding-fenced", 9))
        .await
        .unwrap_or_else(|error| panic!("allocate must hold: {error}"));
    let sandbox_stale = sandbox_provider
        .allocate(sandbox_allocation_request("binding-fenced", 8))
        .await;
    assert!(
        matches!(&sandbox_stale, Err(sandbox_error) if sandbox_error.sandbox_provider_error_kind() == SandboxProviderErrorKind::Rejected),
        "a stale token must be rejected as policy: {sandbox_stale:?}"
    );
    assert_eq!(0, sandbox_broker.sandbox_prepare_count());

    // A stale token on a different operation (start against a fresh binding)
    // is likewise rejected before the broker.
    let sandbox_stale_start = sandbox_provider
        .start(sandbox_start_request("binding-never-allocated", 8))
        .await;
    assert!(matches!(
        sandbox_stale_start,
        Err(sandbox_error) if sandbox_error.sandbox_provider_operation() == SandboxProviderOperation::Start
    ));
    assert_eq!(0, sandbox_broker.sandbox_prepare_count());
}

#[tokio::test]
async fn duplicate_allocation_for_one_binding_conflicts() {
    let (sandbox_provider, _sandbox_broker) = sandbox_provider();
    sandbox_provider
        .allocate(sandbox_allocation_request("binding-dup", 5))
        .await
        .unwrap_or_else(|error| panic!("allocate must hold: {error}"));
    let sandbox_duplicate = sandbox_provider
        .allocate(sandbox_allocation_request("binding-dup", 5))
        .await;
    assert!(
        matches!(&sandbox_duplicate, Err(sandbox_error) if sandbox_error.sandbox_provider_error_kind() == SandboxProviderErrorKind::Conflict),
        "a same-binding replay must conflict: {sandbox_duplicate:?}"
    );
}

#[tokio::test]
async fn start_requires_a_prior_allocation() {
    let (sandbox_provider, sandbox_broker) = sandbox_provider();
    sandbox_broker
        .sandbox_enforcement_holds
        .store(true, Ordering::SeqCst);
    sandbox_broker
        .sandbox_guest_authenticates
        .store(true, Ordering::SeqCst);
    let sandbox_unallocated = sandbox_provider
        .start(sandbox_start_request("binding-ghost", 5))
        .await;
    assert!(matches!(
        sandbox_unallocated,
        Err(sandbox_error) if sandbox_error.sandbox_provider_error_kind() == SandboxProviderErrorKind::Conflict
    ));
    assert_eq!(0, sandbox_broker.sandbox_prepare_count());
}

#[tokio::test]
async fn stop_upgrades_shutdown_to_termination_and_is_idempotent() {
    let (sandbox_provider, sandbox_broker) = sandbox_provider();
    sandbox_broker
        .sandbox_enforcement_holds
        .store(true, Ordering::SeqCst);
    sandbox_broker
        .sandbox_guest_authenticates
        .store(true, Ordering::SeqCst);
    sandbox_provider
        .allocate(sandbox_allocation_request("binding-stop", 5))
        .await
        .unwrap_or_else(|error| panic!("allocate must hold: {error}"));
    sandbox_provider
        .start(sandbox_start_request("binding-stop", 5))
        .await
        .unwrap_or_else(|error| panic!("start must hold: {error}"));
    sandbox_provider
        .stop(sandbox_stop_request("binding-stop", 5))
        .await
        .unwrap_or_else(|error| panic!("stop must hold: {error}"));
    assert_eq!(1, sandbox_broker.sandbox_shutdown_count());
    assert_eq!(1, sandbox_broker.sandbox_terminate_count());

    // The second stop is an idempotent no-op: no further broker work.
    sandbox_provider
        .stop(sandbox_stop_request("binding-stop", 5))
        .await
        .unwrap_or_else(|error| panic!("idempotent stop must hold: {error}"));
    assert_eq!(1, sandbox_broker.sandbox_shutdown_count());
    assert_eq!(1, sandbox_broker.sandbox_terminate_count());
}

#[tokio::test]
async fn destroy_is_idempotent_and_cleans_the_binding() {
    let (sandbox_provider, sandbox_broker) = sandbox_provider();
    sandbox_provider
        .allocate(sandbox_allocation_request("binding-destroy", 5))
        .await
        .unwrap_or_else(|error| panic!("allocate must hold: {error}"));
    sandbox_provider
        .destroy(sandbox_destroy_request("binding-destroy", 5))
        .await
        .unwrap_or_else(|error| panic!("destroy must hold: {error}"));
    assert_eq!(1, sandbox_broker.sandbox_cleanup_count());

    // Destroy of an unknown (already cleaned) binding is idempotent success.
    sandbox_provider
        .destroy(sandbox_destroy_request("binding-destroy", 5))
        .await
        .unwrap_or_else(|error| panic!("idempotent destroy must hold: {error}"));
    assert_eq!(1, sandbox_broker.sandbox_cleanup_count());

    // Destroy of a never-known binding is idempotent success too.
    sandbox_provider
        .destroy(sandbox_destroy_request("binding-never-known", 5))
        .await
        .unwrap_or_else(|error| panic!("unknown-binding destroy must hold: {error}"));
    assert_eq!(1, sandbox_broker.sandbox_cleanup_count());
}

#[tokio::test]
async fn cleanup_failure_quarantines_the_binding_until_a_later_cleanup_succeeds() {
    let (sandbox_provider, sandbox_broker) = sandbox_provider();
    sandbox_broker
        .sandbox_cleanup_fails
        .store(true, Ordering::SeqCst);
    sandbox_provider
        .allocate(sandbox_allocation_request("binding-quarantine", 5))
        .await
        .unwrap_or_else(|error| panic!("allocate must hold: {error}"));
    let sandbox_failed_destroy = sandbox_provider
        .destroy(sandbox_destroy_request("binding-quarantine", 5))
        .await;
    assert!(matches!(
        sandbox_failed_destroy,
        Err(sandbox_error) if sandbox_error.sandbox_provider_error_kind() == SandboxProviderErrorKind::Unavailable
    ));

    // Cleanup recovers: the retried destroy removes the binding.
    sandbox_broker
        .sandbox_cleanup_fails
        .store(false, Ordering::SeqCst);
    sandbox_provider
        .destroy(sandbox_destroy_request("binding-quarantine", 5))
        .await
        .unwrap_or_else(|error| panic!("retried destroy must hold: {error}"));
    assert_eq!(2, sandbox_broker.sandbox_cleanup_count());
}

#[tokio::test]
async fn concurrent_start_of_one_binding_conflicts_instead_of_double_booting() {
    let (sandbox_provider, sandbox_broker) = sandbox_provider();
    sandbox_broker
        .sandbox_enforcement_holds
        .store(true, Ordering::SeqCst);
    sandbox_broker
        .sandbox_guest_authenticates
        .store(true, Ordering::SeqCst);
    sandbox_provider
        .allocate(sandbox_allocation_request("binding-race-start", 5))
        .await
        .unwrap_or_else(|error| panic!("allocate must hold: {error}"));

    // Two concurrent starts of the same binding: exactly one may drive the
    // boot sequence; the other conflicts without touching the broker.
    let sandbox_first = sandbox_provider.start(sandbox_start_request("binding-race-start", 5));
    let sandbox_second = sandbox_provider.start(sandbox_start_request("binding-race-start", 5));
    let (sandbox_first, sandbox_second) = tokio::join!(sandbox_first, sandbox_second);
    let sandbox_outcomes = [sandbox_first, sandbox_second];
    let sandbox_ok_count = sandbox_outcomes.iter().filter(|r| r.is_ok()).count();
    let sandbox_conflict_count = sandbox_outcomes
        .iter()
        .filter(|r| {
            matches!(
                r,
                Err(sandbox_error)
                    if sandbox_error.sandbox_provider_error_kind() == SandboxProviderErrorKind::Conflict
            )
        })
        .count();
    assert_eq!(
        (sandbox_ok_count, sandbox_conflict_count),
        (1, 1),
        "exactly one start wins and one conflicts: {sandbox_outcomes:?}"
    );
    assert_eq!(
        1,
        sandbox_broker.sandbox_boot_count(),
        "the boot sequence must have run exactly once"
    );
    assert_eq!(1, sandbox_broker.sandbox_prepare_count());
}

#[tokio::test]
async fn fencing_survives_a_node_restart_through_the_durable_store() {
    fn sandbox_record_file_name(sandbox_binding: &str) -> String {
        let sandbox_hex: String = sandbox_binding
            .bytes()
            .map(|sandbox_byte| format!("{sandbox_byte:02x}"))
            .collect();
        format!("binding-{sandbox_hex}.token")
    }

    let sandbox_root = std::env::temp_dir().join(format!(
        "sdkwork-sandbox-firecracker-lifecycle-{}",
        uuid::Uuid::new_v4()
    ));
    let sandbox_broker = Arc::new(SandboxFakeBroker::default());
    {
        let sandbox_provider = SandboxFirecrackerProvider::new(
            Arc::new(
                SandboxFirecrackerFileFencingStore::new(sandbox_root.clone())
                    .unwrap_or_else(|error| panic!("store root must create: {error}")),
            ),
            sandbox_broker.clone(),
            Arc::new(SandboxStaticFactsSource::new(sandbox_ready_facts())),
            SandboxFirecrackerCapabilityEvidence::default(),
            SandboxFirecrackerLifecycleBudgets::default(),
        )
        .unwrap_or_else(|error| panic!("provider construction must hold: {error}"));
        sandbox_provider
            .allocate(sandbox_allocation_request("binding-restart", 13))
            .await
            .unwrap_or_else(|error| panic!("allocate must hold: {error}"));
    }
    // The restart: a fresh provider over the same durable store must refuse
    // the older token, because the maximum survived the process.
    let sandbox_restarted = SandboxFirecrackerProvider::new(
        Arc::new(
            SandboxFirecrackerFileFencingStore::new(sandbox_root.clone())
                .unwrap_or_else(|error| panic!("store root must reopen: {error}")),
        ),
        sandbox_broker.clone(),
        Arc::new(SandboxStaticFactsSource::new(sandbox_ready_facts())),
        SandboxFirecrackerCapabilityEvidence::default(),
        SandboxFirecrackerLifecycleBudgets::default(),
    )
    .unwrap_or_else(|error| panic!("provider construction must hold: {error}"));
    let sandbox_stale = sandbox_restarted
        .allocate(sandbox_allocation_request("binding-restart", 12))
        .await;
    assert!(matches!(
        sandbox_stale,
        Err(sandbox_error) if sandbox_error.sandbox_provider_error_kind() == SandboxProviderErrorKind::Rejected
    ));

    // Cleanup of the enumerated record and the enumerated root directory.
    std::fs::remove_file(sandbox_root.join(sandbox_record_file_name("binding-restart")))
        .unwrap_or_else(|error| panic!("enumerated fencing record must remove: {error}"));
    std::fs::remove_dir(sandbox_root)
        .unwrap_or_else(|error| panic!("enumerated fencing root must remove: {error}"));
}
