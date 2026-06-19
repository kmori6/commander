use axum::{Json, extract::State};
use serde::Serialize;

use crate::domain::port::llm_provider::LlmProvider;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct GetModelResponse {
    pub model: String,
}

pub async fn get_model_handler(State(state): State<AppState>) -> Json<GetModelResponse> {
    let model = state.agent_runtime.llm_provider().model().to_string();

    Json(GetModelResponse { model })
}
