use clap::Parser;
use dotenvy::dotenv;
use log::info;

use commander::presentation::channel::slack;
use commander::presentation::cli::{Cli, Commands, chat_cli, serve_cli};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
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
        Commands::Slack { base_url } => {
            info!("Starting Slack channel...");
            slack::run(base_url).await?;
        }
    }

    Ok(())
}
