use std::path::PathBuf;
use std::time::Duration;

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
