use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::Client;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde::{Deserialize, Serialize};
use std::io;
use std::time::Duration;
use uuid::Uuid;

const PROMPT: &str = "\x1b[38;2;0;71;171m❯\x1b[0m ";

#[derive(Debug, Deserialize)]
struct SessionResponse {
    id: Uuid,
    title: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateSessionRequest {
    title: String,
}

#[derive(Debug, Deserialize)]
struct ListSessionsResponse {
    sessions: Vec<SessionResponse>,
}

#[derive(Debug, Serialize)]
struct CreateMessageRequest {
    text: String,
}

#[derive(Debug, serde::Deserialize)]
struct CreateMessageResponse {
    task_id: Uuid,
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
    session_id: Option<Uuid>,
    output: String,
    error: Option<String>,
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

#[derive(Debug, Deserialize)]
struct ListSchedulesResponse {
    schedules: Vec<ScheduleResponse>,
}

#[derive(Debug, Deserialize)]
struct RunScheduleResponse {
    task: TaskResponse,
}

#[derive(Debug, Deserialize)]
struct ScheduleResponse {
    id: Uuid,
    title: String,
    request: String,
    cron: String,
    timezone: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
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

    async fn list_schedules(&self) -> io::Result<Vec<ScheduleResponse>> {
        let response = self
            .http
            .get(format!("{}/v1/schedules", self.base_url))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ListSchedulesResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.schedules)
    }

    async fn get_schedule(&self, schedule_id: Uuid) -> io::Result<ScheduleResponse> {
        self.http
            .get(format!("{}/v1/schedules/{}", self.base_url, schedule_id))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<ScheduleResponse>()
            .await
            .map_err(io::Error::other)
    }

    async fn run_schedule(&self, schedule_id: Uuid) -> io::Result<TaskResponse> {
        let response = self
            .http
            .post(format!(
                "{}/v1/schedules/{}/run",
                self.base_url, schedule_id
            ))
            .send()
            .await
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?
            .json::<RunScheduleResponse>()
            .await
            .map_err(io::Error::other)?;

        Ok(response.task)
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
    let mut current_model = client.get_model().await?;
    let mut prompt = build_prompt(&current_model, session.id);

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
                        prompt = build_prompt(&current_model, session.id);
                        println!("new session: {}", session.id);
                    }
                    "/sessions" => {
                        let sessions = client.list_sessions().await?;

                        if sessions.is_empty() {
                            println!("no sessions");
                        } else {
                            println!("sessions");
                            println!("  {:<36}  title", "session");

                            for session in sessions {
                                println!(
                                    "  {:<36}  {}",
                                    session.id,
                                    session.title.as_deref().unwrap_or("-"),
                                );
                            }
                        }
                    }
                    "/session" => {
                        println!("current session");
                        println!("  id      {}", session.id);
                        println!("  title   {}", session.title.as_deref().unwrap_or("-"));
                    }
                    _ if line.starts_with("/session ") => {
                        let id = line.trim_start_matches("/session ").trim();

                        let Ok(next_session_id) = Uuid::parse_str(id) else {
                            println!("invalid session id: {id}");
                            continue;
                        };

                        session = client.get_session(next_session_id).await?;
                        prompt = build_prompt(&current_model, session.id);

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
                        prompt = build_prompt(&current_model, session.id);

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

                        if let Some(session_id) = task.session_id {
                            println!("  session  {session_id}");
                        }

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
                            && let Some(result) = terminal_task_text(&task)
                        {
                            println!("\nresult");
                            termimad::print_text(result);
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
                    // schedule
                    "/schedules" => {
                        let schedules = client.list_schedules().await?;

                        if schedules.is_empty() {
                            println!("no schedules");
                        } else {
                            println!("schedules");
                            println!(
                                "  {:<36}  {:<7}  {:<18}  {:<16}  title",
                                "schedule", "enabled", "cron", "timezone"
                            );

                            for schedule in schedules {
                                println!(
                                    "  {:<36}  {:<7}  {:<18}  {:<16}  {}",
                                    schedule.id,
                                    schedule.enabled,
                                    schedule.cron,
                                    schedule.timezone,
                                    schedule.title,
                                );
                            }
                        }
                    }
                    _ if line.starts_with("/schedule ") => {
                        let id = line.trim_start_matches("/schedule ").trim();

                        let Ok(schedule_id) = Uuid::parse_str(id) else {
                            println!("invalid schedule id: {id}");
                            continue;
                        };

                        let schedule = client.get_schedule(schedule_id).await?;

                        println!("schedule");
                        println!("  id        {}", schedule.id);
                        println!("  title     {}", schedule.title);
                        println!("  enabled   {}", schedule.enabled);
                        println!("  cron      {}", schedule.cron);
                        println!("  timezone  {}", schedule.timezone);
                        println!("  created   {}", schedule.created_at);
                        println!("  updated   {}", schedule.updated_at);
                        println!("\nrequest");
                        termimad::print_text(&schedule.request);
                    }
                    "/schedule" => {
                        println!("usage: /schedule <schedule_id>");
                    }
                    _ if line.starts_with("/schedule-run ") => {
                        let id = line.trim_start_matches("/schedule-run ").trim();

                        let Ok(schedule_id) = Uuid::parse_str(id) else {
                            println!("invalid schedule id: {id}");
                            continue;
                        };

                        let task = client.run_schedule(schedule_id).await?;

                        println!("schedule run started");
                        println!("  task     {}", task.id);
                        println!("  status   {}", task.status);
                        if let Some(session_id) = task.session_id {
                            println!("  session  {session_id}");
                        }
                        println!("  request  {}", truncate(&task.request, 120));

                        let outcome = wait_events(&client, task.id).await?;

                        if matches!(outcome, AgentTurnOutcome::AwaitingApproval) {
                            awaiting_task_id = Some(task.id);
                        }
                    }
                    "/schedule-run" => {
                        println!("usage: /schedule-run <schedule_id>");
                    }
                    // exit
                    "/exit" => break,
                    _ if line.starts_with('/') => {
                        println!("unknown command: {line}");
                    }
                    // message
                    _ => {
                        let task_id = client.post_message(session.id, line).await?;
                        prompt = build_prompt(&current_model, session.id);

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

fn build_prompt(model: &str, session_id: Uuid) -> String {
    format!("\n\x1b[90m{} | {}\x1b[0m\n{}", model, session_id, PROMPT)
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

                    let task = client.get_task(task_id).await?;
                    if let Some(result) = terminal_task_text(&task) {
                        termimad::print_text(result);
                    }
                    return Ok(AgentTurnOutcome::Completed);
                }
                "task_failed" => {
                    stop_spinner(&mut spinner);

                    match client.get_task(task_id).await {
                        Ok(task) => match terminal_task_text(&task) {
                            Some(result) => termimad::print_text(result),
                            None => println!("[task failed]"),
                        },
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

fn terminal_task_text(task: &TaskResponse) -> Option<&str> {
    if task.status == "failed" {
        return task
            .error
            .as_deref()
            .and_then(non_empty)
            .or_else(|| non_empty(&task.output));
    }

    non_empty(&task.output).or_else(|| task.error.as_deref().and_then(non_empty))
}

fn non_empty(value: &str) -> Option<&str> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
