use crate::application::usecase::digest_usecase::{DigestUsecase, RunDigestInput};
use crate::domain::port::llm_provider::LlmProvider;
use crate::presentation::error::agent_cli_error::AgentCliError;
use chrono::Local;
use std::fs;
use std::path::PathBuf;

const DIGEST_OUTPUT_DIR: &str = "outputs/digest";

pub async fn run<L: LlmProvider>(
    usecase: &DigestUsecase<L>,
    output: Option<PathBuf>,
) -> Result<(), AgentCliError> {
    let date = Local::now().format("%Y-%m-%d").to_string();

    let output_path =
        output.unwrap_or_else(|| PathBuf::from(format!("{DIGEST_OUTPUT_DIR}/{date}.md")));

    let result = usecase.run(RunDigestInput { date: date.clone() }).await?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output_path, &result.report)?;

    println!("Digest saved to: {}", output_path.display());

    Ok(())
}
