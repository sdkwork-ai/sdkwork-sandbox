use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UpdateSandboxInstanceRequest {
    #[serde(rename = "sandboxInstanceName")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_instance_name: Option<String>,
}
