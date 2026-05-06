use crate::application::usecase::agent_usecase::Attachment;
use crate::domain::model::chat_session::ChatSession;
use crate::domain::model::message::MessageContent;
use crate::presentation::error::agent_cli_error::AgentCliError;
use crate::presentation::util::attachment::load_attachment;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;
use termimad::print_text;
use uuid::Uuid;

const PROMPT: &str = "\x1b[38;2;0;71;171m❯\x1b[0m ";
const FILE_ICON: &str = "@";
const MAX_CHARS: usize = 800;

#[derive(Debug, Deserialize)]
struct ListToolsResponse {
    tools: Vec<ToolResponse>,
}

#[derive(Debug, Deserialize)]
struct UpdateToolRuleResponse {
    tool: ToolResponse,
}

#[derive(Debug, Deserialize)]
struct ToolResponse {
    name: String,
    action: String,
    policy: String,
    rule: Option<String>,
    source: String,
}

#[derive(Debug, Deserialize)]
struct SessionUsageResponse {
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_write_tokens: u64,
}

#[derive(Debug, Deserialize)]
struct JobResponse {
    id: Uuid,
    kind: String,
    status: String,
    title: String,
    objective: String,
    session_id: Option<Uuid>,
    parent_job_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct ListJobsResponse {
    jobs: Vec<JobResponse>,
}

#[derive(Debug, Deserialize)]
struct ListJobRunsResponse {
    runs: Vec<JobRunResponse>,
}

#[derive(Debug, Deserialize)]
struct JobRunResponse {
    id: Uuid,
    attempt: i32,
    status: String,
    started_at: String,
    finished_at: Option<String>,
    error_message: Option<String>,
}

struct ChatApiClient {
    base_url: String,
    http: reqwest::Client,
}

impl ChatApiClient {
    fn new(base_url: String) -> Self {
        Self {
            // http://localhost:3000/ -> http://localhost:3000
            base_url: base_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    async fn health(&self) -> Result<(), AgentCliError> {
        self.http
            .get(format!("{}/v1/health", self.base_url))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn get_session(&self, id: Uuid) -> Result<ChatSession, AgentCliError> {
        let session = self
            .http
            .get(format!("{}/v1/sessions/{}", self.base_url, id))
            .send()
            .await?
            .error_for_status()?
            .json::<ChatSession>()
            .await?;

        Ok(session)
    }

    async fn create_session(&self) -> Result<ChatSession, AgentCliError> {
        let session = self
            .http
            .post(format!("{}/v1/sessions", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json::<ChatSession>()
            .await?;

        Ok(session)
    }

    async fn connect_events(&self) -> Result<reqwest::Response, AgentCliError> {
        let response = self
            .http
            .get(format!("{}/v1/events", self.base_url))
            .send()
            .await?
            .error_for_status()?;

        Ok(response)
    }

    async fn post_message(
        &self,
        session_id: Uuid,
        text: &str,
        attached_files: &[PathBuf],
    ) -> Result<(), AgentCliError> {
        let mut content = Vec::<Value>::new();

        for path in attached_files {
            let attachment = load_attachment(path).map_err(|err| {
                AgentCliError::Io(std::io::Error::other(format!(
                    "failed to load attachment {}: {err}",
                    path.display()
                )))
            })?;

            let value = match attachment {
                Attachment::Image(image) => serde_json::to_value(MessageContent::InputImage(image)),
                Attachment::File(file) => serde_json::to_value(MessageContent::InputFile(file)),
            }
            .map_err(|err| {
                AgentCliError::Io(std::io::Error::other(format!(
                    "failed to encode attachment {}: {err}",
                    path.display()
                )))
            })?;

            content.push(value);
        }

        content.insert(
            0,
            json!({
                "type": "input_text",
                "text": text
            }),
        );

        self.http
            .post(format!(
                "{}/v1/sessions/{}/messages",
                self.base_url, session_id
            ))
            .json(&json!({
                "user_message": {
                    "role": "user",
                    "content": content
                }
            }))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn resolve_approval(
        &self,
        session_id: Uuid,
        decision: &str,
    ) -> Result<(), AgentCliError> {
        self.http
            .post(format!(
                "{}/v1/sessions/{}/approvals",
                self.base_url, session_id
            ))
            .json(&json!({
                "decision": decision
            }))
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<ToolResponse>, AgentCliError> {
        let response = self
            .http
            .get(format!("{}/v1/tools", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json::<ListToolsResponse>()
            .await?;

        Ok(response.tools)
    }

    async fn update_tool_rule(
        &self,
        tool_name: &str,
        action: &str,
    ) -> Result<ToolResponse, AgentCliError> {
        let response = self
            .http
            .put(format!("{}/v1/tools/{}/rule", self.base_url, tool_name))
            .json(&json!({
                "action": action
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<UpdateToolRuleResponse>()
            .await?;

        Ok(response.tool)
    }

    async fn get_usage(&self, session_id: Uuid) -> Result<SessionUsageResponse, AgentCliError> {
        let response = self
            .http
            .get(format!(
                "{}/v1/sessions/{}/usage",
                self.base_url, session_id
            ))
            .send()
            .await?
            .error_for_status()?
            .json::<SessionUsageResponse>()
            .await?;

        Ok(response)
    }

    async fn create_job(
        &self,
        session_id: Uuid,
        objective: &str,
    ) -> Result<JobResponse, AgentCliError> {
        let job = self
            .http
            .post(format!("{}/v1/jobs", self.base_url))
            .json(&json!({
                "kind": "general",
                "objective": objective,
                "session_id": session_id,
                "parent_job_id": null,
            }))
            .send()
            .await?
            .error_for_status()?
            .json::<JobResponse>()
            .await?;

        Ok(job)
    }

    async fn list_jobs(&self) -> Result<Vec<JobResponse>, AgentCliError> {
        let response = self
            .http
            .get(format!("{}/v1/jobs?limit=20", self.base_url))
            .send()
            .await?
            .error_for_status()?
            .json::<ListJobsResponse>()
            .await?;

        Ok(response.jobs)
    }

    async fn get_job(&self, job_id: Uuid) -> Result<JobResponse, AgentCliError> {
        let job = self
            .http
            .get(format!("{}/v1/jobs/{}", self.base_url, job_id))
            .send()
            .await?
            .error_for_status()?
            .json::<JobResponse>()
            .await?;

        Ok(job)
    }

    async fn cancel_job(&self, job_id: Uuid) -> Result<JobResponse, AgentCliError> {
        let job = self
            .http
            .post(format!("{}/v1/jobs/{}/cancel", self.base_url, job_id))
            .send()
            .await?
            .error_for_status()?
            .json::<JobResponse>()
            .await?;

        Ok(job)
    }

    async fn start_job(&self, job_id: Uuid) -> Result<JobResponse, AgentCliError> {
        let job = self
            .http
            .post(format!("{}/v1/jobs/{}/start", self.base_url, job_id))
            .send()
            .await?
            .error_for_status()?
            .json::<JobResponse>()
            .await?;

        Ok(job)
    }

    async fn list_job_runs(&self, job_id: Uuid) -> Result<Vec<JobRunResponse>, AgentCliError> {
        let response = self
            .http
            .get(format!("{}/v1/jobs/{}/runs", self.base_url, job_id))
            .send()
            .await?
            .error_for_status()?
            .json::<ListJobRunsResponse>()
            .await?;

        Ok(response.runs)
    }
}

pub async fn run(base_url: String, session_id: Option<Uuid>) -> Result<(), AgentCliError> {
    let client = ChatApiClient::new(base_url);

    // check server health
    client.health().await?;

    let mut session = match session_id {
        Some(id) => client.get_session(id).await?,
        None => client.create_session().await?,
    };

    let mut events = client.connect_events().await?;
    let mut event_buffer = String::new();

    println!("commander chat");
    println!("server: {}", client.base_url);
    println!("session: {}", session.id);

    let mut attached_files = Vec::<PathBuf>::new();
    let mut prompt = format!(
        "\n\x1b[90m{} | files {}\x1b[0m\n{}",
        session.id,
        attached_files.len(),
        PROMPT
    );

    let mut rl = DefaultEditor::new().map_err(|e| AgentCliError::Readline(e.to_string()))?;

    loop {
        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();

                if line.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(line);

                match line {
                    "/exit" => break,
                    "/new" => {
                        session = client.create_session().await?;
                        attached_files.clear();
                        prompt = format!(
                            "\n\x1b[90m{} | files {}\x1b[0m\n{}",
                            session.id,
                            attached_files.len(),
                            PROMPT
                        );
                        println!("new session: {}", session.id);
                    }
                    "/attach" => {
                        println!("usage: /attach <files...>");
                    }
                    _ if line.starts_with("/attach ") => {
                        let paths = line
                            .split_whitespace()
                            .skip(1)
                            .map(|path| path.trim_matches('\'').trim_matches('"'))
                            .map(PathBuf::from)
                            .collect::<Vec<_>>();

                        let mut attached = Vec::new();
                        for path in paths {
                            match std::fs::metadata(&path) {
                                Ok(metadata) if metadata.is_file() => {
                                    let bytes = metadata.len();
                                    let size = if bytes < 1024 {
                                        format!("{bytes} B")
                                    } else if bytes < 1024 * 1024 {
                                        format!("{:.1} KB", bytes as f64 / 1024.0)
                                    } else {
                                        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
                                    };

                                    attached_files.push(path.clone());
                                    attached.push((attached_files.len(), path, size));
                                }
                                Ok(_) => {
                                    println!("not a file: {}", path.display());
                                }
                                Err(err) => {
                                    println!("failed to attach {}: {err}", path.display());
                                }
                            }
                        }

                        if !attached.is_empty() {
                            prompt = format!(
                                "\n\x1b[90m{} | files {}\x1b[0m\n{}",
                                session.id,
                                attached_files.len(),
                                PROMPT
                            );

                            println!("attached");

                            for (index, path, size) in attached {
                                println!("  {FILE_ICON} {index}  {}  {size}", path.display());
                            }
                        }
                    }
                    "/files" => {
                        if attached_files.is_empty() {
                            println!("no attached files");
                        } else {
                            println!("attached files");

                            for (index, path) in attached_files.iter().enumerate() {
                                let size = match std::fs::metadata(path) {
                                    Ok(metadata) => {
                                        let bytes = metadata.len();

                                        if bytes < 1024 {
                                            format!("{bytes} B")
                                        } else if bytes < 1024 * 1024 {
                                            format!("{:.1} KB", bytes as f64 / 1024.0)
                                        } else {
                                            format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
                                        }
                                    }
                                    Err(_) => "missing".to_string(),
                                };

                                println!("  {FILE_ICON} {}  {}  {size}", index + 1, path.display());
                            }
                        }
                    }
                    "/detach" => {
                        println!("usage: /detach <index|all>");
                    }
                    _ if line.starts_with("/detach ") => {
                        let target = line.trim_start_matches("/detach ").trim();

                        if target == "all" {
                            attached_files.clear();

                            prompt = format!(
                                "\n\x1b[90m{} | files {}\x1b[0m\n{}",
                                session.id,
                                attached_files.len(),
                                PROMPT
                            );

                            println!("detached all files");
                        } else {
                            let Ok(index) = target.parse::<usize>() else {
                                println!("invalid file index: {target}");
                                continue;
                            };

                            if index == 0 || index > attached_files.len() {
                                println!("file index out of range: {index}");
                                continue;
                            }

                            let detached = attached_files.remove(index - 1);

                            prompt = format!(
                                "\n\x1b[90m{} | files {}\x1b[0m\n{}",
                                session.id,
                                attached_files.len(),
                                PROMPT
                            );

                            println!("detached");
                            println!("  {FILE_ICON} {index}  {}", detached.display());
                        }
                    }
                    "/jobs" => {
                        let jobs = client.list_jobs().await?;

                        if jobs.is_empty() {
                            println!("no jobs");
                        } else {
                            println!("jobs");

                            for job in jobs {
                                let id = job.id.to_string();
                                let short_id = id.chars().take(8).collect::<String>();

                                println!(
                                    "  {:<8}  {:<8}  {:<10}  {}",
                                    short_id, job.status, job.kind, job.title,
                                );
                            }
                        }
                    }
                    "/status" => {
                        println!("usage: /status <job_id>");
                    }
                    _ if line.starts_with("/status ") => {
                        let id = line.trim_start_matches("/status ").trim();

                        let Ok(job_id) = Uuid::parse_str(id) else {
                            println!("invalid job id: {id}");
                            continue;
                        };

                        let job = client.get_job(job_id).await?;

                        println!("job");
                        println!("  id         {}", job.id);
                        println!("  kind       {}", job.kind);
                        println!("  status     {}", job.status);
                        println!("  title      {}", job.title);
                        println!("  objective  {}", job.objective);

                        if let Some(session_id) = job.session_id {
                            println!("  session    {}", session_id);
                        }

                        if let Some(parent_job_id) = job.parent_job_id {
                            println!("  parent     {}", parent_job_id);
                        }
                    }
                    "/runs" => {
                        println!("usage: /runs <job_id>");
                    }
                    _ if line.starts_with("/runs ") => {
                        let id = line.trim_start_matches("/runs ").trim();

                        let Ok(job_id) = Uuid::parse_str(id) else {
                            println!("invalid job id: {id}");
                            continue;
                        };

                        let runs = client.list_job_runs(job_id).await?;

                        if runs.is_empty() {
                            println!("no job runs");
                        } else {
                            println!("job runs");

                            for run in runs {
                                println!("  #{}  {:<10}  {}", run.attempt, run.status, run.id);
                                println!("      started   {}", run.started_at);

                                if let Some(finished_at) = run.finished_at {
                                    println!("      finished  {}", finished_at);
                                }

                                if let Some(error_message) = run.error_message {
                                    println!("      error     {}", error_message);
                                }
                            }
                        }
                    }
                    "/run" => {
                        println!("usage: /run <job_id>");
                    }
                    _ if line.starts_with("/run ") => {
                        let id = line.trim_start_matches("/run ").trim();

                        let Ok(job_id) = Uuid::parse_str(id) else {
                            println!("invalid job id: {id}");
                            continue;
                        };

                        let job = client.start_job(job_id).await?;

                        println!("started job");
                        println!("  id      {}", job.id);
                        println!("  status  {}", job.status);
                        println!("  title   {}", job.title);

                        let target_session_id = job.session_id.unwrap_or(session.id);

                        client
                            .post_message(target_session_id, &job.objective, &[])
                            .await?;

                        wait_events(&mut events, &mut event_buffer, target_session_id).await?;
                    }

                    "/cancel" => {
                        println!("usage: /cancel <job_id>");
                    }
                    _ if line.starts_with("/cancel ") => {
                        let id = line.trim_start_matches("/cancel ").trim();

                        let Ok(job_id) = Uuid::parse_str(id) else {
                            println!("invalid job id: {id}");
                            continue;
                        };

                        let job = client.cancel_job(job_id).await?;

                        if job.status == "cancel_requested" {
                            println!("cancel requested");
                        } else {
                            println!("cancelled job");
                        }

                        println!("  id      {}", job.id);
                        println!("  status  {}", job.status);
                        println!("  title   {}", job.title);
                    }
                    "/job" => {
                        println!("usage: /job <objective>");
                    }
                    _ if line.starts_with("/job ") => {
                        let objective = line.trim_start_matches("/job ").trim();
                        let objective = objective
                            .strip_prefix('"')
                            .and_then(|value| value.strip_suffix('"'))
                            .or_else(|| {
                                objective
                                    .strip_prefix('\'')
                                    .and_then(|value| value.strip_suffix('\''))
                            })
                            .unwrap_or(objective)
                            .trim();

                        if objective.is_empty() {
                            println!("usage: /job <objective>");
                            continue;
                        }

                        let job = client.create_job(session.id, objective).await?;

                        println!("created job");
                        println!("  id      {}", job.id);
                        println!("  kind    {}", job.kind);
                        println!("  status  {}", job.status);
                        println!("  title   {}", job.title);
                    }
                    "/tools" => {
                        let tools = client.list_tools().await?;

                        if tools.is_empty() {
                            println!("no tools");
                        } else {
                            println!(
                                "  {:<20} {:<6} {:<6} {:<6} source",
                                "tool", "action", "policy", "rule"
                            );

                            for tool in tools {
                                println!(
                                    "  {:<20} {:<6} {:<6} {:<6} {}",
                                    tool.name,
                                    tool.action,
                                    tool.policy,
                                    tool.rule.as_deref().unwrap_or("-"),
                                    tool.source
                                );
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
                        let action = parts[2];

                        if !matches!(action, "allow" | "ask" | "deny") {
                            println!("usage: /tool <tool_name> <allow|ask|deny>");
                            continue;
                        }

                        let tool = client.update_tool_rule(tool_name, action).await?;

                        println!(
                            "tool rule saved: {} -> {}",
                            tool.name,
                            tool.rule.as_deref().unwrap_or("-")
                        );
                        println!(
                            "  {:<20} {:<6} {:<6} {:<6} source",
                            "tool", "action", "policy", "rule"
                        );
                        println!(
                            "  {:<20} {:<6} {:<6} {:<6} {}",
                            tool.name,
                            tool.action,
                            tool.policy,
                            tool.rule.as_deref().unwrap_or("-"),
                            tool.source
                        );
                    }
                    "/approve" => {
                        client.resolve_approval(session.id, "approved").await?;
                        wait_events(&mut events, &mut event_buffer, session.id).await?;
                    }
                    "/deny" => {
                        client.resolve_approval(session.id, "denied").await?;
                        wait_events(&mut events, &mut event_buffer, session.id).await?;
                    }
                    "/usage" => {
                        let usage = client.get_usage(session.id).await?;

                        println!("usage");
                        println!(
                            "  input {:.1}k  output {:.1}k  cache read {:.1}k  cache write {:.1}k",
                            usage.input_tokens as f64 / 1000.0,
                            usage.output_tokens as f64 / 1000.0,
                            usage.cache_read_tokens as f64 / 1000.0,
                            usage.cache_write_tokens as f64 / 1000.0,
                        );
                    }
                    _ if line.starts_with('/') => {
                        println!("unknown command: {line}");
                    }
                    _ => {
                        // Posting a message only starts the agent turn; output arrives later via SSE.
                        client
                            .post_message(session.id, line, &attached_files)
                            .await?;

                        attached_files.clear();

                        // Reconstruct the prompt to show the files still attached for the next message.
                        prompt = format!(
                            "\n\x1b[90m{} | files {}\x1b[0m\n{}",
                            session.id,
                            attached_files.len(),
                            PROMPT
                        );

                        // The event stream is shared by all sessions, so keep only this turn's session.
                        wait_events(&mut events, &mut event_buffer, session.id).await?;
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C
                println!("^C");
                break;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D
                break;
            }
            Err(e) => {
                return Err(AgentCliError::Readline(e.to_string()));
            }
        }
    }

    Ok(())
}

async fn wait_events(
    events: &mut reqwest::Response,
    event_buffer: &mut String,
    session_id: Uuid,
) -> Result<(), AgentCliError> {
    let current_session = session_id.to_string();

    let mut spinner: Option<ProgressBar> = None;

    'turn: while let Some(chunk) = events.chunk().await? {
        event_buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(index) = event_buffer.find("\n\n") {
            let raw_event = event_buffer[..index].to_string();
            *event_buffer = event_buffer[index + 2..].to_string();

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

            let Ok(data) = serde_json::from_str::<Value>(&event_data) else {
                continue;
            };

            if data.get("session_id").and_then(|v| v.as_str()) != Some(current_session.as_str()) {
                continue;
            }

            match event_name {
                "llm_started" if spinner.is_none() => {
                    let progress =
                        ProgressBar::with_draw_target(None, ProgressDrawTarget::stdout());
                    progress.set_style(
                        ProgressStyle::with_template("{spinner} {msg}")
                            .expect("spinner template should be valid")
                            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
                    );
                    progress.set_message("Figuring ...");
                    progress.enable_steady_tick(Duration::from_millis(120));

                    spinner = Some(progress);
                }
                "llm_finished" => {
                    if let Some(progress) = spinner.take() {
                        progress.finish_and_clear();
                    }
                }
                "llm_usage_recorded" => {
                    let usage = data.get("usage").unwrap_or(&Value::Null);

                    let input_tokens = usage
                        .get("input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    let output_tokens = usage
                        .get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);

                    println!(
                        "\x1b[90mtoken input={:.1}k output={:.1}k\x1b[0m",
                        input_tokens as f64 / 1000.0,
                        output_tokens as f64 / 1000.0,
                    );
                }
                "assistant_message_created" => {
                    // Stop the spinner
                    if let Some(progress) = spinner.take() {
                        progress.finish_and_clear();
                    }
                    if let Some(content) = data.get("content").and_then(|v| v.as_str()) {
                        // Use termimad to print the assistant message content with markdown support.
                        print_text(content);
                    }
                }
                "tool_call_started" => {
                    // Stop the spinner
                    if let Some(progress) = spinner.take() {
                        progress.finish_and_clear();
                    }
                    let tool_name = data
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool");
                    println!("[tool call] {tool_name}");

                    if let Some(arguments) = data.get("arguments") {
                        let pretty = serde_json::to_string_pretty(arguments)
                            .unwrap_or_else(|_| arguments.to_string());

                        let arguments = if pretty.chars().count() > MAX_CHARS {
                            let truncated = pretty.chars().take(MAX_CHARS).collect::<String>();

                            format!("{truncated}\n... (truncated)")
                        } else {
                            pretty
                        };

                        println!("[tool call]\n{arguments}");
                    }
                }
                "tool_call_finished" => {
                    let tool_name = data
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool");
                    let status = data
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    println!("[tool call] {tool_name}: {status}");

                    if let Some(output) = data.get("output") {
                        let pretty = serde_json::to_string_pretty(output)
                            .unwrap_or_else(|_| output.to_string());

                        let output = if pretty.chars().count() > MAX_CHARS {
                            let truncated = pretty.chars().take(MAX_CHARS).collect::<String>();

                            format!("{truncated}\n... (truncated)")
                        } else {
                            pretty
                        };

                        println!("[tool call output]\n{output}");
                    }
                }
                "tool_call_approval_requested" => {
                    // Stop the spinner
                    if let Some(progress) = spinner.take() {
                        progress.finish_and_clear();
                    }
                    let tool_name = data
                        .get("tool_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("tool");

                    println!("[approval requested] {tool_name}");

                    if let Some(arguments) = data.get("arguments") {
                        let pretty = serde_json::to_string_pretty(arguments)
                            .unwrap_or_else(|_| arguments.to_string());

                        let arguments = if pretty.chars().count() > MAX_CHARS {
                            let truncated = pretty.chars().take(MAX_CHARS).collect::<String>();

                            format!("{truncated}\n... (truncated)")
                        } else {
                            pretty
                        };

                        println!("[tool arguments]\n{arguments}");
                    }

                    println!("Run /approve or /deny.");
                    break 'turn;
                }
                "agent_turn_completed" => {
                    // Stop waiting once this turn completes.
                    break 'turn;
                }
                "agent_turn_failed" => {
                    // Stop the spinner
                    if let Some(progress) = spinner.take() {
                        progress.finish_and_clear();
                    }

                    let reason = data
                        .get("reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("agent turn failed");

                    println!("\x1b[90m[agent stopped] {reason}\x1b[0m");

                    break 'turn;
                }
                _ => {}
            }
        }
    }

    Ok(())
}
