//! Console-facing Sandbox Instance registry: provisioning and ownership
//! authority, separate from the Provider-neutral Session lifecycle.
//!
//! A `SandboxInstance` is what a user applies for and what the console lists,
//! updates and retires under that user. Runtime lifecycle stays with
//! `SandboxSession`; this module never places, starts or stops a provider.

use std::collections::BTreeSet;

use async_trait::async_trait;
use sdkwork_sandbox_provider_spi::{
    IsolationAssurance, RuntimeCapability, SandboxInstanceId, SandboxInstanceOwnerId,
    SandboxWorkspaceId, TenantId,
};
use thiserror::Error;

use crate::SandboxSessionFailure;

/// Upper bound of the optimistic version, matching the `BIGINT` column bound.
pub const MAX_SANDBOX_INSTANCE_VERSION: u64 = i64::MAX as u64;
pub const MAX_SANDBOX_INSTANCE_NAME_LENGTH: usize = 128;
pub const MAX_SANDBOX_INSTANCE_BASE_IMAGE_LENGTH: usize = 256;
pub const MAX_SANDBOX_INSTANCE_REQUIRED_CAPABILITIES: usize = 32;

pub const MIN_SANDBOX_INSTANCE_VCPU_COUNT: u32 = 1;
pub const MAX_SANDBOX_INSTANCE_VCPU_COUNT: u32 = 64;
pub const MIN_SANDBOX_INSTANCE_MEMORY_MB: u32 = 256;
pub const MAX_SANDBOX_INSTANCE_MEMORY_MB: u32 = 262_144;
pub const MIN_SANDBOX_INSTANCE_DISK_MB: u32 = 1_024;
pub const MAX_SANDBOX_INSTANCE_DISK_MB: u32 = 1_048_576;

/// Console-visible provisioning state of a Sandbox Instance.
///
/// The transition matrix below is the whole authority: `terminated` and
/// `failed` are terminal for state changes, and only a non-`active` instance
/// may be deleted, so a live instance is never removed by a single request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxInstanceState {
    /// Applied for, not yet usable.
    Requested,
    Active,
    Suspended,
    Terminated,
    Failed,
}

impl SandboxInstanceState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Terminated => "terminated",
            Self::Failed => "failed",
        }
    }

    /// Parses the persisted state, returning `None` for an unknown value.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "requested" => Some(Self::Requested),
            "active" => Some(Self::Active),
            "suspended" => Some(Self::Suspended),
            "terminated" => Some(Self::Terminated),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    /// True when no further state change is accepted.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Terminated | Self::Failed)
    }

    /// True when the instance may be deleted. A live (`active`) instance must be
    /// suspended or terminated first, so a single DELETE cannot drop a running
    /// runtime out from under its owner.
    #[must_use]
    pub fn is_deletable(self) -> bool {
        !matches!(self, Self::Active)
    }

    /// The state machine accepted by [`SandboxInstance::transition_state`].
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        if self == target {
            return false;
        }
        matches!(
            (self, target),
            (Self::Requested, Self::Active)
                | (Self::Requested, Self::Suspended)
                | (Self::Requested, Self::Failed)
                | (Self::Active, Self::Suspended)
                | (Self::Active, Self::Terminated)
                | (Self::Active, Self::Failed)
                | (Self::Suspended, Self::Active)
                | (Self::Suspended, Self::Terminated)
                | (Self::Suspended, Self::Failed)
        )
    }
}

/// Resource shape of a provisioned instance. The three public values map onto
/// the declared vCPU/memory/disk envelopes; the console picks a range, not a
/// free-form number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxInstanceProfile {
    Standard,
    MemoryOptimized,
    ComputeOptimized,
}

impl SandboxInstanceProfile {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::MemoryOptimized => "memory_optimized",
            Self::ComputeOptimized => "compute_optimized",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(Self::Standard),
            "memory_optimized" => Some(Self::MemoryOptimized),
            "compute_optimized" => Some(Self::ComputeOptimized),
            _ => None,
        }
    }

    /// The resource envelope this profile accepts.
    #[must_use]
    pub fn bounds(self) -> SandboxInstanceResourceBounds {
        match self {
            Self::Standard => SandboxInstanceResourceBounds {
                max_vcpu_count: 8,
                max_memory_mb: 16_384,
                max_disk_mb: 102_400,
            },
            Self::MemoryOptimized => SandboxInstanceResourceBounds {
                max_vcpu_count: 16,
                max_memory_mb: MAX_SANDBOX_INSTANCE_MEMORY_MB,
                max_disk_mb: 204_800,
            },
            Self::ComputeOptimized => SandboxInstanceResourceBounds {
                max_vcpu_count: MAX_SANDBOX_INSTANCE_VCPU_COUNT,
                max_memory_mb: 65_536,
                max_disk_mb: MAX_SANDBOX_INSTANCE_DISK_MB,
            },
        }
    }
}

/// Per-profile ceiling applied on top of the absolute column bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxInstanceResourceBounds {
    pub max_vcpu_count: u32,
    pub max_memory_mb: u32,
    pub max_disk_mb: u32,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SandboxInstanceRepositoryError {
    #[error("sandbox instance was not found")]
    NotFound,
    #[error("sandbox instance version conflict")]
    VersionConflict,
    #[error("sandbox instance name already exists for this owner")]
    DuplicateName,
    #[error("sandbox instance repository is unavailable")]
    Unavailable,
    #[error("sandbox instance repository contains invalid persisted data")]
    InvalidStoredData,
    #[error("sandbox instance repository requires a PostgreSQL database pool")]
    UnsupportedDatabaseEngine,
    #[error("sandbox instance repository page request is invalid")]
    InvalidPageRequest,
}

pub type SandboxInstanceRepositoryResult<T> = Result<T, SandboxInstanceRepositoryError>;

#[derive(Debug, Error)]
pub enum SandboxInstanceError {
    #[error("sandbox instance {sandbox_instance_id} does not exist in tenant {tenant_id}")]
    NotFound {
        tenant_id: TenantId,
        sandbox_instance_id: SandboxInstanceId,
    },
    #[error("sandbox instance name {sandbox_instance_name} already exists for this owner")]
    DuplicateName { sandbox_instance_name: String },
    #[error("sandbox instance field {field} is invalid: {detail}")]
    Validation {
        field: &'static str,
        detail: &'static str,
    },
    #[error("sandbox instance cannot move from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: SandboxInstanceState,
        to: SandboxInstanceState,
    },
    #[error("sandbox instance in state {sandbox_instance_state:?} cannot be deleted")]
    InstanceNotDeletable {
        sandbox_instance_state: SandboxInstanceState,
    },
    #[error("sandbox instance {sandbox_instance_id} was modified by another request")]
    VersionConflict {
        sandbox_instance_id: SandboxInstanceId,
    },
    #[error("sandbox instance state is internally inconsistent: {0}")]
    InvariantViolation(&'static str),
    #[error(transparent)]
    Repository(#[from] SandboxInstanceRepositoryError),
}

pub type SandboxInstanceResult<T> = Result<T, SandboxInstanceError>;

/// What a PATCH does to `expires_at`.
///
/// An absent JSON field, an explicit `null`, and a timestamp are three
/// different instructions, so the wire shape cannot collapse into
/// `Option<String>`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SandboxInstanceExpiryUpdate {
    #[default]
    Unchanged,
    Clear,
    Set(String),
}

/// Fully validated command for creating one instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateSandboxInstanceCommand {
    pub tenant_id: TenantId,
    pub sandbox_instance_owner_id: SandboxInstanceOwnerId,
    pub sandbox_instance_name: String,
    pub sandbox_instance_profile: SandboxInstanceProfile,
    pub sandbox_instance_base_image: String,
    pub sandbox_instance_vcpu_count: u32,
    pub sandbox_instance_memory_mb: u32,
    pub sandbox_instance_disk_mb: u32,
    pub sandbox_instance_required_capabilities: BTreeSet<RuntimeCapability>,
    pub sandbox_instance_minimum_assurance: IsolationAssurance,
    pub sandbox_instance_auto_start: bool,
    pub sandbox_instance_expires_at: Option<String>,
    pub sandbox_workspace_id: Option<SandboxWorkspaceId>,
}

/// Partial update. `None` leaves the stored value untouched.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UpdateSandboxInstanceCommand {
    pub tenant_id: TenantId,
    pub sandbox_instance_id: SandboxInstanceId,
    pub sandbox_instance_name: Option<String>,
    pub sandbox_instance_profile: Option<SandboxInstanceProfile>,
    pub sandbox_instance_vcpu_count: Option<u32>,
    pub sandbox_instance_memory_mb: Option<u32>,
    pub sandbox_instance_disk_mb: Option<u32>,
    pub sandbox_instance_auto_start: Option<bool>,
    pub sandbox_instance_expires_at: SandboxInstanceExpiryUpdate,
    pub sandbox_workspace_id: Option<SandboxWorkspaceId>,
    pub sandbox_instance_state: Option<SandboxInstanceState>,
}

impl UpdateSandboxInstanceCommand {
    /// A command that changes nothing, addressed at one instance.
    ///
    /// The identity fields have no meaningful default, so this constructor is
    /// the only way to start a partial update; every other field is then set by
    /// the caller.
    #[must_use]
    pub fn for_instance(tenant_id: TenantId, sandbox_instance_id: SandboxInstanceId) -> Self {
        Self {
            tenant_id,
            sandbox_instance_id,
            sandbox_instance_name: None,
            sandbox_instance_profile: None,
            sandbox_instance_vcpu_count: None,
            sandbox_instance_memory_mb: None,
            sandbox_instance_disk_mb: None,
            sandbox_instance_auto_start: None,
            sandbox_instance_expires_at: SandboxInstanceExpiryUpdate::Unchanged,
            sandbox_workspace_id: None,
            sandbox_instance_state: None,
        }
    }
}

/// Tenant- and owner-scoped provisioning record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxInstance {
    tenant_id: TenantId,
    sandbox_instance_id: SandboxInstanceId,
    sandbox_instance_owner_id: SandboxInstanceOwnerId,
    sandbox_instance_name: String,
    sandbox_instance_state: SandboxInstanceState,
    sandbox_instance_profile: SandboxInstanceProfile,
    sandbox_instance_base_image: String,
    sandbox_instance_vcpu_count: u32,
    sandbox_instance_memory_mb: u32,
    sandbox_instance_disk_mb: u32,
    sandbox_instance_required_capabilities: BTreeSet<RuntimeCapability>,
    sandbox_instance_minimum_assurance: IsolationAssurance,
    sandbox_instance_auto_start: bool,
    sandbox_instance_expires_at: Option<String>,
    sandbox_workspace_id: Option<SandboxWorkspaceId>,
    sandbox_instance_last_failure: Option<SandboxSessionFailure>,
    sandbox_version: u64,
    /// Storage-assigned creation timestamp, projected as RFC 3339 UTC. `None`
    /// until a repository adapter has persisted the row, because the value is
    /// the database clock's, not this process's.
    created_at: Option<String>,
    /// Storage-assigned last-write timestamp, same provenance as `created_at`.
    updated_at: Option<String>,
}

impl SandboxInstance {
    /// Validates and builds a `requested` instance.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceError::Validation`] when a field violates its
    /// declared bound. The caller supplies no state: a new instance is always
    /// `requested`, so no request can fabricate a live instance.
    pub fn request(command: CreateSandboxInstanceCommand) -> SandboxInstanceResult<Self> {
        validate_instance_name(&command.sandbox_instance_name)?;
        validate_base_image(&command.sandbox_instance_base_image)?;
        validate_resource_shape(
            command.sandbox_instance_profile,
            command.sandbox_instance_vcpu_count,
            command.sandbox_instance_memory_mb,
            command.sandbox_instance_disk_mb,
        )?;
        validate_capabilities(&command.sandbox_instance_required_capabilities)?;
        if let Some(expires_at) = command.sandbox_instance_expires_at.as_deref() {
            validate_expires_at(expires_at)?;
        }
        Ok(Self {
            tenant_id: command.tenant_id,
            sandbox_instance_id: SandboxInstanceId::generate(),
            sandbox_instance_owner_id: command.sandbox_instance_owner_id,
            sandbox_instance_name: command.sandbox_instance_name,
            sandbox_instance_state: SandboxInstanceState::Requested,
            sandbox_instance_profile: command.sandbox_instance_profile,
            sandbox_instance_base_image: command.sandbox_instance_base_image,
            sandbox_instance_vcpu_count: command.sandbox_instance_vcpu_count,
            sandbox_instance_memory_mb: command.sandbox_instance_memory_mb,
            sandbox_instance_disk_mb: command.sandbox_instance_disk_mb,
            sandbox_instance_required_capabilities: command.sandbox_instance_required_capabilities,
            sandbox_instance_minimum_assurance: command.sandbox_instance_minimum_assurance,
            sandbox_instance_auto_start: command.sandbox_instance_auto_start,
            sandbox_instance_expires_at: command.sandbox_instance_expires_at,
            sandbox_workspace_id: command.sandbox_workspace_id,
            sandbox_instance_last_failure: None,
            sandbox_version: 0,
            created_at: None,
            updated_at: None,
        })
    }

    /// Rehydrates a persisted row without re-validating it. Only the repository
    /// adapter calls this; a value that fails these invariants is a storage bug
    /// and is reported as `InvalidStoredData` by the caller instead.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn restore(
        tenant_id: TenantId,
        sandbox_instance_id: SandboxInstanceId,
        sandbox_instance_owner_id: SandboxInstanceOwnerId,
        sandbox_instance_name: String,
        sandbox_instance_state: SandboxInstanceState,
        sandbox_instance_profile: SandboxInstanceProfile,
        sandbox_instance_base_image: String,
        sandbox_instance_vcpu_count: u32,
        sandbox_instance_memory_mb: u32,
        sandbox_instance_disk_mb: u32,
        sandbox_instance_required_capabilities: BTreeSet<RuntimeCapability>,
        sandbox_instance_minimum_assurance: IsolationAssurance,
        sandbox_instance_auto_start: bool,
        sandbox_instance_expires_at: Option<String>,
        sandbox_workspace_id: Option<SandboxWorkspaceId>,
        sandbox_instance_last_failure: Option<SandboxSessionFailure>,
        sandbox_version: u64,
        created_at: Option<String>,
        updated_at: Option<String>,
    ) -> Self {
        Self {
            tenant_id,
            sandbox_instance_id,
            sandbox_instance_owner_id,
            sandbox_instance_name,
            sandbox_instance_state,
            sandbox_instance_profile,
            sandbox_instance_base_image,
            sandbox_instance_vcpu_count,
            sandbox_instance_memory_mb,
            sandbox_instance_disk_mb,
            sandbox_instance_required_capabilities,
            sandbox_instance_minimum_assurance,
            sandbox_instance_auto_start,
            sandbox_instance_expires_at,
            sandbox_workspace_id,
            sandbox_instance_last_failure,
            sandbox_version,
            created_at,
            updated_at,
        }
    }

    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        &self.tenant_id
    }

    #[must_use]
    pub fn sandbox_instance_id(&self) -> &SandboxInstanceId {
        &self.sandbox_instance_id
    }

    #[must_use]
    pub fn sandbox_instance_owner_id(&self) -> &SandboxInstanceOwnerId {
        &self.sandbox_instance_owner_id
    }

    #[must_use]
    pub fn sandbox_instance_name(&self) -> &str {
        &self.sandbox_instance_name
    }

    #[must_use]
    pub fn sandbox_instance_state(&self) -> SandboxInstanceState {
        self.sandbox_instance_state
    }

    #[must_use]
    pub fn sandbox_instance_profile(&self) -> SandboxInstanceProfile {
        self.sandbox_instance_profile
    }

    #[must_use]
    pub fn sandbox_instance_base_image(&self) -> &str {
        &self.sandbox_instance_base_image
    }

    #[must_use]
    pub fn sandbox_instance_vcpu_count(&self) -> u32 {
        self.sandbox_instance_vcpu_count
    }

    #[must_use]
    pub fn sandbox_instance_memory_mb(&self) -> u32 {
        self.sandbox_instance_memory_mb
    }

    #[must_use]
    pub fn sandbox_instance_disk_mb(&self) -> u32 {
        self.sandbox_instance_disk_mb
    }

    #[must_use]
    pub fn sandbox_instance_required_capabilities(&self) -> &BTreeSet<RuntimeCapability> {
        &self.sandbox_instance_required_capabilities
    }

    #[must_use]
    pub fn sandbox_instance_minimum_assurance(&self) -> IsolationAssurance {
        self.sandbox_instance_minimum_assurance
    }

    #[must_use]
    pub fn sandbox_instance_auto_start(&self) -> bool {
        self.sandbox_instance_auto_start
    }

    #[must_use]
    pub fn sandbox_instance_expires_at(&self) -> Option<&str> {
        self.sandbox_instance_expires_at.as_deref()
    }

    #[must_use]
    pub fn sandbox_workspace_id(&self) -> Option<&SandboxWorkspaceId> {
        self.sandbox_workspace_id.as_ref()
    }

    #[must_use]
    pub fn sandbox_instance_last_failure(&self) -> Option<SandboxSessionFailure> {
        self.sandbox_instance_last_failure
    }

    #[must_use]
    pub fn sandbox_version(&self) -> u64 {
        self.sandbox_version
    }

    /// Storage-assigned creation timestamp as RFC 3339 UTC, or `None` when the
    /// row has not been persisted yet.
    #[must_use]
    pub fn created_at(&self) -> Option<&str> {
        self.created_at.as_deref()
    }

    /// Storage-assigned last-write timestamp as RFC 3339 UTC, or `None` when
    /// the row has not been persisted yet.
    #[must_use]
    pub fn updated_at(&self) -> Option<&str> {
        self.updated_at.as_deref()
    }

    /// Returns this instance carrying the timestamps the storage assigned.
    ///
    /// Repository adapters are the only callers: the values come from the
    /// database clock, so no request body and no service may supply them.
    #[must_use]
    pub fn with_persistence_timestamps(mut self, created_at: String, updated_at: String) -> Self {
        self.created_at = Some(created_at);
        self.updated_at = Some(updated_at);
        self
    }

    /// Applies a partial update in place.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceError::Validation`] for an out-of-bound value,
    /// [`SandboxInstanceError::InvalidStateTransition`] for a rejected state
    /// move, and [`SandboxInstanceError::InvariantViolation`] at the version
    /// ceiling.
    pub fn apply_update(
        &mut self,
        command: &UpdateSandboxInstanceCommand,
    ) -> SandboxInstanceResult<()> {
        if let Some(state) = command.sandbox_instance_state {
            self.transition_state(state)?;
        }
        if let Some(name) = command.sandbox_instance_name.as_deref() {
            validate_instance_name(name)?;
            self.sandbox_instance_name = name.to_owned();
        }
        let profile = command
            .sandbox_instance_profile
            .unwrap_or(self.sandbox_instance_profile);
        let vcpu_count = command
            .sandbox_instance_vcpu_count
            .unwrap_or(self.sandbox_instance_vcpu_count);
        let memory_mb = command
            .sandbox_instance_memory_mb
            .unwrap_or(self.sandbox_instance_memory_mb);
        let disk_mb = command
            .sandbox_instance_disk_mb
            .unwrap_or(self.sandbox_instance_disk_mb);
        validate_resource_shape(profile, vcpu_count, memory_mb, disk_mb)?;
        self.sandbox_instance_profile = profile;
        self.sandbox_instance_vcpu_count = vcpu_count;
        self.sandbox_instance_memory_mb = memory_mb;
        self.sandbox_instance_disk_mb = disk_mb;
        if let Some(auto_start) = command.sandbox_instance_auto_start {
            self.sandbox_instance_auto_start = auto_start;
        }
        match &command.sandbox_instance_expires_at {
            SandboxInstanceExpiryUpdate::Unchanged => {}
            SandboxInstanceExpiryUpdate::Clear => self.sandbox_instance_expires_at = None,
            SandboxInstanceExpiryUpdate::Set(expires_at) => {
                validate_expires_at(expires_at)?;
                self.sandbox_instance_expires_at = Some(expires_at.clone());
            }
        }
        if let Some(workspace_id) = command.sandbox_workspace_id.as_ref() {
            self.sandbox_workspace_id = Some(workspace_id.clone());
        }
        Ok(())
    }

    /// Moves to `target`, clearing a stale failure on any non-failed target.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceError::InvalidStateTransition`] when the matrix
    /// rejects the move.
    pub fn transition_state(&mut self, target: SandboxInstanceState) -> SandboxInstanceResult<()> {
        if !self.sandbox_instance_state.can_transition_to(target) {
            return Err(SandboxInstanceError::InvalidStateTransition {
                from: self.sandbox_instance_state,
                to: target,
            });
        }
        self.sandbox_instance_state = target;
        if target != SandboxInstanceState::Failed {
            self.sandbox_instance_last_failure = None;
        }
        Ok(())
    }

    /// Advances the optimistic version and returns the value the caller must
    /// present as the expected version, mirroring the Session repository.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceError::InvariantViolation`] at the persistence
    /// maximum rather than wrapping.
    pub fn next_sandbox_version(&mut self) -> SandboxInstanceResult<u64> {
        let current_sandbox_version = self.sandbox_version;
        self.sandbox_version = self
            .sandbox_version
            .checked_add(1)
            .filter(|sandbox_version| *sandbox_version <= MAX_SANDBOX_INSTANCE_VERSION)
            .ok_or(SandboxInstanceError::InvariantViolation(
                "sandbox instance version exceeds the persistence maximum",
            ))?;
        Ok(current_sandbox_version)
    }
}

fn validate_instance_name(sandbox_instance_name: &str) -> SandboxInstanceResult<()> {
    let trimmed = sandbox_instance_name.trim();
    if trimmed.is_empty()
        || sandbox_instance_name.chars().count() > MAX_SANDBOX_INSTANCE_NAME_LENGTH
    {
        return Err(SandboxInstanceError::Validation {
            field: "sandboxInstanceName",
            detail: "must be a non-empty name of at most 128 characters",
        });
    }
    Ok(())
}

fn validate_base_image(sandbox_instance_base_image: &str) -> SandboxInstanceResult<()> {
    let trimmed = sandbox_instance_base_image.trim();
    if trimmed.is_empty()
        || sandbox_instance_base_image.chars().count() > MAX_SANDBOX_INSTANCE_BASE_IMAGE_LENGTH
    {
        return Err(SandboxInstanceError::Validation {
            field: "sandboxInstanceBaseImage",
            detail: "must be a non-empty image reference of at most 256 characters",
        });
    }
    Ok(())
}

fn validate_resource_shape(
    sandbox_instance_profile: SandboxInstanceProfile,
    sandbox_instance_vcpu_count: u32,
    sandbox_instance_memory_mb: u32,
    sandbox_instance_disk_mb: u32,
) -> SandboxInstanceResult<()> {
    let bounds = sandbox_instance_profile.bounds();
    if !(MIN_SANDBOX_INSTANCE_VCPU_COUNT..=bounds.max_vcpu_count)
        .contains(&sandbox_instance_vcpu_count)
    {
        return Err(SandboxInstanceError::Validation {
            field: "sandboxInstanceVcpuCount",
            detail: "is outside the selected profile envelope",
        });
    }
    if !(MIN_SANDBOX_INSTANCE_MEMORY_MB..=bounds.max_memory_mb)
        .contains(&sandbox_instance_memory_mb)
    {
        return Err(SandboxInstanceError::Validation {
            field: "sandboxInstanceMemoryMb",
            detail: "is outside the selected profile envelope",
        });
    }
    if !(MIN_SANDBOX_INSTANCE_DISK_MB..=bounds.max_disk_mb).contains(&sandbox_instance_disk_mb) {
        return Err(SandboxInstanceError::Validation {
            field: "sandboxInstanceDiskMb",
            detail: "is outside the selected profile envelope",
        });
    }
    Ok(())
}

fn validate_capabilities(
    sandbox_instance_required_capabilities: &BTreeSet<RuntimeCapability>,
) -> SandboxInstanceResult<()> {
    if sandbox_instance_required_capabilities.len() > MAX_SANDBOX_INSTANCE_REQUIRED_CAPABILITIES {
        return Err(SandboxInstanceError::Validation {
            field: "sandboxInstanceRequiredCapabilities",
            detail: "declares more capabilities than the persistence bound allows",
        });
    }
    Ok(())
}

/// Accepts only the exact `YYYY-MM-DDTHH:MM:SS(.fff)?Z` RFC 3339 UTC shape the
/// column and the wire contract both use, so an expiry can never be persisted
/// in a format the read path cannot re-emit.
fn validate_expires_at(expires_at: &str) -> SandboxInstanceResult<()> {
    let bytes = expires_at.as_bytes();
    let shape_ok = bytes.len() == 20 || bytes.len() == 24;
    let separators_ok = shape_ok
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && *bytes.last().unwrap_or(&b' ') == b'Z';
    let digits_ok = shape_ok
        && [0usize, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .iter()
            .all(|index| bytes.get(*index).is_some_and(u8::is_ascii_digit));
    // A fractional form must carry exactly three fraction digits, so a
    // truncated or malformed fraction cannot be persisted and re-emitted.
    let fraction_ok = bytes.len() == 20
        || (bytes.len() == 24
            && bytes[19] == b'.'
            && [20usize, 21, 22]
                .iter()
                .all(|index| bytes.get(*index).is_some_and(u8::is_ascii_digit)));
    if !separators_ok || !digits_ok || !fraction_ok {
        return Err(SandboxInstanceError::Validation {
            field: "sandboxInstanceExpiresAt",
            detail: "must be an RFC 3339 UTC timestamp",
        });
    }
    Ok(())
}

/// Keyset continuation for [`SandboxInstanceRepository::list_sandbox_instances`].
///
/// The listing sorts by `(created_at, sandbox_instance_id)` descending and the
/// cursor carries the last returned row's key, so the next page starts strictly
/// after it. `created_at` is the storage-assigned RFC 3339 UTC string the read
/// path re-emits, validated here with the same shape rule the column enforces,
/// so a hand-forged cursor cannot smuggle a format the database cannot cast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxInstanceListCursor {
    created_at: String,
    sandbox_instance_id: SandboxInstanceId,
}

impl SandboxInstanceListCursor {
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceError::Validation`] when the timestamp is not
    /// the exact RFC 3339 UTC shape the sort column stores.
    pub fn new(
        created_at: String,
        sandbox_instance_id: SandboxInstanceId,
    ) -> SandboxInstanceResult<Self> {
        validate_expires_at(&created_at).map_err(|mut error| {
            if let SandboxInstanceError::Validation { field, .. } = &mut error {
                *field = "cursor";
            }
            error
        })?;
        Ok(Self {
            created_at,
            sandbox_instance_id,
        })
    }

    #[must_use]
    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    #[must_use]
    pub fn sandbox_instance_id(&self) -> &SandboxInstanceId {
        &self.sandbox_instance_id
    }
}

/// One bounded listing window plus its keyset continuation. `next_cursor` is
/// absent exactly when no further row matches the filters, so the caller never
/// probes the store an extra time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxInstanceListPage {
    pub items: Vec<SandboxInstance>,
    pub next_cursor: Option<SandboxInstanceListCursor>,
}

/// Persistence port owned by this module. The PostgreSQL adapter implements it;
/// the in-memory adapter exists so the service can be tested without a database.
#[async_trait]
pub trait SandboxInstanceRepository: Send + Sync {
    /// Inserts a new row and returns it as stored, so the caller receives the
    /// database-assigned timestamps instead of re-reading the row.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceRepositoryError::DuplicateName`] when the owner
    /// already holds that name in the tenant.
    async fn insert_sandbox_instance(
        &self,
        sandbox_instance: &SandboxInstance,
    ) -> SandboxInstanceRepositoryResult<SandboxInstance>;

    /// Lists one owner's instances, or the whole tenant when `owner` is `None`,
    /// optionally narrowed to one state, as one keyset window
    /// (`PAGINATION_SPEC.md` sections 5-6: seek pagination on a fast-growing
    /// table, never OFFSET).
    async fn list_sandbox_instances(
        &self,
        tenant_id: &TenantId,
        sandbox_instance_owner_id: Option<&SandboxInstanceOwnerId>,
        sandbox_instance_state: Option<SandboxInstanceState>,
        cursor: Option<&SandboxInstanceListCursor>,
        page_size: u32,
    ) -> SandboxInstanceRepositoryResult<SandboxInstanceListPage>;

    async fn get_sandbox_instance(
        &self,
        tenant_id: &TenantId,
        sandbox_instance_id: &SandboxInstanceId,
    ) -> SandboxInstanceRepositoryResult<Option<SandboxInstance>>;

    /// Compare-and-swap on `expected_sandbox_version`; `false` means the row
    /// moved under this caller.
    async fn save_sandbox_instance(
        &self,
        sandbox_instance: &SandboxInstance,
        expected_sandbox_version: u64,
    ) -> SandboxInstanceRepositoryResult<bool>;

    async fn delete_sandbox_instance(
        &self,
        tenant_id: &TenantId,
        sandbox_instance_id: &SandboxInstanceId,
        expected_sandbox_version: u64,
    ) -> SandboxInstanceRepositoryResult<bool>;
}
