pub mod agent_cli;
pub mod research_cli;
pub mod survey_cli;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Agent,
    Research,
    /// Read and summarize an academic paper from a PDF file or URL
    Survey {
        /// Path to a PDF file or URL (e.g. https://arxiv.org/pdf/...)
        source: String,
        /// Output path for the markdown report (default: outputs/survey/{timestamp}.md)
        #[arg(long, short)]
        output: Option<PathBuf>,
    },
}
