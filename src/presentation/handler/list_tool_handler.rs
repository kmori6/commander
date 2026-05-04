use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

use crate::presentation::state::app_state::AppState;

pub async fn list_tool_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.tool_usecase.statuses().await {
        Ok(statuses) => {
            let tools = statuses
                .into_iter()
                .map(|status| {
                    json!({
                        "name": status.tool_name,
                        "action": status.action.as_str(),
                        "policy": status.policy.as_str(),
                        "rule": status.rule.map(|rule| rule.as_str()),
                        "source": status.source.as_str(),
                    })
                })
                .collect::<Vec<_>>();

            (
                StatusCode::OK,
                Json(json!({
                    "tools": tools,
                })),
            )
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_tools",
                    "message": err.to_string(),
                }
            })),
        ),
    }
}
