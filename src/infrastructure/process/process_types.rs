use chrono::{DateTime, Utc};
use serde::Serialize;
use std::fmt;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ProcessId(String);

impl ProcessId {
    pub fn new() -> Self {
        Self(format!("proc_{}", Uuid::new_v4().simple()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ProcessId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ProcessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Running,
    Exited,
    Failed,
    TimedOut,
    Killed,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackgroundProcessStarted {
    pub process_id: ProcessId,
    pub status: ProcessStatus,
    pub command: String,
    pub cwd: PathBuf,
    pub pid: Option<u32>,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessSnapshot {
    pub process_id: ProcessId,
    pub status: ProcessStatus,
    pub command: String,
    pub cwd: PathBuf,
    pub pid: Option<u32>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessLogs {
    pub process_id: ProcessId,
    pub status: ProcessStatus,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ShellProcessOutput {
    Foreground {
        exit_code: i32,
        stdout: String,
        stderr: String,
        truncated: bool,
    },
    Background {
        process_id: ProcessId,
        status: ProcessStatus,
        command: String,
        cwd: PathBuf,
        pid: Option<u32>,
        started_at: DateTime<Utc>,
    },
}
