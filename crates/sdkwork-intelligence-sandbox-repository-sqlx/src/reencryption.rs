//! Tenant-scoped re-encryption sweep for stored provider allocation
//! references (REQ-2026-0006).
//!
//! One [`SandboxProviderAllocationReencryptionPage`] re-encrypts one bounded
//! keyset window of `sandbox_runtime_binding` rows onto the key source's
//! current version and reports `sandbox_conflict_count` for rows whose full
//! old-ciphertext CAS failed. The sweep is intentionally single-pass and
//! forward-only: a conflicted row is never retried inside the page that
//! detected it, so an orchestration loop must keep re-running pages from the
//! returned cursor — and restart the whole scan when a page reports a nonzero
//! conflict count — until one full pass reports zero conflicts before any old
//! key is revoked. That orchestration loop is materialized here as
//! [`SqlxSandboxSessionRepository::reencrypt_sandbox_provider_allocation_references_until_settled`].

use sdkwork_intelligence_sandbox_service::{
    SandboxProtectedProviderAllocationRef, SandboxProviderAllocationProtectionContext,
    SandboxProviderAllocationProtectionVersion, SandboxSessionRepositoryError,
    SandboxSessionRepositoryResult,
};
use sdkwork_sandbox_provider_spi::{SandboxRuntimeBindingId, SandboxSessionId, TenantId};
use sqlx::Row;

use crate::SqlxSandboxSessionRepository;

const MAX_SANDBOX_REENCRYPTION_PAGE_SIZE: u16 = 200;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SandboxProviderAllocationReencryptionPage {
    sandbox_scanned_count: usize,
    sandbox_reencrypted_count: usize,
    sandbox_conflict_count: usize,
    sandbox_next_runtime_binding_id: Option<SandboxRuntimeBindingId>,
}

impl SandboxProviderAllocationReencryptionPage {
    fn new(
        sandbox_scanned_count: usize,
        sandbox_reencrypted_count: usize,
        sandbox_conflict_count: usize,
        sandbox_next_runtime_binding_id: Option<SandboxRuntimeBindingId>,
    ) -> Self {
        Self {
            sandbox_scanned_count,
            sandbox_reencrypted_count,
            sandbox_conflict_count,
            sandbox_next_runtime_binding_id,
        }
    }

    #[must_use]
    pub fn sandbox_scanned_count(&self) -> usize {
        self.sandbox_scanned_count
    }

    #[must_use]
    pub fn sandbox_reencrypted_count(&self) -> usize {
        self.sandbox_reencrypted_count
    }

    #[must_use]
    pub fn sandbox_conflict_count(&self) -> usize {
        self.sandbox_conflict_count
    }

    #[must_use]
    pub fn sandbox_next_runtime_binding_id(&self) -> Option<&SandboxRuntimeBindingId> {
        self.sandbox_next_runtime_binding_id.as_ref()
    }
}

struct SandboxProviderAllocationReencryptionCandidate {
    sandbox_session_id: SandboxSessionId,
    sandbox_runtime_binding_id: SandboxRuntimeBindingId,
    sandbox_protected_allocation_reference: SandboxProtectedProviderAllocationRef,
}

/// Upper bound on whole-scan passes one settlement may take. A live
/// lifecycle writer that keeps re-keying rows every pass would otherwise spin
/// forever; the loop reports failure instead of retrying without bound.
const MAX_SANDBOX_REENCRYPTION_SETTLE_PASSES: u32 = 8;

/// The settled result of a full re-encryption sweep: the pass that reported
/// zero conflicts, plus the totals accumulated across every pass it took.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SandboxProviderAllocationReencryptionSettlement {
    sandbox_passes: u32,
    sandbox_scanned_count: usize,
    sandbox_reencrypted_count: usize,
}

impl SandboxProviderAllocationReencryptionSettlement {
    #[must_use]
    pub fn sandbox_passes(&self) -> u32 {
        self.sandbox_passes
    }

    #[must_use]
    pub fn sandbox_scanned_count(&self) -> usize {
        self.sandbox_scanned_count
    }

    #[must_use]
    pub fn sandbox_reencrypted_count(&self) -> usize {
        self.sandbox_reencrypted_count
    }
}

impl SqlxSandboxSessionRepository {
    /// Drives the sweep to a settled pass, per the module contract: full
    /// keyset passes over the tenant's rows; any page reporting a nonzero
    /// conflict count restarts the whole scan; the first zero-conflict pass
    /// settles the sweep (after which the old key may be revoked). Bounded by
    /// [`MAX_SANDBOX_REENCRYPTION_SETTLE_PASSES`] so a permanently concurrent
    /// writer fails loudly instead of looping forever.
    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::InvalidPageRequest` for an
    /// out-of-range page size, and `ProtectionFailed` when the sweep could
    /// not settle within the pass bound.
    pub async fn reencrypt_sandbox_provider_allocation_references_until_settled(
        &self,
        tenant_id: &TenantId,
        sandbox_page_size: u16,
    ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationReencryptionSettlement> {
        if !(1..=MAX_SANDBOX_REENCRYPTION_PAGE_SIZE).contains(&sandbox_page_size) {
            return Err(SandboxSessionRepositoryError::InvalidPageRequest);
        }
        let mut sandbox_total_scanned = 0usize;
        let mut sandbox_total_reencrypted = 0usize;
        for sandbox_pass in 1..=MAX_SANDBOX_REENCRYPTION_SETTLE_PASSES {
            let mut sandbox_cursor: Option<SandboxRuntimeBindingId> = None;
            let mut sandbox_pass_conflicts = 0usize;
            loop {
                let sandbox_page = self
                    .reencrypt_sandbox_provider_allocation_references_page(
                        tenant_id,
                        sandbox_cursor.as_ref(),
                        sandbox_page_size,
                    )
                    .await?;
                sandbox_total_scanned += sandbox_page.sandbox_scanned_count();
                sandbox_total_reencrypted += sandbox_page.sandbox_reencrypted_count();
                sandbox_pass_conflicts += sandbox_page.sandbox_conflict_count();
                match sandbox_page.sandbox_next_runtime_binding_id() {
                    Some(sandbox_next) => sandbox_cursor = Some(sandbox_next.clone()),
                    None => break,
                }
            }
            if sandbox_pass_conflicts == 0 {
                return Ok(SandboxProviderAllocationReencryptionSettlement {
                    sandbox_passes: sandbox_pass,
                    sandbox_scanned_count: sandbox_total_scanned,
                    sandbox_reencrypted_count: sandbox_total_reencrypted,
                });
            }
        }
        Err(SandboxSessionRepositoryError::ProtectionFailed)
    }
}

fn ensure_sandbox_reencryption_target(
    sandbox_expected_protection_version: &SandboxProviderAllocationProtectionVersion,
    sandbox_reencrypted_allocation_reference: &SandboxProtectedProviderAllocationRef,
) -> SandboxSessionRepositoryResult<()> {
    if !sandbox_expected_protection_version
        .matches_sandbox_protected_allocation_reference(sandbox_reencrypted_allocation_reference)
    {
        return Err(SandboxSessionRepositoryError::ProtectionFailed);
    }
    Ok(())
}

impl SqlxSandboxSessionRepository {
    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::InvalidPageRequest` for an
    /// invalid page request; key-source and persistence failures propagate
    /// unchanged and never silently skip rows.
    pub async fn reencrypt_sandbox_provider_allocation_references_page(
        &self,
        tenant_id: &TenantId,
        sandbox_after_runtime_binding_id: Option<&SandboxRuntimeBindingId>,
        sandbox_page_size: u16,
    ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationReencryptionPage> {
        if !(1..=MAX_SANDBOX_REENCRYPTION_PAGE_SIZE).contains(&sandbox_page_size) {
            return Err(SandboxSessionRepositoryError::InvalidPageRequest);
        }

        let sandbox_current_protection_version = self
            .sandbox_allocation_protector()
            .current_sandbox_allocation_protection_version()?;
        let sandbox_query_limit = i64::from(sandbox_page_size) + 1;
        let sandbox_candidate_rows = sqlx::query(
            "SELECT tenant_id, sandbox_session_id, sandbox_runtime_binding_id, \
                    sandbox_allocation_ciphertext, sandbox_allocation_key_id, \
                    sandbox_allocation_key_version, sandbox_allocation_crypto_version \
             FROM sandbox_runtime_binding \
             WHERE tenant_id = $1 \
               AND sandbox_allocation_ciphertext IS NOT NULL \
               AND NOT (sandbox_allocation_key_id = $2 \
                        AND sandbox_allocation_key_version = $3 \
                        AND sandbox_allocation_crypto_version = $4) \
               AND ($5::TEXT IS NULL OR sandbox_runtime_binding_id > $5) \
             ORDER BY sandbox_runtime_binding_id \
             LIMIT $6",
        )
        .bind(tenant_id.as_str())
        .bind(sandbox_current_protection_version.sandbox_allocation_key_id())
        .bind(
            i64::try_from(sandbox_current_protection_version.sandbox_allocation_key_version())
                .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?,
        )
        .bind(
            i16::try_from(sandbox_current_protection_version.sandbox_allocation_crypto_version())
                .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?,
        )
        .bind(sandbox_after_runtime_binding_id.map(SandboxRuntimeBindingId::as_str))
        .bind(sandbox_query_limit)
        .fetch_all(self.sandbox_postgres_pool()?)
        .await
        .map_err(Self::map_sandbox_sqlx_error)?;

        let sandbox_has_more = sandbox_candidate_rows.len() > usize::from(sandbox_page_size);
        let mut sandbox_candidates = Vec::with_capacity(usize::from(sandbox_page_size));
        for sandbox_candidate_row in sandbox_candidate_rows
            .into_iter()
            .take(usize::from(sandbox_page_size))
        {
            let stored_tenant_id: String = sandbox_candidate_row
                .try_get("tenant_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            if stored_tenant_id != tenant_id.as_str() {
                return Err(SandboxSessionRepositoryError::InvalidStoredData);
            }
            let sandbox_session_id: String = sandbox_candidate_row
                .try_get("sandbox_session_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_runtime_binding_id: String = sandbox_candidate_row
                .try_get("sandbox_runtime_binding_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_allocation_ciphertext: String = sandbox_candidate_row
                .try_get("sandbox_allocation_ciphertext")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_allocation_key_id: String = sandbox_candidate_row
                .try_get("sandbox_allocation_key_id")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_allocation_key_version: i64 = sandbox_candidate_row
                .try_get("sandbox_allocation_key_version")
                .map_err(Self::map_sandbox_sqlx_error)?;
            let sandbox_allocation_crypto_version: i16 = sandbox_candidate_row
                .try_get("sandbox_allocation_crypto_version")
                .map_err(Self::map_sandbox_sqlx_error)?;

            sandbox_candidates.push(SandboxProviderAllocationReencryptionCandidate {
                sandbox_session_id: SandboxSessionId::parse(sandbox_session_id)
                    .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
                sandbox_runtime_binding_id: SandboxRuntimeBindingId::parse(
                    sandbox_runtime_binding_id,
                )
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
                sandbox_protected_allocation_reference: SandboxProtectedProviderAllocationRef::new(
                    sandbox_allocation_ciphertext,
                    sandbox_allocation_key_id,
                    u64::try_from(sandbox_allocation_key_version)
                        .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
                    u16::try_from(sandbox_allocation_crypto_version)
                        .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
                )?,
            });
        }

        let sandbox_next_runtime_binding_id = sandbox_has_more
            .then(|| {
                sandbox_candidates
                    .last()
                    .map(|sandbox_candidate| sandbox_candidate.sandbox_runtime_binding_id.clone())
            })
            .flatten();
        let sandbox_scanned_count = sandbox_candidates.len();
        let mut sandbox_reencrypted_count = 0;
        let mut sandbox_conflict_count = 0;

        for sandbox_candidate in sandbox_candidates {
            let sandbox_protection_context =
                SandboxProviderAllocationProtectionContext::for_repository(
                    tenant_id.clone(),
                    sandbox_candidate.sandbox_session_id.clone(),
                    sandbox_candidate.sandbox_runtime_binding_id.clone(),
                );
            let sandbox_reencrypted_allocation_reference = self
                .sandbox_allocation_protector()
                .reencrypt_sandbox_allocation_reference(
                    &sandbox_protection_context,
                    &sandbox_candidate.sandbox_protected_allocation_reference,
                )?;
            ensure_sandbox_reencryption_target(
                &sandbox_current_protection_version,
                &sandbox_reencrypted_allocation_reference,
            )?;
            let sandbox_update_result = sqlx::query(
                "UPDATE sandbox_runtime_binding SET \
                    sandbox_allocation_ciphertext = $4, \
                    sandbox_allocation_key_id = $5, \
                    sandbox_allocation_key_version = $6, \
                    sandbox_allocation_crypto_version = $7, \
                    updated_at = CURRENT_TIMESTAMP \
                 WHERE tenant_id = $1 \
                   AND sandbox_runtime_binding_id = $2 \
                   AND sandbox_session_id = $3 \
                   AND sandbox_allocation_ciphertext = $8 \
                   AND sandbox_allocation_key_id = $9 \
                   AND sandbox_allocation_key_version = $10 \
                   AND sandbox_allocation_crypto_version = $11",
            )
            .bind(tenant_id.as_str())
            .bind(sandbox_candidate.sandbox_runtime_binding_id.as_str())
            .bind(sandbox_candidate.sandbox_session_id.as_str())
            .bind(sandbox_reencrypted_allocation_reference.sandbox_allocation_ciphertext())
            .bind(sandbox_reencrypted_allocation_reference.sandbox_allocation_key_id())
            .bind(
                i64::try_from(
                    sandbox_reencrypted_allocation_reference.sandbox_allocation_key_version(),
                )
                .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?,
            )
            .bind(
                i16::try_from(
                    sandbox_reencrypted_allocation_reference.sandbox_allocation_crypto_version(),
                )
                .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?,
            )
            .bind(
                sandbox_candidate
                    .sandbox_protected_allocation_reference
                    .sandbox_allocation_ciphertext(),
            )
            .bind(
                sandbox_candidate
                    .sandbox_protected_allocation_reference
                    .sandbox_allocation_key_id(),
            )
            .bind(
                i64::try_from(
                    sandbox_candidate
                        .sandbox_protected_allocation_reference
                        .sandbox_allocation_key_version(),
                )
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
            )
            .bind(
                i16::try_from(
                    sandbox_candidate
                        .sandbox_protected_allocation_reference
                        .sandbox_allocation_crypto_version(),
                )
                .map_err(|_| SandboxSessionRepositoryError::InvalidStoredData)?,
            )
            .execute(self.sandbox_postgres_pool()?)
            .await
            .map_err(Self::map_sandbox_sqlx_error)?;

            if sandbox_update_result.rows_affected() == 1 {
                sandbox_reencrypted_count += 1;
            } else {
                sandbox_conflict_count += 1;
            }
        }

        Ok(SandboxProviderAllocationReencryptionPage::new(
            sandbox_scanned_count,
            sandbox_reencrypted_count,
            sandbox_conflict_count,
            sandbox_next_runtime_binding_id,
        ))
    }
}

#[cfg(test)]
mod tests {
    use sdkwork_intelligence_sandbox_service::{
        SandboxProtectedProviderAllocationRef, SandboxProviderAllocationProtectionVersion,
        SandboxSessionRepositoryError,
    };

    use super::ensure_sandbox_reencryption_target;

    #[test]
    fn sandbox_reencryption_target_rejects_page_version_drift() {
        let sandbox_expected_protection_version =
            SandboxProviderAllocationProtectionVersion::new("sandbox-key", 2, 1)
                .unwrap_or_else(|error| panic!("invalid expected protection version: {error}"));
        let sandbox_matching_allocation_reference =
            SandboxProtectedProviderAllocationRef::new("ciphertext", "sandbox-key", 2, 1)
                .unwrap_or_else(|error| panic!("invalid matching allocation reference: {error}"));
        assert_eq!(
            ensure_sandbox_reencryption_target(
                &sandbox_expected_protection_version,
                &sandbox_matching_allocation_reference,
            ),
            Ok(())
        );

        let sandbox_drifted_allocation_references = [
            SandboxProtectedProviderAllocationRef::new("ciphertext", "other-key", 2, 1),
            SandboxProtectedProviderAllocationRef::new("ciphertext", "sandbox-key", 3, 1),
            SandboxProtectedProviderAllocationRef::new("ciphertext", "sandbox-key", 2, 2),
        ];
        for sandbox_drifted_allocation_reference in sandbox_drifted_allocation_references {
            let sandbox_drifted_allocation_reference = sandbox_drifted_allocation_reference
                .unwrap_or_else(|error| panic!("invalid drifted allocation reference: {error}"));
            assert_eq!(
                ensure_sandbox_reencryption_target(
                    &sandbox_expected_protection_version,
                    &sandbox_drifted_allocation_reference,
                ),
                Err(SandboxSessionRepositoryError::ProtectionFailed)
            );
        }
    }
}
