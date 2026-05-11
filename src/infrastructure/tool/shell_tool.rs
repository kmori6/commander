use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Duration;

use crate::domain::error::tool_executor_error::ToolExecutorError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;
use crate::infrastructure::process::process_runner::{ProcessRequest, ProcessRunner};

const DEFAULT_TIMEOUT_SECONDS: u64 = 120;
const MAX_TIMEOUT_SECONDS: u64 = 600;
const MAX_OUTPUT_BYTES: usize = 32_000;

#[derive(Debug, Clone)]
pub struct ShellTool {
    process_runner: ProcessRunner,
}

impl ShellTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            process_runner: ProcessRunner::new(workspace_root)
                .with_max_output_bytes(MAX_OUTPUT_BYTES),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ShellArguments {
    command: String,
    timeout: Option<u64>,
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Run a single non-interactive command from the workspace root. Use for build/test commands, Git commands, search utilities, scripts, and other CLI work."
    }

    fn default_permission(&self) -> ToolPermissionMode {
        ToolPermissionMode::Ask
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command line to execute. It starts in the workspace root."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Maximum runtime in seconds. Default: 120. Maximum: 600.",
                    "minimum": 1,
                    "maximum": MAX_TIMEOUT_SECONDS
                }
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolExecutorError> {
        let args: ShellArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolExecutorError::InvalidArguments(err.to_string()))?;

        let command = args.command.trim();

        if command.is_empty() {
            return Err(ToolExecutorError::InvalidArguments(
                "command must not be empty".to_string(),
            ));
        }

        validate_hard_block(command)?;

        let timeout_seconds = args.timeout.unwrap_or(DEFAULT_TIMEOUT_SECONDS);

        if timeout_seconds == 0 || timeout_seconds > MAX_TIMEOUT_SECONDS {
            return Err(ToolExecutorError::InvalidArguments(format!(
                "timeout must be between 1 and {MAX_TIMEOUT_SECONDS} seconds"
            )));
        }

        let output = self
            .process_runner
            .run_shell(ProcessRequest {
                command: command.to_string(),
                timeout: Duration::from_secs(timeout_seconds),
            })
            .await
            .map_err(|err| ToolExecutorError::ExecutionFailed(err.to_string()))?;

        Ok(json!({
            "exit_code": output.exit_code,
            "stdout": output.stdout,
            "stderr": output.stderr,
            "truncated": output.truncated
        }))
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
