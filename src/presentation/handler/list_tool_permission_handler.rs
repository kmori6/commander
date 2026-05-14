use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

use crate::domain::model::tool_call::ToolPermission;
use crate::presentation::state::app_state::AppState;

fn permission_json(permission: ToolPermission) -> serde_json::Value {
    json!({
        "tool_name": permission.tool_name,
        "mode": permission.mode.as_str(),
    })
}

pub async fn list_tool_permission_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.tool_usecase.list_permissions().await {
        Ok(permissions) => (
            StatusCode::OK,
            Json(json!({
                "permissions": permissions.into_iter().map(permission_json).collect::<Vec<_>>(),
            })),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_tool_permissions",
                    "message": err.to_string(),
                }
            })),
        ),
    }
}
