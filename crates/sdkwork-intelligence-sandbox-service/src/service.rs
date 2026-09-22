use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    OperationId, SandboxId, SandboxLeaseOwnerId, SandboxProvider, SandboxProviderAllocationRequest,
    SandboxProviderDestroyRequest, SandboxProviderError, SandboxProviderErrorKind,
    SandboxProviderHealthStatus, SandboxProviderId, SandboxProviderOperation,
    SandboxProviderStartRequest, SandboxProviderStopRequest, SandboxRuntimeBindingId,
    SandboxSessionId, TenantId,
};

use crate::{
    CreateSandboxSessionCommand, SandboxLifecycleError, SandboxLifecycleResult,
    SandboxOperationOutcome, SandboxRuntimeBinding, SandboxSession, SandboxSessionFailure,
    SandboxSessionLease, SandboxSessionLifecycleCommand, SandboxSessionLifecyclePort,
    SandboxSessionOperationKind, SandboxSessionReconciliationItem,
    SandboxSessionReconciliationOutcome, SandboxSessionReconciliationPage,
    SandboxSessionRepository, SandboxSessionRepositoryError, SandboxSessionState,
};

const DEFAULT_SANDBOX_LEASE_DURATION: Duration = Duration::from_secs(60);
const DEFAULT_SANDBOX_PROVIDER_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

pub struct SandboxLifecycleService {
    sandbox_session_repository: Arc<dyn SandboxSessionRepository>,
    sandbox_providers: Vec<Arc<dyn SandboxProvider>>,
    sandbox_lease_owner_id: SandboxLeaseOwnerId,
    sandbox_lease_duration: Duration,
    sandbox_provider_operation_timeout: Duration,
}

impl SandboxLifecycleService {
    ///
    /// # Errors
    ///
    /// Returns `SandboxLifecycleError::DuplicateProvider` for a duplicate
    /// registry id and `InvariantViolation` when the default lease and
    /// timeout policy is inconsistent.
    pub fn new(
        sandbox_session_repository: Arc<dyn SandboxSessionRepository>,
        sandbox_providers: Vec<Arc<dyn SandboxProvider>>,
    ) -> SandboxLifecycleResult<Self> {
        Self::new_with_sandbox_operation_policy(
            sandbox_session_repository,
            sandbox_providers,
            SandboxLeaseOwnerId::generate(),
            DEFAULT_SANDBOX_LEASE_DURATION,
            DEFAULT_SANDBOX_PROVIDER_OPERATION_TIMEOUT,
        )
    }

    ///
    /// # Errors
    ///
    /// Returns `SandboxLifecycleError::DuplicateProvider` for a duplicate
    /// registry id and `InvariantViolation` when the lease duration or
    /// provider operation timeout violates the policy bounds.
    pub fn new_with_sandbox_operation_policy(
        sandbox_session_repository: Arc<dyn SandboxSessionRepository>,
        mut sandbox_providers: Vec<Arc<dyn SandboxProvider>>,
        sandbox_lease_owner_id: SandboxLeaseOwnerId,
        sandbox_lease_duration: Duration,
        sandbox_provider_operation_timeout: Duration,
    ) -> SandboxLifecycleResult<Self> {
        if !(Duration::from_millis(1)..=Duration::from_secs(300)).contains(&sandbox_lease_duration)
        {
            return Err(SandboxLifecycleError::InvariantViolation(
                "sandbox lease duration must be between one millisecond and five minutes",
            ));
        }
        if sandbox_provider_operation_timeout.is_zero()
            || sandbox_provider_operation_timeout
                .checked_mul(2)
                .is_none_or(|sandbox_bounded_provider_duration| {
                    sandbox_bounded_provider_duration > sandbox_lease_duration
                })
        {
            return Err(SandboxLifecycleError::InvariantViolation(
                "sandbox provider operation timeout must be nonzero and no greater than half the sandbox lease duration",
            ));
        }
        sandbox_providers.sort_by(|left_sandbox_provider, right_sandbox_provider| {
            left_sandbox_provider
                .sandbox_provider_descriptor()
                .sandbox_provider_id()
                .cmp(
                    right_sandbox_provider
                        .sandbox_provider_descriptor()
                        .sandbox_provider_id(),
                )
        });
        for sandbox_provider_pair in sandbox_providers.windows(2) {
            if sandbox_provider_pair[0]
                .sandbox_provider_descriptor()
                .sandbox_provider_id()
                == sandbox_provider_pair[1]
                    .sandbox_provider_descriptor()
                    .sandbox_provider_id()
            {
                return Err(SandboxLifecycleError::DuplicateProvider {
                    sandbox_provider_id: sandbox_provider_pair[0]
                        .sandbox_provider_descriptor()
                        .sandbox_provider_id()
                        .clone(),
                });
            }
        }
        Ok(Self {
            sandbox_session_repository,
            sandbox_providers,
            sandbox_lease_owner_id,
            sandbox_lease_duration,
            sandbox_provider_operation_timeout,
        })
    }

    async fn acquire_sandbox_session_lease(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
    ) -> SandboxLifecycleResult<SandboxSessionLease> {
        self.sandbox_session_repository
            .acquire_sandbox_session_lease(
                tenant_id,
                sandbox_session_id,
                &self.sandbox_lease_owner_id,
                self.sandbox_lease_duration,
            )
            .await?
            .ok_or(SandboxLifecycleError::LeaseUnavailable)
    }

    async fn release_sandbox_session_lease<T>(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
        sandbox_lifecycle_result: SandboxLifecycleResult<T>,
    ) -> SandboxLifecycleResult<T> {
        let sandbox_release_result = self
            .sandbox_session_repository
            .release_sandbox_session_lease(sandbox_session_lease)
            .await;
        match (sandbox_lifecycle_result, sandbox_release_result) {
            (Err(sandbox_lifecycle_error), _) => Err(sandbox_lifecycle_error),
            (Ok(_), Err(_) | Ok(false)) => Err(SandboxLifecycleError::LeaseLost),
            (Ok(value), Ok(true)) => Ok(value),
        }
    }

    async fn execute_sandbox_provider_call<T, F>(
        &self,
        sandbox_provider: &Arc<dyn SandboxProvider>,
        sandbox_provider_operation: SandboxProviderOperation,
        sandbox_session_lease: &SandboxSessionLease,
        sandbox_provider_future: F,
    ) -> SandboxLifecycleResult<Result<T, SandboxProviderError>>
    where
        F: Future<Output = Result<T, SandboxProviderError>>,
    {
        let renewed_sandbox_session_lease = match self
            .sandbox_session_repository
            .renew_sandbox_session_lease(sandbox_session_lease, self.sandbox_lease_duration)
            .await
        {
            Ok(Some(renewed_sandbox_session_lease)) => renewed_sandbox_session_lease,
            Ok(None) | Err(_) => return Err(SandboxLifecycleError::LeaseLost),
        };
        if renewed_sandbox_session_lease.tenant_id() != sandbox_session_lease.tenant_id()
            || renewed_sandbox_session_lease.sandbox_session_id()
                != sandbox_session_lease.sandbox_session_id()
            || renewed_sandbox_session_lease.sandbox_lease_owner_id()
                != sandbox_session_lease.sandbox_lease_owner_id()
            || renewed_sandbox_session_lease.sandbox_fencing_token()
                != sandbox_session_lease.sandbox_fencing_token()
        {
            return Err(SandboxLifecycleError::LeaseLost);
        }

        match tokio::time::timeout(
            self.sandbox_provider_operation_timeout,
            sandbox_provider_future,
        )
        .await
        {
            Ok(sandbox_provider_result) => Ok(sandbox_provider_result),
            Err(_) => Ok(Err(SandboxProviderError::new(
                sandbox_provider
                    .sandbox_provider_descriptor()
                    .sandbox_provider_id()
                    .clone(),
                sandbox_provider_operation,
                SandboxProviderErrorKind::Timeout,
            ))),
        }
    }

    ///
    /// # Errors
    ///
    /// Returns `SandboxLifecycleError::IdempotencyConflict` when the
    /// operation id was used for a different request,
    /// `SandboxSessionIdConflict` when the session id is reused, and
    /// `Repository` for duplicate operations or persistence failures.
    pub async fn create_sandbox_session(
        &self,
        command: CreateSandboxSessionCommand,
    ) -> SandboxLifecycleResult<SandboxSession> {
        if let Some(existing_sandbox_session) = self
            .sandbox_session_repository
            .find_by_sandbox_operation(&command.tenant_id, &command.sandbox_operation_id)
            .await?
        {
            if existing_sandbox_session.matches_create(
                &command.sandbox_workspace_id,
                &command.sandbox_session_id,
                &command.sandbox_required_capabilities,
                command.sandbox_minimum_assurance,
            ) {
                return Ok(existing_sandbox_session);
            }
            return Err(SandboxLifecycleError::IdempotencyConflict {
                sandbox_operation_id: command.sandbox_operation_id,
            });
        }

        let sandbox_session = SandboxSession::create(
            command.tenant_id.clone(),
            command.sandbox_workspace_id,
            command.sandbox_session_id,
            command.sandbox_operation_id.clone(),
            command.sandbox_required_capabilities,
            command.sandbox_minimum_assurance,
        );

        match self
            .sandbox_session_repository
            .insert_sandbox_session(sandbox_session.clone())
            .await
        {
            Ok(()) => Ok(sandbox_session),
            Err(SandboxSessionRepositoryError::DuplicateOperation) => {
                let existing_sandbox_session = self
                    .sandbox_session_repository
                    .find_by_sandbox_operation(&command.tenant_id, &command.sandbox_operation_id)
                    .await?
                    .ok_or(SandboxLifecycleError::InvariantViolation(
                        "duplicate sandbox create operation has no owning sandbox session",
                    ))?;
                if existing_sandbox_session.matches_create(
                    sandbox_session.sandbox_workspace_id(),
                    sandbox_session.sandbox_session_id(),
                    sandbox_session.sandbox_required_capabilities(),
                    sandbox_session.sandbox_minimum_assurance(),
                ) {
                    Ok(existing_sandbox_session)
                } else {
                    Err(SandboxLifecycleError::IdempotencyConflict {
                        sandbox_operation_id: command.sandbox_operation_id,
                    })
                }
            }
            Err(SandboxSessionRepositoryError::VersionConflict) => {
                // A concurrent identical create committed between the
                // operation lookup and this insert, or the sandbox session id
                // already belongs to another lifecycle operation. Re-read by
                // operation id so a completed identical create returns the
                // authoritative sandbox session instead of a raw conflict.
                let existing_sandbox_session = self
                    .sandbox_session_repository
                    .find_by_sandbox_operation(&command.tenant_id, &command.sandbox_operation_id)
                    .await?
                    .ok_or(SandboxLifecycleError::SandboxSessionIdConflict {
                        tenant_id: command.tenant_id.clone(),
                        sandbox_session_id: sandbox_session.sandbox_session_id().clone(),
                    })?;
                if existing_sandbox_session.matches_create(
                    sandbox_session.sandbox_workspace_id(),
                    sandbox_session.sandbox_session_id(),
                    sandbox_session.sandbox_required_capabilities(),
                    sandbox_session.sandbox_minimum_assurance(),
                ) {
                    Ok(existing_sandbox_session)
                } else {
                    Err(SandboxLifecycleError::IdempotencyConflict {
                        sandbox_operation_id: command.sandbox_operation_id,
                    })
                }
            }
            Err(sandbox_repository_error) => Err(sandbox_repository_error.into()),
        }
    }

    ///
    /// # Errors
    ///
    /// Returns `SandboxLifecycleError::SandboxSessionNotFound` when the
    /// session does not exist and `Repository` when persistence fails or
    /// the persisted data is unreadable.
    pub async fn get_sandbox_session(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
    ) -> SandboxLifecycleResult<SandboxSession> {
        self.sandbox_session_repository
            .get_sandbox_session(tenant_id, sandbox_session_id)
            .await?
            .ok_or_else(|| SandboxLifecycleError::SandboxSessionNotFound {
                tenant_id: tenant_id.clone(),
                sandbox_session_id: sandbox_session_id.clone(),
            })
    }

    ///
    /// # Errors
    ///
    /// Returns `SandboxLifecycleError::InvalidPageRequest` for an
    /// out-of-range page size and `Repository` for infrastructure failures;
    /// item-local data and provider failures are reported on the page
    /// instead of aborting it.
    pub async fn reconcile_sandbox_sessions(
        &self,
        tenant_id: &TenantId,
        after_sandbox_session_id: Option<&SandboxSessionId>,
        sandbox_page_size: u16,
    ) -> SandboxLifecycleResult<SandboxSessionReconciliationPage> {
        if !(1..=200).contains(&sandbox_page_size) {
            return Err(SandboxSessionRepositoryError::InvalidPageRequest.into());
        }
        let sandbox_sessions = self
            .sandbox_session_repository
            .list_sandbox_sessions_requiring_reconciliation(
                tenant_id,
                after_sandbox_session_id,
                sandbox_page_size,
            )
            .await?;
        let next_sandbox_session_id = if sandbox_sessions.len() == usize::from(sandbox_page_size) {
            let last_sandbox_session_id = sandbox_sessions
                .last()
                .map(|sandbox_session| sandbox_session.sandbox_session_id())
                .ok_or(SandboxLifecycleError::InvariantViolation(
                    "full sandbox reconciliation page has no final sandbox session",
                ))?;
            let sandbox_has_more = !self
                .sandbox_session_repository
                .list_sandbox_sessions_requiring_reconciliation(
                    tenant_id,
                    Some(last_sandbox_session_id),
                    1,
                )
                .await?
                .is_empty();
            sandbox_has_more.then(|| last_sandbox_session_id.clone())
        } else {
            None
        };
        let mut sandbox_items = Vec::with_capacity(sandbox_sessions.len());
        for sandbox_session in sandbox_sessions {
            let sandbox_session_id = sandbox_session.sandbox_session_id().clone();
            let sandbox_session_lease = match self
                .sandbox_session_repository
                .acquire_sandbox_session_lease(
                    tenant_id,
                    &sandbox_session_id,
                    &self.sandbox_lease_owner_id,
                    self.sandbox_lease_duration,
                )
                .await
            {
                Ok(Some(sandbox_session_lease)) => sandbox_session_lease,
                // The session vanished between listing and lease acquisition:
                // its row — and the lease row with it — is gone. Report the
                // item as `Vanished` and keep the page converging instead of
                // letting one concurrent destroy abort every other session.
                Err(SandboxSessionRepositoryError::NotFound) => {
                    sandbox_items.push(SandboxSessionReconciliationItem::new(
                        sandbox_session_id,
                        sandbox_session.sandbox_session_state(),
                        SandboxSessionReconciliationOutcome::Vanished,
                    ));
                    continue;
                }
                Err(sandbox_repository_error) => {
                    return Err(sandbox_repository_error.into());
                }
                Ok(None) => {
                    sandbox_items.push(SandboxSessionReconciliationItem::new(
                        sandbox_session_id,
                        sandbox_session.sandbox_session_state(),
                        SandboxSessionReconciliationOutcome::LeaseUnavailable,
                    ));
                    continue;
                }
            };
            let sandbox_reconciliation_result = match self
                .get_sandbox_session(tenant_id, &sandbox_session_id)
                .await
            {
                Ok(authoritative_sandbox_session)
                    if matches!(
                        authoritative_sandbox_session.sandbox_session_state(),
                        SandboxSessionState::Starting
                            | SandboxSessionState::Stopping
                            | SandboxSessionState::Destroying
                    ) =>
                {
                    self.reconcile_sandbox_session_with_lease(
                        authoritative_sandbox_session,
                        &sandbox_session_lease,
                    )
                    .await
                }
                Ok(authoritative_sandbox_session) => Ok(authoritative_sandbox_session),
                Err(SandboxLifecycleError::SandboxSessionNotFound { .. }) => {
                    // The sandbox session disappeared between listing and
                    // lease acquisition; release the lease best-effort and
                    // skip the item instead of aborting the whole page. A
                    // vanished session is a different operational ticket from
                    // a foreign-held lease, so it reports `Vanished`, not
                    // `LeaseUnavailable` (F-06).
                    let _ = self
                        .sandbox_session_repository
                        .release_sandbox_session_lease(&sandbox_session_lease)
                        .await;
                    sandbox_items.push(SandboxSessionReconciliationItem::new(
                        sandbox_session_id,
                        sandbox_session.sandbox_session_state(),
                        SandboxSessionReconciliationOutcome::Vanished,
                    ));
                    continue;
                }
                Err(SandboxLifecycleError::Repository(
                    SandboxSessionRepositoryError::InvalidStoredData,
                )) => {
                    // The session's persisted data is unreadable (for example
                    // an operation history above the retention bound).
                    // Reporting it and moving on keeps one bad session from
                    // freezing the whole tenant's reconciliation; the session
                    // itself is untouched and its retention policy is owned
                    // by REQ-2026-0020.
                    let _ = self
                        .sandbox_session_repository
                        .release_sandbox_session_lease(&sandbox_session_lease)
                        .await;
                    sandbox_items.push(SandboxSessionReconciliationItem::new(
                        sandbox_session_id,
                        sandbox_session.sandbox_session_state(),
                        SandboxSessionReconciliationOutcome::Unreadable,
                    ));
                    continue;
                }
                Err(sandbox_lifecycle_error) => Err(sandbox_lifecycle_error),
            };
            let sandbox_reconciliation_result = self
                .release_sandbox_session_lease(
                    &sandbox_session_lease,
                    sandbox_reconciliation_result,
                )
                .await;
            match sandbox_reconciliation_result {
                Ok(sandbox_session) => {
                    sandbox_items.push(SandboxSessionReconciliationItem::new(
                        sandbox_session_id,
                        sandbox_session.sandbox_session_state(),
                        SandboxSessionReconciliationOutcome::Reconciled,
                    ));
                }
                Err(
                    SandboxLifecycleError::Provider(_)
                    | SandboxLifecycleError::ProviderReadinessRejected { .. }
                    | SandboxLifecycleError::InvariantViolation(_)
                    | SandboxLifecycleError::Repository(
                        SandboxSessionRepositoryError::InvalidStoredData,
                    ),
                ) => {
                    // Provider failures, item-local invariant violations (for
                    // example a runtime binding that references a provider no
                    // longer in the registry), and write-side rejections of
                    // invalid persisted data belong to this session alone:
                    // report the item and keep the page converging instead of
                    // letting one stale reference abort every other session.
                    match self
                        .get_sandbox_session(tenant_id, &sandbox_session_id)
                        .await
                    {
                        Ok(sandbox_session) => {
                            sandbox_items.push(SandboxSessionReconciliationItem::new(
                                sandbox_session_id,
                                sandbox_session.sandbox_session_state(),
                                SandboxSessionReconciliationOutcome::Failed,
                            ));
                        }
                        Err(SandboxLifecycleError::SandboxSessionNotFound { .. }) => {
                            // The sandbox session disappeared while the failed
                            // provider reconciliation was in flight; skip the
                            // item instead of aborting the whole page — and
                            // report `Vanished`: data that is gone is a
                            // different ticket from a foreign-held lease.
                            sandbox_items.push(SandboxSessionReconciliationItem::new(
                                sandbox_session_id,
                                sandbox_session.sandbox_session_state(),
                                SandboxSessionReconciliationOutcome::Vanished,
                            ));
                        }
                        Err(sandbox_lifecycle_error) => {
                            return Err(sandbox_lifecycle_error);
                        }
                    }
                }
                Err(sandbox_lifecycle_error) => return Err(sandbox_lifecycle_error),
            }
        }
        Ok(SandboxSessionReconciliationPage::new(
            sandbox_items,
            next_sandbox_session_id,
        ))
    }

    async fn reconcile_sandbox_session_with_lease(
        &self,
        mut sandbox_session: SandboxSession,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<SandboxSession> {
        match sandbox_session.sandbox_session_state() {
            SandboxSessionState::Starting => {
                let sandbox_operation_id = Self::in_progress_sandbox_operation_id(
                    &sandbox_session,
                    SandboxSessionOperationKind::Start,
                )?;
                let mut sandbox_runtime_binding = sandbox_session
                    .sandbox_runtime_binding()
                    .cloned()
                    .ok_or(SandboxLifecycleError::InvariantViolation(
                        "starting sandbox session has no sandbox runtime binding intent",
                    ))?;
                let sandbox_provider =
                    self.sandbox_provider_by_id(sandbox_runtime_binding.sandbox_provider_id())?;
                if sandbox_runtime_binding
                    .sandbox_allocation_reference()
                    .is_none()
                {
                    let sandbox_allocation = match self
                        .execute_sandbox_provider_call(
                            &sandbox_provider,
                            SandboxProviderOperation::Allocate,
                            sandbox_session_lease,
                            sandbox_provider.allocate(SandboxProviderAllocationRequest {
                                tenant_id: sandbox_session.tenant_id().clone(),
                                sandbox_workspace_id: sandbox_session
                                    .sandbox_workspace_id()
                                    .clone(),
                                sandbox_session_id: sandbox_session.sandbox_session_id().clone(),
                                sandbox_id: sandbox_runtime_binding.sandbox_id().clone(),
                                sandbox_runtime_binding_id: sandbox_runtime_binding
                                    .sandbox_runtime_binding_id()
                                    .clone(),
                                sandbox_fencing_token: sandbox_session_lease
                                    .sandbox_fencing_token(),
                                sandbox_required_capabilities: sandbox_session
                                    .sandbox_required_capabilities()
                                    .clone(),
                                sandbox_minimum_assurance: sandbox_session
                                    .sandbox_minimum_assurance(),
                            }),
                        )
                        .await?
                    {
                        Ok(sandbox_allocation) => sandbox_allocation,
                        Err(sandbox_provider_error) => {
                            return self
                                .fail_with_sandbox_provider_error(
                                    sandbox_session,
                                    &sandbox_operation_id,
                                    SandboxSessionOperationKind::Start,
                                    SandboxSessionFailure::Provider,
                                    sandbox_provider_error,
                                    sandbox_session_lease,
                                )
                                .await;
                        }
                    };
                    sandbox_runtime_binding.set_sandbox_allocation_reference(
                        sandbox_allocation.sandbox_allocation_reference,
                    );
                    sandbox_session.set_sandbox_runtime_binding(sandbox_runtime_binding.clone());
                    sandbox_session = self
                        .persist_sandbox_session(sandbox_session, sandbox_session_lease)
                        .await?;
                }
                let sandbox_provider_readiness = match self
                    .execute_sandbox_provider_call(
                        &sandbox_provider,
                        SandboxProviderOperation::Start,
                        sandbox_session_lease,
                        sandbox_provider.start(Self::sandbox_start_request(
                            &sandbox_session,
                            &sandbox_runtime_binding,
                            sandbox_session_lease,
                        )?),
                    )
                    .await?
                {
                    Ok(sandbox_provider_readiness) => sandbox_provider_readiness,
                    Err(sandbox_provider_error) => {
                        if let Err(sandbox_cleanup_error) = self
                            .execute_sandbox_provider_call(
                                &sandbox_provider,
                                SandboxProviderOperation::Destroy,
                                sandbox_session_lease,
                                sandbox_provider.destroy(Self::sandbox_destroy_request(
                                    &sandbox_session,
                                    &sandbox_runtime_binding,
                                    sandbox_session_lease,
                                )?),
                            )
                            .await?
                        {
                            return self
                                .fail_with_sandbox_provider_error(
                                    sandbox_session,
                                    &sandbox_operation_id,
                                    SandboxSessionOperationKind::Start,
                                    SandboxSessionFailure::Cleanup,
                                    sandbox_cleanup_error,
                                    sandbox_session_lease,
                                )
                                .await;
                        }
                        sandbox_session.clear_sandbox_runtime_binding();
                        return self
                            .fail_with_sandbox_provider_error(
                                sandbox_session,
                                &sandbox_operation_id,
                                SandboxSessionOperationKind::Start,
                                SandboxSessionFailure::Provider,
                                sandbox_provider_error,
                                sandbox_session_lease,
                            )
                            .await;
                    }
                };
                if !sandbox_provider_readiness.is_sandbox_running_ready() {
                    if let Err(sandbox_cleanup_error) = self
                        .execute_sandbox_provider_call(
                            &sandbox_provider,
                            SandboxProviderOperation::Destroy,
                            sandbox_session_lease,
                            sandbox_provider.destroy(Self::sandbox_destroy_request(
                                &sandbox_session,
                                &sandbox_runtime_binding,
                                sandbox_session_lease,
                            )?),
                        )
                        .await?
                    {
                        return self
                            .fail_with_sandbox_provider_error(
                                sandbox_session,
                                &sandbox_operation_id,
                                SandboxSessionOperationKind::Start,
                                SandboxSessionFailure::Cleanup,
                                sandbox_cleanup_error,
                                sandbox_session_lease,
                            )
                            .await;
                    }
                    sandbox_session.clear_sandbox_runtime_binding();
                    sandbox_session.transition_sandbox_session(
                        SandboxSessionState::Failed,
                        SandboxSessionOperationKind::Start,
                    )?;
                    sandbox_session.fail_sandbox_operation(
                        &sandbox_operation_id,
                        SandboxSessionFailure::Readiness,
                    );
                    self.persist_sandbox_session(sandbox_session, sandbox_session_lease)
                        .await?;
                    return Err(SandboxLifecycleError::ProviderReadinessRejected {
                        sandbox_provider_id: sandbox_provider
                            .sandbox_provider_descriptor()
                            .sandbox_provider_id()
                            .clone(),
                    });
                }
                sandbox_session.transition_sandbox_session(
                    SandboxSessionState::Running,
                    SandboxSessionOperationKind::Start,
                )?;
                sandbox_session.complete_sandbox_operation(&sandbox_operation_id);
                self.persist_sandbox_session(sandbox_session, sandbox_session_lease)
                    .await
            }
            SandboxSessionState::Stopping => {
                let sandbox_operation_id = Self::in_progress_sandbox_operation_id(
                    &sandbox_session,
                    SandboxSessionOperationKind::Stop,
                )?;
                let sandbox_runtime_binding = sandbox_session
                    .sandbox_runtime_binding()
                    .cloned()
                    .ok_or(SandboxLifecycleError::InvariantViolation(
                        "stopping sandbox session has no sandbox runtime binding",
                    ))?;
                let sandbox_provider =
                    self.sandbox_provider_by_id(sandbox_runtime_binding.sandbox_provider_id())?;
                if let Err(sandbox_provider_error) = self
                    .execute_sandbox_provider_call(
                        &sandbox_provider,
                        SandboxProviderOperation::Stop,
                        sandbox_session_lease,
                        sandbox_provider.stop(Self::sandbox_stop_request(
                            &sandbox_session,
                            &sandbox_runtime_binding,
                            sandbox_session_lease,
                        )?),
                    )
                    .await?
                {
                    return self
                        .fail_with_sandbox_provider_error(
                            sandbox_session,
                            &sandbox_operation_id,
                            SandboxSessionOperationKind::Stop,
                            SandboxSessionFailure::Provider,
                            sandbox_provider_error,
                            sandbox_session_lease,
                        )
                        .await;
                }
                sandbox_session.transition_sandbox_session(
                    SandboxSessionState::Stopped,
                    SandboxSessionOperationKind::Stop,
                )?;
                sandbox_session.complete_sandbox_operation(&sandbox_operation_id);
                self.persist_sandbox_session(sandbox_session, sandbox_session_lease)
                    .await
            }
            SandboxSessionState::Destroying => {
                let sandbox_operation_id = Self::in_progress_sandbox_operation_id(
                    &sandbox_session,
                    SandboxSessionOperationKind::Destroy,
                )?;
                if let Some(sandbox_runtime_binding) =
                    sandbox_session.sandbox_runtime_binding().cloned()
                {
                    let sandbox_provider =
                        self.sandbox_provider_by_id(sandbox_runtime_binding.sandbox_provider_id())?;
                    if let Err(sandbox_provider_error) = self
                        .execute_sandbox_provider_call(
                            &sandbox_provider,
                            SandboxProviderOperation::Destroy,
                            sandbox_session_lease,
                            sandbox_provider.destroy(Self::sandbox_destroy_request(
                                &sandbox_session,
                                &sandbox_runtime_binding,
                                sandbox_session_lease,
                            )?),
                        )
                        .await?
                    {
                        return self
                            .fail_with_sandbox_provider_error(
                                sandbox_session,
                                &sandbox_operation_id,
                                SandboxSessionOperationKind::Destroy,
                                SandboxSessionFailure::Cleanup,
                                sandbox_provider_error,
                                sandbox_session_lease,
                            )
                            .await;
                    }
                    sandbox_session.clear_sandbox_runtime_binding();
                }
                sandbox_session.transition_sandbox_session(
                    SandboxSessionState::Destroyed,
                    SandboxSessionOperationKind::Destroy,
                )?;
                sandbox_session.complete_sandbox_operation(&sandbox_operation_id);
                self.persist_sandbox_session(sandbox_session, sandbox_session_lease)
                    .await
            }
            _ => Err(SandboxLifecycleError::InvariantViolation(
                "reconciler received a non-transient sandbox session",
            )),
        }
    }

    fn in_progress_sandbox_operation_id(
        sandbox_session: &SandboxSession,
        sandbox_operation_kind: SandboxSessionOperationKind,
    ) -> SandboxLifecycleResult<OperationId> {
        sandbox_session
            .sandbox_operations()
            .iter()
            .rev()
            .find(|sandbox_operation| {
                sandbox_operation.sandbox_operation_kind() == sandbox_operation_kind
                    && sandbox_operation.sandbox_operation_outcome()
                        == SandboxOperationOutcome::InProgress
            })
            .map(|sandbox_operation| sandbox_operation.sandbox_operation_id().clone())
            .ok_or(SandboxLifecycleError::InvariantViolation(
                "transient sandbox session has no in-progress lifecycle operation",
            ))
    }

    ///
    /// # Errors
    ///
    /// Returns `SandboxLifecycleError::IdempotencyConflict`,
    /// `InvalidTransition`, `OperationInProgress`, `OperationPreviouslyFailed`,
    /// `LeaseUnavailable`, `LeaseLost`, `NoEligibleProvider`,
    /// `NoHealthyProvider`, `ProviderReadinessRejected`, `Provider`, or
    /// `Repository`, depending on where the fenced start flow fails.
    pub async fn start_sandbox_session(
        &self,
        command: SandboxSessionLifecycleCommand,
    ) -> SandboxLifecycleResult<SandboxSession> {
        let sandbox_session_lease = self
            .acquire_sandbox_session_lease(&command.tenant_id, &command.sandbox_session_id)
            .await?;
        let sandbox_lifecycle_result = self
            .start_sandbox_session_with_lease(command, &sandbox_session_lease)
            .await;
        self.release_sandbox_session_lease(&sandbox_session_lease, sandbox_lifecycle_result)
            .await
    }

    async fn start_sandbox_session_with_lease(
        &self,
        command: SandboxSessionLifecycleCommand,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<SandboxSession> {
        let mut sandbox_session = self
            .get_sandbox_session(&command.tenant_id, &command.sandbox_session_id)
            .await?;
        if let Some(replayed_sandbox_result) = self.replay_sandbox_operation(
            &sandbox_session,
            &command.sandbox_operation_id,
            SandboxSessionOperationKind::Start,
        )? {
            return replayed_sandbox_result;
        }
        if !matches!(
            sandbox_session.sandbox_session_state(),
            SandboxSessionState::Created
                | SandboxSessionState::Stopped
                | SandboxSessionState::Failed
        ) {
            return Err(SandboxLifecycleError::InvalidTransition {
                sandbox_session_state: sandbox_session.sandbox_session_state(),
                sandbox_operation_kind: SandboxSessionOperationKind::Start,
            });
        }

        let previous_sandbox_runtime_binding = sandbox_session.sandbox_runtime_binding().cloned();
        let sandbox_provider = match previous_sandbox_runtime_binding.as_ref() {
            Some(previous_sandbox_runtime_binding)
                if previous_sandbox_runtime_binding
                    .sandbox_allocation_reference()
                    .is_none() =>
            {
                self.sandbox_provider_by_id(previous_sandbox_runtime_binding.sandbox_provider_id())?
            }
            _ => {
                self.select_sandbox_provider(&sandbox_session, sandbox_session_lease)
                    .await?
            }
        };
        if let Some(previous_sandbox_runtime_binding) = previous_sandbox_runtime_binding.as_ref() {
            if previous_sandbox_runtime_binding
                .sandbox_allocation_reference()
                .is_some()
            {
                let previous_sandbox_provider = self.sandbox_provider_by_id(
                    previous_sandbox_runtime_binding.sandbox_provider_id(),
                )?;
                let sandbox_destroy_request = Self::sandbox_destroy_request(
                    &sandbox_session,
                    previous_sandbox_runtime_binding,
                    sandbox_session_lease,
                )?;
                if let Err(sandbox_provider_error) = self
                    .execute_sandbox_provider_call(
                        &previous_sandbox_provider,
                        SandboxProviderOperation::Destroy,
                        sandbox_session_lease,
                        previous_sandbox_provider.destroy(sandbox_destroy_request),
                    )
                    .await?
                {
                    sandbox_session.begin_sandbox_operation(
                        command.sandbox_operation_id.clone(),
                        SandboxSessionOperationKind::Start,
                    );
                    sandbox_session.transition_sandbox_session(
                        SandboxSessionState::Starting,
                        SandboxSessionOperationKind::Start,
                    )?;
                    return self
                        .fail_with_sandbox_provider_error(
                            sandbox_session,
                            &command.sandbox_operation_id,
                            SandboxSessionOperationKind::Start,
                            SandboxSessionFailure::Cleanup,
                            sandbox_provider_error,
                            sandbox_session_lease,
                        )
                        .await;
                }
            }
        }

        let mut sandbox_runtime_binding = match previous_sandbox_runtime_binding {
            Some(previous_sandbox_runtime_binding)
                if previous_sandbox_runtime_binding
                    .sandbox_allocation_reference()
                    .is_none() =>
            {
                previous_sandbox_runtime_binding
            }
            _ => SandboxRuntimeBinding::new_intent(
                SandboxId::generate(),
                SandboxRuntimeBindingId::generate(),
                sandbox_provider
                    .sandbox_provider_descriptor()
                    .sandbox_provider_id()
                    .clone(),
            ),
        };
        sandbox_session.set_sandbox_runtime_binding(sandbox_runtime_binding.clone());
        sandbox_session.begin_sandbox_operation(
            command.sandbox_operation_id.clone(),
            SandboxSessionOperationKind::Start,
        );
        sandbox_session.transition_sandbox_session(
            SandboxSessionState::Starting,
            SandboxSessionOperationKind::Start,
        )?;
        let mut sandbox_session = self
            .persist_sandbox_session(sandbox_session, sandbox_session_lease)
            .await?;
        let sandbox_allocation = match self
            .execute_sandbox_provider_call(
                &sandbox_provider,
                SandboxProviderOperation::Allocate,
                sandbox_session_lease,
                sandbox_provider.allocate(SandboxProviderAllocationRequest {
                    tenant_id: sandbox_session.tenant_id().clone(),
                    sandbox_workspace_id: sandbox_session.sandbox_workspace_id().clone(),
                    sandbox_session_id: sandbox_session.sandbox_session_id().clone(),
                    sandbox_id: sandbox_runtime_binding.sandbox_id().clone(),
                    sandbox_runtime_binding_id: sandbox_runtime_binding
                        .sandbox_runtime_binding_id()
                        .clone(),
                    sandbox_fencing_token: sandbox_session_lease.sandbox_fencing_token(),
                    sandbox_required_capabilities: sandbox_session
                        .sandbox_required_capabilities()
                        .clone(),
                    sandbox_minimum_assurance: sandbox_session.sandbox_minimum_assurance(),
                }),
            )
            .await?
        {
            Ok(sandbox_allocation) => sandbox_allocation,
            Err(sandbox_provider_error) => {
                return self
                    .fail_with_sandbox_provider_error(
                        sandbox_session,
                        &command.sandbox_operation_id,
                        SandboxSessionOperationKind::Start,
                        SandboxSessionFailure::Provider,
                        sandbox_provider_error,
                        sandbox_session_lease,
                    )
                    .await;
            }
        };

        sandbox_runtime_binding
            .set_sandbox_allocation_reference(sandbox_allocation.sandbox_allocation_reference);
        sandbox_session.set_sandbox_runtime_binding(sandbox_runtime_binding.clone());
        sandbox_session = match self
            .persist_sandbox_session(sandbox_session, sandbox_session_lease)
            .await
        {
            Ok(sandbox_session) => sandbox_session,
            Err(sandbox_lifecycle_error) => {
                let _sandbox_cleanup_result = self
                    .execute_sandbox_provider_call(
                        &sandbox_provider,
                        SandboxProviderOperation::Destroy,
                        sandbox_session_lease,
                        sandbox_provider.destroy(Self::sandbox_destroy_request_for_values(
                            command.tenant_id,
                            command.sandbox_session_id,
                            &sandbox_runtime_binding,
                            sandbox_session_lease,
                        )?),
                    )
                    .await;
                return Err(sandbox_lifecycle_error);
            }
        };

        let sandbox_provider_readiness = match self
            .execute_sandbox_provider_call(
                &sandbox_provider,
                SandboxProviderOperation::Start,
                sandbox_session_lease,
                sandbox_provider.start(Self::sandbox_start_request(
                    &sandbox_session,
                    &sandbox_runtime_binding,
                    sandbox_session_lease,
                )?),
            )
            .await?
        {
            Ok(sandbox_provider_readiness) => sandbox_provider_readiness,
            Err(sandbox_provider_error) => {
                if let Err(sandbox_cleanup_error) = self
                    .execute_sandbox_provider_call(
                        &sandbox_provider,
                        SandboxProviderOperation::Destroy,
                        sandbox_session_lease,
                        sandbox_provider.destroy(Self::sandbox_destroy_request(
                            &sandbox_session,
                            &sandbox_runtime_binding,
                            sandbox_session_lease,
                        )?),
                    )
                    .await?
                {
                    return self
                        .fail_with_sandbox_provider_error(
                            sandbox_session,
                            &command.sandbox_operation_id,
                            SandboxSessionOperationKind::Start,
                            SandboxSessionFailure::Cleanup,
                            sandbox_cleanup_error,
                            sandbox_session_lease,
                        )
                        .await;
                }
                sandbox_session.clear_sandbox_runtime_binding();
                return self
                    .fail_with_sandbox_provider_error(
                        sandbox_session,
                        &command.sandbox_operation_id,
                        SandboxSessionOperationKind::Start,
                        SandboxSessionFailure::Provider,
                        sandbox_provider_error,
                        sandbox_session_lease,
                    )
                    .await;
            }
        };

        if !sandbox_provider_readiness.is_sandbox_running_ready() {
            if let Err(sandbox_cleanup_error) = self
                .execute_sandbox_provider_call(
                    &sandbox_provider,
                    SandboxProviderOperation::Destroy,
                    sandbox_session_lease,
                    sandbox_provider.destroy(Self::sandbox_destroy_request(
                        &sandbox_session,
                        &sandbox_runtime_binding,
                        sandbox_session_lease,
                    )?),
                )
                .await?
            {
                return self
                    .fail_with_sandbox_provider_error(
                        sandbox_session,
                        &command.sandbox_operation_id,
                        SandboxSessionOperationKind::Start,
                        SandboxSessionFailure::Cleanup,
                        sandbox_cleanup_error,
                        sandbox_session_lease,
                    )
                    .await;
            }
            sandbox_session.clear_sandbox_runtime_binding();
            sandbox_session.transition_sandbox_session(
                SandboxSessionState::Failed,
                SandboxSessionOperationKind::Start,
            )?;
            sandbox_session.fail_sandbox_operation(
                &command.sandbox_operation_id,
                SandboxSessionFailure::Readiness,
            );
            self.persist_sandbox_session(sandbox_session, sandbox_session_lease)
                .await?;
            return Err(SandboxLifecycleError::ProviderReadinessRejected {
                sandbox_provider_id: sandbox_provider
                    .sandbox_provider_descriptor()
                    .sandbox_provider_id()
                    .clone(),
            });
        }

        sandbox_session.transition_sandbox_session(
            SandboxSessionState::Running,
            SandboxSessionOperationKind::Start,
        )?;
        sandbox_session.complete_sandbox_operation(&command.sandbox_operation_id);
        self.persist_sandbox_session(sandbox_session, sandbox_session_lease)
            .await
    }

    ///
    /// # Errors
    ///
    /// Returns the same fenced-command error family as start
    /// (`IdempotencyConflict`, `InvalidTransition`, `OperationInProgress`,
    /// `OperationPreviouslyFailed`, `LeaseUnavailable`, `LeaseLost`,
    /// `Provider`, `Repository`) for the stop flow.
    pub async fn stop_sandbox_session(
        &self,
        command: SandboxSessionLifecycleCommand,
    ) -> SandboxLifecycleResult<SandboxSession> {
        let sandbox_session_lease = self
            .acquire_sandbox_session_lease(&command.tenant_id, &command.sandbox_session_id)
            .await?;
        let sandbox_lifecycle_result = self
            .stop_sandbox_session_with_lease(command, &sandbox_session_lease)
            .await;
        self.release_sandbox_session_lease(&sandbox_session_lease, sandbox_lifecycle_result)
            .await
    }

    async fn stop_sandbox_session_with_lease(
        &self,
        command: SandboxSessionLifecycleCommand,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<SandboxSession> {
        let mut sandbox_session = self
            .get_sandbox_session(&command.tenant_id, &command.sandbox_session_id)
            .await?;
        if let Some(replayed_sandbox_result) = self.replay_sandbox_operation(
            &sandbox_session,
            &command.sandbox_operation_id,
            SandboxSessionOperationKind::Stop,
        )? {
            return replayed_sandbox_result;
        }
        if sandbox_session.sandbox_session_state() != SandboxSessionState::Running {
            return Err(SandboxLifecycleError::InvalidTransition {
                sandbox_session_state: sandbox_session.sandbox_session_state(),
                sandbox_operation_kind: SandboxSessionOperationKind::Stop,
            });
        }
        let sandbox_runtime_binding = sandbox_session.sandbox_runtime_binding().cloned().ok_or(
            SandboxLifecycleError::InvariantViolation(
                "running sandbox session has no sandbox runtime binding",
            ),
        )?;
        let sandbox_provider =
            self.sandbox_provider_by_id(sandbox_runtime_binding.sandbox_provider_id())?;

        sandbox_session.begin_sandbox_operation(
            command.sandbox_operation_id.clone(),
            SandboxSessionOperationKind::Stop,
        );
        sandbox_session.transition_sandbox_session(
            SandboxSessionState::Stopping,
            SandboxSessionOperationKind::Stop,
        )?;
        let mut sandbox_session = self
            .persist_sandbox_session(sandbox_session, sandbox_session_lease)
            .await?;
        if let Err(sandbox_provider_error) = self
            .execute_sandbox_provider_call(
                &sandbox_provider,
                SandboxProviderOperation::Stop,
                sandbox_session_lease,
                sandbox_provider.stop(Self::sandbox_stop_request(
                    &sandbox_session,
                    &sandbox_runtime_binding,
                    sandbox_session_lease,
                )?),
            )
            .await?
        {
            return self
                .fail_with_sandbox_provider_error(
                    sandbox_session,
                    &command.sandbox_operation_id,
                    SandboxSessionOperationKind::Stop,
                    SandboxSessionFailure::Provider,
                    sandbox_provider_error,
                    sandbox_session_lease,
                )
                .await;
        }
        sandbox_session.transition_sandbox_session(
            SandboxSessionState::Stopped,
            SandboxSessionOperationKind::Stop,
        )?;
        sandbox_session.complete_sandbox_operation(&command.sandbox_operation_id);
        self.persist_sandbox_session(sandbox_session, sandbox_session_lease)
            .await
    }

    ///
    /// # Errors
    ///
    /// Returns the same fenced-command error family as stop, plus
    /// cleanup-failure classification (`SandboxSessionFailure::Cleanup`)
    /// when the compensating destroy fails.
    pub async fn destroy_sandbox_session(
        &self,
        command: SandboxSessionLifecycleCommand,
    ) -> SandboxLifecycleResult<SandboxSession> {
        let sandbox_session_lease = self
            .acquire_sandbox_session_lease(&command.tenant_id, &command.sandbox_session_id)
            .await?;
        let sandbox_lifecycle_result = self
            .destroy_sandbox_session_with_lease(command, &sandbox_session_lease)
            .await;
        self.release_sandbox_session_lease(&sandbox_session_lease, sandbox_lifecycle_result)
            .await
    }

    async fn destroy_sandbox_session_with_lease(
        &self,
        command: SandboxSessionLifecycleCommand,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<SandboxSession> {
        let mut sandbox_session = self
            .get_sandbox_session(&command.tenant_id, &command.sandbox_session_id)
            .await?;
        if let Some(replayed_sandbox_result) = self.replay_sandbox_operation(
            &sandbox_session,
            &command.sandbox_operation_id,
            SandboxSessionOperationKind::Destroy,
        )? {
            return replayed_sandbox_result;
        }
        if !matches!(
            sandbox_session.sandbox_session_state(),
            SandboxSessionState::Created
                | SandboxSessionState::Stopped
                | SandboxSessionState::Failed
        ) {
            return Err(SandboxLifecycleError::InvalidTransition {
                sandbox_session_state: sandbox_session.sandbox_session_state(),
                sandbox_operation_kind: SandboxSessionOperationKind::Destroy,
            });
        }
        let sandbox_runtime_binding = sandbox_session.sandbox_runtime_binding().cloned();
        let sandbox_provider = sandbox_runtime_binding
            .as_ref()
            .map(|sandbox_runtime_binding| {
                self.sandbox_provider_by_id(sandbox_runtime_binding.sandbox_provider_id())
            })
            .transpose()?;

        sandbox_session.begin_sandbox_operation(
            command.sandbox_operation_id.clone(),
            SandboxSessionOperationKind::Destroy,
        );
        sandbox_session.transition_sandbox_session(
            SandboxSessionState::Destroying,
            SandboxSessionOperationKind::Destroy,
        )?;
        let mut sandbox_session = self
            .persist_sandbox_session(sandbox_session, sandbox_session_lease)
            .await?;
        if let (Some(sandbox_runtime_binding), Some(sandbox_provider)) =
            (sandbox_runtime_binding, sandbox_provider)
        {
            if let Err(sandbox_provider_error) = self
                .execute_sandbox_provider_call(
                    &sandbox_provider,
                    SandboxProviderOperation::Destroy,
                    sandbox_session_lease,
                    sandbox_provider.destroy(Self::sandbox_destroy_request(
                        &sandbox_session,
                        &sandbox_runtime_binding,
                        sandbox_session_lease,
                    )?),
                )
                .await?
            {
                return self
                    .fail_with_sandbox_provider_error(
                        sandbox_session,
                        &command.sandbox_operation_id,
                        SandboxSessionOperationKind::Destroy,
                        SandboxSessionFailure::Cleanup,
                        sandbox_provider_error,
                        sandbox_session_lease,
                    )
                    .await;
            }
            sandbox_session.clear_sandbox_runtime_binding();
        }
        sandbox_session.transition_sandbox_session(
            SandboxSessionState::Destroyed,
            SandboxSessionOperationKind::Destroy,
        )?;
        sandbox_session.complete_sandbox_operation(&command.sandbox_operation_id);
        self.persist_sandbox_session(sandbox_session, sandbox_session_lease)
            .await
    }

    fn replay_sandbox_operation(
        &self,
        sandbox_session: &SandboxSession,
        sandbox_operation_id: &OperationId,
        sandbox_operation_kind: SandboxSessionOperationKind,
    ) -> SandboxLifecycleResult<Option<SandboxLifecycleResult<SandboxSession>>> {
        let Some(sandbox_operation_outcome) = sandbox_session
            .replay_sandbox_operation(sandbox_operation_id, sandbox_operation_kind)?
        else {
            return Ok(None);
        };
        let replayed_sandbox_result = match sandbox_operation_outcome {
            SandboxOperationOutcome::Succeeded => Ok(sandbox_session.clone()),
            SandboxOperationOutcome::InProgress => {
                Err(SandboxLifecycleError::OperationInProgress {
                    sandbox_operation_id: sandbox_operation_id.clone(),
                })
            }
            SandboxOperationOutcome::Failed(sandbox_session_failure) => {
                Err(SandboxLifecycleError::OperationPreviouslyFailed {
                    sandbox_operation_id: sandbox_operation_id.clone(),
                    sandbox_session_failure,
                })
            }
        };
        Ok(Some(replayed_sandbox_result))
    }

    async fn select_sandbox_provider(
        &self,
        sandbox_session: &SandboxSession,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<Arc<dyn SandboxProvider>> {
        let mut found_eligible_sandbox_provider = false;
        for sandbox_provider in &self.sandbox_providers {
            if !sandbox_provider
                .sandbox_provider_descriptor()
                .satisfies_sandbox_requirements(
                    sandbox_session.sandbox_required_capabilities(),
                    sandbox_session.sandbox_minimum_assurance(),
                )
            {
                continue;
            }
            found_eligible_sandbox_provider = true;
            // Health probing is the only provider call that repeats inside one
            // command, so it goes through the same lease-renewing wrapper as
            // every other call: with the policy's maximum provider timeout, a
            // handful of unhealthy providers would otherwise consume the whole
            // lease window and surface as LeaseLost on the allocate that
            // follows. A provider that times out or errors here is unhealthy
            // for this request, not a lifecycle failure; the next eligible
            // provider still gets probed.
            let sandbox_provider_health = match self
                .execute_sandbox_provider_call(
                    sandbox_provider,
                    SandboxProviderOperation::Health,
                    sandbox_session_lease,
                    sandbox_provider.sandbox_provider_health(),
                )
                .await
            {
                Ok(Ok(sandbox_provider_health)) => sandbox_provider_health,
                Ok(Err(_)) => continue,
                Err(sandbox_lifecycle_error) => return Err(sandbox_lifecycle_error),
            };
            if sandbox_provider_health.sandbox_provider_health_status
                == SandboxProviderHealthStatus::Ready
            {
                return Ok(Arc::clone(sandbox_provider));
            }
        }
        if found_eligible_sandbox_provider {
            Err(SandboxLifecycleError::NoHealthyProvider)
        } else {
            Err(SandboxLifecycleError::NoEligibleProvider)
        }
    }

    fn sandbox_provider_by_id(
        &self,
        sandbox_provider_id: &SandboxProviderId,
    ) -> SandboxLifecycleResult<Arc<dyn SandboxProvider>> {
        self.sandbox_providers
            .iter()
            .find(|sandbox_provider| {
                sandbox_provider
                    .sandbox_provider_descriptor()
                    .sandbox_provider_id()
                    == sandbox_provider_id
            })
            .map(Arc::clone)
            .ok_or(SandboxLifecycleError::InvariantViolation(
                "sandbox runtime binding references an unregistered sandbox provider",
            ))
    }

    async fn persist_sandbox_session(
        &self,
        mut sandbox_session: SandboxSession,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<SandboxSession> {
        let expected_sandbox_version = sandbox_session.next_sandbox_version()?;
        match self
            .sandbox_session_repository
            .save_sandbox_session(
                sandbox_session.clone(),
                expected_sandbox_version,
                sandbox_session_lease,
            )
            .await
        {
            Ok(()) => Ok(sandbox_session),
            Err(SandboxSessionRepositoryError::LeaseConflict) => {
                Err(SandboxLifecycleError::LeaseLost)
            }
            Err(sandbox_repository_error) => Err(sandbox_repository_error.into()),
        }
    }

    async fn fail_with_sandbox_provider_error(
        &self,
        mut sandbox_session: SandboxSession,
        sandbox_operation_id: &OperationId,
        sandbox_operation_kind: SandboxSessionOperationKind,
        sandbox_session_failure: SandboxSessionFailure,
        sandbox_provider_error: SandboxProviderError,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<SandboxSession> {
        sandbox_session
            .transition_sandbox_session(SandboxSessionState::Failed, sandbox_operation_kind)?;
        sandbox_session.fail_sandbox_operation(sandbox_operation_id, sandbox_session_failure);
        self.persist_sandbox_session(sandbox_session, sandbox_session_lease)
            .await?;
        Err(sandbox_provider_error.into())
    }

    fn sandbox_start_request(
        sandbox_session: &SandboxSession,
        sandbox_runtime_binding: &SandboxRuntimeBinding,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<SandboxProviderStartRequest> {
        Ok(SandboxProviderStartRequest {
            tenant_id: sandbox_session.tenant_id().clone(),
            sandbox_workspace_id: sandbox_session.sandbox_workspace_id().clone(),
            sandbox_session_id: sandbox_session.sandbox_session_id().clone(),
            sandbox_id: sandbox_runtime_binding.sandbox_id().clone(),
            sandbox_runtime_binding_id: sandbox_runtime_binding
                .sandbox_runtime_binding_id()
                .clone(),
            sandbox_fencing_token: sandbox_session_lease.sandbox_fencing_token(),
            sandbox_allocation_reference: sandbox_runtime_binding
                .sandbox_allocation_reference()
                .ok_or(SandboxLifecycleError::InvariantViolation(
                    "sandbox runtime binding has no provider allocation reference",
                ))?
                .clone(),
        })
    }

    fn sandbox_stop_request(
        sandbox_session: &SandboxSession,
        sandbox_runtime_binding: &SandboxRuntimeBinding,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<SandboxProviderStopRequest> {
        Ok(SandboxProviderStopRequest {
            tenant_id: sandbox_session.tenant_id().clone(),
            sandbox_session_id: sandbox_session.sandbox_session_id().clone(),
            sandbox_id: sandbox_runtime_binding.sandbox_id().clone(),
            sandbox_runtime_binding_id: sandbox_runtime_binding
                .sandbox_runtime_binding_id()
                .clone(),
            sandbox_fencing_token: sandbox_session_lease.sandbox_fencing_token(),
            sandbox_allocation_reference: sandbox_runtime_binding
                .sandbox_allocation_reference()
                .ok_or(SandboxLifecycleError::InvariantViolation(
                    "sandbox runtime binding has no provider allocation reference",
                ))?
                .clone(),
        })
    }

    fn sandbox_destroy_request(
        sandbox_session: &SandboxSession,
        sandbox_runtime_binding: &SandboxRuntimeBinding,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<SandboxProviderDestroyRequest> {
        Self::sandbox_destroy_request_for_values(
            sandbox_session.tenant_id().clone(),
            sandbox_session.sandbox_session_id().clone(),
            sandbox_runtime_binding,
            sandbox_session_lease,
        )
    }

    fn sandbox_destroy_request_for_values(
        tenant_id: TenantId,
        sandbox_session_id: SandboxSessionId,
        sandbox_runtime_binding: &SandboxRuntimeBinding,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxLifecycleResult<SandboxProviderDestroyRequest> {
        Ok(SandboxProviderDestroyRequest {
            tenant_id,
            sandbox_session_id,
            sandbox_id: sandbox_runtime_binding.sandbox_id().clone(),
            sandbox_runtime_binding_id: sandbox_runtime_binding
                .sandbox_runtime_binding_id()
                .clone(),
            sandbox_fencing_token: sandbox_session_lease.sandbox_fencing_token(),
            sandbox_allocation_reference: sandbox_runtime_binding
                .sandbox_allocation_reference()
                .cloned(),
        })
    }
}

#[async_trait]
impl SandboxSessionLifecyclePort for SandboxLifecycleService {
    async fn create_sandbox_session(
        &self,
        command: CreateSandboxSessionCommand,
    ) -> SandboxLifecycleResult<SandboxSession> {
        SandboxLifecycleService::create_sandbox_session(self, command).await
    }

    async fn get_sandbox_session(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
    ) -> SandboxLifecycleResult<SandboxSession> {
        SandboxLifecycleService::get_sandbox_session(self, tenant_id, sandbox_session_id).await
    }

    async fn start_sandbox_session(
        &self,
        command: SandboxSessionLifecycleCommand,
    ) -> SandboxLifecycleResult<SandboxSession> {
        SandboxLifecycleService::start_sandbox_session(self, command).await
    }

    async fn stop_sandbox_session(
        &self,
        command: SandboxSessionLifecycleCommand,
    ) -> SandboxLifecycleResult<SandboxSession> {
        SandboxLifecycleService::stop_sandbox_session(self, command).await
    }

    async fn destroy_sandbox_session(
        &self,
        command: SandboxSessionLifecycleCommand,
    ) -> SandboxLifecycleResult<SandboxSession> {
        SandboxLifecycleService::destroy_sandbox_session(self, command).await
    }
}
