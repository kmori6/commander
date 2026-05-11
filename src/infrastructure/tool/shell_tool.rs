use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::domain::error::tool_executor_error::ToolExecutorError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;
use crate::infrastructure::process::process_manager::ProcessManager;
use crate::infrastructure::process::process_runner::{ProcessRequest, ProcessRunner};

const DEFAULT_FOREGROUND_TIMEOUT_SECONDS: u64 = 120;
const DEFAULT_BACKGROUND_TIMEOUT_SECONDS: u64 = 1800;
const MAX_FOREGROUND_TIMEOUT_SECONDS: u64 = 600;
const MAX_BACKGROUND_TIMEOUT_SECONDS: u64 = 3600;
const MAX_FOREGROUND_OUTPUT_BYTES: usize = 32_000;
const MAX_BACKGROUND_LOG_BYTES: usize = 64_000;

#[derive(Clone)]
pub struct ShellTool {
    workspace_root: PathBuf,
    process_runner: ProcessRunner,
    process_manager: ProcessManager,
}

impl ShellTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root: workspace_root.clone(),
            process_runner: ProcessRunner::new(workspace_root.clone(), MAX_FOREGROUND_OUTPUT_BYTES),
            process_manager: ProcessManager::new(workspace_root, MAX_BACKGROUND_LOG_BYTES),
        }
    }

    async fn execute_run(&self, args: ShellArguments) -> Result<Value, ToolExecutorError> {
        let command = required_command(&args)?;
        validate_hard_block(command)?;

        let background = args.background.unwrap_or(false);

        let default_timeout = if background {
            DEFAULT_BACKGROUND_TIMEOUT_SECONDS
        } else {
            DEFAULT_FOREGROUND_TIMEOUT_SECONDS
        };

        let max_timeout = if background {
            MAX_BACKGROUND_TIMEOUT_SECONDS
        } else {
            MAX_FOREGROUND_TIMEOUT_SECONDS
        };

        let timeout_seconds = args.timeout.unwrap_or(default_timeout);

        if timeout_seconds == 0 || timeout_seconds > max_timeout {
            return Err(ToolExecutorError::InvalidArguments(format!(
                "timeout must be between 1 and {max_timeout} seconds"
            )));
        }

        let cwd = resolve_workspace_cwd(&self.workspace_root, args.cwd.as_deref())?;

        let request = ProcessRequest {
            command: command.to_string(),
            timeout: Duration::from_secs(timeout_seconds),
            cwd,
        };

        if background {
            let started = self
                .process_manager
                .start_shell(request)
                .await
                .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

            return Ok(json!({
                "mode": "background",
                "process_id": started.process_id,
                "status": started.status,
                "command": started.command,
                "cwd": started.cwd,
                "pid": started.pid,
                "started_at": started.started_at
            }));
        }

        let output = self
            .process_runner
            .run_shell(request)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        Ok(json!({
            "mode": "foreground",
            "exit_code": output.exit_code,
            "stdout": output.stdout,
            "stderr": output.stderr,
            "truncated": output.truncated
        }))
    }

    async fn execute_status(&self, args: ShellArguments) -> Result<Value, ToolExecutorError> {
        let process_id = required_process_id(&args)?;

        let snapshot = self
            .process_manager
            .status(process_id)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        serde_json::to_value(snapshot)
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))
    }

    async fn execute_logs(&self, args: ShellArguments) -> Result<Value, ToolExecutorError> {
        let process_id = required_process_id(&args)?;

        let logs = self
            .process_manager
            .logs(process_id)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        serde_json::to_value(logs)
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))
    }

    async fn execute_kill(&self, args: ShellArguments) -> Result<Value, ToolExecutorError> {
        let process_id = required_process_id(&args)?;

        let snapshot = self
            .process_manager
            .kill(process_id)
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        serde_json::to_value(snapshot)
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ShellAction {
    Run,
    Status,
    Logs,
    Kill,
}

fn default_shell_action() -> ShellAction {
    ShellAction::Run
}

#[derive(Debug, Deserialize)]
struct ShellArguments {
    #[serde(default = "default_shell_action")]
    action: ShellAction,
    command: Option<String>,
    timeout: Option<u64>,
    cwd: Option<String>,
    background: Option<bool>,
    process_id: Option<String>,
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Run shell commands in the workspace. Use action=run with background=false for quick commands that should return stdout/stderr immediately. Use action=run with background=true for long-running commands, coding CLIs, and dev servers, then use action=status/logs/kill with the returned process_id."
    }

    fn default_permission(&self) -> ToolPermissionMode {
        ToolPermissionMode::Ask
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["run", "status", "logs", "kill"],
                    "description": "Required. run starts a command; status shows background process state; logs returns captured stdout/stderr; kill stops a background process."
                },
                "command": {
                    "type": "string",
                    "description": "Required when action=run. Non-interactive shell command to execute."
                },
                "process_id": {
                    "type": "string",
                    "description": "Required when action=status, action=logs, or action=kill. Use the process_id returned by action=run with background=true."
                },
                "background": {
                    "type": "boolean",
                    "description": "Only used when action=run. false waits and returns stdout/stderr/exit_code; true returns immediately with process_id. Use true for long-running commands, coding CLIs, and dev servers."
                },
                "cwd": {
                    "type": "string",
                    "description": "Only used when action=run. Workspace-relative working directory; omit to run from the workspace root."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Only used when action=run. Maximum runtime in seconds. Foreground default: 120, max: 600. Background default: 1800, max: 3600.",
                    "minimum": 1,
                    "maximum": MAX_BACKGROUND_TIMEOUT_SECONDS
                },
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolExecutorError> {
        let args: ShellArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolExecutorError::InvalidArguments(err.to_string()))?;

        match args.action {
            ShellAction::Run => self.execute_run(args).await,
            ShellAction::Status => self.execute_status(args).await,
            ShellAction::Logs => self.execute_logs(args).await,
            ShellAction::Kill => self.execute_kill(args).await,
        }
    }
}

fn validate_hard_block(command: &str) -> Result<(), ToolExecutorError> {
    let normalized = command.to_lowercase();

    let blocked_patterns = [
        "rm -rf /",
        "rm -fr /",
        "rm -rf /*",
        "rm -fr /*",
        "rm -rf ~",
        "rm -fr ~",
        "rm -rf $home",
        "rm -fr $home",
        "mkfs",
        "wipefs",
        "blkdiscard",
        "dd if=/dev/zero",
        "dd of=/dev/",
        "shutdown",
        "reboot",
        "poweroff",
        "halt",
        "/etc/shadow",
        "~/.ssh",
        "$home/.ssh",
        "/var/run/docker.sock",
        "docker.sock",
        "--privileged",
        "-v /:/",
        "--volume /:/",
    ];

    if blocked_patterns
        .iter()
        .any(|pattern| normalized.contains(pattern))
    {
        return Err(ToolExecutorError::ExecutionFailed(
            "command is blocked by shell safety policy".to_string(),
        ));
    }

    for word in ["sudo", "sudoedit", "su", "doas", "pkexec"] {
        if contains_shell_word(&normalized, word) {
            return Err(ToolExecutorError::ExecutionFailed(
                "command is blocked by shell safety policy".to_string(),
            ));
        }
    }

    Ok(())
}

fn contains_shell_word(command: &str, word: &str) -> bool {
    command
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .any(|part| part == word)
}

fn resolve_workspace_cwd(
    workspace_root: &Path,
    cwd: Option<&str>,
) -> Result<Option<PathBuf>, ToolExecutorError> {
    let Some(cwd) = cwd.map(str::trim).filter(|cwd| !cwd.is_empty()) else {
        return Ok(None);
    };

    let path = Path::new(cwd);

    if path.is_absolute() {
        return Err(ToolExecutorError::InvalidArguments(
            "cwd must be relative to the workspace root".to_string(),
        ));
    }

    let candidate = workspace_root.join(path);

    let workspace_root = workspace_root
        .canonicalize()
        .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

    let candidate = candidate.canonicalize().map_err(|err| {
        ToolExecutorError::InvalidArguments(format!(
            "cwd must be an existing directory inside the workspace: {err}"
        ))
    })?;

    if !candidate.starts_with(&workspace_root) {
        return Err(ToolExecutorError::InvalidArguments(
            "cwd must stay inside the workspace root".to_string(),
        ));
    }

    if !candidate.is_dir() {
        return Err(ToolExecutorError::InvalidArguments(
            "cwd must be a directory".to_string(),
        ));
    }

    Ok(Some(candidate))
}

fn required_command(args: &ShellArguments) -> Result<&str, ToolExecutorError> {
    args.command
        .as_deref()
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .ok_or_else(|| {
            ToolExecutorError::InvalidArguments("command is required when action=run".to_string())
        })
}

fn required_process_id(args: &ShellArguments) -> Result<&str, ToolExecutorError> {
    args.process_id
        .as_deref()
        .map(str::trim)
        .filter(|process_id| !process_id.is_empty())
        .ok_or_else(|| {
            ToolExecutorError::InvalidArguments(
                "process_id is required for this action".to_string(),
            )
        })
}
