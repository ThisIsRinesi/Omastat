use anyhow::Result;
use clap::Parser;
use omastat::{
    cli::{self, Cli, Commands},
    config::Config,
    export::{self, ExportOptions},
    steam::SteamResolver,
    storage::Storage,
    tui,
};
use std::fs;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref())?;
    let mut storage = Storage::open(cli.database.as_deref(), &config)?;
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
        Commands::Summary { days } => {
            let report = cli::summary_report(&storage, &mut steam, days)?;
            cli::print_summary(&report)?;
        }
        Commands::Export {
            lens,
            offset,
            output,
            title,
        } => {
            if let Some(parent) = output.parent()
                && !parent.as_os_str().is_empty()
            {
                fs::create_dir_all(parent)?;
            }
            let html = export::render_html(
                &storage,
                &mut steam,
                ExportOptions {
                    lens: lens.into(),
                    offset,
                    title,
                },
            )?;
            fs::write(&output, html)?;
            println!("Exported {}", output.display());
        }
        Commands::Tui => {
            tui::run(storage)?;
        }
        Commands::RepairTitles { dry_run } => {
            let repair = storage.repair_titles(&mut steam, dry_run)?;
            cli::print_title_repair(&repair, cli.json)?;
        }
        Commands::Doctor => {
            cli::doctor(&config, &storage).await?;
        }
    }

    Ok(())
}
