use serde::{Deserialize, Serialize};

use crate::models::{SandboxInstance};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SdkWorkResourceData {
    pub item: SandboxInstance,
}
