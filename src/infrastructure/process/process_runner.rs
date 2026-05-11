use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use crate::infrastructure::error::process_runner_error::ProcessRunnerError;

#[derive(Debug, Clone)]
pub struct ProcessRequest {
    pub command: String,
    pub timeout: Duration,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}

#[derive(Debug, Clone)]
pub struct ProcessRunner {
    workspace_root: PathBuf,
    max_output_bytes: usize,
}

impl ProcessRunner {
    pub fn new(workspace_root: PathBuf, max_output_bytes: usize) -> Self {
        Self {
            workspace_root,
            max_output_bytes,
        }
    }

    pub async fn run_shell(
        &self,
        request: ProcessRequest,
    ) -> Result<ProcessOutput, ProcessRunnerError> {
        let timeout_seconds = request.timeout.as_secs();

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());

        let cwd = request.cwd.as_ref().unwrap_or(&self.workspace_root);

        let output = timeout(
            request.timeout,
            Command::new(shell)
                .arg("-lc")
                .arg(&request.command)
                .current_dir(cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| ProcessRunnerError::TimedOut {
            seconds: timeout_seconds,
        })?
        .map_err(|err| ProcessRunnerError::ExecutionFailed(err.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let (stdout, stdout_truncated) = truncate_text(&stdout, self.max_output_bytes);
        let (stderr, stderr_truncated) = truncate_text(&stderr, self.max_output_bytes);

        Ok(ProcessOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout,
            stderr,
            truncated: stdout_truncated || stderr_truncated,
        })
    }
}

fn truncate_text(text: &str, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_string(), false);
    }

    let mut end = max_bytes;

    while !text.is_char_boundary(end) {
        end -= 1;
    }

    (format!("{}\n... [truncated]", &text[..end]), true)
}
