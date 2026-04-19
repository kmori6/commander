use crate::application::usecase::agent_usecase::{
    AgentEvent, AgentUsecase, HandleAgentInput, HandleAgentOutput,
};
use crate::domain::model::chat_session::ChatSession;
use crate::domain::port::llm_provider::LlmProvider;
use crate::domain::repository::chat_message_repository::ChatMessageRepository;
use crate::domain::repository::chat_session_repository::ChatSessionRepository;
use crate::domain::service::agent_service::AgentProgressEvent;
use crate::presentation::error::agent_cli_error::AgentCliError;
use indicatif::{ProgressBar, ProgressStyle};
use reedline::{
    MouseClickMode, Prompt, PromptEditMode, PromptHistorySearch, PromptHistorySearchStatus,
    Reedline, Signal,
};
use serde_json::Value;
use std::borrow::Cow;
use std::time::Duration;
use termimad::print_text;
use uuid::Uuid;

const MAX_ARGUMENT_PREVIEW_CHARS: usize = 800;
const SESSION_LIST_LIMIT: usize = 10;

pub async fn run<L, S, M>(usecase: &AgentUsecase<L, S, M>) -> Result<(), AgentCliError>
where
    L: LlmProvider,
    S: ChatSessionRepository,
    M: ChatMessageRepository,
{
    println!("Agent CLI");
    println!("type /help for commands");

    let mut line_editor = build_line_editor();
    let prompt = AgentPrompt;
    let mut current_session = usecase.start_session().await?;
    print_current_session(current_session.id);

    loop {
        let Some(line) = read_command(&mut line_editor, &prompt)? else {
            println!();
            break;
        };

        let Some(command) = parse_command(line) else {
            continue;
        };

        match command {
            CliCommand::Help => print_help(),
            CliCommand::NewSession => {
                current_session = usecase.start_session().await?;
                print_current_session(current_session.id);
            }
            CliCommand::Sessions => {
                let sessions = usecase.list_sessions(SESSION_LIST_LIMIT).await?;
                print_sessions(&sessions, current_session.id);
            }
            CliCommand::Use(raw_id) => {
                if let Some(session) = switch_session(usecase, &raw_id).await? {
                    current_session = session;
                    print_current_session(current_session.id);
                }
            }
            CliCommand::Exit => break,
            CliCommand::Unknown(name) => println!("unknown command: {name}"),
            CliCommand::UserMessage(message) => {
                handle_user_message(usecase, current_session.id, message).await?;
            }
        }
    }

    Ok(())
}

struct AgentPrompt;

impl Prompt for AgentPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("commander")
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _prompt_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed(" > ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("... ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        let status = match history_search.status {
            PromptHistorySearchStatus::Passing => "history",
            PromptHistorySearchStatus::Failing => "history-failed",
        };

        format!("({status}: {}) ", history_search.term).into()
    }
}

#[derive(Debug)]
enum CliCommand {
    Help,
    NewSession,
    Sessions,
    Use(String),
    Exit,
    Unknown(String),
    UserMessage(String),
}

struct CliProgressReporter {
    spinner: Option<ProgressBar>,
}

impl CliProgressReporter {
    fn new() -> Self {
        Self { spinner: None }
    }

    fn handle(&mut self, event: AgentProgressEvent) {
        match event {
            AgentProgressEvent::LlmThinkingStarted => self.start_thinking(),
            AgentProgressEvent::LlmThinkingFinished => self.stop_thinking(),
            AgentProgressEvent::ToolCallRequested {
                call_id,
                tool_name,
                arguments,
            } => {
                self.stop_thinking();
                self.println(format!("[tool call] {tool_name} ({call_id})"));
                self.println(format_arguments(&arguments));
            }
            AgentProgressEvent::ToolExecutionFinished {
                call_id,
                tool_name,
                success,
            } => {
                self.stop_thinking();
                self.println(format!(
                    "[tool result] {tool_name} ({call_id}): {}",
                    if success { "success" } else { "failed" }
                ));
            }
        }
    }

    fn finish(&mut self) {
        self.stop_thinking();
    }

    fn start_thinking(&mut self) {
        if self.spinner.is_some() {
            return;
        }

        let spinner = ProgressBar::new_spinner();
        let style = ProgressStyle::with_template("{spinner} {msg}")
            .expect("spinner template should be valid")
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]);

        spinner.set_style(style);
        spinner.set_message("Thinking...");
        spinner.enable_steady_tick(Duration::from_millis(120));
        self.spinner = Some(spinner);
    }

    fn stop_thinking(&mut self) {
        if let Some(spinner) = self.spinner.take() {
            spinner.finish_and_clear();
        }
    }

    fn println(&self, message: impl Into<String>) {
        let message = message.into();

        if let Some(spinner) = &self.spinner {
            spinner.println(message);
        } else {
            println!("{message}");
        }
    }
}

fn build_line_editor() -> Reedline {
    Reedline::create().with_mouse_click(MouseClickMode::EnabledWithOsc133)
}

fn read_command(
    line_editor: &mut Reedline,
    prompt: &impl Prompt,
) -> Result<Option<String>, AgentCliError> {
    let signal = line_editor
        .read_line(prompt)
        .map_err(|err| AgentCliError::Readline(err.to_string()))?;

    match signal {
        Signal::Success(line) => Ok(Some(line)),
        Signal::CtrlC => {
            println!();
            Ok(Some(String::new()))
        }
        Signal::CtrlD => Ok(None),
        Signal::ExternalBreak(line) => Ok(Some(line)),
        other => Err(AgentCliError::Readline(format!(
            "unsupported reedline signal: {other:?}"
        ))),
    }
}

fn parse_command(line: String) -> Option<CliCommand> {
    let input = line.trim();

    if input.is_empty() {
        return None;
    }

    Some(match input {
        "/help" => CliCommand::Help,
        "/reset" | "/new" => CliCommand::NewSession,
        "/sessions" => CliCommand::Sessions,
        "/exit" | "/quit" => CliCommand::Exit,
        _ if input.starts_with("/use ") => {
            CliCommand::Use(input.trim_start_matches("/use ").trim().to_string())
        }
        _ if input.starts_with('/') => CliCommand::Unknown(input.to_string()),
        _ => CliCommand::UserMessage(input.to_string()),
    })
}

async fn switch_session<L, S, M>(
    usecase: &AgentUsecase<L, S, M>,
    raw_id: &str,
) -> Result<Option<ChatSession>, AgentCliError>
where
    L: LlmProvider,
    S: ChatSessionRepository,
    M: ChatMessageRepository,
{
    let Ok(session_id) = Uuid::parse_str(raw_id) else {
        println!("invalid session id: {raw_id}");
        return Ok(None);
    };

    match usecase.find_session(session_id).await? {
        Some(session) => Ok(Some(session)),
        None => {
            println!("session not found: {session_id}");
            Ok(None)
        }
    }
}

async fn handle_user_message<L, S, M>(
    usecase: &AgentUsecase<L, S, M>,
    session_id: Uuid,
    message: String,
) -> Result<(), AgentCliError>
where
    L: LlmProvider,
    S: ChatSessionRepository,
    M: ChatMessageRepository,
{
    let mut reporter = CliProgressReporter::new();

    let output = usecase
        .handle_with_progress(
            HandleAgentInput {
                session_id,
                user_input: message,
            },
            |event| reporter.handle(event),
        )
        .await?;

    reporter.finish();
    print_agent_output(output);

    Ok(())
}

fn print_agent_output(output: HandleAgentOutput) {
    for event in output.reply {
        match event {
            AgentEvent::AssistantMessage(message) => print_text(&message),
        }
    }
}

fn print_current_session(session_id: Uuid) {
    println!("session: {session_id}");
}

fn print_sessions(sessions: &[ChatSession], current_session_id: Uuid) {
    if sessions.is_empty() {
        println!("no sessions");
        return;
    }

    for session in sessions {
        let marker = if session.id == current_session_id {
            "*"
        } else {
            " "
        };
        println!(
            "{marker} {}  updated_at={}  created_at={}",
            session.id, session.updated_at, session.created_at
        );
    }
}

fn format_arguments(arguments: &Value) -> String {
    let pretty = serde_json::to_string_pretty(arguments).unwrap_or_else(|_| arguments.to_string());
    truncate_for_cli(pretty, MAX_ARGUMENT_PREVIEW_CHARS)
}

fn truncate_for_cli(text: String, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text;
    }

    let truncated: String = text.chars().take(max_chars).collect();
    format!("{truncated}\n... (truncated)")
}

fn print_help() {
    println!("/help      show help");
    println!("/new       start a new session");
    println!("/reset     alias of /new");
    println!("/sessions  show recent sessions");
    println!("/use <id>  switch to a session");
    println!("/exit      quit");
}
