use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::domain::model::tool_call::ToolApprovalStatus;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::tool::ToolApprovalResponse;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct ListToolApprovalQuery {
    pub status: Option<ToolApprovalStatus>,
}

pub async fn list_tool_approval_handler(
    State(state): State<AppState>,
    Query(query): Query<ListToolApprovalQuery>,
) -> Response {
    match state.tool_approval_usecase.list(query.status).await {
        Ok(approvals) => (
            StatusCode::OK,
            Json(json!({
                "approvals": approvals
                    .into_iter()
                    .map(ToolApprovalResponse::from)
                    .collect::<Vec<_>>(),
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "failed_to_list_tool_approvals",
                err.to_string(),
            )),
        )
            .into_response(),
    }
}
