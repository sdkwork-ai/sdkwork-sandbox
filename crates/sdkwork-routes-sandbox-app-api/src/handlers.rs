//! HTTP handlers for the Sandbox app-api surface.
//!
//! A handler resolves identity from the verified request context, converts the
//! wire payload into a domain command, calls the service, and renders the
//! result. It contains no validation rule and no state machine: those live in
//! the service so every caller — HTTP, CLI, or scheduler — gets the same
//! answers.

use axum::{
    extract::{Extension, Path, Query, State},
    response::Response,
    Json,
};
use sdkwork_intelligence_sandbox_service::{SandboxInstance, SandboxInstanceRepository};
use sdkwork_sandbox_provider_spi::{SandboxInstanceId, SandboxInstanceOwnerId, TenantId};
use sdkwork_web_core::WebRequestContext;

use crate::errors::SandboxAppApiError;
use crate::payloads::{
    CreateSandboxInstanceRequest, SandboxInstanceListQuery, SandboxInstanceView,
    UpdateSandboxInstanceRequest,
};
use crate::ports::SandboxAppRequestContext;
use crate::response::{
    finish_api_created, finish_api_json, finish_api_no_content, item_data, offset_page_data,
};
use crate::AppState;

/// Resolves the tenant from the verified principal context, failing closed.
///
/// There is no default tenant: a request without the projected context is
/// rejected, never re-scoped (`API_SPEC.md` section 12, `REQ-2026-0030`).
fn resolve_tenant(context: &SandboxAppRequestContext) -> Result<TenantId, SandboxAppApiError> {
    TenantId::parse(&context.tenant_id).map_err(|_| {
        SandboxAppApiError::bad_request(
            "the request tenant is not a valid SDKWork tenant identifier",
        )
    })
}

/// Resolves the verified caller, which is the instance owner for every
/// operation on this face.
fn resolve_owner(
    context: &SandboxAppRequestContext,
) -> Result<SandboxInstanceOwnerId, SandboxAppApiError> {
    SandboxInstanceOwnerId::parse(&context.actor_id).map_err(|_| {
        SandboxAppApiError::new(
            axum::http::StatusCode::FORBIDDEN,
            "the verified caller principal is not a usable sandbox instance owner",
        )
    })
}

fn resolve_sandbox_instance_id(raw: &str) -> Result<SandboxInstanceId, SandboxAppApiError> {
    SandboxInstanceId::parse(raw).map_err(|_| SandboxAppApiError::invalid_sandbox_instance_id())
}

/// Fails the operation with the absent-row answer when the stored instance is
/// not the caller's.
///
/// Ownership is immutable after creation — no update path writes
/// `sandbox_instance_owner_id` — so a read-then-act guard here cannot be
/// raced into a cross-owner write: once this check passes, the row this
/// caller addresses keeps its owner, and the optimistic-concurrency CAS
/// covers every other field.
fn ensure_owned(
    sandbox_instance: &SandboxInstance,
    owner: &SandboxInstanceOwnerId,
) -> Result<(), SandboxAppApiError> {
    if sandbox_instance.sandbox_instance_owner_id() == owner {
        Ok(())
    } else {
        // Same answer an absent row gets: ownership is never disclosed.
        Err(SandboxAppApiError::invalid_sandbox_instance_id())
    }
}

/// `GET /app/v3/api/sandbox/sandbox_instances`
pub async fn list_sandbox_instances<R>(
    ctx: WebRequestContext,
    State(state): State<AppState<R>>,
    Query(query): Query<SandboxInstanceListQuery>,
    Extension(context): Extension<SandboxAppRequestContext>,
) -> Response
where
    R: SandboxInstanceRepository + Send + Sync,
{
    finish_api_json(
        &ctx,
        async {
            let tenant_id = resolve_tenant(&context)?;
            let owner = resolve_owner(&context)?;
            let state_filter = query.state_filter()?;
            let page = query.offset_params()?;
            // Self-scoped by construction: the listing never leaves the
            // verified caller's own rows (`REQ-2026-0030`).
            let listing = state
                .service
                .list_offset(
                    &tenant_id,
                    Some(&owner),
                    state_filter,
                    page.page,
                    page.page_size,
                )
                .await?;
            let items: Vec<SandboxInstanceView> = listing
                .items
                .iter()
                .map(SandboxInstanceView::from)
                .collect();
            Ok(offset_page_data(items, listing.total_items, page.params))
        }
        .await,
    )
}

/// `POST /app/v3/api/sandbox/sandbox_instances`
pub async fn create_sandbox_instance<R>(
    ctx: WebRequestContext,
    State(state): State<AppState<R>>,
    Extension(context): Extension<SandboxAppRequestContext>,
    Json(body): Json<CreateSandboxInstanceRequest>,
) -> Response
where
    R: SandboxInstanceRepository + Send + Sync,
{
    finish_api_created(
        &ctx,
        async {
            let tenant_id = resolve_tenant(&context)?;
            let owner = resolve_owner(&context)?;
            let command = body.into_command(tenant_id, owner)?;
            let created = state.service.create(command).await?;
            Ok(item_data(SandboxInstanceView::from(&created)))
        }
        .await,
    )
}

/// `GET /app/v3/api/sandbox/sandbox_instances/{sandboxInstanceId}`
pub async fn retrieve_sandbox_instance<R>(
    ctx: WebRequestContext,
    State(state): State<AppState<R>>,
    Path(sandbox_instance_id): Path<String>,
    Extension(context): Extension<SandboxAppRequestContext>,
) -> Response
where
    R: SandboxInstanceRepository + Send + Sync,
{
    finish_api_json(
        &ctx,
        async {
            let tenant_id = resolve_tenant(&context)?;
            let owner = resolve_owner(&context)?;
            let sandbox_instance_id = resolve_sandbox_instance_id(&sandbox_instance_id)?;
            let instance = state
                .service
                .retrieve(&tenant_id, &sandbox_instance_id)
                .await?;
            ensure_owned(&instance, &owner)?;
            Ok(item_data(SandboxInstanceView::from(&instance)))
        }
        .await,
    )
}

/// `PATCH /app/v3/api/sandbox/sandbox_instances/{sandboxInstanceId}`
pub async fn update_sandbox_instance<R>(
    ctx: WebRequestContext,
    State(state): State<AppState<R>>,
    Path(sandbox_instance_id): Path<String>,
    Extension(context): Extension<SandboxAppRequestContext>,
    Json(body): Json<UpdateSandboxInstanceRequest>,
) -> Response
where
    R: SandboxInstanceRepository + Send + Sync,
{
    finish_api_json(
        &ctx,
        async {
            let tenant_id = resolve_tenant(&context)?;
            let owner = resolve_owner(&context)?;
            let sandbox_instance_id = resolve_sandbox_instance_id(&sandbox_instance_id)?;
            let instance = state
                .service
                .retrieve(&tenant_id, &sandbox_instance_id)
                .await?;
            ensure_owned(&instance, &owner)?;
            let command = body.into_command(tenant_id, sandbox_instance_id)?;
            let updated = state.service.update(command).await?;
            Ok(item_data(SandboxInstanceView::from(&updated)))
        }
        .await,
    )
}

/// `DELETE /app/v3/api/sandbox/sandbox_instances/{sandboxInstanceId}`
pub async fn delete_sandbox_instance<R>(
    ctx: WebRequestContext,
    State(state): State<AppState<R>>,
    Path(sandbox_instance_id): Path<String>,
    Extension(context): Extension<SandboxAppRequestContext>,
) -> Response
where
    R: SandboxInstanceRepository + Send + Sync,
{
    finish_api_no_content(
        &ctx,
        async {
            let tenant_id = resolve_tenant(&context)?;
            let owner = resolve_owner(&context)?;
            let sandbox_instance_id = resolve_sandbox_instance_id(&sandbox_instance_id)?;
            let instance = state
                .service
                .retrieve(&tenant_id, &sandbox_instance_id)
                .await?;
            ensure_owned(&instance, &owner)?;
            state
                .service
                .delete(&tenant_id, &sandbox_instance_id)
                .await?;
            Ok(())
        }
        .await,
    )
}
