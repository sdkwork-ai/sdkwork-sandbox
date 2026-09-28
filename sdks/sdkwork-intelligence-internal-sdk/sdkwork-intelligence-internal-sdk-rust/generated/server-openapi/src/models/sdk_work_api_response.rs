use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SdkWorkApiResponse {
    /// Numeric success result code; MUST be 0 on HTTP 2xx (API_SPEC.md section 15.3)
    pub code: i64,

    /// Operation-specific payload; typed per operation through allOf
    pub data: serde_json::Value,

    /// Server-owned request correlation id; clients must not supply this value
    #[serde(rename = "traceId")]
    pub trace_id: String,
}
