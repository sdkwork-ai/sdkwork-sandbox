//! Wire payloads for the Sandbox internal-api surface.
//!
//! Request shapes are separate from the domain commands on purpose: the body
//! never carries `tenantId`, an owner, a version, or a creation timestamp
//! (`API_SPEC.md` section 12), and every enum crosses the boundary as its
//! documented `lower_snake_case` spelling rather than as an implementation
//! detail of the Rust enum. Query-string parameters use `lower_snake_case`
//! wire names (`PAGINATION_SPEC.md` section 0); response bodies keep the
//! standard JSON `camelCase`.

use std::collections::BTreeSet;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sdkwork_intelligence_sandbox_service::{
    CreateSandboxInstanceCommand, SandboxInstance, SandboxInstanceExpiryUpdate,
    SandboxInstanceListCursor, SandboxInstanceProfile, SandboxInstanceState,
    UpdateSandboxInstanceCommand,
};
use sdkwork_sandbox_provider_spi::{
    IsolationAssurance, RuntimeCapability, SandboxInstanceId, SandboxInstanceOwnerId,
    SandboxWorkspaceId, TenantId,
};
use serde::{Deserialize, Serialize};

use crate::errors::SandboxApiError;

/// `GET /internal/v3/api/intelligence/sandbox/sandbox_instances` query.
///
/// Cursor mode only (`PAGINATION_SPEC.md` section 3): `page_size` defaults to
/// 20 and is rejected — never clamped — outside `1..=200`; `cursor` is the
/// opaque continuation token the previous page returned.
#[derive(Debug, Default, Deserialize)]
pub struct SandboxInstanceListQuery {
    pub page_size: Option<u32>,
    pub cursor: Option<String>,
    /// Narrows the tenant-wide listing to one owner. Absent lists the tenant.
    pub sandbox_instance_owner_id: Option<String>,
    /// Narrows the listing to one state.
    pub sandbox_instance_state: Option<String>,
}

/// The JSON shape encoded inside an opaque listing cursor.
#[derive(Serialize, Deserialize)]
struct SandboxInstanceCursorPayload {
    created_at: String,
    sandbox_instance_id: String,
}

impl SandboxInstanceListQuery {
    /// # Errors
    ///
    /// Returns [`SandboxApiError`] when a filter value is not in its vocabulary.
    pub fn owner_filter(&self) -> Result<Option<SandboxInstanceOwnerId>, SandboxApiError> {
        self.sandbox_instance_owner_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                SandboxInstanceOwnerId::parse(value).map_err(|_| {
                    SandboxApiError::bad_request("sandbox_instance_owner_id is invalid")
                })
            })
            .transpose()
    }

    /// # Errors
    ///
    /// Returns [`SandboxApiError`] when the requested state is not in the
    /// provisioning vocabulary.
    pub fn state_filter(&self) -> Result<Option<SandboxInstanceState>, SandboxApiError> {
        self.sandbox_instance_state
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                SandboxInstanceState::parse(value).ok_or_else(|| {
                    SandboxApiError::bad_request(
                        "sandbox_instance_state must be one of requested, active, suspended, terminated, failed",
                    )
                })
            })
            .transpose()
    }

    /// Validates `page_size` against the standard bound
    /// (`PAGINATION_SPEC.md` section 3: default 20, maximum 200, rejected —
    /// never clamped — outside `1..=200`).
    ///
    /// # Errors
    ///
    /// Returns [`SandboxApiError`] when `page_size` is outside the bound.
    pub fn validated_page_size(&self) -> Result<u32, SandboxApiError> {
        match self.page_size {
            None => Ok(sdkwork_utils_rust::DEFAULT_LIST_PAGE_SIZE as u32),
            Some(value @ 1..=200) => Ok(value),
            Some(_) => Err(SandboxApiError::bad_request(
                "page_size must be between 1 and 200",
            )),
        }
    }

    /// Decodes the opaque cursor into its keyset parts
    /// (`PAGINATION_SPEC.md` section 3: cursors are opaque to clients and
    /// validated by the service before any store access).
    ///
    /// # Errors
    ///
    /// Returns [`SandboxApiError`] when the cursor is not a token this surface
    /// issued (bad base64, bad JSON, or a key outside the stored shape).
    pub fn decoded_cursor(&self) -> Result<Option<SandboxInstanceListCursor>, SandboxApiError> {
        let Some(raw) = self
            .cursor
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(None);
        };
        let decoded = URL_SAFE_NO_PAD
            .decode(raw)
            .map_err(|_| SandboxApiError::invalid_list_cursor())?;
        let payload: SandboxInstanceCursorPayload =
            serde_json::from_slice(&decoded).map_err(|_| SandboxApiError::invalid_list_cursor())?;
        let sandbox_instance_id = SandboxInstanceId::parse(&payload.sandbox_instance_id)
            .map_err(|_| SandboxApiError::invalid_list_cursor())?;
        SandboxInstanceListCursor::new(payload.created_at, sandbox_instance_id)
            .map_err(|_| SandboxApiError::invalid_list_cursor())
            .map(Some)
    }
}

/// Encodes the keyset continuation of one page as the opaque cursor the next
/// request presents.
#[must_use]
pub fn encode_sandbox_instance_cursor(cursor: &SandboxInstanceListCursor) -> String {
    let payload = SandboxInstanceCursorPayload {
        created_at: cursor.created_at().to_owned(),
        sandbox_instance_id: cursor.sandbox_instance_id().as_str().to_owned(),
    };
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap_or_default())
}

/// `POST /internal/v3/api/intelligence/sandbox/sandbox_instances` body.
///
/// The owner is the verified caller identity from the request context and is
/// deliberately absent here: a body field would let one caller provision under
/// another's identity.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSandboxInstanceRequest {
    pub sandbox_instance_name: String,
    pub sandbox_instance_profile: String,
    pub sandbox_instance_base_image: String,
    pub sandbox_instance_vcpu_count: u32,
    pub sandbox_instance_memory_mb: u32,
    pub sandbox_instance_disk_mb: u32,
    #[serde(default)]
    pub sandbox_instance_required_capabilities: Vec<String>,
    pub sandbox_instance_minimum_assurance: String,
    #[serde(default)]
    pub sandbox_instance_auto_start: bool,
    #[serde(default)]
    pub sandbox_instance_expires_at: Option<String>,
    #[serde(default)]
    pub sandbox_workspace_id: Option<String>,
}

impl CreateSandboxInstanceRequest {
    /// # Errors
    ///
    /// Returns [`SandboxApiError`] when any vocabulary or identifier in the body
    /// is not one this surface accepts.
    pub fn into_command(
        self,
        tenant_id: TenantId,
        sandbox_instance_owner_id: SandboxInstanceOwnerId,
    ) -> Result<CreateSandboxInstanceCommand, SandboxApiError> {
        Ok(CreateSandboxInstanceCommand {
            tenant_id,
            sandbox_instance_owner_id,
            sandbox_instance_name: self.sandbox_instance_name,
            sandbox_instance_profile: parse_profile(&self.sandbox_instance_profile)?,
            sandbox_instance_base_image: self.sandbox_instance_base_image,
            sandbox_instance_vcpu_count: self.sandbox_instance_vcpu_count,
            sandbox_instance_memory_mb: self.sandbox_instance_memory_mb,
            sandbox_instance_disk_mb: self.sandbox_instance_disk_mb,
            sandbox_instance_required_capabilities: parse_capabilities(
                &self.sandbox_instance_required_capabilities,
            )?,
            sandbox_instance_minimum_assurance: parse_assurance(
                &self.sandbox_instance_minimum_assurance,
            )?,
            sandbox_instance_auto_start: self.sandbox_instance_auto_start,
            sandbox_instance_expires_at: self.sandbox_instance_expires_at,
            sandbox_workspace_id: parse_workspace_id(self.sandbox_workspace_id)?,
        })
    }
}

/// `PATCH /internal/v3/api/intelligence/sandbox/sandbox_instances/{sandboxInstanceId}` body.
///
/// Every field is optional; absent leaves the stored value alone.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSandboxInstanceRequest {
    #[serde(default)]
    pub sandbox_instance_name: Option<String>,
    #[serde(default)]
    pub sandbox_instance_profile: Option<String>,
    #[serde(default)]
    pub sandbox_instance_vcpu_count: Option<u32>,
    #[serde(default)]
    pub sandbox_instance_memory_mb: Option<u32>,
    #[serde(default)]
    pub sandbox_instance_disk_mb: Option<u32>,
    #[serde(default)]
    pub sandbox_instance_auto_start: Option<bool>,
    /// Three-way on purpose: absent leaves the expiry, `null` clears it, and a
    /// timestamp sets it.
    #[serde(default, deserialize_with = "deserialize_expiry_update")]
    pub sandbox_instance_expires_at: SandboxInstanceExpiryUpdate,
    #[serde(default)]
    pub sandbox_workspace_id: Option<String>,
    #[serde(default)]
    pub sandbox_instance_state: Option<String>,
}

impl UpdateSandboxInstanceRequest {
    /// # Errors
    ///
    /// Returns [`SandboxApiError`] when a supplied vocabulary value is not one
    /// this surface accepts.
    pub fn into_command(
        self,
        tenant_id: TenantId,
        sandbox_instance_id: SandboxInstanceId,
    ) -> Result<UpdateSandboxInstanceCommand, SandboxApiError> {
        let mut command =
            UpdateSandboxInstanceCommand::for_instance(tenant_id, sandbox_instance_id);
        command.sandbox_instance_name = self.sandbox_instance_name;
        command.sandbox_instance_profile = self
            .sandbox_instance_profile
            .as_deref()
            .map(parse_profile)
            .transpose()?;
        command.sandbox_instance_vcpu_count = self.sandbox_instance_vcpu_count;
        command.sandbox_instance_memory_mb = self.sandbox_instance_memory_mb;
        command.sandbox_instance_disk_mb = self.sandbox_instance_disk_mb;
        command.sandbox_instance_auto_start = self.sandbox_instance_auto_start;
        command.sandbox_instance_expires_at = self.sandbox_instance_expires_at;
        command.sandbox_workspace_id = parse_workspace_id(self.sandbox_workspace_id)?;
        command.sandbox_instance_state = self
            .sandbox_instance_state
            .as_deref()
            .map(|value| {
                SandboxInstanceState::parse(value).ok_or_else(|| {
                    SandboxApiError::bad_request(
                        "sandboxInstanceState must be one of requested, active, suspended, terminated, failed",
                    )
                })
            })
            .transpose()?;
        Ok(command)
    }
}

/// One sandbox instance as the caller receives it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxInstanceView {
    pub sandbox_instance_id: String,
    pub sandbox_instance_owner_id: String,
    pub sandbox_instance_name: String,
    pub sandbox_instance_state: &'static str,
    pub sandbox_instance_profile: &'static str,
    pub sandbox_instance_base_image: String,
    pub sandbox_instance_vcpu_count: u32,
    pub sandbox_instance_memory_mb: u32,
    pub sandbox_instance_disk_mb: u32,
    pub sandbox_instance_required_capabilities: Vec<&'static str>,
    pub sandbox_instance_minimum_assurance: &'static str,
    pub sandbox_instance_auto_start: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_instance_expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_workspace_id: Option<String>,
    /// `int64` on the wire (`API_SPEC.md` section 13.6): a browser must not
    /// round the optimistic-concurrency version.
    #[serde(with = "sdkwork_utils_rust::serde_uint64")]
    pub sandbox_version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl From<&SandboxInstance> for SandboxInstanceView {
    fn from(sandbox_instance: &SandboxInstance) -> Self {
        Self {
            sandbox_instance_id: sandbox_instance.sandbox_instance_id().as_str().to_owned(),
            sandbox_instance_owner_id: sandbox_instance
                .sandbox_instance_owner_id()
                .as_str()
                .to_owned(),
            sandbox_instance_name: sandbox_instance.sandbox_instance_name().to_owned(),
            sandbox_instance_state: sandbox_instance.sandbox_instance_state().as_str(),
            sandbox_instance_profile: sandbox_instance.sandbox_instance_profile().as_str(),
            sandbox_instance_base_image: sandbox_instance.sandbox_instance_base_image().to_owned(),
            sandbox_instance_vcpu_count: sandbox_instance.sandbox_instance_vcpu_count(),
            sandbox_instance_memory_mb: sandbox_instance.sandbox_instance_memory_mb(),
            sandbox_instance_disk_mb: sandbox_instance.sandbox_instance_disk_mb(),
            sandbox_instance_required_capabilities: sandbox_instance
                .sandbox_instance_required_capabilities()
                .iter()
                .map(|capability| capability.as_str())
                .collect(),
            sandbox_instance_minimum_assurance: sandbox_instance
                .sandbox_instance_minimum_assurance()
                .as_str(),
            sandbox_instance_auto_start: sandbox_instance.sandbox_instance_auto_start(),
            sandbox_instance_expires_at: sandbox_instance
                .sandbox_instance_expires_at()
                .map(str::to_owned),
            sandbox_workspace_id: sandbox_instance
                .sandbox_workspace_id()
                .map(|workspace_id| workspace_id.as_str().to_owned()),
            sandbox_version: sandbox_instance.sandbox_version(),
            created_at: sandbox_instance.created_at().map(str::to_owned),
            updated_at: sandbox_instance.updated_at().map(str::to_owned),
        }
    }
}

fn parse_profile(value: &str) -> Result<SandboxInstanceProfile, SandboxApiError> {
    SandboxInstanceProfile::parse(value).ok_or_else(|| {
        SandboxApiError::bad_request(
            "sandboxInstanceProfile must be one of standard, memory_optimized, compute_optimized",
        )
    })
}

fn parse_assurance(value: &str) -> Result<IsolationAssurance, SandboxApiError> {
    IsolationAssurance::parse(value).ok_or_else(|| {
        SandboxApiError::bad_request(
            "sandboxInstanceMinimumAssurance must be one of host_user, container, \
             user_space_kernel, micro_vm, dedicated_vm",
        )
    })
}

fn parse_capabilities(values: &[String]) -> Result<BTreeSet<RuntimeCapability>, SandboxApiError> {
    let mut capabilities = BTreeSet::new();
    for value in values {
        let capability = RuntimeCapability::parse(value.trim()).ok_or_else(|| {
            SandboxApiError::bad_request(format!(
                "sandboxInstanceRequiredCapabilities contains an unknown capability: {value}"
            ))
        })?;
        if !capabilities.insert(capability) {
            return Err(SandboxApiError::bad_request(format!(
                "sandboxInstanceRequiredCapabilities repeats {value}"
            )));
        }
    }
    Ok(capabilities)
}

fn parse_workspace_id(
    value: Option<String>,
) -> Result<Option<SandboxWorkspaceId>, SandboxApiError> {
    value
        .map(|value| {
            SandboxWorkspaceId::parse(value)
                .map_err(|_| SandboxApiError::bad_request("sandboxWorkspaceId is invalid"))
        })
        .transpose()
}

fn deserialize_expiry_update<'de, D>(
    deserializer: D,
) -> Result<SandboxInstanceExpiryUpdate, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(match value {
        Some(value) => SandboxInstanceExpiryUpdate::Set(value),
        None => SandboxInstanceExpiryUpdate::Clear,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_query_defaults_page_size_and_absent_filters() {
        let query = SandboxInstanceListQuery::default();
        assert_eq!(20, query.validated_page_size().expect("default page size"));
        assert!(query.owner_filter().expect("absent owner filter").is_none());
        assert!(query.decoded_cursor().expect("absent cursor").is_none());
    }

    #[test]
    fn list_query_rejects_an_out_of_range_page_size_instead_of_clamping() {
        let undersized = SandboxInstanceListQuery {
            page_size: Some(0),
            ..SandboxInstanceListQuery::default()
        };
        assert_eq!(
            Some(axum::http::StatusCode::BAD_REQUEST),
            undersized
                .validated_page_size()
                .err()
                .map(|error| error.status())
        );
        let oversized = SandboxInstanceListQuery {
            page_size: Some(201),
            ..SandboxInstanceListQuery::default()
        };
        assert_eq!(
            Some(axum::http::StatusCode::BAD_REQUEST),
            oversized
                .validated_page_size()
                .err()
                .map(|error| error.status())
        );
        let boundary = SandboxInstanceListQuery {
            page_size: Some(200),
            ..SandboxInstanceListQuery::default()
        };
        assert_eq!(200, boundary.validated_page_size().expect("boundary"));
    }

    #[test]
    fn list_query_cursor_round_trips_and_rejects_forgeries() {
        let id = SandboxInstanceId::parse("sandbox-instance-1")
            .unwrap_or_else(|error| panic!("id: {error}"));
        let cursor = SandboxInstanceListCursor::new("2026-09-24T00:00:00Z".to_owned(), id)
            .unwrap_or_else(|error| panic!("cursor: {error}"));
        let token = encode_sandbox_instance_cursor(&cursor);
        assert!(
            !token.contains('='),
            "cursor must be url-safe without padding"
        );
        let decoded = SandboxInstanceListQuery {
            cursor: Some(token),
            ..SandboxInstanceListQuery::default()
        }
        .decoded_cursor()
        .expect("round trip");
        assert_eq!(Some(cursor), decoded);

        // An absent or blank cursor means "first page", never an error.
        let blank = SandboxInstanceListQuery {
            cursor: Some("   ".to_owned()),
            ..SandboxInstanceListQuery::default()
        };
        assert!(blank.decoded_cursor().expect("blank is absent").is_none());

        for forged in ["not-base64!!", "YWJj"] {
            let query = SandboxInstanceListQuery {
                cursor: Some(forged.to_owned()),
                ..SandboxInstanceListQuery::default()
            };
            assert!(
                query.decoded_cursor().is_err(),
                "forged cursor {forged} must be rejected"
            );
        }
    }

    #[test]
    fn list_query_rejects_an_unknown_state_and_an_invalid_owner() {
        let unknown_state = SandboxInstanceListQuery {
            sandbox_instance_state: Some("paused".to_owned()),
            ..SandboxInstanceListQuery::default()
        };
        assert_eq!(
            Some(axum::http::StatusCode::BAD_REQUEST),
            unknown_state
                .state_filter()
                .err()
                .map(|error| error.status())
        );

        let invalid_owner = SandboxInstanceListQuery {
            sandbox_instance_owner_id: Some("has a space".to_owned()),
            ..SandboxInstanceListQuery::default()
        };
        assert!(invalid_owner.owner_filter().is_err());
    }

    #[test]
    fn update_body_distinguishes_absent_null_and_timestamp_expiry() {
        let absent: UpdateSandboxInstanceRequest =
            serde_json::from_str("{}").expect("absent expiry must parse");
        assert_eq!(
            SandboxInstanceExpiryUpdate::Unchanged,
            absent.sandbox_instance_expires_at
        );

        let cleared: UpdateSandboxInstanceRequest =
            serde_json::from_str(r#"{"sandboxInstanceExpiresAt": null}"#)
                .expect("null expiry must parse");
        assert_eq!(
            SandboxInstanceExpiryUpdate::Clear,
            cleared.sandbox_instance_expires_at
        );

        let set: UpdateSandboxInstanceRequest =
            serde_json::from_str(r#"{"sandboxInstanceExpiresAt": "2030-01-01T00:00:00Z"}"#)
                .expect("timestamp expiry must parse");
        assert_eq!(
            SandboxInstanceExpiryUpdate::Set("2030-01-01T00:00:00Z".to_owned()),
            set.sandbox_instance_expires_at
        );
    }

    #[test]
    fn create_body_rejects_unknown_capability_vocabulary_and_duplicates() {
        let unknown = parse_capabilities(&["terminal".to_owned(), "shell".to_owned()]);
        assert!(unknown.is_err());
        let duplicated = parse_capabilities(&["git".to_owned(), "git".to_owned()]);
        assert!(duplicated.is_err());
        let accepted = parse_capabilities(&["git".to_owned(), "port_forward".to_owned()])
            .expect("known capabilities must parse");
        assert_eq!(2, accepted.len());
    }
}
