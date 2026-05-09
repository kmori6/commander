use axum::{Json, extract::State};
use serde::Serialize;

use crate::presentation::state::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct GetModelResponse {
    pub model: String,
}

pub async fn get_model_handler(State(state): State<AppState>) -> Json<GetModelResponse> {
    Json(GetModelResponse {
        model: state.agent_runtime.llm_provider().default_model_id().await,
    })
}
