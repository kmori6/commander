use clap::Parser;
use dotenvy::dotenv;
use log::info;
use work_agent::{
    application::usecase::agent_usecase::AgentUsecase,
    infrastructure::llm::bedrock_llm_client::BedrockLlmClient,
    presentation::{
        cli::{Cli, Commands, agent_cli},
        error::agent_cli_error::AgentCliError,
    },
};

#[tokio::main]
async fn main() -> Result<(), AgentCliError> {
    dotenv().ok();

    env_logger::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Agent => {
            info!("Starting agent...");
            let llm_client = BedrockLlmClient::from_default_config().await?;
            let usecase = AgentUsecase::new(llm_client);
            agent_cli::run(&usecase).await?;
        }
    }

    Ok(())
}
