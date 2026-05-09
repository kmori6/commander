use axum::{Json, extract::State};
use serde::Serialize;

use crate::domain::model::llm::ModelSpec;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Serialize)]
pub struct ListModelResponse {
    pub models: Vec<ModelSpec>,
}

pub async fn list_model_handler(State(state): State<AppState>) -> Json<ListModelResponse> {
    Json(ListModelResponse {
        models: state.agent_runtime.llm_provider().list_models().await,
    })
}
