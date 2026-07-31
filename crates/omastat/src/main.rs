use anyhow::Result;
use clap::Parser;
use omastat::{
    cli::{self, Cli, Commands},
    config::Config,
    steam::SteamResolver,
    storage::Storage,
    tui,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref())?;
    let storage = Storage::open(cli.database.as_deref(), &config)?;
    let mut steam = SteamResolver::default();

    match cli.command {
        Commands::Today => {
            cli::print_report(
                "Today",
                steam.resolve_totals(storage.totals_for_today()?),
                cli.json,
            )?;
        }
        Commands::Week => {
            cli::print_report(
                "This Week",
                steam.resolve_totals(storage.totals_for_week()?),
                cli.json,
            )?;
        }
        Commands::Apps => {
            cli::print_report(
                "All Time",
                steam.resolve_totals(storage.totals_all_time()?),
                cli.json,
            )?;
        }
        Commands::Range { from, to } => {
            let rows = storage.totals_for_date_range(&from, &to)?;
            cli::print_report(
                &format!("{from} through {to}"),
                steam.resolve_totals(rows),
                cli.json,
            )?;
        }
        Commands::Tui => {
            tui::run(storage)?;
        }
        Commands::Doctor => {
            cli::doctor(&config, &storage).await?;
        }
    }

    Ok(())
}
