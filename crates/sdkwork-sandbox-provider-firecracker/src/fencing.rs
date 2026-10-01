//! Provider-private allocation fencing state (`REQ-2026-0008`).
//!
//! Every `SandboxRuntimeBindingId` carries one monotonic maximum observed
//! fencing token, persisted provider-privately. Every mutating provider
//! operation rejects a token below the recorded maximum *before* any side
//! effect, and the record must survive a node process restart. The store seam
//! keeps that decision pure: [`SandboxFirecrackerFencingStore`] is the port,
//! [`SandboxFirecrackerFileFencingStore`] is the durable production form
//! (atomic temporary-file write plus rename, strict parse, fail-closed on a
//! corrupt record), and [`SandboxFirecrackerInMemoryFencingStore`] is the
//! test/dev form.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use async_trait::async_trait;
use thiserror::Error;

/// Why a fencing store operation failed. A corrupt or unreadable record fails
/// closed: the caller may not proceed on the assumption that no token was
/// recorded, because that is exactly how a stale writer would win.
#[derive(Debug, Error)]
pub enum SandboxFirecrackerFencingStoreError {
    /// The durable record exists but does not parse as one fencing token.
    #[error("sandbox fencing record is corrupt for a runtime binding")]
    RecordCorrupt,
    /// The recorded token is below the store's maximum: a stale write.
    #[error("sandbox fencing record is below the recorded maximum")]
    StaleRecord,
    /// The durable store could not be read or written.
    #[error("sandbox fencing store is unavailable")]
    StoreUnavailable(#[from] std::io::Error),
    /// The store refused a new binding because its tracking bound is
    /// exhausted.
    #[error("sandbox fencing store tracking bound is exhausted")]
    TrackingBoundExhausted,
}

/// The outcome of recording one newly observed fencing token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxFirecrackerFencingDecision {
    /// The token is strictly newer than every recorded observation; the store
    /// now records it and the caller may proceed to its side effects.
    Advanced,
    /// The token equals the recorded maximum: the same authority replaying its
    /// own observation. The caller resolves the replay against its own
    /// registry.
    Replayed,
}

/// The provider-private fencing store port. Implementations own durability;
/// callers own the decision ordering (reject stale before any side effect).
#[async_trait]
pub trait SandboxFirecrackerFencingStore: Send + Sync {
    /// Loads the highest fencing token recorded for one binding; zero when the
    /// binding has no record yet.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxFirecrackerFencingStoreError`] when the record is
    /// corrupt or the store is unreadable; the caller must fail closed.
    async fn sandbox_max_observed_fencing_token(
        &self,
        sandbox_runtime_binding_id: &str,
    ) -> Result<u64, SandboxFirecrackerFencingStoreError>;

    /// Records a newly observed fencing token for one binding. Recording a
    /// token lower than the recorded maximum is a stale write and is refused;
    /// recording the equal token is an idempotent replay of the same
    /// authority.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxFirecrackerFencingStoreError::StaleRecord`] for a
    /// stale (lower) token, [`RecordCorrupt`](Self::sandbox_max_observed_fencing_token)
    /// failure families for I/O and parse failure.
    async fn sandbox_record_observed_fencing_token(
        &self,
        sandbox_runtime_binding_id: &str,
        sandbox_fencing_token: u64,
    ) -> Result<SandboxFirecrackerFencingDecision, SandboxFirecrackerFencingStoreError>;
}

/// The durable production store: one strictly-parsed decimal token file per
/// binding under a composition-owned runtime data root. Writes go to a
/// temporary file that is flushed and then renamed over the record, so a
/// crash mid-write leaves the previous maximum intact.
///
/// The record decision is a read-modify-write, so concurrent writers are
/// serialized under a process-local lock: without it, two racing writers
/// could both observe the old maximum and the lower write could land after
/// the higher one, silently breaking the monotonicity the fencing authority
/// depends on. The lock guards only the synchronous file work (one tiny
/// token read plus one atomic rename - no await is held under it); a
/// multi-process node replaces this store via the same port.
pub struct SandboxFirecrackerFileFencingStore {
    sandbox_records_root: PathBuf,
    sandbox_write_lock: std::sync::Mutex<()>,
}

impl SandboxFirecrackerFileFencingStore {
    /// Builds the store over a composition-owned runtime data root, creating
    /// the records directory when missing.
    ///
    /// # Errors
    ///
    /// Returns the underlying I/O error when the records directory cannot be
    /// created; the provider must report Unavailable instead of running
    /// unfenced.
    pub fn new(sandbox_records_root: PathBuf) -> Result<Self, SandboxFirecrackerFencingStoreError> {
        std::fs::create_dir_all(&sandbox_records_root)?;
        Ok(Self {
            sandbox_records_root,
            sandbox_write_lock: std::sync::Mutex::new(()),
        })
    }

    /// The store directory handed over at construction.
    #[must_use]
    pub fn sandbox_records_root(&self) -> &Path {
        &self.sandbox_records_root
    }

    /// The record file for one binding. Binding identifiers are opaque and may
    /// legally contain characters that are invalid in host filenames (the
    /// identifier vocabulary includes `:`), so the file name is the fixed hex
    /// encoding of the identifier bytes: deterministic, collision-free, and
    /// host-filesystem-safe on every supported node.
    fn sandbox_record_path(&self, sandbox_runtime_binding_id: &str) -> PathBuf {
        const SANDBOX_HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut sandbox_file_name =
            String::with_capacity(sandbox_runtime_binding_id.len() * 2 + "binding-.token".len());
        sandbox_file_name.push_str("binding-");
        for sandbox_byte in sandbox_runtime_binding_id.bytes() {
            sandbox_file_name.push(SANDBOX_HEX_DIGITS[usize::from(sandbox_byte >> 4)] as char);
            sandbox_file_name.push(SANDBOX_HEX_DIGITS[usize::from(sandbox_byte & 0x0F)] as char);
        }
        sandbox_file_name.push_str(".token");
        self.sandbox_records_root.join(sandbox_file_name)
    }

    fn sandbox_parse_record(
        sandbox_contents: &str,
    ) -> Result<u64, SandboxFirecrackerFencingStoreError> {
        let sandbox_trimmed = sandbox_contents.trim_ascii();
        if sandbox_trimmed.is_empty()
            || sandbox_trimmed.len() > 20
            || !sandbox_trimmed
                .bytes()
                .all(|sandbox_byte| sandbox_byte.is_ascii_digit())
        {
            return Err(SandboxFirecrackerFencingStoreError::RecordCorrupt);
        }
        let sandbox_parsed: u64 = sandbox_trimmed
            .parse()
            .map_err(|_| SandboxFirecrackerFencingStoreError::RecordCorrupt)?;
        if sandbox_parsed == 0 {
            return Err(SandboxFirecrackerFencingStoreError::RecordCorrupt);
        }
        Ok(sandbox_parsed)
    }

    fn sandbox_read_record_sync(
        &self,
        sandbox_record_path: &Path,
    ) -> Result<u64, SandboxFirecrackerFencingStoreError> {
        match std::fs::read_to_string(sandbox_record_path) {
            Ok(sandbox_contents) => Self::sandbox_parse_record(&sandbox_contents),
            Err(sandbox_error) if sandbox_error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(sandbox_error) => Err(SandboxFirecrackerFencingStoreError::StoreUnavailable(
                sandbox_error,
            )),
        }
    }

    /// Synchronous atomic record write: temporary file, full sync, rename.
    /// Callers hold [`Self::sandbox_write_lock`] so only this serialized
    /// section mutates a record.
    fn sandbox_write_record_sync(
        &self,
        sandbox_record_path: &Path,
        sandbox_fencing_token: u64,
    ) -> Result<(), SandboxFirecrackerFencingStoreError> {
        let mut sandbox_temporary_name = sandbox_record_path.as_os_str().to_os_string();
        sandbox_temporary_name.push(".tmp");
        let sandbox_temporary_path = PathBuf::from(sandbox_temporary_name);
        {
            use std::io::Write;
            let mut sandbox_file = std::fs::File::create(&sandbox_temporary_path)?;
            sandbox_file.write_all(sandbox_fencing_token.to_string().as_bytes())?;
            sandbox_file.sync_all()?;
        }
        std::fs::rename(&sandbox_temporary_path, sandbox_record_path)?;
        Ok(())
    }
}

#[async_trait]
impl SandboxFirecrackerFencingStore for SandboxFirecrackerFileFencingStore {
    async fn sandbox_max_observed_fencing_token(
        &self,
        sandbox_runtime_binding_id: &str,
    ) -> Result<u64, SandboxFirecrackerFencingStoreError> {
        let sandbox_record_path = self.sandbox_record_path(sandbox_runtime_binding_id);
        match tokio::fs::read_to_string(&sandbox_record_path).await {
            Ok(sandbox_contents) => Self::sandbox_parse_record(&sandbox_contents),
            Err(sandbox_error) if sandbox_error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(sandbox_error) => Err(SandboxFirecrackerFencingStoreError::StoreUnavailable(
                sandbox_error,
            )),
        }
    }

    async fn sandbox_record_observed_fencing_token(
        &self,
        sandbox_runtime_binding_id: &str,
        sandbox_fencing_token: u64,
    ) -> Result<SandboxFirecrackerFencingDecision, SandboxFirecrackerFencingStoreError> {
        // The serialized read-modify-write section: a std lock around
        // synchronous file work, so no await is ever held under a guard and
        // two racing writers cannot interleave read-decide-write steps.
        let sandbox_guard = self
            .sandbox_write_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let sandbox_record_path = self.sandbox_record_path(sandbox_runtime_binding_id);
        let sandbox_recorded = self.sandbox_read_record_sync(&sandbox_record_path)?;
        let sandbox_decision = if sandbox_fencing_token < sandbox_recorded {
            return Err(SandboxFirecrackerFencingStoreError::StaleRecord);
        } else if sandbox_fencing_token == sandbox_recorded {
            SandboxFirecrackerFencingDecision::Replayed
        } else {
            self.sandbox_write_record_sync(&sandbox_record_path, sandbox_fencing_token)?;
            SandboxFirecrackerFencingDecision::Advanced
        };
        drop(sandbox_guard);
        Ok(sandbox_decision)
    }
}

/// The test/dev store: an in-memory map with the same monotonic semantics as
/// the durable store, so every fencing decision is exercised identically
/// without a runtime data root.
#[derive(Default)]
pub struct SandboxFirecrackerInMemoryFencingStore {
    sandbox_records: Mutex<HashMap<String, u64>>,
}

impl SandboxFirecrackerInMemoryFencingStore {
    fn sandbox_lock(&self) -> MutexGuard<'_, HashMap<String, u64>> {
        self.sandbox_records
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

#[async_trait]
impl SandboxFirecrackerFencingStore for SandboxFirecrackerInMemoryFencingStore {
    async fn sandbox_max_observed_fencing_token(
        &self,
        sandbox_runtime_binding_id: &str,
    ) -> Result<u64, SandboxFirecrackerFencingStoreError> {
        Ok(self
            .sandbox_lock()
            .get(sandbox_runtime_binding_id)
            .copied()
            .unwrap_or_default())
    }

    async fn sandbox_record_observed_fencing_token(
        &self,
        sandbox_runtime_binding_id: &str,
        sandbox_fencing_token: u64,
    ) -> Result<SandboxFirecrackerFencingDecision, SandboxFirecrackerFencingStoreError> {
        let mut sandbox_records = self.sandbox_lock();
        match sandbox_records.get(sandbox_runtime_binding_id).copied() {
            Some(sandbox_recorded) if sandbox_fencing_token < sandbox_recorded => {
                Err(SandboxFirecrackerFencingStoreError::StaleRecord)
            }
            Some(sandbox_recorded) if sandbox_fencing_token == sandbox_recorded => {
                Ok(SandboxFirecrackerFencingDecision::Replayed)
            }
            _ => {
                if sandbox_records.len() >= MAX_SANDBOX_IN_MEMORY_TRACKED_BINDINGS
                    && !sandbox_records.contains_key(sandbox_runtime_binding_id)
                {
                    return Err(SandboxFirecrackerFencingStoreError::TrackingBoundExhausted);
                }
                sandbox_records
                    .insert(sandbox_runtime_binding_id.to_owned(), sandbox_fencing_token);
                Ok(SandboxFirecrackerFencingDecision::Advanced)
            }
        }
    }
}

/// Upper bound on tracked binding records in the in-memory store. A dev or
/// test host that never retires bindings would otherwise grow the map with
/// request volume; the durable store carries no such bound because records
/// live in per-binding files owned by the runtime data root.
const MAX_SANDBOX_IN_MEMORY_TRACKED_BINDINGS: usize = 65_536;

/// Why a fenced authority check refused to proceed.
#[derive(Debug, Error)]
pub enum SandboxFirecrackerFencingAuthorityError {
    /// The token is older than the recorded maximum, or is the zero
    /// unset-lease sentinel: a stale writer must never reach a side effect.
    #[error("sandbox fencing token is stale")]
    StaleFencing,
    /// The store could not prove or record the maximum; the operation fails
    /// closed instead of running unfenced.
    #[error("sandbox fencing store could not prove authority")]
    StoreUnavailable(#[source] Box<SandboxFirecrackerFencingStoreError>),
}

/// The fenced-authority sequence every mutating provider operation runs before
/// any side effect: reject the unset sentinel, record the observed maximum,
/// and let the caller resolve [`Replayed`](SandboxFirecrackerFencingDecision::Replayed)
/// against its own registry.
///
/// # Errors
///
/// Returns [`SandboxFirecrackerFencingAuthorityError::StaleFencing`] when the
/// token is below the recorded maximum, so the caller refuses the operation
/// before touching the registry, the broker, or any binding resource.
pub async fn sandbox_check_fencing_authority(
    sandbox_store: &dyn SandboxFirecrackerFencingStore,
    sandbox_runtime_binding_id: &str,
    sandbox_fencing_token: u64,
) -> Result<SandboxFirecrackerFencingDecision, SandboxFirecrackerFencingAuthorityError> {
    if sandbox_fencing_token == 0 {
        return Err(SandboxFirecrackerFencingAuthorityError::StaleFencing);
    }
    match sandbox_store
        .sandbox_record_observed_fencing_token(sandbox_runtime_binding_id, sandbox_fencing_token)
        .await
    {
        Ok(sandbox_decision) => Ok(sandbox_decision),
        Err(SandboxFirecrackerFencingStoreError::StaleRecord) => {
            Err(SandboxFirecrackerFencingAuthorityError::StaleFencing)
        }
        Err(sandbox_error) => {
            tracing::warn!("sandbox fencing store refused a fencing record: {sandbox_error}");
            Err(SandboxFirecrackerFencingAuthorityError::StoreUnavailable(
                Box::new(sandbox_error),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        sandbox_check_fencing_authority, SandboxFirecrackerFencingAuthorityError,
        SandboxFirecrackerFencingDecision, SandboxFirecrackerFencingStore,
        SandboxFirecrackerFileFencingStore, SandboxFirecrackerInMemoryFencingStore,
    };

    fn sandbox_temp_root(sandbox_label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "sdkwork-sandbox-firecracker-fencing-{sandbox_label}-{}",
            uuid::Uuid::new_v4()
        ))
    }

    #[tokio::test]
    async fn fencing_rejects_unset_and_stale_tokens_before_recording() {
        let sandbox_store = SandboxFirecrackerInMemoryFencingStore::default();
        assert!(matches!(
            sandbox_check_fencing_authority(&sandbox_store, "binding-1", 0).await,
            Err(SandboxFirecrackerFencingAuthorityError::StaleFencing)
        ));
        assert!(matches!(
            sandbox_check_fencing_authority(&sandbox_store, "binding-1", 7).await,
            Ok(SandboxFirecrackerFencingDecision::Advanced)
        ));
        assert!(matches!(
            sandbox_check_fencing_authority(&sandbox_store, "binding-1", 7).await,
            Ok(SandboxFirecrackerFencingDecision::Replayed)
        ));
        assert!(matches!(
            sandbox_check_fencing_authority(&sandbox_store, "binding-1", 6).await,
            Err(SandboxFirecrackerFencingAuthorityError::StaleFencing)
        ));
        // A rejected stale write must not move the recorded maximum.
        assert!(matches!(
            sandbox_store
                .sandbox_max_observed_fencing_token("binding-1")
                .await,
            Ok(7)
        ));
    }

    #[tokio::test]
    async fn file_store_survives_a_node_process_restart() {
        let sandbox_root = sandbox_temp_root("restart");
        {
            let sandbox_store = SandboxFirecrackerFileFencingStore::new(sandbox_root.clone())
                .expect("fencing records root must create");
            assert!(matches!(
                sandbox_check_fencing_authority(&sandbox_store, "binding/odd:id", 41).await,
                Ok(SandboxFirecrackerFencingDecision::Advanced)
            ));
        }
        // A fresh store instance over the same root is the restart form of the
        // same node: the maximum must come back exactly.
        let sandbox_restarted = SandboxFirecrackerFileFencingStore::new(sandbox_root.clone())
            .expect("fencing records root must reopen");
        assert!(matches!(
            SandboxFirecrackerFencingStore::sandbox_max_observed_fencing_token(
                &sandbox_restarted,
                "binding/odd:id"
            )
            .await,
            Ok(41)
        ));
        assert!(matches!(
            sandbox_check_fencing_authority(&sandbox_restarted, "binding/odd:id", 40).await,
            Err(SandboxFirecrackerFencingAuthorityError::StaleFencing)
        ));
        std::fs::remove_file(sandbox_root.join("binding-62696e64696e672f6f64643a6964.token"))
            .expect("enumerated fencing record must remove");
        std::fs::remove_dir(sandbox_root).expect("enumerated fencing root must remove");
    }

    #[tokio::test]
    async fn racing_writers_cannot_lose_the_higher_token() {
        let sandbox_root = sandbox_temp_root("race");
        let sandbox_store = std::sync::Arc::new(
            SandboxFirecrackerFileFencingStore::new(sandbox_root.clone())
                .expect("fencing records root must create"),
        );
        // Two racing records for one binding: whatever the interleaving, the
        // serialized read-modify-write must keep the maximum monotonic - the
        // record ends at the higher token and later rejects the lower one.
        let sandbox_writer_low = std::sync::Arc::clone(&sandbox_store);
        let sandbox_writer_high = std::sync::Arc::clone(&sandbox_store);
        let (sandbox_low, sandbox_high) = tokio::join!(
            async move {
                SandboxFirecrackerFencingStore::sandbox_record_observed_fencing_token(
                    sandbox_writer_low.as_ref(),
                    "binding-race",
                    8,
                )
                .await
            },
            async move {
                SandboxFirecrackerFencingStore::sandbox_record_observed_fencing_token(
                    sandbox_writer_high.as_ref(),
                    "binding-race",
                    9,
                )
                .await
            },
        );
        // Exactly one advance wins per token; the loser of the ordering sees
        // its write land while the record already holds an equal-or-higher
        // maximum, which is either an Advanced (it wrote the new max) or a
        // Replayed (the other writer had already recorded the same value) -
        // never a stale overwrite.
        assert!(
            sandbox_low.is_ok() && sandbox_high.is_ok(),
            "both racing records must succeed: {sandbox_low:?} / {sandbox_high:?}"
        );
        assert!(
            matches!(
                SandboxFirecrackerFencingStore::sandbox_max_observed_fencing_token(
                    sandbox_store.as_ref(),
                    "binding-race"
                )
                .await,
                Ok(9)
            ),
            "the maximum must survive the race at the higher token"
        );
        assert!(
            matches!(
                SandboxFirecrackerFencingStore::sandbox_record_observed_fencing_token(
                    sandbox_store.as_ref(),
                    "binding-race",
                    8
                )
                .await,
                Err(super::SandboxFirecrackerFencingStoreError::StaleRecord)
            ),
            "the lower token must be stale after the race"
        );
        std::fs::remove_file(sandbox_root.join("binding-62696e64696e672d72616365.token"))
            .expect("enumerated fencing record must remove");
        std::fs::remove_dir(sandbox_root).expect("enumerated fencing root must remove");
    }

    #[tokio::test]
    async fn file_store_fails_closed_on_a_corrupt_record() {
        let sandbox_root = sandbox_temp_root("corrupt");
        let sandbox_store = SandboxFirecrackerFileFencingStore::new(sandbox_root.clone())
            .expect("fencing records root must create");
        let sandbox_record_path = sandbox_root.join("binding-62696e64696e672d31.token");
        std::fs::write(&sandbox_record_path, b"not-a-token").expect("record must write");
        assert!(matches!(
            SandboxFirecrackerFencingStore::sandbox_max_observed_fencing_token(
                &sandbox_store,
                "binding-1"
            )
            .await,
            Err(super::SandboxFirecrackerFencingStoreError::RecordCorrupt)
        ));
        // A corrupt record refuses the operation instead of resetting to zero.
        assert!(matches!(
            sandbox_check_fencing_authority(&sandbox_store, "binding-1", 9).await,
            Err(SandboxFirecrackerFencingAuthorityError::StoreUnavailable(_))
        ));
        std::fs::remove_file(&sandbox_record_path).expect("enumerated record must remove");
        std::fs::remove_dir(sandbox_root).expect("enumerated root must remove");
    }
}
