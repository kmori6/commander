use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

use crate::domain::model::tool::Tool;
use crate::presentation::state::app_state::AppState;

fn tool_json(tool: Tool) -> serde_json::Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "default_permission": tool.default_permission.as_str(),
    })
}

pub async fn list_tool_handler(State(state): State<AppState>) -> impl IntoResponse {
    let tools = state
        .tool_usecase
        .list_tools()
        .into_iter()
        .map(tool_json)
        .collect::<Vec<_>>();

    (
        StatusCode::OK,
        Json(json!({
            "tools": tools,
        })),
    )
}
