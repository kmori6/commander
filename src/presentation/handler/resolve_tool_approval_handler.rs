use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

use crate::application::error::tool_approval_usecase_error::ToolApprovalUsecaseError;
use crate::domain::error::tool_approval_repository_error::ToolApprovalRepositoryError;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::tool::ToolApprovalResponse;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Clone, Copy)]
pub enum ToolApprovalResolution {
    Approve,
    Reject,
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
            (StatusCode::OK, Json(ToolApprovalResponse::from(approval))).into_response()
        }
        Err(ToolApprovalUsecaseError::ToolApprovalRepository(
            ToolApprovalRepositoryError::NotFound(_),
        )) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "tool_approval_not_found",
                format!("tool approval not found: {approval_id}"),
            )),
        )
            .into_response(),
        Err(ToolApprovalUsecaseError::ToolApprovalRepository(
            ToolApprovalRepositoryError::InvalidApproval(message),
        )) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("invalid_tool_approval", message)),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_resolve_tool_approval",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
