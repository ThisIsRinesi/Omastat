use anyhow::Result;
use clap::Parser;
use omastat::{
    cli::{self, Cli, Commands, DataExportFormatArg},
    config::Config,
    export::{self, DataExportOptions, ExportOptions},
    steam::SteamResolver,
    storage::{Storage, StorageOpenMode},
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

    if let Commands::Doctor = &cli.command {
        cli::doctor(&config, cli.database.as_deref()).await?;
        return Ok(());
    }

    let storage_mode = match &cli.command {
        Commands::RepairTitles { .. } | Commands::Purge { .. } => StorageOpenMode::ReadWriteMigrate,
        _ => StorageOpenMode::ReadOnly,
    };
    let mut storage = Storage::open_with_mode(cli.database.as_deref(), &config, storage_mode)?;
    let mut steam = SteamResolver::default();

    match cli.command {
        Commands::Today => {
            cli::print_report(
                "Today",
                steam.resolve_totals(storage.totals_for_today()?),
                &config,
                cli.json,
            )?;
        }
        Commands::Week => {
            cli::print_report(
                "This Week",
                steam.resolve_totals(storage.totals_for_week()?),
                &config,
                cli.json,
            )?;
        }
        Commands::Apps => {
            cli::print_report(
                "All Time",
                steam.resolve_totals(storage.totals_all_time()?),
                &config,
                cli.json,
            )?;
        }
        Commands::Range { from, to } => {
            let rows = storage.totals_for_date_range(&from, &to)?;
            cli::print_report(
                &format!("{from} through {to}"),
                steam.resolve_totals(rows),
                &config,
                cli.json,
            )?;
        }
        Commands::Summary { days } => {
            let report = cli::summary_report(&storage, &mut steam, &config, days)?;
            cli::print_summary(&report)?;
        }
        Commands::Insights { lens, offset } => {
            let report = cli::insights_report(&storage, &mut steam, &config, lens.into(), offset)?;
            cli::print_insights(&report, cli.json)?;
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
                &config,
                ExportOptions {
                    lens: lens.into(),
                    offset,
                    title,
                },
            )?;
            fs::write(&output, html)?;
            println!("Exported {}", output.display());
        }
        Commands::ExportData {
            lens,
            offset,
            format,
            scope,
            output,
        } => {
            let data = export::build_data_export(
                &storage,
                &mut steam,
                &config,
                DataExportOptions {
                    lens: lens.into(),
                    offset,
                    scope: scope.into(),
                },
            )?;
            match format {
                DataExportFormatArg::Json => {
                    if let Some(parent) = output.parent()
                        && !parent.as_os_str().is_empty()
                    {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&output, serde_json::to_string_pretty(&data)?)?;
                    println!("Exported {}", output.display());
                }
                DataExportFormatArg::Csv => {
                    export::write_data_export_csv(&data, &output)?;
                    println!("Exported CSV bundle {}", output.display());
                }
            }
        }
        Commands::Purge {
            before,
            older_than_days,
            all,
            dry_run,
            vacuum,
            confirm,
        } => {
            if !dry_run && !confirm {
                anyhow::bail!("purge is destructive; rerun with --confirm or use --dry-run");
            }
            let cutoff = cli::purge_cutoff(before.as_deref(), older_than_days, all)?;
            let report = storage.purge_before(cutoff, dry_run, vacuum)?;
            cli::print_purge_report(&report, cli.json)?;
        }
        Commands::Goals { lens, offset } => {
            let report = cli::goal_report(&storage, &mut steam, &config, lens.into(), offset)?;
            cli::print_goal_report(&report, cli.json)?;
        }
        Commands::Digest { lens, offset } => {
            let report = cli::digest_report(&storage, &mut steam, &config, lens.into(), offset)?;
            cli::print_digest(&report, cli.json)?;
        }
        Commands::WidgetInsight { lens, offset } => {
            let insight =
                cli::widget_insight_report(&storage, &mut steam, &config, lens.into(), offset)?;
            cli::print_widget_insight(insight, cli.json)?;
        }
        Commands::Tui => {
            tui::run(storage, config)?;
        }
        Commands::RepairTitles { dry_run } => {
            let repair = storage.repair_titles(&mut steam, dry_run)?;
            cli::print_title_repair(&repair, cli.json)?;
        }
        Commands::Doctor => unreachable!("doctor is handled before storage opens"),
    }

    Ok(())
}
