//! Application service for the console-facing Sandbox Instance registry.
//!
//! The service owns validation, the optimistic-version handshake and the
//! delete guard; the repository owns only persistence. No provider, scheduler
//! or host capability is touched here.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use sdkwork_sandbox_provider_spi::{SandboxInstanceId, SandboxInstanceOwnerId, TenantId};

use crate::instance::{
    CreateSandboxInstanceCommand, SandboxInstance, SandboxInstanceError, SandboxInstanceListCursor,
    SandboxInstanceListPage, SandboxInstanceRepository, SandboxInstanceRepositoryError,
    SandboxInstanceRepositoryResult, SandboxInstanceResult, SandboxInstanceState,
    UpdateSandboxInstanceCommand,
};

pub const DEFAULT_SANDBOX_INSTANCE_PAGE_SIZE: u32 = 20;
pub const MAX_SANDBOX_INSTANCE_PAGE_SIZE: u32 = 200;

/// Upper bound for one authoritative-store call, matching the baseline
/// `statement_timeout` header. A wedged store must surface as a retryable
/// `Repository(Unavailable)` instead of hanging the request indefinitely.
const SANDBOX_INSTANCE_REPOSITORY_TIMEOUT: Duration = Duration::from_secs(30);

/// Bounds one repository call; a timed-out call maps to `Unavailable`.
async fn bounded_repository<T>(
    sandbox_repository_future: impl Future<Output = SandboxInstanceRepositoryResult<T>>,
) -> SandboxInstanceRepositoryResult<T> {
    match tokio::time::timeout(
        SANDBOX_INSTANCE_REPOSITORY_TIMEOUT,
        sandbox_repository_future,
    )
    .await
    {
        Ok(sandbox_repository_result) => sandbox_repository_result,
        Err(_) => Err(SandboxInstanceRepositoryError::Unavailable),
    }
}

/// Console CRUD over the Sandbox Instance registry.
pub struct SandboxInstanceService<R: SandboxInstanceRepository> {
    repository: Arc<R>,
}

impl<R: SandboxInstanceRepository> SandboxInstanceService<R> {
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }

    /// Applies for a new instance. The stored state is always `requested`.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceError::Validation`] for an invalid field,
    /// [`SandboxInstanceError::DuplicateName`] when the owner already holds that
    /// name, and the repository error otherwise.
    pub async fn create(
        &self,
        command: CreateSandboxInstanceCommand,
    ) -> SandboxInstanceResult<SandboxInstance> {
        let sandbox_instance = SandboxInstance::request(command)?;
        match bounded_repository(self.repository.insert_sandbox_instance(&sandbox_instance)).await {
            Ok(stored) => Ok(stored),
            Err(SandboxInstanceRepositoryError::DuplicateName) => {
                Err(SandboxInstanceError::DuplicateName {
                    sandbox_instance_name: sandbox_instance.sandbox_instance_name().to_owned(),
                })
            }
            Err(error) => Err(SandboxInstanceError::Repository(error)),
        }
    }

    /// Lists one keyset window of instances, optionally narrowed to one owner.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceError::Validation`] for an out-of-range page
    /// size or a malformed cursor, and the repository error otherwise.
    pub async fn list(
        &self,
        tenant_id: &TenantId,
        sandbox_instance_owner_id: Option<&SandboxInstanceOwnerId>,
        sandbox_instance_state: Option<SandboxInstanceState>,
        cursor: Option<&SandboxInstanceListCursor>,
        page_size: u32,
    ) -> SandboxInstanceResult<SandboxInstanceListPage> {
        let page_size = normalize_page_size(page_size)?;
        bounded_repository(self.repository.list_sandbox_instances(
            tenant_id,
            sandbox_instance_owner_id,
            sandbox_instance_state,
            cursor,
            page_size,
        ))
        .await
        .map_err(SandboxInstanceError::Repository)
    }

    /// Reads one instance scoped to the caller's tenant.
    ///
    /// A tenant-mismatched id is reported as `NotFound` rather than `Forbidden`:
    /// the lookup itself is tenant-scoped, so existence is not disclosed.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceError::NotFound`] when the tenant has no such
    /// instance, and the repository error otherwise.
    pub async fn retrieve(
        &self,
        tenant_id: &TenantId,
        sandbox_instance_id: &SandboxInstanceId,
    ) -> SandboxInstanceResult<SandboxInstance> {
        bounded_repository(
            self.repository
                .get_sandbox_instance(tenant_id, sandbox_instance_id),
        )
        .await
        .map_err(SandboxInstanceError::Repository)?
        .ok_or_else(|| SandboxInstanceError::NotFound {
            tenant_id: tenant_id.clone(),
            sandbox_instance_id: sandbox_instance_id.clone(),
        })
    }

    /// Applies a partial update under optimistic concurrency.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceError::NotFound`], a validation or transition
    /// error from [`SandboxInstance::apply_update`], and
    /// [`SandboxInstanceError::VersionConflict`] when the row moved under the
    /// caller.
    pub async fn update(
        &self,
        command: UpdateSandboxInstanceCommand,
    ) -> SandboxInstanceResult<SandboxInstance> {
        let mut sandbox_instance = self
            .retrieve(&command.tenant_id, &command.sandbox_instance_id)
            .await?;
        sandbox_instance.apply_update(&command)?;
        let expected_sandbox_version = sandbox_instance.next_sandbox_version()?;
        let stored = bounded_repository(
            self.repository
                .save_sandbox_instance(&sandbox_instance, expected_sandbox_version),
        )
        .await
        .map_err(SandboxInstanceError::Repository)?;
        if !stored {
            return Err(SandboxInstanceError::VersionConflict {
                sandbox_instance_id: command.sandbox_instance_id,
            });
        }
        // The store assigns `updated_at`; re-read so the caller sees the same
        // timestamps a subsequent retrieve would (`create` already returns the
        // stored row).
        self.retrieve(&command.tenant_id, &command.sandbox_instance_id)
            .await
    }

    /// Retires an instance. A live instance must be suspended first.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxInstanceError::NotFound`],
    /// [`SandboxInstanceError::InstanceNotDeletable`] for a live instance, and
    /// [`SandboxInstanceError::VersionConflict`] when the row moved under the
    /// caller.
    pub async fn delete(
        &self,
        tenant_id: &TenantId,
        sandbox_instance_id: &SandboxInstanceId,
    ) -> SandboxInstanceResult<()> {
        let sandbox_instance = self.retrieve(tenant_id, sandbox_instance_id).await?;
        if !sandbox_instance.sandbox_instance_state().is_deletable() {
            return Err(SandboxInstanceError::InstanceNotDeletable {
                sandbox_instance_state: sandbox_instance.sandbox_instance_state(),
            });
        }
        let deleted = bounded_repository(self.repository.delete_sandbox_instance(
            tenant_id,
            sandbox_instance_id,
            sandbox_instance.sandbox_version(),
        ))
        .await
        .map_err(SandboxInstanceError::Repository)?;
        if !deleted {
            return Err(SandboxInstanceError::VersionConflict {
                sandbox_instance_id: sandbox_instance_id.clone(),
            });
        }
        Ok(())
    }
}

fn normalize_page_size(page_size: u32) -> SandboxInstanceResult<u32> {
    if page_size == 0 || page_size > MAX_SANDBOX_INSTANCE_PAGE_SIZE {
        return Err(SandboxInstanceError::Validation {
            field: "page_size",
            detail: "must be between 1 and 200",
        });
    }
    Ok(page_size)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use sdkwork_sandbox_provider_spi::{IsolationAssurance, SandboxInstanceOwnerId};

    use super::*;
    use crate::instance::{
        SandboxInstanceExpiryUpdate, SandboxInstanceProfile, SandboxInstanceRepositoryResult,
        SandboxInstanceState,
    };

    #[derive(Default)]
    struct InMemorySandboxInstances {
        rows: Mutex<BTreeMap<(String, String), SandboxInstance>>,
    }

    fn key(sandbox_instance: &SandboxInstance) -> (String, String) {
        (
            sandbox_instance.tenant_id().as_str().to_owned(),
            sandbox_instance.sandbox_instance_id().as_str().to_owned(),
        )
    }

    /// Fixed storage clock so the in-memory adapter is deterministic: a real
    /// adapter reads the database clock, which a test must not depend on.
    const TEST_STORAGE_CLOCK: &str = "2026-09-24T00:00:00Z";

    #[async_trait]
    impl SandboxInstanceRepository for InMemorySandboxInstances {
        async fn insert_sandbox_instance(
            &self,
            sandbox_instance: &SandboxInstance,
        ) -> SandboxInstanceRepositoryResult<SandboxInstance> {
            let mut rows = self.rows.lock().expect("instance store lock");
            let duplicate = rows.values().any(|row| {
                row.tenant_id() == sandbox_instance.tenant_id()
                    && row.sandbox_instance_owner_id()
                        == sandbox_instance.sandbox_instance_owner_id()
                    && row.sandbox_instance_name() == sandbox_instance.sandbox_instance_name()
            });
            if duplicate {
                return Err(SandboxInstanceRepositoryError::DuplicateName);
            }
            let stored = sandbox_instance.clone().with_persistence_timestamps(
                TEST_STORAGE_CLOCK.to_owned(),
                TEST_STORAGE_CLOCK.to_owned(),
            );
            rows.insert(key(sandbox_instance), stored.clone());
            Ok(stored)
        }

        async fn list_sandbox_instances(
            &self,
            tenant_id: &TenantId,
            sandbox_instance_owner_id: Option<&SandboxInstanceOwnerId>,
            sandbox_instance_state: Option<SandboxInstanceState>,
            cursor: Option<&SandboxInstanceListCursor>,
            page_size: u32,
        ) -> SandboxInstanceRepositoryResult<SandboxInstanceListPage> {
            if page_size == 0 {
                return Err(SandboxInstanceRepositoryError::InvalidPageRequest);
            }
            let rows = self.rows.lock().expect("instance store lock");
            // Same sort key as the PostgreSQL adapter; RFC 3339 UTC strings
            // with a fixed shape sort lexicographically like their instants.
            let mut filtered: Vec<SandboxInstance> = rows
                .values()
                .filter(|row| row.tenant_id() == tenant_id)
                .filter(|row| {
                    sandbox_instance_owner_id
                        .is_none_or(|owner| row.sandbox_instance_owner_id() == owner)
                })
                .filter(|row| {
                    sandbox_instance_state.is_none_or(|state| row.sandbox_instance_state() == state)
                })
                .cloned()
                .collect();
            filtered.sort_by(|left, right| {
                let left_key = (
                    left.created_at().unwrap_or_default(),
                    left.sandbox_instance_id().as_str(),
                );
                let right_key = (
                    right.created_at().unwrap_or_default(),
                    right.sandbox_instance_id().as_str(),
                );
                right_key.cmp(&left_key)
            });
            let window_start = match cursor {
                None => 0,
                Some(cursor) => filtered
                    .iter()
                    .position(|row| {
                        let row_key = (
                            row.created_at().unwrap_or_default(),
                            row.sandbox_instance_id().as_str(),
                        );
                        let cursor_key =
                            (cursor.created_at(), cursor.sandbox_instance_id().as_str());
                        row_key < cursor_key
                    })
                    .ok_or(SandboxInstanceRepositoryError::InvalidPageRequest)?,
            };
            let window = &filtered[window_start.min(filtered.len())..];
            let has_more = window.len() > page_size as usize;
            let items: Vec<SandboxInstance> =
                window.iter().take(page_size as usize).cloned().collect();
            let next_cursor = if has_more {
                let last = items
                    .last()
                    .ok_or(SandboxInstanceRepositoryError::InvalidStoredData)?;
                Some(
                    SandboxInstanceListCursor::new(
                        last.created_at().unwrap_or_default().to_owned(),
                        last.sandbox_instance_id().clone(),
                    )
                    .map_err(|_| SandboxInstanceRepositoryError::InvalidStoredData)?,
                )
            } else {
                None
            };
            Ok(SandboxInstanceListPage { items, next_cursor })
        }

        async fn get_sandbox_instance(
            &self,
            tenant_id: &TenantId,
            sandbox_instance_id: &SandboxInstanceId,
        ) -> SandboxInstanceRepositoryResult<Option<SandboxInstance>> {
            let rows = self.rows.lock().expect("instance store lock");
            Ok(rows
                .get(&(
                    tenant_id.as_str().to_owned(),
                    sandbox_instance_id.as_str().to_owned(),
                ))
                .cloned())
        }

        async fn save_sandbox_instance(
            &self,
            sandbox_instance: &SandboxInstance,
            expected_sandbox_version: u64,
        ) -> SandboxInstanceRepositoryResult<bool> {
            let mut rows = self.rows.lock().expect("instance store lock");
            let entry = key(sandbox_instance);
            let matches = rows
                .get(&entry)
                .is_some_and(|row| row.sandbox_version() == expected_sandbox_version);
            if matches {
                // Same uniqueness the PostgreSQL `uk_sandbox_instance_owner_name`
                // constraint enforces on UPDATE: a rename onto a sibling's name
                // is a `DuplicateName`, not a silent takeover.
                let duplicate = rows.values().any(|row| {
                    row.tenant_id() == sandbox_instance.tenant_id()
                        && row.sandbox_instance_owner_id()
                            == sandbox_instance.sandbox_instance_owner_id()
                        && row.sandbox_instance_name() == sandbox_instance.sandbox_instance_name()
                        && row.sandbox_instance_id() != sandbox_instance.sandbox_instance_id()
                });
                if duplicate {
                    return Err(SandboxInstanceRepositoryError::DuplicateName);
                }
                let created_at = rows
                    .get(&entry)
                    .and_then(SandboxInstance::created_at)
                    .unwrap_or(TEST_STORAGE_CLOCK)
                    .to_owned();
                rows.insert(
                    entry,
                    sandbox_instance
                        .clone()
                        .with_persistence_timestamps(created_at, TEST_STORAGE_CLOCK.to_owned()),
                );
            }
            Ok(matches)
        }

        async fn delete_sandbox_instance(
            &self,
            tenant_id: &TenantId,
            sandbox_instance_id: &SandboxInstanceId,
            expected_sandbox_version: u64,
        ) -> SandboxInstanceRepositoryResult<bool> {
            let mut rows = self.rows.lock().expect("instance store lock");
            let entry = (
                tenant_id.as_str().to_owned(),
                sandbox_instance_id.as_str().to_owned(),
            );
            let matches = rows
                .get(&entry)
                .is_some_and(|row| row.sandbox_version() == expected_sandbox_version);
            if matches {
                rows.remove(&entry);
            }
            Ok(matches)
        }
    }

    fn tenant() -> TenantId {
        TenantId::parse("tenant-instance").unwrap_or_else(|error| panic!("tenant: {error}"))
    }

    fn owner() -> SandboxInstanceOwnerId {
        SandboxInstanceOwnerId::parse("user-instance")
            .unwrap_or_else(|error| panic!("owner: {error}"))
    }

    fn create_command(name: &str) -> CreateSandboxInstanceCommand {
        CreateSandboxInstanceCommand {
            tenant_id: tenant(),
            sandbox_instance_owner_id: owner(),
            sandbox_instance_name: name.to_owned(),
            sandbox_instance_profile: SandboxInstanceProfile::Standard,
            sandbox_instance_base_image: "sdkwork/sandbox:0.1.0".to_owned(),
            sandbox_instance_vcpu_count: 2,
            sandbox_instance_memory_mb: 4_096,
            sandbox_instance_disk_mb: 20_480,
            sandbox_instance_required_capabilities: std::collections::BTreeSet::new(),
            sandbox_instance_minimum_assurance: IsolationAssurance::HostUser,
            sandbox_instance_auto_start: false,
            sandbox_instance_expires_at: None,
            sandbox_workspace_id: None,
        }
    }

    fn service() -> SandboxInstanceService<InMemorySandboxInstances> {
        SandboxInstanceService::new(Arc::new(InMemorySandboxInstances::default()))
    }

    #[tokio::test]
    async fn create_starts_requested_and_lists_under_its_owner() {
        let service = service();
        let created = service
            .create(create_command("primary"))
            .await
            .unwrap_or_else(|error| panic!("create: {error}"));
        assert_eq!(
            SandboxInstanceState::Requested,
            created.sandbox_instance_state()
        );
        assert_eq!(0, created.sandbox_version());
        assert_eq!(
            Some(TEST_STORAGE_CLOCK),
            created.created_at(),
            "create must return the storage-assigned creation timestamp"
        );
        assert_eq!(Some(TEST_STORAGE_CLOCK), created.updated_at());
        let page = service
            .list(&tenant(), Some(&owner()), None, None, 20)
            .await
            .unwrap_or_else(|error| panic!("list: {error}"));
        assert!(page.next_cursor.is_none(), "one row must end enumeration");
        assert_eq!(1, page.items.len());
    }

    #[tokio::test]
    async fn create_rejects_a_duplicate_name_for_the_same_owner() {
        let service = service();
        service
            .create(create_command("primary"))
            .await
            .unwrap_or_else(|error| panic!("first create: {error}"));
        assert!(matches!(
            service.create(create_command("primary")).await,
            Err(SandboxInstanceError::DuplicateName { .. })
        ));
    }

    #[tokio::test]
    async fn create_rejects_a_resource_shape_outside_the_selected_profile() {
        let service = service();
        let mut command = create_command("oversized");
        command.sandbox_instance_vcpu_count = 32;
        assert!(matches!(
            service.create(command).await,
            Err(SandboxInstanceError::Validation {
                field: "sandboxInstanceVcpuCount",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn update_advances_the_version_and_persists_the_change() {
        let service = service();
        let created = service
            .create(create_command("primary"))
            .await
            .unwrap_or_else(|error| panic!("create: {error}"));
        let updated = service
            .update(UpdateSandboxInstanceCommand {
                tenant_id: tenant(),
                sandbox_instance_id: created.sandbox_instance_id().clone(),
                sandbox_instance_name: Some("renamed".to_owned()),
                sandbox_instance_state: Some(SandboxInstanceState::Active),
                sandbox_instance_expires_at: SandboxInstanceExpiryUpdate::Set(
                    "2030-01-01T00:00:00Z".to_owned(),
                ),
                ..UpdateSandboxInstanceCommand::for_instance(
                    tenant(),
                    created.sandbox_instance_id().clone(),
                )
            })
            .await
            .unwrap_or_else(|error| panic!("update: {error}"));
        assert_eq!("renamed", updated.sandbox_instance_name());
        assert_eq!(
            SandboxInstanceState::Active,
            updated.sandbox_instance_state()
        );
        assert_eq!(1, updated.sandbox_version());
        let reloaded = service
            .retrieve(&tenant(), created.sandbox_instance_id())
            .await
            .unwrap_or_else(|error| panic!("retrieve: {error}"));
        assert_eq!("renamed", reloaded.sandbox_instance_name());
        assert_eq!(
            Some("2030-01-01T00:00:00Z"),
            reloaded.sandbox_instance_expires_at()
        );
    }

    #[tokio::test]
    async fn update_rejects_a_transition_out_of_a_terminal_state() {
        let service = service();
        let created = service
            .create(create_command("primary"))
            .await
            .unwrap_or_else(|error| panic!("create: {error}"));
        // `requested` cannot reach `terminated` directly, so the instance is
        // activated first: the point of this test is the terminal-state guard,
        // not the admission matrix, which the model tests already cover.
        let activated = service
            .update(UpdateSandboxInstanceCommand {
                sandbox_instance_state: Some(SandboxInstanceState::Active),
                ..UpdateSandboxInstanceCommand::for_instance(
                    tenant(),
                    created.sandbox_instance_id().clone(),
                )
            })
            .await
            .unwrap_or_else(|error| panic!("activate: {error}"));
        service
            .update(UpdateSandboxInstanceCommand {
                sandbox_instance_state: Some(SandboxInstanceState::Terminated),
                ..UpdateSandboxInstanceCommand::for_instance(
                    tenant(),
                    activated.sandbox_instance_id().clone(),
                )
            })
            .await
            .unwrap_or_else(|error| panic!("terminate: {error}"));
        assert!(matches!(
            service
                .update(UpdateSandboxInstanceCommand {
                    sandbox_instance_state: Some(SandboxInstanceState::Suspended),
                    ..UpdateSandboxInstanceCommand::for_instance(
                        tenant(),
                        created.sandbox_instance_id().clone(),
                    )
                })
                .await,
            Err(SandboxInstanceError::InvalidStateTransition { .. })
        ));
    }

    #[tokio::test]
    async fn delete_refuses_a_live_instance_and_accepts_a_suspended_one() {
        let service = service();
        let created = service
            .create(create_command("primary"))
            .await
            .unwrap_or_else(|error| panic!("create: {error}"));
        let activated = service
            .update(UpdateSandboxInstanceCommand {
                sandbox_instance_state: Some(SandboxInstanceState::Active),
                ..UpdateSandboxInstanceCommand::for_instance(
                    tenant(),
                    created.sandbox_instance_id().clone(),
                )
            })
            .await
            .unwrap_or_else(|error| panic!("activate: {error}"));
        assert!(matches!(
            service
                .delete(&tenant(), activated.sandbox_instance_id())
                .await,
            Err(SandboxInstanceError::InstanceNotDeletable { .. })
        ));
        let suspended = service
            .update(UpdateSandboxInstanceCommand {
                sandbox_instance_state: Some(SandboxInstanceState::Suspended),
                ..UpdateSandboxInstanceCommand::for_instance(
                    tenant(),
                    activated.sandbox_instance_id().clone(),
                )
            })
            .await
            .unwrap_or_else(|error| panic!("suspend: {error}"));
        service
            .delete(&tenant(), suspended.sandbox_instance_id())
            .await
            .unwrap_or_else(|error| panic!("delete: {error}"));
        assert!(matches!(
            service
                .retrieve(&tenant(), suspended.sandbox_instance_id())
                .await,
            Err(SandboxInstanceError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn retrieve_hides_another_tenants_instance() {
        let service = service();
        let created = service
            .create(create_command("primary"))
            .await
            .unwrap_or_else(|error| panic!("create: {error}"));
        let other_tenant =
            TenantId::parse("tenant-other").unwrap_or_else(|error| panic!("tenant: {error}"));
        assert!(matches!(
            service
                .retrieve(&other_tenant, created.sandbox_instance_id())
                .await,
            Err(SandboxInstanceError::NotFound { .. })
        ));
    }

    #[tokio::test]
    async fn list_rejects_an_out_of_range_page_size() {
        let service = service();
        assert!(matches!(
            service.list(&tenant(), None, None, None, 0).await,
            Err(SandboxInstanceError::Validation {
                field: "page_size",
                ..
            })
        ));
        assert!(matches!(
            service.list(&tenant(), None, None, None, 201).await,
            Err(SandboxInstanceError::Validation {
                field: "page_size",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn list_pages_by_keyset_cursor_until_enumeration_ends() {
        let store = Arc::new(InMemorySandboxInstances::default());
        let service = SandboxInstanceService::new(store.clone());
        for ordinal in 0..5 {
            let instance = SandboxInstance::request(create_command(&format!("instance-{ordinal}")))
                .unwrap_or_else(|error| panic!("request: {error}"))
                .with_persistence_timestamps(
                    format!("2026-09-2{}T00:00:00Z", ordinal),
                    format!("2026-09-2{}T00:00:00Z", ordinal),
                );
            store.rows.lock().expect("instance store lock").insert(
                (
                    instance.tenant_id().as_str().to_owned(),
                    instance.sandbox_instance_id().as_str().to_owned(),
                ),
                instance,
            );
        }

        let mut visited = Vec::new();
        let mut cursor = None;
        loop {
            let page = service
                .list(&tenant(), None, None, cursor.as_ref(), 2)
                .await
                .unwrap_or_else(|error| panic!("list: {error}"));
            let reached_end = page.next_cursor.is_none();
            for item in &page.items {
                visited.push(item.sandbox_instance_name().to_owned());
            }
            cursor = page.next_cursor;
            if reached_end {
                break;
            }
        }
        assert_eq!(
            vec![
                "instance-4",
                "instance-3",
                "instance-2",
                "instance-1",
                "instance-0"
            ],
            visited,
            "cursor paging must visit every instance exactly once, newest first"
        );
    }

    #[test]
    fn cursor_rejects_a_timestamp_outside_the_stored_shape() {
        let cursor = SandboxInstanceListCursor::new(
            "not-a-timestamp".to_owned(),
            sdkwork_sandbox_provider_spi::SandboxInstanceId::parse("instance-0")
                .unwrap_or_else(|error| panic!("id: {error}")),
        );
        assert!(matches!(
            cursor,
            Err(SandboxInstanceError::Validation {
                field: "cursor",
                ..
            })
        ));
    }
}
