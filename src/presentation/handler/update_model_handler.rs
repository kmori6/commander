use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::domain::model::llm::Llm;
use crate::presentation::state::app_state::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateModelRequest {
    pub model: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateModelResponse {
    pub model: Llm,
}

pub async fn update_model_handler(
    State(state): State<AppState>,
    Json(request): Json<UpdateModelRequest>,
) -> Result<Json<UpdateModelResponse>, Response> {
    let model = state
        .agent_runtime
        .llm_provider()
        .select_model(&request.model)
        .await
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()).into_response())?;

    state.agent_runtime.set_model(model.id.clone()).await;

    Ok(Json(UpdateModelResponse { model }))
}
