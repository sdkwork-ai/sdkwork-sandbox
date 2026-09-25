//! Response envelope and problem mapping for the Sandbox internal-api surface.
//!
//! Every success and every failure leaves through this module so the envelope,
//! the trace header and the error taxonomy stay in one place
//! (`API_SPEC.md` sections 14, 16 and 17).

use axum::{
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use sdkwork_utils_rust::{
    PageInfo, PageMode, SdkWorkApiResponse, SdkWorkPageData, SdkWorkResourceData,
};
use sdkwork_web_core::{
    problem_response, WebFrameworkError, WebFrameworkErrorKind, WebRequestContext,
};

use serde::Serialize;

use crate::errors::SandboxApiError;

/// Result alias for handlers.
pub type ApiResult<T> = Result<T, SandboxApiError>;

/// Wraps one resource in the standard envelope payload.
#[must_use]
pub fn item_data<T>(item: T) -> SdkWorkResourceData<T> {
    SdkWorkResourceData { item }
}

/// Wraps one cursor page in the standard envelope payload
/// (`PAGINATION_SPEC.md` sections 2.4 and 3: `mode: "cursor"` with an opaque
/// `nextCursor`; a page that ends the enumeration carries no cursor).
#[must_use]
pub fn cursor_page_data<T>(
    items: Vec<T>,
    next_cursor: Option<String>,
    page_size: u32,
) -> SdkWorkPageData<T> {
    SdkWorkPageData {
        items,
        page_info: PageInfo {
            mode: PageMode::Cursor,
            page: None,
            page_size: Some(i32::try_from(page_size).unwrap_or(i32::MAX)),
            total_items: None,
            total_pages: None,
            has_more: Some(next_cursor.is_some()),
            next_cursor,
        },
    }
}

fn success_response<T: Serialize>(
    ctx: &WebRequestContext,
    data: T,
) -> Result<Response, SandboxApiError> {
    let trace_id = ctx.resolved_trace_id();
    let envelope = SdkWorkApiResponse::success(data, trace_id.clone());
    let mut response = (StatusCode::OK, Json(envelope)).into_response();
    if let Ok(value) = HeaderValue::from_str(&trace_id) {
        response
            .headers_mut()
            .insert(HeaderName::from_static("x-sdkwork-trace-id"), value);
    }
    Ok(response)
}

/// Renders one handler result as an HTTP response.
#[must_use]
pub fn finish_api_json<T: Serialize>(ctx: &WebRequestContext, result: ApiResult<T>) -> Response {
    match result {
        Ok(data) => {
            success_response(ctx, data).unwrap_or_else(|problem| problem.into_response_for(ctx))
        }
        Err(problem) => problem.into_response_for(ctx),
    }
}

/// Extension trait so handlers can render a typed result without importing the
/// framework error types.
pub trait SandboxApiProblemResponse {
    fn into_response_for(self, ctx: &WebRequestContext) -> Response;
}

impl SandboxApiProblemResponse for SandboxApiError {
    fn into_response_for(self, ctx: &WebRequestContext) -> Response {
        let kind = match self.status() {
            StatusCode::BAD_REQUEST => WebFrameworkErrorKind::BadRequest,
            StatusCode::NOT_FOUND => WebFrameworkErrorKind::NotFound,
            StatusCode::CONFLICT => WebFrameworkErrorKind::Conflict,
            StatusCode::FORBIDDEN => WebFrameworkErrorKind::Forbidden,
            StatusCode::SERVICE_UNAVAILABLE => WebFrameworkErrorKind::DependencyUnavailable,
            _ => WebFrameworkErrorKind::InternalServerError,
        };
        let error = WebFrameworkError {
            kind,
            message: self.message().to_owned(),
            retry_after_seconds: None,
            auth_profile: None,
            failed_stage: None,
            reason: None,
        };
        problem_response(&error, ctx.problem_correlation())
    }
}
