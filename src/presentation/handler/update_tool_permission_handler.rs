use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::application::error::tool_service_error::ToolServiceError;
use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::tool::ToolPermissionResponse;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateToolPermissionRequest {
    pub mode: ToolPermissionMode,
}

pub async fn update_tool_permission_handler(
    State(state): State<AppState>,
    Path(tool_name): Path<String>,
    Json(request): Json<UpdateToolPermissionRequest>,
) -> Response {
    match state
        .tool_usecase
        .update_permission(&tool_name, request.mode)
        .await
    {
        Ok(permission) => (
            StatusCode::OK,
            Json(ToolPermissionResponse::from(permission)),
        )
            .into_response(),
        Err(ToolServiceError::ToolNotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::new(
                "tool_not_found",
                format!("tool not found: {tool_name}"),
            )),
        )
            .into_response(),
        Err(ToolServiceError::PermissionRepository(
            ToolPermissionRepositoryError::InvalidPermission(message),
        )) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new("invalid_tool_permission", message)),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_update_tool_permission",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
