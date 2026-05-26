use axum::response::{IntoResponse, Response};
use axum::{Json, extract::State, http::StatusCode};
use serde::Serialize;

use crate::domain::port::llm_provider::LlmProvider;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct GetModelResponse {
    pub model: String,
}

pub async fn get_model_handler(
    State(state): State<AppState>,
) -> Result<Json<GetModelResponse>, Response> {
    let model = state
        .agent_runtime
        .llm_provider()
        .current_model_id()
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response())?;

    Ok(Json(GetModelResponse { model }))
}
