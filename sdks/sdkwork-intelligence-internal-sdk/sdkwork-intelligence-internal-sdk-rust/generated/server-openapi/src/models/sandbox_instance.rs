use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SandboxInstance {
    #[serde(rename = "sandboxInstanceId")]
    pub sandbox_instance_id: String,

    #[serde(rename = "sandboxInstanceName")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_instance_name: Option<String>,

    #[serde(rename = "sandboxInstanceOwnerId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_instance_owner_id: Option<String>,

    #[serde(rename = "sandboxInstanceState")]
    pub sandbox_instance_state: String,

    #[serde(rename = "sandboxInstanceCreatedAt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_instance_created_at: Option<String>,

    #[serde(rename = "sandboxInstanceExpiresAt")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_instance_expires_at: Option<String>,

    /// Optimistic-concurrency version; int64-as-string per API_SPEC.md section 13.6
    #[serde(rename = "sandboxVersion")]
    pub sandbox_version: String,
}
