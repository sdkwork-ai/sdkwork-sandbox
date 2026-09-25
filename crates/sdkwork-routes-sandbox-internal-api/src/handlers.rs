//! HTTP handlers for the Sandbox internal-api surface.
//!
//! A handler resolves identity from the verified request context, converts the
//! wire payload into a domain command, calls the service, and renders the
//! result. It contains no validation rule and no state machine: those live in
//! the service so every caller — HTTP, CLI, or scheduler — gets the same
//! answers.

use axum::{
    extract::{Extension, Path, Query, State},
    http::HeaderMap,
    response::Response,
    Json,
};
use sdkwork_sandbox_provider_spi::{SandboxInstanceId, SandboxInstanceOwnerId, TenantId};
use sdkwork_web_core::WebRequestContext;

use crate::errors::SandboxApiError;
use crate::payloads::{
    encode_sandbox_instance_cursor, CreateSandboxInstanceRequest, SandboxInstanceListQuery,
    SandboxInstanceView, UpdateSandboxInstanceRequest,
};
use crate::ports::SandboxInternalRequestContext;
use crate::response::{cursor_page_data, finish_api_json, item_data};
use crate::AppState;

/// Resolves the tenant from the verified context, failing closed.
///
/// The composed identity extension wins; the framework context is the fallback.
/// There is no default tenant: a request without a verified tenant is rejected,
/// never re-scoped (`INTERNAL_API_SPEC.md` section 4, `API_SPEC.md` section 12).
fn resolve_tenant(
    ctx: &WebRequestContext,
    context: Option<&Extension<SandboxInternalRequestContext>>,
) -> Result<TenantId, SandboxApiError> {
    let raw = context
        .map(|extension| extension.0.tenant_id.as_str())
        .or_else(|| ctx.tenant_id())
        .ok_or_else(|| {
            SandboxApiError::new(
                axum::http::StatusCode::FORBIDDEN,
                "the request carries no verified tenant context",
            )
        })?;
    TenantId::parse(raw).map_err(|_| {
        SandboxApiError::bad_request("the request tenant is not a valid SDKWork tenant identifier")
    })
}

/// Resolves the acting caller, which is also the instance owner on create.
fn resolve_owner(
    context: Option<&Extension<SandboxInternalRequestContext>>,
) -> Result<SandboxInstanceOwnerId, SandboxApiError> {
    let actor = context
        .and_then(|extension| extension.0.actor_id.as_deref())
        .ok_or_else(|| {
            SandboxApiError::new(
                axum::http::StatusCode::FORBIDDEN,
                "provisioning a sandbox instance requires a verified caller principal",
            )
        })?;
    SandboxInstanceOwnerId::parse(actor).map_err(|_| {
        SandboxApiError::new(
            axum::http::StatusCode::FORBIDDEN,
            "the verified caller principal is not a usable sandbox instance owner",
        )
    })
}

fn resolve_sandbox_instance_id(raw: &str) -> Result<SandboxInstanceId, SandboxApiError> {
    SandboxInstanceId::parse(raw).map_err(|_| SandboxApiError::invalid_sandbox_instance_id())
}

/// `GET /internal/v3/api/intelligence/sandbox/sandbox_instances`
pub async fn list_sandbox_instances<R>(
    ctx: WebRequestContext,
    State(state): State<AppState<R>>,
    _headers: HeaderMap,
    Query(query): Query<SandboxInstanceListQuery>,
    context: Option<Extension<SandboxInternalRequestContext>>,
) -> Response
where
    R: sdkwork_intelligence_sandbox_service::SandboxInstanceRepository + Send + Sync,
{
    finish_api_json(
        &ctx,
        async {
            let tenant_id = resolve_tenant(&ctx, context.as_ref())?;
            let owner = query.owner_filter()?;
            let state_filter = query.state_filter()?;
            let page_size = query.validated_page_size()?;
            let cursor = query.decoded_cursor()?;
            let page = state
                .service
                .list(
                    &tenant_id,
                    owner.as_ref(),
                    state_filter,
                    cursor.as_ref(),
                    page_size,
                )
                .await?;
            let items: Vec<SandboxInstanceView> =
                page.items.iter().map(SandboxInstanceView::from).collect();
            Ok(cursor_page_data(
                items,
                page.next_cursor
                    .as_ref()
                    .map(encode_sandbox_instance_cursor),
                page_size,
            ))
        }
        .await,
    )
}

/// `POST /internal/v3/api/intelligence/sandbox/sandbox_instances`
pub async fn create_sandbox_instance<R>(
    ctx: WebRequestContext,
    State(state): State<AppState<R>>,
    _headers: HeaderMap,
    context: Option<Extension<SandboxInternalRequestContext>>,
    Json(body): Json<CreateSandboxInstanceRequest>,
) -> Response
where
    R: sdkwork_intelligence_sandbox_service::SandboxInstanceRepository + Send + Sync,
{
    finish_api_json(
        &ctx,
        async {
            let tenant_id = resolve_tenant(&ctx, context.as_ref())?;
            let owner = resolve_owner(context.as_ref())?;
            let command = body.into_command(tenant_id, owner)?;
            let created = state.service.create(command).await?;
            Ok(item_data(SandboxInstanceView::from(&created)))
        }
        .await,
    )
}

/// `GET /internal/v3/api/intelligence/sandbox/sandbox_instances/{sandboxInstanceId}`
pub async fn retrieve_sandbox_instance<R>(
    ctx: WebRequestContext,
    State(state): State<AppState<R>>,
    _headers: HeaderMap,
    Path(sandbox_instance_id): Path<String>,
    context: Option<Extension<SandboxInternalRequestContext>>,
) -> Response
where
    R: sdkwork_intelligence_sandbox_service::SandboxInstanceRepository + Send + Sync,
{
    finish_api_json(
        &ctx,
        async {
            let tenant_id = resolve_tenant(&ctx, context.as_ref())?;
            let sandbox_instance_id = resolve_sandbox_instance_id(&sandbox_instance_id)?;
            let instance = state
                .service
                .retrieve(&tenant_id, &sandbox_instance_id)
                .await?;
            Ok(item_data(SandboxInstanceView::from(&instance)))
        }
        .await,
    )
}

/// `PATCH /internal/v3/api/intelligence/sandbox/sandbox_instances/{sandboxInstanceId}`
pub async fn update_sandbox_instance<R>(
    ctx: WebRequestContext,
    State(state): State<AppState<R>>,
    _headers: HeaderMap,
    Path(sandbox_instance_id): Path<String>,
    context: Option<Extension<SandboxInternalRequestContext>>,
    Json(body): Json<UpdateSandboxInstanceRequest>,
) -> Response
where
    R: sdkwork_intelligence_sandbox_service::SandboxInstanceRepository + Send + Sync,
{
    finish_api_json(
        &ctx,
        async {
            let tenant_id = resolve_tenant(&ctx, context.as_ref())?;
            let sandbox_instance_id = resolve_sandbox_instance_id(&sandbox_instance_id)?;
            let command = body.into_command(tenant_id, sandbox_instance_id)?;
            let updated = state.service.update(command).await?;
            Ok(item_data(SandboxInstanceView::from(&updated)))
        }
        .await,
    )
}

/// `DELETE /internal/v3/api/intelligence/sandbox/sandbox_instances/{sandboxInstanceId}`
pub async fn delete_sandbox_instance<R>(
    ctx: WebRequestContext,
    State(state): State<AppState<R>>,
    _headers: HeaderMap,
    Path(sandbox_instance_id): Path<String>,
    context: Option<Extension<SandboxInternalRequestContext>>,
) -> Response
where
    R: sdkwork_intelligence_sandbox_service::SandboxInstanceRepository + Send + Sync,
{
    finish_api_json(
        &ctx,
        async {
            let tenant_id = resolve_tenant(&ctx, context.as_ref())?;
            let sandbox_instance_id = resolve_sandbox_instance_id(&sandbox_instance_id)?;
            state
                .service
                .delete(&tenant_id, &sandbox_instance_id)
                .await?;
            Ok(item_data(serde_json::json!({
                "sandboxInstanceId": sandbox_instance_id.as_str(),
                "deleted": true,
            })))
        }
        .await,
    )
}
