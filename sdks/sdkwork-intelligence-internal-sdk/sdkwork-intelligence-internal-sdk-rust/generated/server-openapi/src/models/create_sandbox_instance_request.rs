use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CreateSandboxInstanceRequest {
    #[serde(rename = "sandboxInstanceName")]
    pub sandbox_instance_name: String,
}
