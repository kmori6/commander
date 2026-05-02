use crate::domain::service::agent_service::AgentEvent as AgentProgressEvent;
use crate::presentation::state::app_state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ResolveApprovalRequest {
    pub decision: ApprovalDecisionRequest,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecisionRequest {
    Approved,
    Denied,
}

impl ApprovalDecisionRequest {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Denied => "denied",
        }
    }
}

pub async fn resolve_approval_handler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<ResolveApprovalRequest>,
) -> Response {
    let decision = request.decision;
    let decision_text = decision.as_str();

    let agent_usecase = state.agent_usecase.clone();

    tokio::spawn(async move {
        let (progress_tx, mut progress_rx) = mpsc::channel::<AgentProgressEvent>(32);

        let progress_drain =
            tokio::spawn(async move { while progress_rx.recv().await.is_some() {} });

        let result = match decision {
            ApprovalDecisionRequest::Approved => {
                agent_usecase
                    .approve_approval(session_id, progress_tx)
                    .await
            }
            ApprovalDecisionRequest::Denied => {
                agent_usecase.deny_approval(session_id, progress_tx).await
            }
        };

        if let Err(err) = result {
            log::warn!("failed to resolve approval for session {session_id}: {err}");
        }

        if let Err(err) = progress_drain.await {
            log::warn!("failed to drain approval progress for session {session_id}: {err}");
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "session_id": session_id.to_string(),
            "decision": decision_text,
        })),
    )
        .into_response()
}
