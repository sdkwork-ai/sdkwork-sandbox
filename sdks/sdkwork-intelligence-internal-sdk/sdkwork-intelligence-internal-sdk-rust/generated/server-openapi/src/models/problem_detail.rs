use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ProblemDetail {
    pub r#type: String,

    pub title: String,

    pub status: i64,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,

    /// Request endpoint occurrence: {METHOD} {routeTemplate}
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,

    #[serde(rename = "operationId")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,

    /// Numeric error result code; MUST be non-zero (API_SPEC.md section 15.3)
    pub code: i64,

    #[serde(rename = "traceId")]
    pub trace_id: String,
}
