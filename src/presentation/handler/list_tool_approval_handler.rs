use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::domain::model::tool_call::{ToolApproval, ToolApprovalStatus};
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListToolApprovalQuery {
    pub status: Option<ToolApprovalStatus>,
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

pub async fn list_tool_approval_handler(
    State(state): State<AppState>,
    Query(query): Query<ListToolApprovalQuery>,
) -> impl IntoResponse {
    match state.tool_approval_usecase.list(query.status).await {
        Ok(approvals) => (
            StatusCode::OK,
            Json(json!({
                "approvals": approvals.into_iter().map(approval_json).collect::<Vec<_>>(),
            })),
        ),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_list_tool_approvals",
                    "message": err.to_string(),
                }
            })),
        ),
    }
}
