pub mod chat_cli;
pub mod serve_cli;

use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Parser, Debug)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Serve {
        #[arg(long, default_value = "0.0.0.0:3000")]
        addr: SocketAddr,
    },
    Chat {
        #[arg(long, default_value = "http://localhost:3000")]
        base_url: String,

        #[arg(long)]
        session_id: Option<Uuid>,
    },
    Slack {
        #[arg(long, default_value = "http://localhost:3000")]
        base_url: String,
    },
}
