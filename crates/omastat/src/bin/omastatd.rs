use anyhow::Result;
use clap::Parser;
use omastat::{config::Config, storage::Storage, tracker::Tracker};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "omastatd")]
#[command(about = "Omastat foreground daemon for Hyprland/Omarchy")]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,

    #[arg(long)]
    database: Option<PathBuf>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let config = Config::load(args.config.as_deref())?;
    config.log_warnings();
    let storage = Storage::open(args.database.as_deref(), &config)?;
    let mut tracker = Tracker::new(storage, config);
    tracker.run().await
}
