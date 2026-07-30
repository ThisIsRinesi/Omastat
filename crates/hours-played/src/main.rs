mod cli;
mod config;
mod hyprland;
mod storage;
mod tracker;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use config::Config;
use storage::Storage;
use tracker::Tracker;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref())?;
    let storage = Storage::open(cli.database.as_deref(), &config)?;

    match cli.command {
        Commands::Daemon => {
            let mut tracker = Tracker::new(storage, config);
            tracker.run().await?;
        }
        Commands::Today => {
            cli::print_report("Today", storage.totals_for_today()?);
        }
        Commands::Week => {
            cli::print_report("This Week", storage.totals_for_week()?);
        }
        Commands::Apps => {
            cli::print_report("All Time", storage.totals_all_time()?);
        }
        Commands::Doctor => {
            cli::doctor(&config, &storage).await?;
        }
    }

    Ok(())
}
