use crate::infrastructure::error::process_runner_error::ProcessRunnerError;
use crate::infrastructure::process::process_types::{ProcessOutput, ProcessRequest};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use uuid::Uuid;

const CONTAINER_PREFIX: &str = "commander-shell-";

#[derive(Debug, Clone)]
pub struct DockerSandboxRunner {
    workspace_root: PathBuf,
    env_file: PathBuf,
    image: String,
    max_output_bytes: usize,
}

#[derive(Debug)]
struct DockerCommandOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug)]
enum DockerCommandError {
    TimedOut,
    ExecutionFailed(String),
}

impl DockerSandboxRunner {
    pub fn new(
        workspace_root: PathBuf,
        env_file: PathBuf,
        image: String,
        max_output_bytes: usize,
    ) -> Self {
        let workspace_root = workspace_root.canonicalize().unwrap_or(workspace_root);

        Self {
            workspace_root,
            env_file,
            image,
            max_output_bytes,
        }
    }

    pub async fn run_shell(
        &self,
        request: ProcessRequest,
    ) -> Result<ProcessOutput, ProcessRunnerError> {
        let container_cwd = self
            .container_cwd(request.cwd.as_deref())
            .map_err(ProcessRunnerError::ExecutionFailed)?;

        let container_name = format!("{CONTAINER_PREFIX}{}", Uuid::new_v4().simple());

        let mut args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--name".to_string(),
            container_name.clone(),
        ];

        args.extend(self.base_run_args(&container_cwd));
        args.push(self.image.clone());
        args.push("/bin/sh".to_string());
        args.push("-lc".to_string());
        args.push(request.command);

        let output = match run_docker(args, request.timeout).await {
            Ok(output) => output,
            Err(DockerCommandError::TimedOut) => {
                let _ = run_docker(
                    vec!["rm".to_string(), "-f".to_string(), container_name],
                    Duration::from_secs(10),
                )
                .await;

                return Err(ProcessRunnerError::TimedOut {
                    seconds: request.timeout.as_secs(),
                });
            }
            Err(DockerCommandError::ExecutionFailed(err)) => {
                return Err(ProcessRunnerError::ExecutionFailed(err));
            }
        };

        let (stdout, stdout_truncated) = truncate_text(&output.stdout, self.max_output_bytes);
        let (stderr, stderr_truncated) = truncate_text(&output.stderr, self.max_output_bytes);

        Ok(ProcessOutput {
            exit_code: output.exit_code,
            stdout,
            stderr,
            truncated: stdout_truncated || stderr_truncated,
        })
    }

    fn base_run_args(&self, container_cwd: &str) -> Vec<String> {
        let mut args = vec![
            "--read-only".to_string(),
            "--tmpfs".to_string(),
            "/tmp".to_string(),
            "--tmpfs".to_string(),
            "/var/tmp".to_string(),
            "--cap-drop".to_string(),
            "ALL".to_string(),
            "--security-opt".to_string(),
            "no-new-privileges".to_string(),
            "-v".to_string(),
            format!("{}:/workspace", self.workspace_root.display()),
            "-w".to_string(),
            container_cwd.to_string(),
            "-e".to_string(),
            "HOME=/workspace".to_string(),
            "-e".to_string(),
            "LANG=C.UTF-8".to_string(),
            "-e".to_string(),
            "LC_ALL=C.UTF-8".to_string(),
        ];

        if self.env_file.exists() {
            args.push("--env-file".to_string());
            args.push(self.env_file.display().to_string());
        }

        if let Some(user) = workspace_owner_user(&self.workspace_root) {
            args.push("--user".to_string());
            args.push(user);
        }

        args
    }

    fn container_cwd(&self, cwd: Option<&Path>) -> Result<String, String> {
        let host_cwd = cwd.unwrap_or(&self.workspace_root);
        let rel = host_cwd
            .strip_prefix(&self.workspace_root)
            .map_err(|err| err.to_string())?;

        if rel.as_os_str().is_empty() {
            Ok("/workspace".to_string())
        } else {
            Ok(format!(
                "/workspace/{}",
                rel.to_string_lossy().replace('\\', "/")
            ))
        }
    }
}

async fn run_docker(
    args: Vec<String>,
    run_timeout: Duration,
) -> Result<DockerCommandOutput, DockerCommandError> {
    let output = timeout(
        run_timeout,
        Command::new("docker")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| DockerCommandError::TimedOut)?
    .map_err(|err| DockerCommandError::ExecutionFailed(err.to_string()))?;

    Ok(DockerCommandOutput {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
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

#[cfg(unix)]
fn workspace_owner_user(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(format!("{}:{}", metadata.uid(), metadata.gid()))
}
