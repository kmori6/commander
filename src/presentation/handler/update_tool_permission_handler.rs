use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::application::error::tool_usecase_error::ToolUsecaseError;
use crate::domain::error::tool_permission_repository_error::ToolPermissionRepositoryError;
use crate::domain::model::tool_call::{ToolPermission, ToolPermissionMode};
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateToolPermissionRequest {
    pub mode: ToolPermissionMode,
}

fn permission_json(permission: ToolPermission) -> serde_json::Value {
    json!({
        "tool_name": permission.tool_name,
        "mode": permission.mode.as_str(),
    })
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
        Ok(permission) => (StatusCode::OK, Json(permission_json(permission))).into_response(),
        Err(ToolUsecaseError::ToolNotFound(_)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "tool_not_found",
                    "message": format!("tool not found: {tool_name}"),
                }
            })),
        )
            .into_response(),
        Err(ToolUsecaseError::ToolPermissionRepository(
            ToolPermissionRepositoryError::InvalidPermission(message),
        )) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_tool_permission",
                    "message": message,
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_update_tool_permission",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
