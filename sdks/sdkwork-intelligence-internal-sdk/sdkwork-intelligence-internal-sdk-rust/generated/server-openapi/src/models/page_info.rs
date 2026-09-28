use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct PageInfo {
    pub mode: String,

    /// Effective page size for this response
    #[serde(rename = "pageSize")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<i64>,

    /// Opaque next-page token; a page that ends the enumeration omits it
    #[serde(rename = "nextCursor")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,

    #[serde(rename = "hasMore")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}
