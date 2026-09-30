mod cli;
mod device;
mod prompt;
mod shell;

use crate::{cli::Cli, shell::Shell};
use anyhow::{Context, Result};
use btleplug::platform::Manager;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    Cli::parse();

    let manager = Manager::new()
        .await
        .context("initialising Bluetooth (is BlueZ running?)")?;
    let adapter = device::pick_adapter(&manager).await?;

    Shell::new(adapter).run().await
}
