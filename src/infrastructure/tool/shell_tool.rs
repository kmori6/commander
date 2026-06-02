use crate::domain::error::tool_error::ToolError;
use crate::domain::model::tool_call::ToolPermissionMode;
use crate::domain::port::tool::Tool;
use crate::infrastructure::process::docker_sandbox_runner::DockerSandboxRunner;
use crate::infrastructure::process::process_types::ProcessRequest;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_TIMEOUT_SECONDS: u64 = 120;
const MAX_TIMEOUT_SECONDS: u64 = 600;
const MAX_OUTPUT_BYTES: usize = 32_000;

#[derive(Clone)]
pub struct ShellTool {
    workspace_root: PathBuf,
    sandbox_runner: DockerSandboxRunner,
}

impl ShellTool {
    pub fn new(workspace_root: PathBuf, env_file: PathBuf, image: String) -> Self {
        Self {
            workspace_root: workspace_root.clone(),
            sandbox_runner: DockerSandboxRunner::new(
                workspace_root,
                env_file,
                image,
                MAX_OUTPUT_BYTES,
            ),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ShellArguments {
    command: String,
    timeout: Option<u64>,
    cwd: Option<String>,
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Run a non-interactive shell command in the Docker sandbox and return stdout, stderr, and exit_code."
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
                    "description": "Non-interactive shell command to execute."
                },
                "cwd": {
                    "type": "string",
                    "description": "Workspace-relative working directory; omit to run from the workspace root."
                },
                "timeout": {
                    "type": "integer",
                    "description": "Maximum runtime in seconds. Default: 120, max: 600.",
                    "minimum": 1,
                    "maximum": MAX_TIMEOUT_SECONDS
                }
            },
            "required": ["command"],
            "additionalProperties": false
        })
    }

    async fn execute(&self, arguments: Value) -> Result<Value, ToolError> {
        let args: ShellArguments = serde_json::from_value(arguments)
            .map_err(|err| ToolError::InvalidArguments(err.to_string()))?;

        let command = args.command.trim();
        if command.is_empty() {
            return Err(ToolError::InvalidArguments(
                "command must not be empty".to_string(),
            ));
        }

        validate_hard_block(command)?;

        let timeout_seconds = args.timeout.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
        if timeout_seconds == 0 || timeout_seconds > MAX_TIMEOUT_SECONDS {
            return Err(ToolError::InvalidArguments(format!(
                "timeout must be between 1 and {MAX_TIMEOUT_SECONDS} seconds"
            )));
        }

        let request = ProcessRequest {
            command: command.to_string(),
            timeout: Duration::from_secs(timeout_seconds),
            cwd: resolve_workspace_cwd(&self.workspace_root, args.cwd.as_deref())?,
        };

        let output = self
            .sandbox_runner
            .run_shell(request)
            .await
            .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

        Ok(json!({
            "exit_code": output.exit_code,
            "stdout": output.stdout,
            "stderr": output.stderr,
            "truncated": output.truncated
        }))
    }
}

fn validate_hard_block(command: &str) -> Result<(), ToolError> {
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
        return Err(ToolError::ExecutionFailed(
            "command is blocked by shell safety policy".to_string(),
        ));
    }

    for word in ["sudo", "sudoedit", "su", "doas", "pkexec"] {
        if contains_shell_word(&normalized, word) {
            return Err(ToolError::ExecutionFailed(
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
) -> Result<Option<PathBuf>, ToolError> {
    let Some(cwd) = cwd.map(str::trim).filter(|cwd| !cwd.is_empty()) else {
        return Ok(None);
    };

    let path = Path::new(cwd);

    if path.is_absolute() {
        return Err(ToolError::InvalidArguments(
            "cwd must be relative to the workspace root".to_string(),
        ));
    }

    let candidate = workspace_root.join(path);

    let workspace_root = workspace_root
        .canonicalize()
        .map_err(|err| ToolError::ExecutionFailed(err.to_string()))?;

    let candidate = candidate.canonicalize().map_err(|err| {
        ToolError::InvalidArguments(format!(
            "cwd must be an existing directory inside the workspace: {err}"
        ))
    })?;

    if !candidate.starts_with(&workspace_root) {
        return Err(ToolError::InvalidArguments(
            "cwd must stay inside the workspace root".to_string(),
        ));
    }

    if !candidate.is_dir() {
        return Err(ToolError::InvalidArguments(
            "cwd must be a directory".to_string(),
        ));
    }

    Ok(Some(candidate))
}
