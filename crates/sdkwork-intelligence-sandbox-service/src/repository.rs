use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    IsolationAssurance, OperationId, RuntimeCapability, SandboxFencingToken, SandboxId,
    SandboxLeaseOwnerId, SandboxProviderAllocationRef, SandboxProviderId, SandboxRuntimeBindingId,
    SandboxSessionId, SandboxWorkspaceId, TenantId,
};
use thiserror::Error;

use crate::{
    model::MAX_SANDBOX_SESSION_VERSION, SandboxOperationOutcome, SandboxRuntimeBinding,
    SandboxSession, SandboxSessionFailure, SandboxSessionOperation, SandboxSessionOperationKind,
    SandboxSessionReconciliationCandidate, SandboxSessionState,
};

fn is_safe_sandbox_allocation_key_id(sandbox_allocation_key_id: &str) -> bool {
    !sandbox_allocation_key_id.is_empty()
        && sandbox_allocation_key_id.len() <= 128
        && sandbox_allocation_key_id
            .bytes()
            .all(|sandbox_key_id_byte| sandbox_key_id_byte.is_ascii_graphic())
}

pub type SandboxSessionRepositoryResult<T> = Result<T, SandboxSessionRepositoryError>;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SandboxSessionRepositoryError {
    #[error("sandbox session was not found")]
    NotFound,
    #[error("sandbox session version conflict")]
    VersionConflict,
    #[error("sandbox operation id already belongs to another sandbox session")]
    DuplicateOperation,
    #[error("sandbox session repository is unavailable")]
    Unavailable,
    #[error("sandbox session repository contains invalid persisted data")]
    InvalidStoredData,
    #[error("sandbox provider allocation protection failed")]
    ProtectionFailed,
    #[error("sandbox session repository requires a PostgreSQL database pool")]
    UnsupportedDatabaseEngine,
    #[error("sandbox session lease is owned by another controller or has expired")]
    LeaseConflict,
    #[error("sandbox repository page request is invalid")]
    InvalidPageRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxSessionLease {
    tenant_id: TenantId,
    sandbox_session_id: SandboxSessionId,
    sandbox_lease_owner_id: SandboxLeaseOwnerId,
    sandbox_fencing_token: SandboxFencingToken,
    sandbox_lease_expires_at_unix_millis: i64,
}

impl SandboxSessionLease {
    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::InvalidStoredData` when the
    /// expiry violates the lease bounds.
    pub fn new(
        tenant_id: TenantId,
        sandbox_session_id: SandboxSessionId,
        sandbox_lease_owner_id: SandboxLeaseOwnerId,
        sandbox_fencing_token: SandboxFencingToken,
        sandbox_lease_expires_at_unix_millis: i64,
    ) -> SandboxSessionRepositoryResult<Self> {
        if sandbox_lease_expires_at_unix_millis <= 0 {
            return Err(SandboxSessionRepositoryError::InvalidStoredData);
        }
        Ok(Self {
            tenant_id,
            sandbox_session_id,
            sandbox_lease_owner_id,
            sandbox_fencing_token,
            sandbox_lease_expires_at_unix_millis,
        })
    }

    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    #[must_use]
    pub fn sandbox_session_id(&self) -> &SandboxSessionId {
        &self.sandbox_session_id
    }

    #[must_use]
    pub fn sandbox_lease_owner_id(&self) -> &SandboxLeaseOwnerId {
        &self.sandbox_lease_owner_id
    }

    #[must_use]
    pub fn sandbox_fencing_token(&self) -> SandboxFencingToken {
        self.sandbox_fencing_token
    }

    #[must_use]
    pub fn sandbox_lease_expires_at_unix_millis(&self) -> i64 {
        self.sandbox_lease_expires_at_unix_millis
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProviderAllocationProtectionContext {
    tenant_id: TenantId,
    sandbox_session_id: SandboxSessionId,
    sandbox_runtime_binding_id: SandboxRuntimeBindingId,
}

impl SandboxProviderAllocationProtectionContext {
    #[must_use]
    pub fn for_repository(
        tenant_id: TenantId,
        sandbox_session_id: SandboxSessionId,
        sandbox_runtime_binding_id: SandboxRuntimeBindingId,
    ) -> Self {
        Self {
            tenant_id,
            sandbox_session_id,
            sandbox_runtime_binding_id,
        }
    }

    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    #[must_use]
    pub fn sandbox_session_id(&self) -> &SandboxSessionId {
        &self.sandbox_session_id
    }

    #[must_use]
    pub fn sandbox_runtime_binding_id(&self) -> &SandboxRuntimeBindingId {
        &self.sandbox_runtime_binding_id
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct SandboxProtectedProviderAllocationRef {
    sandbox_allocation_ciphertext: String,
    sandbox_allocation_key_id: String,
    sandbox_allocation_key_version: u64,
    sandbox_allocation_crypto_version: u16,
}

impl SandboxProtectedProviderAllocationRef {
    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::InvalidStoredData` when the
    /// protected reference text violates its bounds.
    pub fn new(
        sandbox_allocation_ciphertext: impl Into<String>,
        sandbox_allocation_key_id: impl Into<String>,
        sandbox_allocation_key_version: u64,
        sandbox_allocation_crypto_version: u16,
    ) -> SandboxSessionRepositoryResult<Self> {
        let sandbox_allocation_ciphertext = sandbox_allocation_ciphertext.into();
        let sandbox_allocation_key_id = sandbox_allocation_key_id.into();
        if sandbox_allocation_ciphertext.is_empty()
            || sandbox_allocation_ciphertext.len() > 8_192
            || !is_safe_sandbox_allocation_key_id(&sandbox_allocation_key_id)
            || !(1..=i64::MAX as u64).contains(&sandbox_allocation_key_version)
            || !(1..=i16::MAX as u16).contains(&sandbox_allocation_crypto_version)
        {
            return Err(SandboxSessionRepositoryError::InvalidStoredData);
        }
        Ok(Self {
            sandbox_allocation_ciphertext,
            sandbox_allocation_key_id,
            sandbox_allocation_key_version,
            sandbox_allocation_crypto_version,
        })
    }

    #[must_use]
    pub fn sandbox_allocation_ciphertext(&self) -> &str {
        &self.sandbox_allocation_ciphertext
    }

    #[must_use]
    pub fn sandbox_allocation_key_id(&self) -> &str {
        &self.sandbox_allocation_key_id
    }

    #[must_use]
    pub fn sandbox_allocation_key_version(&self) -> u64 {
        self.sandbox_allocation_key_version
    }

    #[must_use]
    pub fn sandbox_allocation_crypto_version(&self) -> u16 {
        self.sandbox_allocation_crypto_version
    }
}

impl fmt::Debug for SandboxProtectedProviderAllocationRef {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SandboxProtectedProviderAllocationRef")
            .field("sandbox_allocation_ciphertext", &"[REDACTED]")
            .field("sandbox_allocation_key_id", &self.sandbox_allocation_key_id)
            .field(
                "sandbox_allocation_key_version",
                &self.sandbox_allocation_key_version,
            )
            .field(
                "sandbox_allocation_crypto_version",
                &self.sandbox_allocation_crypto_version,
            )
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProviderAllocationProtectionVersion {
    sandbox_allocation_key_id: String,
    sandbox_allocation_key_version: u64,
    sandbox_allocation_crypto_version: u16,
}

impl SandboxProviderAllocationProtectionVersion {
    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::ProtectionFailed` when the
    /// protection version is outside the accepted range.
    pub fn new(
        sandbox_allocation_key_id: impl Into<String>,
        sandbox_allocation_key_version: u64,
        sandbox_allocation_crypto_version: u16,
    ) -> SandboxSessionRepositoryResult<Self> {
        let sandbox_allocation_key_id = sandbox_allocation_key_id.into();
        if !is_safe_sandbox_allocation_key_id(&sandbox_allocation_key_id)
            || !(1..=i64::MAX as u64).contains(&sandbox_allocation_key_version)
            || !(1..=i16::MAX as u16).contains(&sandbox_allocation_crypto_version)
        {
            return Err(SandboxSessionRepositoryError::ProtectionFailed);
        }
        Ok(Self {
            sandbox_allocation_key_id,
            sandbox_allocation_key_version,
            sandbox_allocation_crypto_version,
        })
    }

    #[must_use]
    pub fn sandbox_allocation_key_id(&self) -> &str {
        &self.sandbox_allocation_key_id
    }

    #[must_use]
    pub fn sandbox_allocation_key_version(&self) -> u64 {
        self.sandbox_allocation_key_version
    }

    #[must_use]
    pub fn sandbox_allocation_crypto_version(&self) -> u16 {
        self.sandbox_allocation_crypto_version
    }

    #[must_use]
    pub fn matches_sandbox_protected_allocation_reference(
        &self,
        sandbox_protected_allocation_reference: &SandboxProtectedProviderAllocationRef,
    ) -> bool {
        self.sandbox_allocation_key_id
            == sandbox_protected_allocation_reference.sandbox_allocation_key_id()
            && self.sandbox_allocation_key_version
                == sandbox_protected_allocation_reference.sandbox_allocation_key_version()
            && self.sandbox_allocation_crypto_version
                == sandbox_protected_allocation_reference.sandbox_allocation_crypto_version()
    }
}

pub trait SandboxProviderAllocationProtector: Send + Sync {
    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::ProtectionFailed` when the
    /// current protection version cannot be resolved from the key source.
    fn current_sandbox_allocation_protection_version(
        &self,
    ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationProtectionVersion>;

    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::ProtectionFailed` when the
    /// allocation reference cannot be protected under the context.
    fn protect_sandbox_allocation_reference(
        &self,
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
        sandbox_allocation_reference: &SandboxProviderAllocationRef,
    ) -> SandboxSessionRepositoryResult<SandboxProtectedProviderAllocationRef>;

    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::ProtectionFailed` when the
    /// protected reference cannot be restored under the context.
    fn restore_sandbox_allocation_reference(
        &self,
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
        sandbox_protected_allocation_reference: &SandboxProtectedProviderAllocationRef,
    ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationRef>;

    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::ProtectionFailed` when the
    /// reference cannot be re-encrypted under the target key version.
    fn reencrypt_sandbox_allocation_reference(
        &self,
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
        sandbox_protected_allocation_reference: &SandboxProtectedProviderAllocationRef,
    ) -> SandboxSessionRepositoryResult<SandboxProtectedProviderAllocationRef>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxSessionOperationRepositorySnapshot {
    sandbox_operation_id: OperationId,
    sandbox_operation_kind: SandboxSessionOperationKind,
    sandbox_operation_outcome: SandboxOperationOutcome,
}

impl SandboxSessionOperationRepositorySnapshot {
    #[must_use]
    pub fn new(
        sandbox_operation_id: OperationId,
        sandbox_operation_kind: SandboxSessionOperationKind,
        sandbox_operation_outcome: SandboxOperationOutcome,
    ) -> Self {
        Self {
            sandbox_operation_id,
            sandbox_operation_kind,
            sandbox_operation_outcome,
        }
    }

    #[must_use]
    pub fn sandbox_operation_id(&self) -> &OperationId {
        &self.sandbox_operation_id
    }

    #[must_use]
    pub fn sandbox_operation_kind(&self) -> SandboxSessionOperationKind {
        self.sandbox_operation_kind
    }

    #[must_use]
    pub fn sandbox_operation_outcome(&self) -> SandboxOperationOutcome {
        self.sandbox_operation_outcome
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxRuntimeBindingRepositorySnapshot {
    sandbox_id: SandboxId,
    sandbox_runtime_binding_id: SandboxRuntimeBindingId,
    sandbox_provider_id: SandboxProviderId,
    sandbox_protected_allocation_reference: Option<SandboxProtectedProviderAllocationRef>,
}

impl SandboxRuntimeBindingRepositorySnapshot {
    #[must_use]
    pub fn new(
        sandbox_id: SandboxId,
        sandbox_runtime_binding_id: SandboxRuntimeBindingId,
        sandbox_provider_id: SandboxProviderId,
        sandbox_protected_allocation_reference: Option<SandboxProtectedProviderAllocationRef>,
    ) -> Self {
        Self {
            sandbox_id,
            sandbox_runtime_binding_id,
            sandbox_provider_id,
            sandbox_protected_allocation_reference,
        }
    }

    #[must_use]
    pub fn sandbox_id(&self) -> &SandboxId {
        &self.sandbox_id
    }

    #[must_use]
    pub fn sandbox_runtime_binding_id(&self) -> &SandboxRuntimeBindingId {
        &self.sandbox_runtime_binding_id
    }

    #[must_use]
    pub fn sandbox_provider_id(&self) -> &SandboxProviderId {
        &self.sandbox_provider_id
    }

    #[must_use]
    pub fn sandbox_protected_allocation_reference(
        &self,
    ) -> Option<&SandboxProtectedProviderAllocationRef> {
        self.sandbox_protected_allocation_reference.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxSessionRepositorySnapshot {
    tenant_id: TenantId,
    sandbox_workspace_id: SandboxWorkspaceId,
    sandbox_session_id: SandboxSessionId,
    sandbox_session_state: SandboxSessionState,
    sandbox_required_capabilities: BTreeSet<RuntimeCapability>,
    sandbox_minimum_assurance: IsolationAssurance,
    sandbox_runtime_binding: Option<SandboxRuntimeBindingRepositorySnapshot>,
    sandbox_last_failure: Option<SandboxSessionFailure>,
    sandbox_operations: Vec<SandboxSessionOperationRepositorySnapshot>,
    sandbox_version: u64,
}

impl SandboxSessionRepositorySnapshot {
    fn validate_sandbox_persisted_invariants(&self) -> SandboxSessionRepositoryResult<()> {
        validate_sandbox_persisted_state(
            self.sandbox_session_state,
            self.sandbox_last_failure,
            self.sandbox_version,
            self.sandbox_runtime_binding.is_some(),
            self.sandbox_runtime_binding
                .as_ref()
                .is_some_and(|sandbox_runtime_binding| {
                    sandbox_runtime_binding
                        .sandbox_protected_allocation_reference
                        .is_some()
                }),
            &self.sandbox_operations,
        )
    }
}

/// The shared persisted-state invariant core behind
/// [`SandboxSessionRepositorySnapshot::validate_sandbox_persisted_invariants`]
/// and [`validate_sandbox_session_persisted_invariants`].
fn validate_sandbox_persisted_state(
    sandbox_session_state: SandboxSessionState,
    sandbox_last_failure: Option<SandboxSessionFailure>,
    sandbox_version: u64,
    sandbox_has_runtime_binding: bool,
    sandbox_has_allocation_reference: bool,
    sandbox_operations: &[SandboxSessionOperationRepositorySnapshot],
) -> SandboxSessionRepositoryResult<()> {
    if sandbox_version > MAX_SANDBOX_SESSION_VERSION {
        return Err(SandboxSessionRepositoryError::InvalidStoredData);
    }
    let mut sandbox_operation_ids = BTreeSet::new();
    let mut sandbox_operations = sandbox_operations.iter();
    let Some(sandbox_create_operation) = sandbox_operations.next() else {
        return Err(SandboxSessionRepositoryError::InvalidStoredData);
    };
    if sandbox_create_operation.sandbox_operation_kind() != SandboxSessionOperationKind::Create
        || sandbox_create_operation.sandbox_operation_outcome()
            != SandboxOperationOutcome::Succeeded
        || !sandbox_operation_ids.insert(sandbox_create_operation.sandbox_operation_id())
    {
        return Err(SandboxSessionRepositoryError::InvalidStoredData);
    }

    let mut replayed_sandbox_session_state = SandboxSessionState::Created;
    let mut replayed_sandbox_last_failure = None;
    for sandbox_operation in sandbox_operations {
        if !sandbox_operation_ids.insert(sandbox_operation.sandbox_operation_id()) {
            return Err(SandboxSessionRepositoryError::InvalidStoredData);
        }
        let sandbox_operation_kind = sandbox_operation.sandbox_operation_kind();
        let sandbox_operation_outcome = sandbox_operation.sandbox_operation_outcome();
        let sandbox_failure_kind_is_valid = matches!(
            (sandbox_operation_kind, sandbox_operation_outcome),
            (
                SandboxSessionOperationKind::Start,
                SandboxOperationOutcome::InProgress | SandboxOperationOutcome::Succeeded
            ) | (
                SandboxSessionOperationKind::Start,
                SandboxOperationOutcome::Failed(
                    SandboxSessionFailure::Provider
                        | SandboxSessionFailure::Readiness
                        | SandboxSessionFailure::Cleanup
                )
            ) | (
                SandboxSessionOperationKind::Stop,
                SandboxOperationOutcome::InProgress | SandboxOperationOutcome::Succeeded
            ) | (
                SandboxSessionOperationKind::Stop,
                SandboxOperationOutcome::Failed(SandboxSessionFailure::Provider)
            ) | (
                SandboxSessionOperationKind::Destroy,
                SandboxOperationOutcome::InProgress | SandboxOperationOutcome::Succeeded
            ) | (
                SandboxSessionOperationKind::Destroy,
                SandboxOperationOutcome::Failed(SandboxSessionFailure::Cleanup)
            )
        );
        if !sandbox_failure_kind_is_valid {
            return Err(SandboxSessionRepositoryError::InvalidStoredData);
        }

        replayed_sandbox_session_state = match (
            replayed_sandbox_session_state,
            sandbox_operation_kind,
            sandbox_operation_outcome,
        ) {
            (
                SandboxSessionState::Created
                | SandboxSessionState::Stopped
                | SandboxSessionState::Failed,
                SandboxSessionOperationKind::Start,
                SandboxOperationOutcome::InProgress,
            ) => SandboxSessionState::Starting,
            (
                SandboxSessionState::Created
                | SandboxSessionState::Stopped
                | SandboxSessionState::Failed,
                SandboxSessionOperationKind::Start,
                SandboxOperationOutcome::Succeeded,
            ) => SandboxSessionState::Running,
            (
                SandboxSessionState::Created
                | SandboxSessionState::Stopped
                | SandboxSessionState::Failed,
                SandboxSessionOperationKind::Start,
                SandboxOperationOutcome::Failed(_),
            ) => SandboxSessionState::Failed,
            (
                SandboxSessionState::Running,
                SandboxSessionOperationKind::Stop,
                SandboxOperationOutcome::InProgress,
            ) => SandboxSessionState::Stopping,
            (
                SandboxSessionState::Running,
                SandboxSessionOperationKind::Stop,
                SandboxOperationOutcome::Succeeded,
            ) => SandboxSessionState::Stopped,
            (
                SandboxSessionState::Running,
                SandboxSessionOperationKind::Stop,
                SandboxOperationOutcome::Failed(_),
            ) => SandboxSessionState::Failed,
            (
                SandboxSessionState::Created
                | SandboxSessionState::Stopped
                | SandboxSessionState::Failed,
                SandboxSessionOperationKind::Destroy,
                SandboxOperationOutcome::InProgress,
            ) => SandboxSessionState::Destroying,
            (
                SandboxSessionState::Created
                | SandboxSessionState::Stopped
                | SandboxSessionState::Failed,
                SandboxSessionOperationKind::Destroy,
                SandboxOperationOutcome::Succeeded,
            ) => SandboxSessionState::Destroyed,
            (
                SandboxSessionState::Created
                | SandboxSessionState::Stopped
                | SandboxSessionState::Failed,
                SandboxSessionOperationKind::Destroy,
                SandboxOperationOutcome::Failed(_),
            ) => SandboxSessionState::Failed,
            _ => return Err(SandboxSessionRepositoryError::InvalidStoredData),
        };
        replayed_sandbox_last_failure = match sandbox_operation_outcome {
            SandboxOperationOutcome::Failed(sandbox_session_failure) => {
                Some(sandbox_session_failure)
            }
            SandboxOperationOutcome::InProgress | SandboxOperationOutcome::Succeeded => None,
        };
    }

    if replayed_sandbox_session_state != sandbox_session_state
        || replayed_sandbox_last_failure != sandbox_last_failure
    {
        return Err(SandboxSessionRepositoryError::InvalidStoredData);
    }

    let sandbox_state_is_consistent = match sandbox_session_state {
        SandboxSessionState::Created => !sandbox_has_runtime_binding,
        SandboxSessionState::Starting => sandbox_has_runtime_binding,
        SandboxSessionState::Running => sandbox_has_allocation_reference,
        SandboxSessionState::Stopping => sandbox_has_allocation_reference,
        SandboxSessionState::Stopped => sandbox_has_allocation_reference,
        SandboxSessionState::Failed => true,
        SandboxSessionState::Destroying => true,
        SandboxSessionState::Destroyed => !sandbox_has_runtime_binding,
    };
    if !sandbox_state_is_consistent {
        return Err(SandboxSessionRepositoryError::InvalidStoredData);
    }

    Ok(())
}

impl SandboxSessionRepositorySnapshot {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        tenant_id: TenantId,
        sandbox_workspace_id: SandboxWorkspaceId,
        sandbox_session_id: SandboxSessionId,
        sandbox_session_state: SandboxSessionState,
        sandbox_required_capabilities: BTreeSet<RuntimeCapability>,
        sandbox_minimum_assurance: IsolationAssurance,
        sandbox_runtime_binding: Option<SandboxRuntimeBindingRepositorySnapshot>,
        sandbox_last_failure: Option<SandboxSessionFailure>,
        sandbox_operations: Vec<SandboxSessionOperationRepositorySnapshot>,
        sandbox_version: u64,
    ) -> Self {
        Self {
            tenant_id,
            sandbox_workspace_id,
            sandbox_session_id,
            sandbox_session_state,
            sandbox_required_capabilities,
            sandbox_minimum_assurance,
            sandbox_runtime_binding,
            sandbox_last_failure,
            sandbox_operations,
            sandbox_version,
        }
    }

    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::ProtectionFailed` when the
    /// allocation protector rejects the reference, and
    /// `InvalidStoredData` when the persisted-state invariants fail.
    pub fn capture(
        sandbox_session: &SandboxSession,
        sandbox_allocation_protector: &dyn SandboxProviderAllocationProtector,
    ) -> SandboxSessionRepositoryResult<Self> {
        let sandbox_runtime_binding = sandbox_session
            .sandbox_runtime_binding()
            .map(|sandbox_runtime_binding| {
                let sandbox_protection_context =
                    SandboxProviderAllocationProtectionContext::for_repository(
                        sandbox_session.tenant_id().clone(),
                        sandbox_session.sandbox_session_id().clone(),
                        sandbox_runtime_binding.sandbox_runtime_binding_id().clone(),
                    );
                let sandbox_protected_allocation_reference = sandbox_runtime_binding
                    .sandbox_allocation_reference()
                    .map(|sandbox_allocation_reference| {
                        sandbox_allocation_protector.protect_sandbox_allocation_reference(
                            &sandbox_protection_context,
                            sandbox_allocation_reference,
                        )
                    })
                    .transpose()?;
                Ok(SandboxRuntimeBindingRepositorySnapshot::new(
                    sandbox_runtime_binding.sandbox_id().clone(),
                    sandbox_runtime_binding.sandbox_runtime_binding_id().clone(),
                    sandbox_runtime_binding.sandbox_provider_id().clone(),
                    sandbox_protected_allocation_reference,
                ))
            })
            .transpose()?;
        let sandbox_operations = sandbox_session
            .sandbox_operations()
            .iter()
            .map(|sandbox_operation| {
                SandboxSessionOperationRepositorySnapshot::new(
                    sandbox_operation.sandbox_operation_id().clone(),
                    sandbox_operation.sandbox_operation_kind(),
                    sandbox_operation.sandbox_operation_outcome(),
                )
            })
            .collect();
        let sandbox_snapshot = Self::new(
            sandbox_session.tenant_id().clone(),
            sandbox_session.sandbox_workspace_id().clone(),
            sandbox_session.sandbox_session_id().clone(),
            sandbox_session.sandbox_session_state(),
            sandbox_session.sandbox_required_capabilities().clone(),
            sandbox_session.sandbox_minimum_assurance(),
            sandbox_runtime_binding,
            sandbox_session.sandbox_last_failure(),
            sandbox_operations,
            sandbox_session.sandbox_version(),
        );
        sandbox_snapshot.validate_sandbox_persisted_invariants()?;
        Ok(sandbox_snapshot)
    }

    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::ProtectionFailed` when the
    /// allocation protector rejects the reference, and
    /// `InvalidStoredData` when the persisted-state invariants fail.
    pub fn restore(
        self,
        sandbox_allocation_protector: &dyn SandboxProviderAllocationProtector,
    ) -> SandboxSessionRepositoryResult<SandboxSession> {
        self.validate_sandbox_persisted_invariants()?;
        let sandbox_runtime_binding = self
            .sandbox_runtime_binding
            .map(|sandbox_runtime_binding| {
                let sandbox_protection_context =
                    SandboxProviderAllocationProtectionContext::for_repository(
                        self.tenant_id.clone(),
                        self.sandbox_session_id.clone(),
                        sandbox_runtime_binding.sandbox_runtime_binding_id.clone(),
                    );
                let sandbox_allocation_reference = sandbox_runtime_binding
                    .sandbox_protected_allocation_reference
                    .as_ref()
                    .map(|sandbox_protected_allocation_reference| {
                        sandbox_allocation_protector.restore_sandbox_allocation_reference(
                            &sandbox_protection_context,
                            sandbox_protected_allocation_reference,
                        )
                    })
                    .transpose()?;
                Ok(SandboxRuntimeBinding::restore(
                    sandbox_runtime_binding.sandbox_id,
                    sandbox_runtime_binding.sandbox_runtime_binding_id,
                    sandbox_runtime_binding.sandbox_provider_id,
                    sandbox_allocation_reference,
                ))
            })
            .transpose()?;
        let sandbox_operations = self
            .sandbox_operations
            .into_iter()
            .map(|sandbox_operation| {
                SandboxSessionOperation::restore(
                    sandbox_operation.sandbox_operation_id,
                    sandbox_operation.sandbox_operation_kind,
                    sandbox_operation.sandbox_operation_outcome,
                )
            })
            .collect();
        Ok(SandboxSession::restore(
            self.tenant_id,
            self.sandbox_workspace_id,
            self.sandbox_session_id,
            self.sandbox_session_state,
            self.sandbox_required_capabilities,
            self.sandbox_minimum_assurance,
            sandbox_runtime_binding,
            self.sandbox_last_failure,
            sandbox_operations,
            self.sandbox_version,
        ))
    }

    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    #[must_use]
    pub fn sandbox_workspace_id(&self) -> &SandboxWorkspaceId {
        &self.sandbox_workspace_id
    }

    #[must_use]
    pub fn sandbox_session_id(&self) -> &SandboxSessionId {
        &self.sandbox_session_id
    }

    #[must_use]
    pub fn sandbox_session_state(&self) -> SandboxSessionState {
        self.sandbox_session_state
    }

    #[must_use]
    pub fn sandbox_required_capabilities(&self) -> &BTreeSet<RuntimeCapability> {
        &self.sandbox_required_capabilities
    }

    #[must_use]
    pub fn sandbox_minimum_assurance(&self) -> IsolationAssurance {
        self.sandbox_minimum_assurance
    }

    #[must_use]
    pub fn sandbox_runtime_binding(&self) -> Option<&SandboxRuntimeBindingRepositorySnapshot> {
        self.sandbox_runtime_binding.as_ref()
    }

    #[must_use]
    pub fn sandbox_last_failure(&self) -> Option<SandboxSessionFailure> {
        self.sandbox_last_failure
    }

    #[must_use]
    pub fn sandbox_operations(&self) -> &[SandboxSessionOperationRepositorySnapshot] {
        &self.sandbox_operations
    }

    #[must_use]
    pub fn sandbox_version(&self) -> u64 {
        self.sandbox_version
    }
}

#[async_trait]
pub trait SandboxSessionRepository: Send + Sync {
    async fn find_by_sandbox_operation(
        &self,
        tenant_id: &TenantId,
        sandbox_operation_id: &OperationId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>>;

    async fn get_sandbox_session(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>>;

    async fn insert_sandbox_session(
        &self,
        sandbox_session: SandboxSession,
    ) -> SandboxSessionRepositoryResult<()>;

    async fn save_sandbox_session(
        &self,
        sandbox_session: SandboxSession,
        expected_sandbox_version: u64,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<()>;

    async fn acquire_sandbox_session_lease(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
        sandbox_lease_owner_id: &SandboxLeaseOwnerId,
        sandbox_lease_duration: Duration,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSessionLease>>;

    async fn renew_sandbox_session_lease(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
        sandbox_lease_duration: Duration,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSessionLease>>;

    async fn release_sandbox_session_lease(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<bool>;

    async fn list_sandbox_sessions_requiring_reconciliation(
        &self,
        tenant_id: &TenantId,
        after_sandbox_session_id: Option<&SandboxSessionId>,
        sandbox_page_size: u16,
    ) -> SandboxSessionRepositoryResult<Vec<SandboxSessionReconciliationCandidate>>;
}

/// Wraps any [`SandboxSessionRepository`] so every call is bounded by a
/// timeout. A wedged store (black-holed TCP, exhausted pool outside its own
/// acquire timeout) must surface as a retryable `Repository(Unavailable)`
/// instead of hanging a lease holder until the lease expires — every other
/// controller for that session blocks behind the lease.
///
/// The composition root wraps the concrete adapter once, so every current and
/// future call site inherits the bound without per-site timeouts.
pub struct BoundedSandboxSessionRepository {
    sandbox_session_repository: Arc<dyn SandboxSessionRepository>,
    sandbox_call_timeout: Duration,
}

impl BoundedSandboxSessionRepository {
    #[must_use]
    pub fn new(
        sandbox_session_repository: Arc<dyn SandboxSessionRepository>,
        sandbox_call_timeout: Duration,
    ) -> Self {
        Self {
            sandbox_session_repository,
            sandbox_call_timeout,
        }
    }

    async fn bounded<T>(
        &self,
        sandbox_repository_future: impl std::future::Future<Output = SandboxSessionRepositoryResult<T>>,
    ) -> SandboxSessionRepositoryResult<T> {
        match tokio::time::timeout(self.sandbox_call_timeout, sandbox_repository_future).await {
            Ok(sandbox_repository_result) => sandbox_repository_result,
            Err(_) => Err(SandboxSessionRepositoryError::Unavailable),
        }
    }
}

#[async_trait]
impl SandboxSessionRepository for BoundedSandboxSessionRepository {
    async fn find_by_sandbox_operation(
        &self,
        tenant_id: &TenantId,
        sandbox_operation_id: &OperationId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>> {
        self.bounded(
            self.sandbox_session_repository
                .find_by_sandbox_operation(tenant_id, sandbox_operation_id),
        )
        .await
    }

    async fn get_sandbox_session(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSession>> {
        self.bounded(
            self.sandbox_session_repository
                .get_sandbox_session(tenant_id, sandbox_session_id),
        )
        .await
    }

    async fn insert_sandbox_session(
        &self,
        sandbox_session: SandboxSession,
    ) -> SandboxSessionRepositoryResult<()> {
        self.bounded(
            self.sandbox_session_repository
                .insert_sandbox_session(sandbox_session),
        )
        .await
    }

    async fn save_sandbox_session(
        &self,
        sandbox_session: SandboxSession,
        expected_sandbox_version: u64,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<()> {
        self.bounded(self.sandbox_session_repository.save_sandbox_session(
            sandbox_session,
            expected_sandbox_version,
            sandbox_session_lease,
        ))
        .await
    }

    async fn acquire_sandbox_session_lease(
        &self,
        tenant_id: &TenantId,
        sandbox_session_id: &SandboxSessionId,
        sandbox_lease_owner_id: &SandboxLeaseOwnerId,
        sandbox_lease_duration: Duration,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSessionLease>> {
        self.bounded(
            self.sandbox_session_repository
                .acquire_sandbox_session_lease(
                    tenant_id,
                    sandbox_session_id,
                    sandbox_lease_owner_id,
                    sandbox_lease_duration,
                ),
        )
        .await
    }

    async fn renew_sandbox_session_lease(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
        sandbox_lease_duration: Duration,
    ) -> SandboxSessionRepositoryResult<Option<SandboxSessionLease>> {
        self.bounded(
            self.sandbox_session_repository
                .renew_sandbox_session_lease(sandbox_session_lease, sandbox_lease_duration),
        )
        .await
    }

    async fn release_sandbox_session_lease(
        &self,
        sandbox_session_lease: &SandboxSessionLease,
    ) -> SandboxSessionRepositoryResult<bool> {
        self.bounded(
            self.sandbox_session_repository
                .release_sandbox_session_lease(sandbox_session_lease),
        )
        .await
    }

    async fn list_sandbox_sessions_requiring_reconciliation(
        &self,
        tenant_id: &TenantId,
        after_sandbox_session_id: Option<&SandboxSessionId>,
        sandbox_page_size: u16,
    ) -> SandboxSessionRepositoryResult<Vec<SandboxSessionReconciliationCandidate>> {
        self.bounded(
            self.sandbox_session_repository
                .list_sandbox_sessions_requiring_reconciliation(
                    tenant_id,
                    after_sandbox_session_id,
                    sandbox_page_size,
                ),
        )
        .await
    }
}

/// Validates the persisted-state invariants of a sandbox session without
/// touching allocation protection: the operation ledger must replay to the
/// session's stored state and last failure, the first operation must be a
/// succeeded create, operation ids must be unique, and the runtime binding /
/// allocation reference presence must match the state matrix. The PostgreSQL
/// adapter runs this on every write through
/// [`SandboxSessionRepositorySnapshot::capture`]; the in-memory adapter calls
/// it directly so both adapters admit the same writes
/// (REQ-2026-0005 acceptance: memory and PostgreSQL semantics must agree).
///
/// # Errors
///
/// Returns `SandboxSessionRepositoryError::InvalidStoredData` when the
/// ledger replay, operation bounds, or state/binding matrix disagree
/// with the session's persisted fields.
pub fn validate_sandbox_session_persisted_invariants(
    sandbox_session: &SandboxSession,
) -> SandboxSessionRepositoryResult<()> {
    let sandbox_operations = sandbox_session
        .sandbox_operations()
        .iter()
        .map(|sandbox_operation| {
            SandboxSessionOperationRepositorySnapshot::new(
                sandbox_operation.sandbox_operation_id().clone(),
                sandbox_operation.sandbox_operation_kind(),
                sandbox_operation.sandbox_operation_outcome(),
            )
        })
        .collect::<Vec<_>>();
    validate_sandbox_persisted_state(
        sandbox_session.sandbox_session_state(),
        sandbox_session.sandbox_last_failure(),
        sandbox_session.sandbox_version(),
        sandbox_session.sandbox_runtime_binding().is_some(),
        sandbox_session
            .sandbox_runtime_binding()
            .is_some_and(|sandbox_runtime_binding| {
                sandbox_runtime_binding
                    .sandbox_allocation_reference()
                    .is_some()
            }),
        &sandbox_operations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PanicSandboxAllocationProtector;

    impl SandboxProviderAllocationProtector for PanicSandboxAllocationProtector {
        fn current_sandbox_allocation_protection_version(
            &self,
        ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationProtectionVersion> {
            panic!("invalid sandbox snapshot must fail before allocation protection lookup")
        }

        fn protect_sandbox_allocation_reference(
            &self,
            _sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
            _sandbox_allocation_reference: &SandboxProviderAllocationRef,
        ) -> SandboxSessionRepositoryResult<SandboxProtectedProviderAllocationRef> {
            panic!("invalid sandbox snapshot must fail before allocation protection")
        }

        fn restore_sandbox_allocation_reference(
            &self,
            _sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
            _sandbox_protected_allocation_reference: &SandboxProtectedProviderAllocationRef,
        ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationRef> {
            panic!("invalid sandbox snapshot must fail before allocation restoration")
        }

        fn reencrypt_sandbox_allocation_reference(
            &self,
            _sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
            _sandbox_protected_allocation_reference: &SandboxProtectedProviderAllocationRef,
        ) -> SandboxSessionRepositoryResult<SandboxProtectedProviderAllocationRef> {
            panic!("invalid sandbox snapshot must fail before allocation re-encryption")
        }
    }

    fn sandbox_runtime_binding(
        sandbox_with_allocation_reference: bool,
    ) -> SandboxRuntimeBindingRepositorySnapshot {
        SandboxRuntimeBindingRepositorySnapshot::new(
            SandboxId::generate(),
            SandboxRuntimeBindingId::generate(),
            SandboxProviderId::parse("provider-test")
                .unwrap_or_else(|error| panic!("invalid test sandbox provider id: {error}")),
            sandbox_with_allocation_reference.then(|| {
                SandboxProtectedProviderAllocationRef::new(
                    "protected-allocation",
                    "sandbox-test-key",
                    1,
                    1,
                )
                .unwrap_or_else(|error| {
                    panic!("invalid test protected sandbox allocation reference: {error}")
                })
            }),
        )
    }

    fn sandbox_operation(
        sandbox_operation_kind: SandboxSessionOperationKind,
        sandbox_operation_outcome: SandboxOperationOutcome,
    ) -> SandboxSessionOperationRepositorySnapshot {
        SandboxSessionOperationRepositorySnapshot::new(
            OperationId::generate(),
            sandbox_operation_kind,
            sandbox_operation_outcome,
        )
    }

    fn sandbox_create_operation() -> SandboxSessionOperationRepositorySnapshot {
        sandbox_operation(
            SandboxSessionOperationKind::Create,
            SandboxOperationOutcome::Succeeded,
        )
    }

    fn sandbox_snapshot(
        sandbox_session_state: SandboxSessionState,
        sandbox_runtime_binding: Option<SandboxRuntimeBindingRepositorySnapshot>,
        sandbox_last_failure: Option<SandboxSessionFailure>,
        sandbox_operations: Vec<SandboxSessionOperationRepositorySnapshot>,
    ) -> SandboxSessionRepositorySnapshot {
        SandboxSessionRepositorySnapshot::new(
            TenantId::parse("tenant-test")
                .unwrap_or_else(|error| panic!("invalid test tenant id: {error}")),
            SandboxWorkspaceId::parse("workspace-test")
                .unwrap_or_else(|error| panic!("invalid test sandbox workspace id: {error}")),
            SandboxSessionId::parse("session-test")
                .unwrap_or_else(|error| panic!("invalid test sandbox session id: {error}")),
            sandbox_session_state,
            BTreeSet::new(),
            IsolationAssurance::HostUser,
            sandbox_runtime_binding,
            sandbox_last_failure,
            sandbox_operations,
            0,
        )
    }

    #[test]
    fn sandbox_snapshot_validation_accepts_the_persisted_state_matrix() {
        let sandbox_valid_snapshots = [
            sandbox_snapshot(
                SandboxSessionState::Created,
                None,
                None,
                vec![sandbox_create_operation()],
            ),
            sandbox_snapshot(
                SandboxSessionState::Starting,
                Some(sandbox_runtime_binding(false)),
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::InProgress,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Running,
                Some(sandbox_runtime_binding(true)),
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::Succeeded,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Stopping,
                Some(sandbox_runtime_binding(true)),
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::Succeeded,
                    ),
                    sandbox_operation(
                        SandboxSessionOperationKind::Stop,
                        SandboxOperationOutcome::InProgress,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Stopped,
                Some(sandbox_runtime_binding(true)),
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::Succeeded,
                    ),
                    sandbox_operation(
                        SandboxSessionOperationKind::Stop,
                        SandboxOperationOutcome::Succeeded,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Failed,
                None,
                Some(SandboxSessionFailure::Readiness),
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::Failed(SandboxSessionFailure::Readiness),
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Destroying,
                None,
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Destroy,
                        SandboxOperationOutcome::InProgress,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Destroyed,
                None,
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Destroy,
                        SandboxOperationOutcome::Succeeded,
                    ),
                ],
            ),
        ];

        for sandbox_snapshot in sandbox_valid_snapshots {
            assert_eq!(
                sandbox_snapshot.validate_sandbox_persisted_invariants(),
                Ok(())
            );
        }
    }

    #[test]
    fn sandbox_snapshot_validation_rejects_invalid_cross_field_combinations() {
        let sandbox_duplicate_operation_id = OperationId::generate();
        let sandbox_invalid_snapshots = [
            sandbox_snapshot(
                SandboxSessionState::Created,
                Some(sandbox_runtime_binding(false)),
                None,
                vec![sandbox_create_operation()],
            ),
            sandbox_snapshot(
                SandboxSessionState::Starting,
                None,
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::InProgress,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Starting,
                Some(sandbox_runtime_binding(false)),
                None,
                vec![sandbox_create_operation()],
            ),
            sandbox_snapshot(
                SandboxSessionState::Running,
                None,
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::Succeeded,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Running,
                Some(sandbox_runtime_binding(false)),
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::Succeeded,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Stopping,
                Some(sandbox_runtime_binding(true)),
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Destroy,
                        SandboxOperationOutcome::InProgress,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Stopped,
                Some(sandbox_runtime_binding(false)),
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Stop,
                        SandboxOperationOutcome::Succeeded,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Failed,
                None,
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::Failed(SandboxSessionFailure::Provider),
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Running,
                Some(sandbox_runtime_binding(true)),
                Some(SandboxSessionFailure::Provider),
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::Succeeded,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Destroyed,
                Some(sandbox_runtime_binding(true)),
                None,
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Destroy,
                        SandboxOperationOutcome::Succeeded,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Created,
                None,
                None,
                vec![
                    SandboxSessionOperationRepositorySnapshot::new(
                        sandbox_duplicate_operation_id.clone(),
                        SandboxSessionOperationKind::Create,
                        SandboxOperationOutcome::Succeeded,
                    ),
                    SandboxSessionOperationRepositorySnapshot::new(
                        sandbox_duplicate_operation_id,
                        SandboxSessionOperationKind::Start,
                        SandboxOperationOutcome::Succeeded,
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Failed,
                Some(sandbox_runtime_binding(true)),
                Some(SandboxSessionFailure::Cleanup),
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Stop,
                        SandboxOperationOutcome::Failed(SandboxSessionFailure::Cleanup),
                    ),
                ],
            ),
            sandbox_snapshot(
                SandboxSessionState::Failed,
                Some(sandbox_runtime_binding(true)),
                Some(SandboxSessionFailure::Provider),
                vec![
                    sandbox_create_operation(),
                    sandbox_operation(
                        SandboxSessionOperationKind::Destroy,
                        SandboxOperationOutcome::Failed(SandboxSessionFailure::Provider),
                    ),
                ],
            ),
        ];

        for sandbox_snapshot in sandbox_invalid_snapshots {
            assert_eq!(
                sandbox_snapshot.validate_sandbox_persisted_invariants(),
                Err(SandboxSessionRepositoryError::InvalidStoredData)
            );
        }
    }

    #[test]
    fn sandbox_snapshot_restore_rejects_invalid_state_before_decryption() {
        let sandbox_snapshot = sandbox_snapshot(
            SandboxSessionState::Running,
            Some(sandbox_runtime_binding(true)),
            None,
            vec![sandbox_create_operation()],
        );

        assert_eq!(
            sandbox_snapshot.restore(&PanicSandboxAllocationProtector),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
    }

    #[test]
    fn sandbox_snapshot_rejects_versions_above_the_persistence_maximum() {
        let mut sandbox_snapshot = sandbox_snapshot(
            SandboxSessionState::Created,
            None,
            None,
            vec![sandbox_create_operation()],
        );
        sandbox_snapshot.sandbox_version = MAX_SANDBOX_SESSION_VERSION + 1;

        assert_eq!(
            sandbox_snapshot.validate_sandbox_persisted_invariants(),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
    }

    #[test]
    fn sandbox_protected_allocation_reference_enforces_storage_bounds() {
        assert_eq!(
            SandboxProtectedProviderAllocationRef::new("", "sandbox-key", 1, 1),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
        assert_eq!(
            SandboxProtectedProviderAllocationRef::new("x".repeat(8_193), "sandbox-key", 1, 1,),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
        for sandbox_invalid_key_id in ["", "key id", "key\nid", "密钥"] {
            assert_eq!(
                SandboxProtectedProviderAllocationRef::new(
                    "ciphertext",
                    sandbox_invalid_key_id,
                    1,
                    1,
                ),
                Err(SandboxSessionRepositoryError::InvalidStoredData)
            );
        }
        assert_eq!(
            SandboxProtectedProviderAllocationRef::new("ciphertext", "sandbox-key", 0, 1),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
        assert_eq!(
            SandboxProtectedProviderAllocationRef::new("ciphertext", "sandbox-key", 1, 0),
            Err(SandboxSessionRepositoryError::InvalidStoredData)
        );
        assert!(SandboxProtectedProviderAllocationRef::new(
            "x".repeat(8_192),
            "kms/key:v2",
            i64::MAX as u64,
            i16::MAX as u16,
        )
        .is_ok());
    }

    #[test]
    fn sandbox_session_lease_rejects_non_positive_expiry() {
        for sandbox_invalid_expiry in [0, -1] {
            assert_eq!(
                SandboxSessionLease::new(
                    TenantId::parse("tenant-test")
                        .unwrap_or_else(|error| panic!("invalid test tenant id: {error}")),
                    SandboxSessionId::parse("session-test")
                        .unwrap_or_else(|error| panic!("invalid test session id: {error}")),
                    SandboxLeaseOwnerId::generate(),
                    SandboxFencingToken::new(1)
                        .unwrap_or_else(|error| panic!("invalid test fencing token: {error}")),
                    sandbox_invalid_expiry,
                ),
                Err(SandboxSessionRepositoryError::InvalidStoredData)
            );
        }
        assert!(SandboxSessionLease::new(
            TenantId::parse("tenant-test")
                .unwrap_or_else(|error| panic!("invalid test tenant id: {error}")),
            SandboxSessionId::parse("session-test")
                .unwrap_or_else(|error| panic!("invalid test session id: {error}")),
            SandboxLeaseOwnerId::generate(),
            SandboxFencingToken::new(1)
                .unwrap_or_else(|error| panic!("invalid test fencing token: {error}")),
            1,
        )
        .is_ok());
    }
}
