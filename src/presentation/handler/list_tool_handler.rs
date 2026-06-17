use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

use crate::presentation::dto::tool::ToolResponse;
use crate::presentation::state::app_state::AppState;

pub async fn list_tool_handler(State(state): State<AppState>) -> impl IntoResponse {
    let tools = state
        .tool_service
        .list_tools()
        .into_iter()
        .map(ToolResponse::from)
        .collect::<Vec<_>>();

    (
        StatusCode::OK,
        Json(json!({
            "tools": tools,
        })),
    )
}
