use std::fmt;
use std::sync::Arc;

use sdkwork_intelligence_sandbox_service::{
    SandboxProtectedProviderAllocationRef, SandboxProviderAllocationProtectionContext,
    SandboxProviderAllocationProtectionVersion, SandboxProviderAllocationProtector,
    SandboxSessionRepositoryError, SandboxSessionRepositoryResult,
};
use sdkwork_sandbox_provider_spi::SandboxProviderAllocationRef;
use sdkwork_utils_rust::crypto::{aes_gcm_decrypt, aes_gcm_encrypt, derive_aes_256_key};
use zeroize::{Zeroize, Zeroizing};

const SANDBOX_ALLOCATION_CRYPTO_VERSION: u16 = 1;
const SANDBOX_ALLOCATION_KEY_INFO: &[u8] = b"sdkwork-sandbox-provider-allocation:aes-256-gcm:v1";

pub struct SandboxProviderAllocationKey {
    sandbox_allocation_key_id: String,
    sandbox_allocation_key_version: u64,
    sandbox_allocation_key_material: Zeroizing<Vec<u8>>,
}

impl SandboxProviderAllocationKey {
    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::ProtectionFailed` when the
    /// injected key material or key identity fails validation.
    pub fn new(
        sandbox_allocation_key_id: impl Into<String>,
        sandbox_allocation_key_version: u64,
        sandbox_allocation_key_material: Vec<u8>,
    ) -> SandboxSessionRepositoryResult<Self> {
        let sandbox_allocation_key_id = sandbox_allocation_key_id.into();
        let sandbox_allocation_key_material = Zeroizing::new(sandbox_allocation_key_material);
        if sandbox_allocation_key_id.is_empty()
            || sandbox_allocation_key_id.len() > 128
            || !sandbox_allocation_key_id
                .bytes()
                .all(|sandbox_key_id_byte| sandbox_key_id_byte.is_ascii_graphic())
            || !(1..=i64::MAX as u64).contains(&sandbox_allocation_key_version)
            || !(32..=1_024).contains(&sandbox_allocation_key_material.len())
        {
            return Err(SandboxSessionRepositoryError::ProtectionFailed);
        }
        Ok(Self {
            sandbox_allocation_key_id,
            sandbox_allocation_key_version,
            sandbox_allocation_key_material,
        })
    }
}

impl fmt::Debug for SandboxProviderAllocationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SandboxProviderAllocationKey")
            .field("sandbox_allocation_key_id", &self.sandbox_allocation_key_id)
            .field(
                "sandbox_allocation_key_version",
                &self.sandbox_allocation_key_version,
            )
            .field("sandbox_allocation_key_material", &"[REDACTED]")
            .finish()
    }
}

pub trait SandboxProviderAllocationKeySource: Send + Sync {
    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::ProtectionFailed` when the
    /// active allocation key cannot be resolved from the injected key source.
    fn current_sandbox_allocation_key(
        &self,
    ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationKey>;

    ///
    /// # Errors
    ///
    /// Returns `SandboxSessionRepositoryError::ProtectionFailed` when the
    /// requested key version cannot be resolved from the injected key source.
    fn sandbox_allocation_key(
        &self,
        sandbox_allocation_key_id: &str,
        sandbox_allocation_key_version: u64,
    ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationKey>;
}

pub struct SdkworkUtilsSandboxProviderAllocationProtector {
    sandbox_allocation_key_source: Arc<dyn SandboxProviderAllocationKeySource>,
}

impl SdkworkUtilsSandboxProviderAllocationProtector {
    pub fn new(sandbox_allocation_key_source: Arc<dyn SandboxProviderAllocationKeySource>) -> Self {
        Self {
            sandbox_allocation_key_source,
        }
    }

    fn sandbox_protection_salt(
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
    ) -> Vec<u8> {
        let sandbox_context_parts = [
            sandbox_protection_context.tenant_id().as_str(),
            sandbox_protection_context.sandbox_session_id().as_str(),
            sandbox_protection_context
                .sandbox_runtime_binding_id()
                .as_str(),
        ];
        let mut sandbox_protection_salt = b"sdkwork-sandbox-allocation-context-v1".to_vec();
        for sandbox_context_part in sandbox_context_parts {
            sandbox_protection_salt
                .extend_from_slice(&(sandbox_context_part.len() as u64).to_be_bytes());
            sandbox_protection_salt.extend_from_slice(sandbox_context_part.as_bytes());
        }
        sandbox_protection_salt
    }

    fn derive_sandbox_allocation_key(
        sandbox_allocation_key: &SandboxProviderAllocationKey,
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
    ) -> Zeroizing<[u8; 32]> {
        Zeroizing::new(derive_aes_256_key(
            &sandbox_allocation_key.sandbox_allocation_key_material,
            &Self::sandbox_protection_salt(sandbox_protection_context),
            SANDBOX_ALLOCATION_KEY_INFO,
        ))
    }
}

impl SandboxProviderAllocationProtector for SdkworkUtilsSandboxProviderAllocationProtector {
    fn current_sandbox_allocation_protection_version(
        &self,
    ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationProtectionVersion> {
        let sandbox_allocation_key = self
            .sandbox_allocation_key_source
            .current_sandbox_allocation_key()?;
        SandboxProviderAllocationProtectionVersion::new(
            sandbox_allocation_key.sandbox_allocation_key_id.clone(),
            sandbox_allocation_key.sandbox_allocation_key_version,
            SANDBOX_ALLOCATION_CRYPTO_VERSION,
        )
    }

    fn protect_sandbox_allocation_reference(
        &self,
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
        sandbox_allocation_reference: &SandboxProviderAllocationRef,
    ) -> SandboxSessionRepositoryResult<SandboxProtectedProviderAllocationRef> {
        let sandbox_allocation_key = self
            .sandbox_allocation_key_source
            .current_sandbox_allocation_key()?;
        let sandbox_derived_key = Self::derive_sandbox_allocation_key(
            &sandbox_allocation_key,
            sandbox_protection_context,
        );
        let sandbox_encryption_result = aes_gcm_encrypt(
            sandbox_derived_key.as_ref(),
            sandbox_allocation_reference.expose_to_provider().as_bytes(),
        );
        let sandbox_allocation_ciphertext = sandbox_encryption_result
            .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?;
        SandboxProtectedProviderAllocationRef::new(
            sandbox_allocation_ciphertext,
            sandbox_allocation_key.sandbox_allocation_key_id.clone(),
            sandbox_allocation_key.sandbox_allocation_key_version,
            SANDBOX_ALLOCATION_CRYPTO_VERSION,
        )
    }

    fn restore_sandbox_allocation_reference(
        &self,
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
        sandbox_protected_allocation_reference: &SandboxProtectedProviderAllocationRef,
    ) -> SandboxSessionRepositoryResult<SandboxProviderAllocationRef> {
        if sandbox_protected_allocation_reference.sandbox_allocation_crypto_version()
            != SANDBOX_ALLOCATION_CRYPTO_VERSION
        {
            return Err(SandboxSessionRepositoryError::ProtectionFailed);
        }
        let sandbox_allocation_key = self.sandbox_allocation_key_source.sandbox_allocation_key(
            sandbox_protected_allocation_reference.sandbox_allocation_key_id(),
            sandbox_protected_allocation_reference.sandbox_allocation_key_version(),
        )?;
        if sandbox_allocation_key.sandbox_allocation_key_id
            != sandbox_protected_allocation_reference.sandbox_allocation_key_id()
            || sandbox_allocation_key.sandbox_allocation_key_version
                != sandbox_protected_allocation_reference.sandbox_allocation_key_version()
        {
            return Err(SandboxSessionRepositoryError::ProtectionFailed);
        }
        let sandbox_derived_key = Self::derive_sandbox_allocation_key(
            &sandbox_allocation_key,
            sandbox_protection_context,
        );
        let sandbox_decryption_result = aes_gcm_decrypt(
            sandbox_derived_key.as_ref(),
            sandbox_protected_allocation_reference.sandbox_allocation_ciphertext(),
        );
        let sandbox_allocation_plaintext = sandbox_decryption_result
            .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?;
        let sandbox_allocation_value = match String::from_utf8(sandbox_allocation_plaintext) {
            Ok(sandbox_allocation_value) => sandbox_allocation_value,
            Err(error) => {
                let mut sandbox_invalid_plaintext = error.into_bytes();
                sandbox_invalid_plaintext.zeroize();
                return Err(SandboxSessionRepositoryError::ProtectionFailed);
            }
        };
        SandboxProviderAllocationRef::new(sandbox_allocation_value)
            .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)
    }

    fn reencrypt_sandbox_allocation_reference(
        &self,
        sandbox_protection_context: &SandboxProviderAllocationProtectionContext,
        sandbox_protected_allocation_reference: &SandboxProtectedProviderAllocationRef,
    ) -> SandboxSessionRepositoryResult<SandboxProtectedProviderAllocationRef> {
        let sandbox_allocation_reference = self.restore_sandbox_allocation_reference(
            sandbox_protection_context,
            sandbox_protected_allocation_reference,
        )?;
        self.protect_sandbox_allocation_reference(
            sandbox_protection_context,
            &sandbox_allocation_reference,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, RwLock};

    use sdkwork_intelligence_sandbox_service::{
        SandboxProviderAllocationProtectionContext, SandboxProviderAllocationProtector,
        SandboxSessionRepositoryError,
    };
    use sdkwork_sandbox_provider_spi::{
        SandboxProviderAllocationRef, SandboxRuntimeBindingId, SandboxSessionId, TenantId,
    };

    use super::{
        SandboxProviderAllocationKey, SandboxProviderAllocationKeySource,
        SdkworkUtilsSandboxProviderAllocationProtector,
    };

    const SANDBOX_KEY_ID: &str = "test-key";
    const SANDBOX_PRIVATE_REFERENCE: &str = "private-provider-handle";

    struct TestSandboxAllocationKeySource {
        sandbox_current_key_version: RwLock<u64>,
        sandbox_key_material_by_version: RwLock<BTreeMap<u64, Vec<u8>>>,
    }

    impl TestSandboxAllocationKeySource {
        fn with_v1() -> Self {
            Self {
                sandbox_current_key_version: RwLock::new(1),
                sandbox_key_material_by_version: RwLock::new(BTreeMap::from([(1, vec![7; 32])])),
            }
        }

        fn rotate_to_v2(&self) {
            self.sandbox_key_material_by_version
                .write()
                .unwrap_or_else(|error| panic!("sandbox key material lock poisoned: {error}"))
                .insert(2, vec![11; 32]);
            *self
                .sandbox_current_key_version
                .write()
                .unwrap_or_else(|error| panic!("sandbox current key lock poisoned: {error}")) = 2;
        }

        fn retire_v1(&self) {
            self.sandbox_key_material_by_version
                .write()
                .unwrap_or_else(|error| panic!("sandbox key material lock poisoned: {error}"))
                .remove(&1);
        }

        fn sandbox_key(
            &self,
            sandbox_allocation_key_version: u64,
        ) -> Result<SandboxProviderAllocationKey, SandboxSessionRepositoryError> {
            let sandbox_key_material = self
                .sandbox_key_material_by_version
                .read()
                .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?
                .get(&sandbox_allocation_key_version)
                .cloned()
                .ok_or(SandboxSessionRepositoryError::ProtectionFailed)?;
            SandboxProviderAllocationKey::new(
                SANDBOX_KEY_ID,
                sandbox_allocation_key_version,
                sandbox_key_material,
            )
        }
    }

    impl SandboxProviderAllocationKeySource for TestSandboxAllocationKeySource {
        fn current_sandbox_allocation_key(
            &self,
        ) -> Result<SandboxProviderAllocationKey, SandboxSessionRepositoryError> {
            let sandbox_current_key_version = *self
                .sandbox_current_key_version
                .read()
                .map_err(|_| SandboxSessionRepositoryError::ProtectionFailed)?;
            self.sandbox_key(sandbox_current_key_version)
        }

        fn sandbox_allocation_key(
            &self,
            sandbox_allocation_key_id: &str,
            sandbox_allocation_key_version: u64,
        ) -> Result<SandboxProviderAllocationKey, SandboxSessionRepositoryError> {
            if sandbox_allocation_key_id != SANDBOX_KEY_ID {
                return Err(SandboxSessionRepositoryError::ProtectionFailed);
            }
            self.sandbox_key(sandbox_allocation_key_version)
        }
    }

    struct WrongIdentitySandboxAllocationKeySource;

    impl SandboxProviderAllocationKeySource for WrongIdentitySandboxAllocationKeySource {
        fn current_sandbox_allocation_key(
            &self,
        ) -> Result<SandboxProviderAllocationKey, SandboxSessionRepositoryError> {
            SandboxProviderAllocationKey::new(SANDBOX_KEY_ID, 1, vec![7; 32])
        }

        fn sandbox_allocation_key(
            &self,
            _sandbox_allocation_key_id: &str,
            sandbox_allocation_key_version: u64,
        ) -> Result<SandboxProviderAllocationKey, SandboxSessionRepositoryError> {
            SandboxProviderAllocationKey::new(
                "wrong-key-id",
                sandbox_allocation_key_version,
                vec![7; 32],
            )
        }
    }

    fn protection_context(tenant: &str) -> SandboxProviderAllocationProtectionContext {
        SandboxProviderAllocationProtectionContext::for_repository(
            TenantId::parse(tenant).unwrap_or_else(|error| panic!("tenant: {error}")),
            SandboxSessionId::parse("session-a").unwrap_or_else(|error| panic!("session: {error}")),
            SandboxRuntimeBindingId::parse("binding-a")
                .unwrap_or_else(|error| panic!("binding: {error}")),
        )
    }

    #[test]
    fn sandbox_allocation_reference_is_encrypted_redacted_and_context_bound() {
        let sandbox_key_source = Arc::new(TestSandboxAllocationKeySource::with_v1());
        let sandbox_protector = SdkworkUtilsSandboxProviderAllocationProtector::new(Arc::<
            TestSandboxAllocationKeySource,
        >::clone(
            &sandbox_key_source,
        ));
        let sandbox_context = protection_context("tenant-a");
        let sandbox_reference = SandboxProviderAllocationRef::new(SANDBOX_PRIVATE_REFERENCE)
            .unwrap_or_else(|error| panic!("reference: {error}"));

        let sandbox_protected_v1 = sandbox_protector
            .protect_sandbox_allocation_reference(&sandbox_context, &sandbox_reference)
            .unwrap_or_else(|error| panic!("protect: {error}"));
        assert_eq!(sandbox_protected_v1.sandbox_allocation_key_version(), 1);
        assert!(!sandbox_protected_v1
            .sandbox_allocation_ciphertext()
            .contains(SANDBOX_PRIVATE_REFERENCE));
        let sandbox_protected_v1_debug = format!("{sandbox_protected_v1:?}");
        assert!(!sandbox_protected_v1_debug.contains(SANDBOX_PRIVATE_REFERENCE));
        assert!(!sandbox_protected_v1_debug
            .contains(sandbox_protected_v1.sandbox_allocation_ciphertext()));

        let sandbox_restored_v1 = sandbox_protector
            .restore_sandbox_allocation_reference(&sandbox_context, &sandbox_protected_v1)
            .unwrap_or_else(|error| panic!("restore: {error}"));
        assert_eq!(
            sandbox_restored_v1.expose_to_provider(),
            SANDBOX_PRIVATE_REFERENCE
        );
        assert!(sandbox_protector
            .restore_sandbox_allocation_reference(
                &protection_context("tenant-b"),
                &sandbox_protected_v1,
            )
            .is_err());

        sandbox_key_source.rotate_to_v2();
        let sandbox_current_version = sandbox_protector
            .current_sandbox_allocation_protection_version()
            .unwrap_or_else(|error| panic!("current protection version: {error}"));
        assert_eq!(sandbox_current_version.sandbox_allocation_key_version(), 2);
        assert!(!sandbox_current_version
            .matches_sandbox_protected_allocation_reference(&sandbox_protected_v1));
        assert_eq!(
            sandbox_protector
                .restore_sandbox_allocation_reference(&sandbox_context, &sandbox_protected_v1)
                .unwrap_or_else(|error| panic!("historical restore: {error}"))
                .expose_to_provider(),
            SANDBOX_PRIVATE_REFERENCE
        );

        let sandbox_protected_v2 = sandbox_protector
            .reencrypt_sandbox_allocation_reference(&sandbox_context, &sandbox_protected_v1)
            .unwrap_or_else(|error| panic!("re-encrypt: {error}"));
        assert_eq!(sandbox_protected_v2.sandbox_allocation_key_version(), 2);
        assert!(sandbox_current_version
            .matches_sandbox_protected_allocation_reference(&sandbox_protected_v2));
        assert_eq!(
            sandbox_protector
                .restore_sandbox_allocation_reference(&sandbox_context, &sandbox_protected_v2)
                .unwrap_or_else(|error| panic!("current restore: {error}"))
                .expose_to_provider(),
            SANDBOX_PRIVATE_REFERENCE
        );

        sandbox_key_source.retire_v1();
        assert_eq!(
            sandbox_protector
                .restore_sandbox_allocation_reference(&sandbox_context, &sandbox_protected_v1,),
            Err(SandboxSessionRepositoryError::ProtectionFailed)
        );
        assert_eq!(
            sandbox_protector
                .restore_sandbox_allocation_reference(&sandbox_context, &sandbox_protected_v2)
                .unwrap_or_else(|error| panic!("post-retirement restore: {error}"))
                .expose_to_provider(),
            SANDBOX_PRIVATE_REFERENCE
        );
    }

    #[test]
    fn sandbox_allocation_restore_rejects_wrong_key_identity() {
        let sandbox_context = protection_context("tenant-a");
        let sandbox_reference = SandboxProviderAllocationRef::new(SANDBOX_PRIVATE_REFERENCE)
            .unwrap_or_else(|error| panic!("reference: {error}"));
        let sandbox_source_protector = SdkworkUtilsSandboxProviderAllocationProtector::new(
            Arc::new(TestSandboxAllocationKeySource::with_v1()),
        );
        let sandbox_protected = sandbox_source_protector
            .protect_sandbox_allocation_reference(&sandbox_context, &sandbox_reference)
            .unwrap_or_else(|error| panic!("protect: {error}"));
        let sandbox_wrong_identity_protector = SdkworkUtilsSandboxProviderAllocationProtector::new(
            Arc::new(WrongIdentitySandboxAllocationKeySource),
        );

        assert_eq!(
            sandbox_wrong_identity_protector
                .restore_sandbox_allocation_reference(&sandbox_context, &sandbox_protected),
            Err(SandboxSessionRepositoryError::ProtectionFailed)
        );
    }

    #[test]
    fn sandbox_allocation_key_debug_output_redacts_key_material() {
        let sandbox_key_material_marker = "key-material-must-not-appear";
        let sandbox_allocation_key = SandboxProviderAllocationKey::new(
            SANDBOX_KEY_ID,
            1,
            sandbox_key_material_marker.as_bytes().repeat(2),
        )
        .unwrap_or_else(|error| panic!("key: {error}"));

        let sandbox_debug_output = format!("{sandbox_allocation_key:?}");
        assert!(!sandbox_debug_output.contains(sandbox_key_material_marker));
        assert!(sandbox_debug_output.contains("[REDACTED]"));
    }

    #[test]
    fn sandbox_allocation_key_rejects_unsafe_key_identity_and_invalid_material_bounds() {
        for sandbox_invalid_key_id in ["", "key id", "key\nid", "密钥"] {
            assert!(matches!(
                SandboxProviderAllocationKey::new(sandbox_invalid_key_id, 1, vec![7; 32]),
                Err(SandboxSessionRepositoryError::ProtectionFailed)
            ));
        }
        assert!(SandboxProviderAllocationKey::new("kms/key:v2", 1, vec![7; 32]).is_ok());
        assert!(matches!(
            SandboxProviderAllocationKey::new("kms/key:v2", 1, vec![7; 31]),
            Err(SandboxSessionRepositoryError::ProtectionFailed)
        ));
        assert!(matches!(
            SandboxProviderAllocationKey::new("kms/key:v2", 1, vec![7; 1_025]),
            Err(SandboxSessionRepositoryError::ProtectionFailed)
        ));
    }
}
