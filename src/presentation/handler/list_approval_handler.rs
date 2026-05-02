use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;

use crate::presentation::state::app_state::AppState;

pub async fn list_approval_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.agent_usecase.list_awaiting_approvals().await {
        Ok(approvals) => {
            let approvals = approvals
                .into_iter()
                .map(|approval| {
                    json!({
                        "session_id": approval.session_id.to_string(),
                        "assistant_message_id": approval.assistant_message_id.to_string(),
                        "tool_call_id": approval.tool_call_id,
                    })
                })
                .collect::<Vec<_>>();

            (
                StatusCode::OK,
                Json(json!({
                    "approvals": approvals,
                })),
            )
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_approvals",
                    "message": err.to_string(),
                }
            })),
        ),
    }
}
