use clap::Parser;
use dotenvy::dotenv;
use log::info;

use commander::application::usecase::{
    digest_usecase::DigestUsecase, research_usecase::ResearchUsecase, survey_usecase::SurveyUsecase,
};
use commander::domain::service::deep_research_service::DeepResearchService;
use commander::infrastructure::{
    llm::bedrock_llm_provider::BedrockLlmProvider,
    search::tavily_search_provider::TavilySearchProvider,
};
use commander::presentation::{
    cli::{Cli, Commands, chat_cli, digest_cli, research_cli, serve_cli, survey_cli},
    error::agent_cli_error::AgentCliError,
};

#[tokio::main]
async fn main() -> Result<(), AgentCliError> {
    dotenv().ok();
    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { addr } => {
            info!("Starting server on {}", addr);
            serve_cli::run(addr).await?;
        }
        Commands::Chat {
            base_url,
            session_id,
        } => {
            info!("Starting chat CLI...");
            chat_cli::run(base_url, session_id).await?;
        }
        Commands::Research => {
            info!("Starting research...");
            let llm_client = BedrockLlmProvider::from_default_config().await;
            let search_provider = TavilySearchProvider::from_env()?;
            let usecase =
                ResearchUsecase::new(DeepResearchService::new(llm_client, search_provider));
            research_cli::run(&usecase).await?;
        }
        Commands::Survey { source, output } => {
            info!("Starting survey...");
            let llm_client = BedrockLlmProvider::from_default_config().await;
            let usecase = SurveyUsecase::new(llm_client);
            survey_cli::run(&usecase, &source, output).await?;
        }
        Commands::Digest { date, output } => {
            info!("Starting digest...");
            let llm_client = BedrockLlmProvider::from_default_config().await;
            let usecase = DigestUsecase::new(llm_client);
            digest_cli::run(&usecase, date, output).await?;
        }
    }

    Ok(())
}
