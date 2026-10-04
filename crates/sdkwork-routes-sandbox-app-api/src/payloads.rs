//! Wire payloads for the Sandbox app-api surface.
//!
//! Request shapes are separate from the domain commands on purpose: the body
//! never carries `tenantId`, an owner, a version, or a creation timestamp
//! (`API_SPEC.md` section 12), and every enum crosses the boundary as its
//! documented `lower_snake_case` spelling rather than as an implementation
//! detail of the Rust enum. Query-string parameters use `lower_snake_case`
//! wire names (`PAGINATION_SPEC.md` section 0); response bodies keep the
//! standard JSON `camelCase`.

use std::collections::BTreeSet;

use sdkwork_intelligence_sandbox_service::{
    CreateSandboxInstanceCommand, SandboxInstance, SandboxInstanceExpiryUpdate,
    SandboxInstanceProfile, SandboxInstanceState, UpdateSandboxInstanceCommand,
};
use sdkwork_sandbox_provider_spi::{
    IsolationAssurance, RuntimeCapability, SandboxInstanceId, SandboxInstanceOwnerId,
    SandboxWorkspaceId, TenantId,
};
use sdkwork_utils_rust::{validated_offset_list_params, OffsetListPageParams};
use serde::{Deserialize, Serialize};

use crate::errors::SandboxAppApiError;

/// One validated offset window: the platform params the response echoes plus
/// the same values in the service's unsigned vocabulary.
#[derive(Debug, Clone, Copy)]
pub struct ValidatedOffsetPage {
    /// The validated parameters the page header renders.
    pub params: OffsetListPageParams,
    /// 1-based page number, `1..=10000`.
    pub page: u32,
    /// Window size, `1..=200`.
    pub page_size: u32,
}

/// `GET /app/v3/api/sandbox/sandbox_instances` query.
///
/// Offset mode only (`PAGINATION_SPEC.md` section 3): `page` defaults to 1,
/// `page_size` defaults to 20, and out-of-range values are rejected — never
/// clamped. The listing is always narrowed to the verified caller, so no owner
/// parameter exists: a client-supplied owner would be a tenant-wide read
/// primitive this face must never offer.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxInstanceListQuery {
    pub page: Option<i32>,
    pub page_size: Option<i32>,
    /// Narrows the listing to one provisioning state.
    pub sandbox_instance_state: Option<String>,
}

impl SandboxInstanceListQuery {
    /// Validates the pagination parameters against the platform bounds
    /// (`PAGINATION_SPEC.md` section 3: page `1..=10000`, `page_size`
    /// `1..=200`, defaults 1 and 20, rejected — never clamped — outside).
    ///
    /// # Errors
    ///
    /// Returns [`SandboxAppApiError`] when a value is outside its bound.
    pub fn offset_params(&self) -> Result<ValidatedOffsetPage, SandboxAppApiError> {
        let params =
            validated_offset_list_params(self.page.map(i64::from), self.page_size.map(i64::from))
                .map_err(|_| {
                SandboxAppApiError::bad_request(
                    "page must be between 1 and 10000 and page_size must be between 1 and 200",
                )
            })?;
        // The validated bounds (page `1..=10000`, `page_size` `1..=200`) sit
        // far inside `u32`, so both conversions cannot fail for a value that
        // passed them; a failure is a 400, not a silent clamp.
        let page = u32::try_from(params.page)
            .map_err(|_| SandboxAppApiError::bad_request("page is out of range"))?;
        let page_size = u32::try_from(params.page_size)
            .map_err(|_| SandboxAppApiError::bad_request("page_size is out of range"))?;
        Ok(ValidatedOffsetPage {
            params,
            page,
            page_size,
        })
    }

    /// # Errors
    ///
    /// Returns [`SandboxAppApiError`] when the requested state is not in the
    /// provisioning vocabulary.
    pub fn state_filter(&self) -> Result<Option<SandboxInstanceState>, SandboxAppApiError> {
        self.sandbox_instance_state
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| {
                SandboxInstanceState::parse(value).ok_or_else(|| {
                    SandboxAppApiError::bad_request(
                        "sandbox_instance_state must be one of requested, active, suspended, terminated, failed",
                    )
                })
            })
            .transpose()
    }
}

/// `POST /app/v3/api/sandbox/sandbox_instances` body.
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
    /// Returns [`SandboxAppApiError`] when any vocabulary or identifier in the
    /// body is not one this surface accepts.
    pub fn into_command(
        self,
        tenant_id: TenantId,
        sandbox_instance_owner_id: SandboxInstanceOwnerId,
    ) -> Result<CreateSandboxInstanceCommand, SandboxAppApiError> {
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

/// `PATCH /app/v3/api/sandbox/sandbox_instances/{sandboxInstanceId}` body.
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
    /// Returns [`SandboxAppApiError`] when a supplied vocabulary value is not
    /// one this surface accepts.
    pub fn into_command(
        self,
        tenant_id: TenantId,
        sandbox_instance_id: SandboxInstanceId,
    ) -> Result<UpdateSandboxInstanceCommand, SandboxAppApiError> {
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
                    SandboxAppApiError::bad_request(
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

fn parse_profile(value: &str) -> Result<SandboxInstanceProfile, SandboxAppApiError> {
    SandboxInstanceProfile::parse(value).ok_or_else(|| {
        SandboxAppApiError::bad_request(
            "sandboxInstanceProfile must be one of standard, memory_optimized, compute_optimized",
        )
    })
}

fn parse_assurance(value: &str) -> Result<IsolationAssurance, SandboxAppApiError> {
    IsolationAssurance::parse(value).ok_or_else(|| {
        SandboxAppApiError::bad_request(
            "sandboxInstanceMinimumAssurance must be one of host_user, container, \
             user_space_kernel, micro_vm, dedicated_vm",
        )
    })
}

fn parse_capabilities(
    values: &[String],
) -> Result<BTreeSet<RuntimeCapability>, SandboxAppApiError> {
    let mut capabilities = BTreeSet::new();
    for value in values {
        let capability = RuntimeCapability::parse(value.trim()).ok_or_else(|| {
            SandboxAppApiError::bad_request(format!(
                "sandboxInstanceRequiredCapabilities contains an unknown capability: {value}"
            ))
        })?;
        if !capabilities.insert(capability) {
            return Err(SandboxAppApiError::bad_request(format!(
                "sandboxInstanceRequiredCapabilities repeats {value}"
            )));
        }
    }
    Ok(capabilities)
}

fn parse_workspace_id(
    value: Option<String>,
) -> Result<Option<SandboxWorkspaceId>, SandboxAppApiError> {
    value
        .map(|value| {
            SandboxWorkspaceId::parse(value)
                .map_err(|_| SandboxAppApiError::bad_request("sandboxWorkspaceId is invalid"))
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
    fn list_query_defaults_pagination_and_absent_filters() {
        let query = SandboxInstanceListQuery::default();
        let page = query.offset_params().expect("default params");
        assert_eq!(1, page.params.page);
        assert_eq!(20, page.params.page_size);
        assert_eq!((1, 20), (page.page, page.page_size));
        assert!(query.state_filter().expect("absent state filter").is_none());
    }

    #[test]
    fn list_query_rejects_out_of_range_paging_instead_of_clamping() {
        for (page, page_size) in [(0, 20), (10_001, 20), (1, 0), (1, 201)] {
            let query = SandboxInstanceListQuery {
                page: Some(page),
                page_size: Some(page_size),
                ..SandboxInstanceListQuery::default()
            };
            assert_eq!(
                Some(axum::http::StatusCode::BAD_REQUEST),
                query.offset_params().err().map(|error| error.status()),
                "page {page} / page_size {page_size} must reject"
            );
        }
        let boundary = SandboxInstanceListQuery {
            page: Some(10_000),
            page_size: Some(200),
            ..SandboxInstanceListQuery::default()
        };
        let page = boundary.offset_params().expect("boundary");
        assert_eq!(10_000, page.params.page);
        assert_eq!(200, page.params.page_size);
        assert_eq!((10_000, 200), (page.page, page.page_size));
    }

    #[test]
    fn list_query_rejects_forbidden_pagination_and_scope_aliases() {
        // `PAGINATION_SPEC.md` section 10.1: `pageSize`, `limit`, and their
        // aliases must be rejected, never silently ignored or honored. The
        // owner filter is likewise absent by contract — a client-supplied
        // owner would widen the listing beyond the verified caller.
        for alias in [
            "pageSize=5",
            "limit=5",
            "page_no=2",
            "pageNo=2",
            "per_page=5",
            "size=5",
            "offset=20",
            "cursor=abc",
            "sandboxInstanceOwnerId=user-1",
            "sandbox_instance_owner_id=user-1",
        ] {
            let uri = format!("http://sandbox.test/sandbox_instances?{alias}")
                .parse::<axum::http::Uri>()
                .expect("alias uri");
            assert!(
                axum::extract::Query::<SandboxInstanceListQuery>::try_from_uri(&uri).is_err(),
                "the forbidden alias `{alias}` must be rejected"
            );
        }
    }

    #[test]
    fn list_query_accepts_its_declared_parameters() {
        let uri = "http://sandbox.test/sandbox_instances?page=2&page_size=5&sandbox_instance_state=requested"
            .parse::<axum::http::Uri>()
            .expect("declared uri");
        let query = axum::extract::Query::<SandboxInstanceListQuery>::try_from_uri(&uri)
            .expect("declared parameters parse");
        assert_eq!(Some(2), query.page);
        assert_eq!(Some(5), query.page_size);
        assert_eq!(Some("requested"), query.sandbox_instance_state.as_deref());
    }

    #[test]
    fn list_query_rejects_an_unknown_state() {
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

    #[test]
    fn instance_view_serializes_the_version_as_an_int64_string() {
        let parsed_owner = SandboxInstanceOwnerId::parse("user-instance").expect("owner parses");
        let view = SandboxInstanceView {
            sandbox_instance_id: "sandbox-instance-1".to_owned(),
            sandbox_instance_owner_id: parsed_owner.as_str().to_owned(),
            sandbox_instance_name: "primary".to_owned(),
            sandbox_instance_state: "requested",
            sandbox_instance_profile: "standard",
            sandbox_instance_base_image: "sdkwork/sandbox:0.1.0".to_owned(),
            sandbox_instance_vcpu_count: 2,
            sandbox_instance_memory_mb: 4_096,
            sandbox_instance_disk_mb: 20_480,
            sandbox_instance_required_capabilities: Vec::new(),
            sandbox_instance_minimum_assurance: "host_user",
            sandbox_instance_auto_start: false,
            sandbox_instance_expires_at: None,
            sandbox_workspace_id: None,
            sandbox_version: 7,
            created_at: None,
            updated_at: None,
        };
        let encoded = serde_json::to_value(&view).expect("view serializes");
        assert_eq!(
            "7",
            encoded["sandboxVersion"].as_str().expect("string version"),
            "sandboxVersion must cross the boundary as an int64 string"
        );
    }
}
