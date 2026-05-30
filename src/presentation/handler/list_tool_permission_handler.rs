use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::tool::ToolPermissionResponse;
use crate::presentation::state::app_state::AppState;

pub async fn list_tool_permission_handler(State(state): State<AppState>) -> Response {
    match state.tool_usecase.list_permissions().await {
        Ok(permissions) => (
            StatusCode::OK,
            Json(json!({
                "permissions": permissions
                    .into_iter()
                    .map(ToolPermissionResponse::from)
                    .collect::<Vec<_>>(),
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_list_tool_permissions",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
