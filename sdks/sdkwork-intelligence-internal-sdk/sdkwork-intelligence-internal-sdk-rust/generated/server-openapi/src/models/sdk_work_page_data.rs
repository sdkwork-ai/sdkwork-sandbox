use serde::{Deserialize, Serialize};

use crate::models::{PageInfo, SandboxInstance};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SdkWorkPageData {
    pub items: Vec<SandboxInstance>,

    #[serde(rename = "pageInfo")]
    pub page_info: PageInfo,
}
