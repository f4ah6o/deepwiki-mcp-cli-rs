mod cli;
mod mcp;
mod commands;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let config = mcp::McpConfig {
        url: cli.url,
        token: cli.token,
        timeout_secs: cli.timeout,
        output_format: cli.output,
    };

    match cli.command {
        Commands::Ask { repo, question } => {
            commands::ask::run(&config, &repo, &question).await?;
        }
        Commands::Read { repo, topic } => {
            commands::read::run(&config, &repo, topic.as_deref()).await?;
        }
        Commands::Check { repo } => {
            commands::check::run(&config, &repo).await?;
        }
        Commands::Search { repo, query } => {
            commands::search::run(&config, &repo, &query).await?;
        }
    }

    Ok(())
}
