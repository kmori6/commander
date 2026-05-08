use std::io;

use reqwest::Client;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde::{Deserialize, Serialize};
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

#[derive(Debug, serde::Serialize)]
struct CreateMessageRequest {
    text: String,
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

    async fn post_message(&self, session_id: Uuid, text: &str) -> io::Result<Uuid> {
        let response = self
            .http
            .post(format!(
                "{}/v1/sessions/{}/messages",
                self.base_url, session_id
            ))
            .json(&CreateMessageRequest {
                text: text.to_string(),
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

    let mut prompt = build_prompt(session.id);
    let mut awaiting_task_id: Option<Uuid> = None;

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
                        prompt = build_prompt(session.id);
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
                        prompt = build_prompt(session.id);

                        println!("switched session: {}", session.id);
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

                    // exit
                    "/exit" => break,
                    _ if line.starts_with('/') => {
                        println!("unknown command: {line}");
                    }
                    // message
                    _ => {
                        let task_id = client.post_message(session.id, line).await?;
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

fn build_prompt(session_id: Uuid) -> String {
    format!("\n\x1b[90m{}\x1b[0m\n{}", session_id, PROMPT)
}

async fn wait_events(client: &ChatApiClient, task_id: Uuid) -> io::Result<AgentTurnOutcome> {
    let mut events = client.connect_events(task_id).await?;
    let mut event_buffer = String::new();

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
                    println!("[llm] started");
                }
                "llm_finished" => {
                    let input = payload
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let output = payload
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    println!("[llm] finished input={} output={}", input, output);
                }
                "tool_call_started" => {
                    let tool_name = payload
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool");

                    println!("[tool] {tool_name}");
                }
                "tool_call_finished" => {
                    let status = payload
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    println!("[tool] finished {status}");
                }
                "tool_approval_requested" => {
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
                    let result = client.get_task_result(task_id).await?;
                    termimad::print_text(&result.output);
                    return Ok(AgentTurnOutcome::Completed);
                }
                "task_failed" => {
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
