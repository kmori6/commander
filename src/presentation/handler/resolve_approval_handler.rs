use crate::application::usecase::agent_usecase::AgentEvent;
use crate::domain::service::agent_service::AgentEvent as AgentProgressEvent;
use crate::presentation::state::app_state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures::stream;
use serde::Deserialize;
use serde_json::json;
use std::{convert::Infallible, time::Duration};
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

pub async fn resolve_approval_handler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Json(request): Json<ResolveApprovalRequest>,
) -> Response {
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);
    let (progress_tx, mut progress_rx) = mpsc::channel::<AgentProgressEvent>(32);
    let agent_usecase = state.agent_usecase.clone();

    tokio::spawn(async move {
        let result = match request.decision {
            ApprovalDecisionRequest::Approved => {
                agent_usecase
                    .approve_approval(session_id, progress_tx)
                    .await
            }
            ApprovalDecisionRequest::Denied => {
                agent_usecase.deny_approval(session_id, progress_tx).await
            }
        };

        while let Ok(progress) = progress_rx.try_recv() {
            let event = match progress {
                AgentProgressEvent::LlmStarted => Event::default()
                    .event("message")
                    .data(json!({"status": "llm_started"}).to_string()),
                AgentProgressEvent::LlmFinished => Event::default()
                    .event("message")
                    .data(json!({"status": "llm_finished"}).to_string()),
                AgentProgressEvent::ToolStarted { call_id, tool_name } => {
                    Event::default().event("message").data(
                        json!({
                            "status": "tool_started",
                            "call_id": call_id,
                            "tool_name": tool_name,
                        })
                        .to_string(),
                    )
                }
                AgentProgressEvent::ToolFinished {
                    call_id,
                    tool_name,
                    success,
                } => Event::default().event("message").data(
                    json!({
                        "status": "tool_finished",
                        "call_id": call_id,
                        "tool_name": tool_name,
                        "success": success,
                    })
                    .to_string(),
                ),
            };

            let _ = tx.send(Ok(event)).await;
        }

        match result {
            Ok(output) => {
                for event in output.events {
                    match event {
                        AgentEvent::AssistantMessage(content) => {
                            let _ = tx
                                .send(Ok(Event::default().event("message").data(
                                    json!({
                                        "role": "assistant",
                                        "content": content,
                                    })
                                    .to_string(),
                                )))
                                .await;
                        }
                        AgentEvent::ToolConfirmationRequested {
                            call_id,
                            tool_name,
                            arguments,
                            policy,
                        } => {
                            let _ = tx
                                .send(Ok(Event::default().event("approval_required").data(
                                    json!({
                                        "call_id": call_id,
                                        "tool_name": tool_name,
                                        "arguments": arguments,
                                        "policy": policy.as_str(),
                                    })
                                    .to_string(),
                                )))
                                .await;
                        }
                    }
                }

                let _ = tx
                    .send(Ok(Event::default()
                        .event("done")
                        .data(json!({"status": "done"}).to_string())))
                    .await;
            }
            Err(err) => {
                let _ = tx
                    .send(Ok(Event::default().event("error").data(
                        json!({
                            "error": {
                                "code": "failed_to_resolve_approval",
                                "message": err.to_string(),
                            }
                        })
                        .to_string(),
                    )))
                    .await;
            }
        }
    });

    let stream = stream::unfold(rx, |mut rx| async {
        rx.recv().await.map(|event| (event, rx))
    });

    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response()
}
