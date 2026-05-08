use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use uuid::Uuid;

use crate::application::error::tool_approval_usecase_error::ToolApprovalUsecaseError;
use crate::domain::error::tool_approval_repository_error::ToolApprovalRepositoryError;
use crate::domain::model::tool::ToolApproval;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Clone, Copy)]
pub enum ToolApprovalResolution {
    Approve,
    Reject,
}

fn approval_json(approval: ToolApproval) -> serde_json::Value {
    json!({
        "id": approval.id.to_string(),
        "message_id": approval.message_id.to_string(),
        "call_id": approval.call_id,
        "status": approval.status.as_str(),
        "requested_at": approval.requested_at.to_rfc3339(),
        "resolved_at": approval.resolved_at.map(|dt| dt.to_rfc3339()),
    })
}

pub async fn approve_tool_approval_handler(
    State(state): State<AppState>,
    Path(approval_id): Path<Uuid>,
) -> Response {
    resolve(state, approval_id, ToolApprovalResolution::Approve).await
}

pub async fn reject_tool_approval_handler(
    State(state): State<AppState>,
    Path(approval_id): Path<Uuid>,
) -> Response {
    resolve(state, approval_id, ToolApprovalResolution::Reject).await
}

async fn resolve(
    state: AppState,
    approval_id: Uuid,
    resolution: ToolApprovalResolution,
) -> Response {
    let result = match resolution {
        ToolApprovalResolution::Approve => state.tool_approval_usecase.approve(approval_id).await,
        ToolApprovalResolution::Reject => state.tool_approval_usecase.reject(approval_id).await,
    };

    match result {
        Ok(approval) => {
            let runtime = state.agent_runtime.clone();
            let approval_id = approval.id;

            tokio::spawn(async move {
                if let Err(err) = runtime.resume(approval_id).await {
                    log::error!("failed to resume task from approval {approval_id}: {err}");
                }
            });

            (StatusCode::OK, Json(approval_json(approval))).into_response()
        }
        Err(ToolApprovalUsecaseError::ToolApprovalRepository(
            ToolApprovalRepositoryError::NotFound(_),
        )) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "tool_approval_not_found",
                    "message": format!("tool approval not found: {approval_id}"),
                }
            })),
        )
            .into_response(),
        Err(ToolApprovalUsecaseError::ToolApprovalRepository(
            ToolApprovalRepositoryError::InvalidApproval(message),
        )) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_tool_approval",
                    "message": message,
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_resolve_tool_approval",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
