use crate::domain::model::task::TaskStatus;
use crate::presentation::dto::error::ErrorResponse;
use crate::presentation::dto::task::TaskResponse;
use crate::presentation::state::app_state::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

const DEFAULT_LIMIT: usize = 20;
const MAX_LIMIT: usize = 100;

#[derive(Debug, Deserialize)]
pub struct ListTaskQuery {
    pub status: Option<TaskStatus>,
    pub limit: Option<usize>,
}

pub async fn list_task_handler(
    State(state): State<AppState>,
    Query(query): Query<ListTaskQuery>,
) -> Response {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

    match state.task_usecase.list(query.status, limit).await {
        Ok(tasks) => (
            StatusCode::OK,
            Json(json!({
                "tasks": tasks.into_iter().map(TaskResponse::from).collect::<Vec<_>>(),
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new("failed_to_list_tasks", err.to_string())),
        )
            .into_response(),
    }
}
