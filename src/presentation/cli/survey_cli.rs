use crate::application::usecase::survey_usecase::{RunSurveyInput, SurveyUsecase};
use crate::domain::port::llm_provider::LlmProvider;
use crate::presentation::error::agent_cli_error::AgentCliError;
use chrono::Local;
use std::fs;
use std::path::PathBuf;

const SURVEY_OUTPUT_DIR: &str = "outputs/survey";

pub async fn run<L: LlmProvider>(
    usecase: &SurveyUsecase<L>,
    source: &str,
    output: Option<PathBuf>,
) -> Result<(), AgentCliError> {
    println!("Survey CLI");
    println!("source: {}", source);

    let result = usecase
        .run(RunSurveyInput {
            source: source.to_string(),
        })
        .await?;

    println!("{}", result.report);

    let saved_path = save_markdown_report(&result.report, output)?;
    println!("saved report: {}", saved_path.display());

    Ok(())
}

fn save_markdown_report(markdown: &str, output: Option<PathBuf>) -> Result<PathBuf, AgentCliError> {
    let path = match output {
        Some(p) => p,
        None => {
            let output_dir = PathBuf::from(SURVEY_OUTPUT_DIR);
            fs::create_dir_all(&output_dir)?;
            let filename = format!("{}.md", Local::now().format("%Y-%m-%d_%H%M%S"));
            output_dir.join(filename)
        }
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, markdown)?;

    Ok(path)
}
