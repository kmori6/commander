use crate::domain::util::data_uri::encode_data_uri;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::Client;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const PROMPT: &str = "\x1b[38;2;0;71;171m❯\x1b[0m ";

#[derive(Debug, Deserialize)]
struct SessionResponse {
    id: Uuid,
    title: Option<String>,
    status: String,
}

#[derive(Debug, Serialize)]
struct CreateSessionRequest {
    kind: &'static str,
    title: String,
}

#[derive(Debug, Deserialize)]
struct ListSessionsResponse {
    sessions: Vec<SessionResponse>,
}

#[derive(Debug, Serialize)]
struct CreateMessageRequest {
    text: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    input_images: Vec<CreateInputImage>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    input_files: Vec<CreateInputFile>,
}

#[derive(Debug, Serialize)]
struct CreateInputImage {
    image_url: String,
}

#[derive(Debug, Serialize)]
struct CreateInputFile {
    filename: String,
    file_data: String,
}

#[derive(Debug, serde::Deserialize)]
struct CreateMessageResponse {
    task_id: Uuid,
}

#[derive(Debug, serde::Deserialize)]
struct TaskResultResponse {
    output: String,
}

enum AgentTurnOutcome {
    Completed,
    Failed,
    AwaitingApproval,
}

#[derive(Debug, Deserialize)]
struct ListToolsResponse {
    tools: Vec<ToolResponse>,
}

#[derive(Debug, Deserialize)]
struct ToolResponse {
    name: String,
    description: String,
}

#[derive(Debug, Deserialize)]
struct ToolPermissionResponse {
    tool_name: String,
    mode: String,
}

#[derive(Debug, Deserialize)]
struct ListToolApprovalsResponse {
    approvals: Vec<ToolApprovalResponse>,
}

#[derive(Debug, Deserialize)]
struct ToolApprovalResponse {
    id: Uuid,
    call_id: String,
    status: String,
}

#[derive(Debug, Deserialize)]
struct ListTasksResponse {
    tasks: Vec<TaskResponse>,
}

#[derive(Debug, Clone)]
struct PendingAttachment {
    path: std::path::PathBuf,
    filename: String,
    media_type: String,
    data_uri: String,
    kind: AttachmentKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttachmentKind {
    Image,
    File,
}

#[derive(Debug, Deserialize)]
struct TaskResponse {
    id: Uuid,
    request: String,
    status: String,
    session_id: Uuid,
    started_at: Option<String>,
    finished_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListTaskEventsResponse {
    events: Vec<TaskEventResponse>,
}

#[derive(Debug, Deserialize)]
struct TaskEventResponse {
    event_type: String,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct ListModelsResponse {
    models: Vec<ModelResponse>,
}

#[derive(Debug, Deserialize)]
struct GetModelResponse {
    model: String,
}

#[derive(Debug, Serialize)]
struct UpdateModelRequest {
    model: String,
}

#[derive(Debug, Deserialize)]
struct UpdateModelResponse {
    model: ModelResponse,
}

#[derive(Debug, Deserialize)]
struct ModelResponse {
    id: String,
    provider: String,
    model: String,
    context_window: i64,
}

struct ChatApiClient {
    base_url: String,
    http: Client,
}

impl ChatApiClient {
    fn new(base_url: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: Client::new(),
        }
    }

    async fn health(&self) -> io::Result<()> {
        self.http
            .get(format!("{}/v1/health", self.base_url))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?;

        Ok(())
    }

    async fn get_session(&self, id: Uuid) -> io::Result<SessionResponse> {
        self.http
            .get(format!("{}/v1/sessions/{}", self.base_url, id))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<SessionResponse>()
            .await
            .map_err(io::Error::other)
    }

    async fn create_session(&self) -> io::Result<SessionResponse> {
        self.http
            .post(format!("{}/v1/sessions", self.base_url))
            .json(&CreateSessionRequest {
                kind: "chat",
                title: "Commander Chat".to_string(),
            })
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<SessionResponse>()
            .await
            .map_err(io::Error::other)
    }

    async fn list_sessions(&self) -> io::Result<Vec<SessionResponse>> {
        let response = self
            .http
            .get(format!("{}/v1/sessions", self.base_url))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ListSessionsResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.sessions)
    }

    async fn post_message(
        &self,
        session_id: Uuid,
        text: &str,
        attachments: &[PendingAttachment],
    ) -> io::Result<Uuid> {
        let input_images = attachments
            .iter()
            .filter(|attachment| attachment.kind == AttachmentKind::Image)
            .map(|attachment| CreateInputImage {
                image_url: attachment.data_uri.clone(),
            })
            .collect::<Vec<_>>();

        let input_files = attachments
            .iter()
            .filter(|attachment| attachment.kind == AttachmentKind::File)
            .map(|attachment| CreateInputFile {
                filename: attachment.filename.clone(),
                file_data: attachment.data_uri.clone(),
            })
            .collect::<Vec<_>>();

        let response = self
            .http
            .post(format!(
                "{}/v1/sessions/{}/messages",
                self.base_url, session_id
            ))
            .json(&CreateMessageRequest {
                text: text.to_string(),
                input_images,
                input_files,
            })
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<CreateMessageResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.task_id)
    }

    async fn connect_events(&self, task_id: Uuid) -> io::Result<reqwest::Response> {
        self.http
            .get(format!("{}/v1/events?task_id={}", self.base_url, task_id))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)
    }

    async fn get_task_result(&self, task_id: Uuid) -> io::Result<TaskResultResponse> {
        self.http
            .get(format!("{}/v1/tasks/{}/result", self.base_url, task_id))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<TaskResultResponse>()
            .await
            .map_err(io::Error::other)
    }

    async fn list_tools(&self) -> io::Result<Vec<ToolResponse>> {
        let response = self
            .http
            .get(format!("{}/v1/tools", self.base_url))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ListToolsResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.tools)
    }

    async fn list_models(&self) -> io::Result<Vec<ModelResponse>> {
        let response = self
            .http
            .get(format!("{}/v1/models", self.base_url))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ListModelsResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.models)
    }

    async fn get_model(&self) -> io::Result<String> {
        let response = self
            .http
            .get(format!("{}/v1/model", self.base_url))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<GetModelResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.model)
    }

    async fn update_model(&self, model: &str) -> io::Result<ModelResponse> {
        let response = self
            .http
            .put(format!("{}/v1/model", self.base_url))
            .json(&UpdateModelRequest {
                model: model.to_string(),
            })
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<UpdateModelResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.model)
    }

    async fn update_tool_permission(
        &self,
        tool_name: &str,
        mode: &str,
    ) -> io::Result<ToolPermissionResponse> {
        self.http
            .put(format!(
                "{}/v1/tools/permissions/{}",
                self.base_url, tool_name
            ))
            .json(&serde_json::json!({ "mode": mode }))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ToolPermissionResponse>()
            .await
            .map_err(io::Error::other)
    }

    async fn list_tool_approvals(&self) -> io::Result<Vec<ToolApprovalResponse>> {
        let response = self
            .http
            .get(format!(
                "{}/v1/tools/approvals?status=pending",
                self.base_url
            ))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ListToolApprovalsResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.approvals)
    }

    async fn approve(&self, approval_id: Uuid) -> io::Result<ToolApprovalResponse> {
        self.http
            .post(format!(
                "{}/v1/tools/approvals/{}/approve",
                self.base_url, approval_id
            ))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ToolApprovalResponse>()
            .await
            .map_err(io::Error::other)
    }

    async fn reject(&self, approval_id: Uuid) -> io::Result<ToolApprovalResponse> {
        self.http
            .post(format!(
                "{}/v1/tools/approvals/{}/reject",
                self.base_url, approval_id
            ))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ToolApprovalResponse>()
            .await
            .map_err(io::Error::other)
    }

    async fn list_tasks(&self) -> io::Result<Vec<TaskResponse>> {
        let response = self
            .http
            .get(format!("{}/v1/tasks", self.base_url))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ListTasksResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.tasks)
    }

    async fn get_task(&self, task_id: Uuid) -> io::Result<TaskResponse> {
        self.http
            .get(format!("{}/v1/tasks/{}", self.base_url, task_id))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<TaskResponse>()
            .await
            .map_err(io::Error::other)
    }

    async fn list_task_events(&self, task_id: Uuid) -> io::Result<Vec<TaskEventResponse>> {
        let response = self
            .http
            .get(format!("{}/v1/tasks/{}/events", self.base_url, task_id))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ListTaskEventsResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.events)
    }

    async fn cancel_task(&self, task_id: Uuid) -> io::Result<TaskResponse> {
        self.http
            .post(format!("{}/v1/tasks/{}/cancel", self.base_url, task_id))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<TaskResponse>()
            .await
            .map_err(io::Error::other)
    }
}

pub async fn run(base_url: String, session_id: Option<Uuid>) -> Result<(), io::Error> {
    let client = ChatApiClient::new(base_url);

    client.health().await?;

    let mut session = match session_id {
        Some(id) => client.get_session(id).await?,
        None => client.create_session().await?,
    };

    let mut awaiting_task_id: Option<Uuid> = None;
    let mut pending_attachments = Vec::<PendingAttachment>::new();
    let mut current_model = client.get_model().await?;
    let mut prompt = build_prompt(&current_model, session.id, pending_attachments.len());

    println!("commander chat");
    println!("server: {}", client.base_url);
    println!("session: {}", session.id);

    let mut rl = DefaultEditor::new().map_err(io::Error::other)?;

    loop {
        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();

                if line.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(line);

                match line {
                    // session
                    "/new" => {
                        session = client.create_session().await?;
                        prompt =
                            build_prompt(&current_model, session.id, pending_attachments.len());
                        println!("new session: {}", session.id);
                    }
                    "/sessions" => {
                        let sessions = client.list_sessions().await?;

                        if sessions.is_empty() {
                            println!("no sessions");
                        } else {
                            println!("sessions");
                            println!("  {:<36}  {:<10}  title", "session", "status");

                            for session in sessions {
                                println!(
                                    "  {:<36}  {:<10}  {}",
                                    session.id,
                                    session.status,
                                    session.title.as_deref().unwrap_or("-"),
                                );
                            }
                        }
                    }
                    "/session" => {
                        println!("current session");
                        println!("  id      {}", session.id);
                        println!("  status  {}", session.status);
                        println!("  title   {}", session.title.as_deref().unwrap_or("-"));
                    }
                    _ if line.starts_with("/session ") => {
                        let id = line.trim_start_matches("/session ").trim();

                        let Ok(next_session_id) = Uuid::parse_str(id) else {
                            println!("invalid session id: {id}");
                            continue;
                        };

                        session = client.get_session(next_session_id).await?;
                        prompt =
                            build_prompt(&current_model, session.id, pending_attachments.len());

                        println!("switched session: {}", session.id);
                    }
                    // model
                    "/models" => {
                        let current_model = client.get_model().await?;
                        let models = client.list_models().await?;

                        if models.is_empty() {
                            println!("no models");
                        } else {
                            println!("models");
                            println!(
                                "  {:<1}  {:<22}  {:<10}  {:<28}  context",
                                "", "id", "provider", "model"
                            );

                            for model in models {
                                let marker = if model.id == current_model { "*" } else { " " };

                                println!(
                                    "  {:<1}  {:<22}  {:<10}  {:<28}  {}",
                                    marker,
                                    model.id,
                                    model.provider,
                                    model.model,
                                    model.context_window,
                                );
                            }
                        }
                    }
                    "/model" => {
                        let model = client.get_model().await?;
                        println!("current model: {model}");
                    }
                    _ if line.starts_with("/model ") => {
                        let model_id = line.trim_start_matches("/model ").trim();

                        if model_id.is_empty() {
                            println!("usage: /model <model>");
                            continue;
                        }

                        let model = client.update_model(model_id).await?;
                        current_model = model.id.clone();
                        prompt =
                            build_prompt(&current_model, session.id, pending_attachments.len());

                        println!("model switched");
                        println!("  id       {}", model.id);
                        println!("  provider {}", model.provider);
                        println!("  model    {}", model.model);
                        println!("  context  {}", model.context_window);
                    }
                    // tool
                    "/tools" => {
                        let tools = client.list_tools().await?;

                        if tools.is_empty() {
                            println!("no tools");
                        } else {
                            println!("tools");

                            for tool in tools {
                                println!("  {:<20} {}", tool.name, tool.description);
                            }
                        }
                    }
                    _ if line.starts_with("/tool ") => {
                        let parts = line.split_whitespace().collect::<Vec<_>>();

                        if parts.len() != 3 {
                            println!("usage: /tool <tool_name> <allow|ask|deny>");
                            continue;
                        }

                        let tool_name = parts[1];
                        let mode = parts[2];

                        if !matches!(mode, "allow" | "ask" | "deny") {
                            println!("usage: /tool <tool_name> <allow|ask|deny>");
                            continue;
                        }

                        let permission = client.update_tool_permission(tool_name, mode).await?;

                        println!("tool permission saved");
                        println!("  tool  {}", permission.tool_name);
                        println!("  mode  {}", permission.mode);
                    }
                    "/approvals" => {
                        let approvals = client.list_tool_approvals().await?;

                        if approvals.is_empty() {
                            println!("no pending approvals");
                        } else {
                            println!("pending approvals");
                            println!("  {:<36}  {:<10}  call", "approval", "status");

                            for approval in approvals {
                                println!(
                                    "  {:<36}  {:<10}  {}",
                                    approval.id, approval.status, approval.call_id
                                );
                            }
                        }
                    }
                    _ if line.starts_with("/approve ") => {
                        let id = line.trim_start_matches("/approve ").trim();

                        let Ok(approval_id) = Uuid::parse_str(id) else {
                            println!("invalid approval id: {id}");
                            continue;
                        };

                        let approval = client.approve(approval_id).await?;
                        println!("approved: {}", approval.id);

                        if let Some(task_id) = awaiting_task_id.take() {
                            let outcome = wait_events(&client, task_id).await?;

                            if matches!(outcome, AgentTurnOutcome::AwaitingApproval) {
                                awaiting_task_id = Some(task_id);
                            }
                        }
                    }
                    _ if line.starts_with("/reject ") => {
                        let id = line.trim_start_matches("/reject ").trim();

                        let Ok(approval_id) = Uuid::parse_str(id) else {
                            println!("invalid approval id: {id}");
                            continue;
                        };

                        let approval = client.reject(approval_id).await?;
                        println!("rejected: {}", approval.id);

                        if let Some(task_id) = awaiting_task_id.take() {
                            let outcome = wait_events(&client, task_id).await?;

                            if matches!(outcome, AgentTurnOutcome::AwaitingApproval) {
                                awaiting_task_id = Some(task_id);
                            }
                        }
                    }
                    // task
                    "/tasks" => {
                        let tasks = client.list_tasks().await?;

                        if tasks.is_empty() {
                            println!("no tasks");
                        } else {
                            println!("tasks");
                            println!("  {:<36}  {:<16}  request", "task", "status");

                            for task in tasks {
                                let request = truncate(&task.request, 80);

                                println!("  {:<36}  {:<16}  {}", task.id, task.status, request);
                            }
                        }
                    }
                    _ if line.starts_with("/task ") => {
                        let id = line.trim_start_matches("/task ").trim();

                        let Ok(task_id) = Uuid::parse_str(id) else {
                            println!("invalid task id: {id}");
                            continue;
                        };

                        let task = client.get_task(task_id).await?;

                        println!("task");
                        println!("  id       {}", task.id);
                        println!("  status   {}", task.status);
                        println!("  request  {}", task.request);
                        println!("  session  {}", task.session_id);

                        if let Some(started_at) = task.started_at.as_deref() {
                            println!("  started  {started_at}");
                        }

                        if let Some(finished_at) = task.finished_at.as_deref() {
                            println!("  finished {finished_at}");
                        }

                        let events = client.list_task_events(task_id).await?;

                        if !events.is_empty() {
                            println!("\nevents");

                            for event in events {
                                println!("  {:<28} {}", event.event_type, event.created_at);
                            }
                        }

                        if matches!(task.status.as_str(), "completed" | "failed" | "cancelled")
                            && let Ok(result) = client.get_task_result(task_id).await
                        {
                            println!("\nresult");
                            termimad::print_text(&result.output);
                        }
                    }
                    _ if line.starts_with("/cancel ") => {
                        let id = line.trim_start_matches("/cancel ").trim();

                        let Ok(task_id) = Uuid::parse_str(id) else {
                            println!("invalid task id: {id}");
                            continue;
                        };

                        let task = client.cancel_task(task_id).await?;

                        println!("cancel requested");
                        println!("  id      {}", task.id);
                        println!("  status  {}", task.status);
                    }
                    // attachment
                    "/files" => {
                        if pending_attachments.is_empty() {
                            println!("no attached files");
                        } else {
                            println!("attached files");
                            println!("  {:<4}  {:<8}  {:<24}  path", "no", "kind", "media_type");

                            for (index, attachment) in pending_attachments.iter().enumerate() {
                                let kind = match attachment.kind {
                                    AttachmentKind::Image => "image",
                                    AttachmentKind::File => "file",
                                };

                                println!(
                                    "  {:<4}  {:<8}  {:<24}  {}",
                                    index + 1,
                                    kind,
                                    attachment.media_type,
                                    attachment.path.display(),
                                );
                            }
                        }
                    }
                    _ if line.starts_with("/attach ") => {
                        let path = line.trim_start_matches("/attach ").trim();

                        match build_attachment(path) {
                            Ok(attachment) => {
                                println!("attached: {}", attachment.path.display());
                                pending_attachments.push(attachment);
                                prompt = build_prompt(
                                    &current_model,
                                    session.id,
                                    pending_attachments.len(),
                                );
                            }
                            Err(err) => {
                                println!("failed to attach file: {err}");
                            }
                        }
                    }
                    _ if line.starts_with("/detach ") => {
                        let value = line.trim_start_matches("/detach ").trim();

                        if value == "all" {
                            pending_attachments.clear();
                            prompt =
                                build_prompt(&current_model, session.id, pending_attachments.len());
                            println!("detached all files");
                            continue;
                        }

                        let Ok(index) = value.parse::<usize>() else {
                            println!("usage: /detach <no|all>");
                            continue;
                        };

                        if index == 0 || index > pending_attachments.len() {
                            println!("attachment not found: {value}");
                            continue;
                        }

                        let removed = pending_attachments.remove(index - 1);
                        prompt =
                            build_prompt(&current_model, session.id, pending_attachments.len());
                        println!("detached: {}", removed.path.display());
                    }

                    // exit
                    "/exit" => break,
                    _ if line.starts_with('/') => {
                        println!("unknown command: {line}");
                    }
                    // message
                    _ => {
                        let task_id = client
                            .post_message(session.id, line, &pending_attachments)
                            .await?;

                        pending_attachments.clear();
                        prompt =
                            build_prompt(&current_model, session.id, pending_attachments.len());

                        let outcome = wait_events(&client, task_id).await?;

                        if matches!(outcome, AgentTurnOutcome::AwaitingApproval) {
                            awaiting_task_id = Some(task_id);
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
                break;
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                return Err(io::Error::other(err));
            }
        }
    }

    Ok(())
}

fn build_prompt(model: &str, session_id: Uuid, attachment_count: usize) -> String {
    format!(
        "\n\x1b[90m{} | {} | files {}\x1b[0m\n{}",
        model, session_id, attachment_count, PROMPT
    )
}

fn start_spinner(spinner: &mut Option<ProgressBar>) {
    if spinner.is_some() {
        return;
    }

    let progress = ProgressBar::with_draw_target(None, ProgressDrawTarget::stdout());

    progress.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .expect("spinner template should be valid")
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );

    progress.set_message("Figuring...");
    progress.enable_steady_tick(Duration::from_millis(100));

    *spinner = Some(progress);
}

fn stop_spinner(spinner: &mut Option<ProgressBar>) {
    if let Some(progress) = spinner.take() {
        progress.finish_and_clear();
    }
}

async fn wait_events(client: &ChatApiClient, task_id: Uuid) -> io::Result<AgentTurnOutcome> {
    let mut events = client.connect_events(task_id).await?;
    let mut event_buffer = String::new();
    let mut spinner: Option<ProgressBar> = None;

    while let Some(chunk) = events.chunk().await.map_err(io::Error::other)? {
        event_buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(index) = event_buffer.find("\n\n") {
            let raw_event = event_buffer[..index].to_string();
            event_buffer = event_buffer[index + 2..].to_string();

            let mut event_name = "";
            let mut event_data = String::new();

            for line in raw_event.lines() {
                let line = line.trim_end_matches('\r');

                if let Some(value) = line.strip_prefix("event:") {
                    event_name = value.trim();
                } else if let Some(value) = line.strip_prefix("data:") {
                    event_data.push_str(value.trim());
                }
            }

            if event_name.is_empty() || event_data.is_empty() {
                continue;
            }

            let Ok(data) = serde_json::from_str::<serde_json::Value>(&event_data) else {
                continue;
            };

            let payload = data.get("payload").unwrap_or(&serde_json::Value::Null);

            match event_name {
                "llm_started" => {
                    start_spinner(&mut spinner);
                }
                "llm_finished" => {
                    stop_spinner(&mut spinner);

                    let input = payload
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let output = payload
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    println!(
                        "\x1b[90mtoken input={:.1}k output={:.1}k\x1b[0m",
                        input as f64 / 1000.0,
                        output as f64 / 1000.0,
                    );
                }
                "tool_call_started" => {
                    stop_spinner(&mut spinner);

                    let tool_name = payload
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool");

                    println!("[tool] {tool_name}");

                    if let Some(arguments) = payload.get("arguments") {
                        let pretty = serde_json::to_string_pretty(arguments)
                            .unwrap_or_else(|_| arguments.to_string());

                        println!("  args:");
                        println!("{}", truncate(&pretty, 2000));
                    }
                }
                "tool_call_finished" => {
                    let status = payload
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    println!("[tool result] {status}");

                    if let Some(output) = payload.get("output") {
                        let pretty = serde_json::to_string_pretty(output)
                            .unwrap_or_else(|_| output.to_string());

                        println!("{}", truncate(&pretty, 2000));
                    }

                    start_spinner(&mut spinner);
                }
                "tool_approval_requested" => {
                    stop_spinner(&mut spinner);

                    let approval_id = payload
                        .get("approval_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-");

                    let tool_name = payload
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool");

                    println!("[approval requested] {tool_name}");
                    println!("Run /approve {approval_id} or /reject {approval_id}.");

                    return Ok(AgentTurnOutcome::AwaitingApproval);
                }
                "task_completed" => {
                    stop_spinner(&mut spinner);

                    let result = client.get_task_result(task_id).await?;
                    termimad::print_text(&result.output);
                    return Ok(AgentTurnOutcome::Completed);
                }
                "task_failed" => {
                    stop_spinner(&mut spinner);

                    match client.get_task_result(task_id).await {
                        Ok(result) => termimad::print_text(&result.output),
                        Err(_) => println!("[task failed]"),
                    }
                    return Ok(AgentTurnOutcome::Failed);
                }
                _ => {}
            }
        }
    }

    stop_spinner(&mut spinner);
    Ok(AgentTurnOutcome::Failed)
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn build_attachment(path: &str) -> io::Result<PendingAttachment> {
    let path = PathBuf::from(path.trim_matches('"').trim_matches('\''));
    let bytes = fs::read(&path)?;

    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("attachment")
        .to_string();

    let media_type = detect_media_type(&path)?;
    let kind = if media_type.starts_with("image/") {
        AttachmentKind::Image
    } else {
        AttachmentKind::File
    };

    let data_uri = encode_data_uri(&media_type, &bytes);

    Ok(PendingAttachment {
        path,
        filename,
        media_type,
        data_uri,
        kind,
    })
}

fn detect_media_type(path: &Path) -> io::Result<String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let media_type = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "html" | "htm" => "text/html",
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "txt" | "log" | "rs" | "toml" | "json" | "yaml" | "yml" | "js" | "ts" | "tsx" | "jsx"
        | "py" | "go" | "java" | "c" | "cc" | "cpp" | "h" | "hpp" | "sh" | "sql" => "text/plain",
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported attachment type: {}", path.display()),
            ));
        }
    };

    Ok(media_type.to_string())
}
