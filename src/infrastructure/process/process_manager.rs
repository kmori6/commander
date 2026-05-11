use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::sync::{Mutex, Notify, RwLock, oneshot};
use tokio::time::{Duration, timeout};

use crate::infrastructure::error::process_manager_error::ProcessManagerError;
use crate::infrastructure::process::process_log_buffer::ProcessLogBuffer;
use crate::infrastructure::process::process_runner::ProcessRequest;
use crate::infrastructure::process::process_types::{
    BackgroundProcessStarted, ProcessId, ProcessLogs, ProcessSnapshot, ProcessStatus,
};

#[derive(Clone)]
pub struct ProcessManager {
    workspace_root: PathBuf,
    max_log_bytes: usize,
    processes: Arc<RwLock<HashMap<String, Arc<ManagedProcess>>>>,
}

struct ManagedProcess {
    state: RwLock<ManagedProcessState>,
    logs: Mutex<ProcessLogBuffer>,
    kill_sender: Mutex<Option<oneshot::Sender<()>>>,
    finished: Notify,
}

#[derive(Debug, Clone)]
struct ManagedProcessState {
    process_id: ProcessId,
    status: ProcessStatus,
    command: String,
    cwd: PathBuf,
    pid: Option<u32>,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
    exit_code: Option<i32>,
}

impl ProcessManager {
    pub fn new(workspace_root: PathBuf, max_log_bytes: usize) -> Self {
        Self {
            workspace_root,
            max_log_bytes,
            processes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_shell(
        &self,
        request: ProcessRequest,
    ) -> Result<BackgroundProcessStarted, ProcessManagerError> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let cwd = request
            .cwd
            .clone()
            .unwrap_or_else(|| self.workspace_root.clone());

        let mut child = Command::new(shell)
            .arg("-lc")
            .arg(&request.command)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| ProcessManagerError::StartFailed(err.to_string()))?;

        let process_id = ProcessId::new();
        let pid = child.id();
        let started_at = Utc::now();
        let (kill_tx, kill_rx) = oneshot::channel();

        let process = Arc::new(ManagedProcess {
            state: RwLock::new(ManagedProcessState {
                process_id: process_id.clone(),
                status: ProcessStatus::Running,
                command: request.command.clone(),
                cwd: cwd.clone(),
                pid,
                started_at,
                finished_at: None,
                exit_code: None,
            }),
            logs: Mutex::new(ProcessLogBuffer::new(self.max_log_bytes)),
            kill_sender: Mutex::new(Some(kill_tx)),
            finished: Notify::new(),
        });

        if let Some(stdout) = child.stdout.take() {
            spawn_stdout_reader(stdout, process.clone());
        }

        if let Some(stderr) = child.stderr.take() {
            spawn_stderr_reader(stderr, process.clone());
        }

        self.processes
            .write()
            .await
            .insert(process_id.as_str().to_string(), process.clone());

        tokio::spawn(supervise_process(child, process, request.timeout, kill_rx));

        Ok(BackgroundProcessStarted {
            process_id,
            status: ProcessStatus::Running,
            command: request.command,
            cwd,
            pid,
            started_at,
        })
    }

    pub async fn status(&self, process_id: &str) -> Result<ProcessSnapshot, ProcessManagerError> {
        let process = self.get_process(process_id).await?;
        let state = process.state.read().await;

        Ok(snapshot_from_state(&state))
    }

    pub async fn logs(&self, process_id: &str) -> Result<ProcessLogs, ProcessManagerError> {
        let process = self.get_process(process_id).await?;
        let state = process.state.read().await;
        let logs = process.logs.lock().await;

        Ok(ProcessLogs {
            process_id: state.process_id.clone(),
            status: state.status,
            stdout: logs.stdout().to_string(),
            stderr: logs.stderr().to_string(),
            truncated: logs.truncated(),
        })
    }

    pub async fn kill(&self, process_id: &str) -> Result<ProcessSnapshot, ProcessManagerError> {
        let process = self.get_process(process_id).await?;
        let notified = process.finished.notified();

        if let Some(sender) = process.kill_sender.lock().await.take() {
            let _ = sender.send(());
            let _ = timeout(Duration::from_secs(5), notified).await;
        }

        self.status(process_id).await
    }

    async fn get_process(
        &self,
        process_id: &str,
    ) -> Result<Arc<ManagedProcess>, ProcessManagerError> {
        self.processes
            .read()
            .await
            .get(process_id)
            .cloned()
            .ok_or_else(|| ProcessManagerError::NotFound(process_id.to_string()))
    }
}

fn spawn_stdout_reader(stdout: ChildStdout, process: Arc<ManagedProcess>) {
    tokio::spawn(async move {
        let mut stdout = stdout;
        let mut buffer = [0_u8; 4096];

        loop {
            match stdout.read(&mut buffer).await {
                Ok(0) => break,
                Ok(bytes_read) => {
                    let text = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let mut logs = process.logs.lock().await;
                    logs.append_stdout(&text);
                }
                Err(err) => {
                    let mut logs = process.logs.lock().await;
                    logs.append_stderr(&format!(
                        "\n[process_manager] failed to read stdout: {err}\n"
                    ));
                    break;
                }
            }
        }
    });
}

fn spawn_stderr_reader(stderr: ChildStderr, process: Arc<ManagedProcess>) {
    tokio::spawn(async move {
        let mut stderr = stderr;
        let mut buffer = [0_u8; 4096];

        loop {
            match stderr.read(&mut buffer).await {
                Ok(0) => break,
                Ok(bytes_read) => {
                    let text = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let mut logs = process.logs.lock().await;
                    logs.append_stderr(&text);
                }
                Err(err) => {
                    let mut logs = process.logs.lock().await;
                    logs.append_stderr(&format!(
                        "\n[process_manager] failed to read stderr: {err}\n"
                    ));
                    break;
                }
            }
        }
    });
}

async fn supervise_process(
    mut child: Child,
    process: Arc<ManagedProcess>,
    run_timeout: Duration,
    kill_rx: oneshot::Receiver<()>,
) {
    let (status, exit_code) = tokio::select! {
        wait_result = timeout(run_timeout, child.wait()) => {
            match wait_result {
                Ok(Ok(exit_status)) => {
                    (ProcessStatus::Exited, exit_status.code())
                }
                Ok(Err(err)) => {
                    append_process_error(&process, format!("failed to wait process: {err}")).await;
                    (ProcessStatus::Failed, None)
                }
                Err(_) => {
                    let _ = child.start_kill();
                    let _ = timeout(Duration::from_secs(5), child.wait()).await;
                    (ProcessStatus::TimedOut, None)
                }
            }
        }
        _ = kill_rx => {
            match child.start_kill() {
                Ok(()) => {
                    let exit_code = match timeout(Duration::from_secs(5), child.wait()).await {
                        Ok(Ok(exit_status)) => exit_status.code(),
                        _ => None,
                    };

                    (ProcessStatus::Killed, exit_code)
                }
                Err(err) => {
                    append_process_error(&process, format!("failed to kill process: {err}")).await;
                    (ProcessStatus::Failed, None)
                }
            }
        }
    };

    {
        let mut state = process.state.write().await;
        state.status = status;
        state.exit_code = exit_code;
        state.finished_at = Some(Utc::now());
    }

    let _ = process.kill_sender.lock().await.take();
    process.finished.notify_waiters();
}

async fn append_process_error(process: &ManagedProcess, message: String) {
    let mut logs = process.logs.lock().await;
    logs.append_stderr(&format!("\n[process_manager] {message}\n"));
}

fn snapshot_from_state(state: &ManagedProcessState) -> ProcessSnapshot {
    ProcessSnapshot {
        process_id: state.process_id.clone(),
        status: state.status,
        command: state.command.clone(),
        cwd: state.cwd.clone(),
        pid: state.pid,
        started_at: state.started_at,
        finished_at: state.finished_at,
        exit_code: state.exit_code,
    }
}
