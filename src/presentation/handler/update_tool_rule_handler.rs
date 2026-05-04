use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::application::error::tool_usecase_error::ToolUsecaseError;
use crate::domain::model::tool_execution_rule::ToolExecutionRuleAction;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateToolRuleRequest {
    pub action: String,
}

pub async fn update_tool_rule_handler(
    State(state): State<AppState>,
    Path(tool_name): Path<String>,
    Json(request): Json<UpdateToolRuleRequest>,
) -> Response {
    let Ok(action) = request.action.parse::<ToolExecutionRuleAction>() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "code": "invalid_tool_rule_action",
                    "message": "action must be one of: allow, ask, deny",
                }
            })),
        )
            .into_response();
    };

    match state.tool_usecase.set_rule(tool_name, action).await {
        Ok(status) => (
            StatusCode::OK,
            Json(json!({
                "tool": {
                    "name": status.tool_name,
                    "action": status.action.as_str(),
                    "policy": status.policy.as_str(),
                    "rule": status.rule.map(|rule| rule.as_str()),
                    "source": status.source.as_str(),
                }
            })),
        )
            .into_response(),
        Err(ToolUsecaseError::ToolNotFound(tool_name)) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": {
                    "code": "tool_not_found",
                    "message": format!("tool not found: {tool_name}"),
                }
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "code": "failed_to_update_tool_rule",
                    "message": err.to_string(),
                }
            })),
        )
            .into_response(),
    }
}
