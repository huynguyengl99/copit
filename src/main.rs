mod cli;
mod commands;
mod config;
mod sources;

use clap::Parser;
use cli::{Args, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args.command {
        Command::Init => commands::init::run()?,
        Command::Add(cmd) => commands::add::run(&cmd).await?,
        Command::Remove(cmd) => commands::remove::run(&cmd)?,
        Command::Update(cmd) => commands::update::run(&cmd).await?,
        Command::UpdateAll(cmd) => commands::update_all::run(&cmd).await?,
        Command::LicensesSync(cmd) => commands::licenses_sync::run(&cmd)?,
    }

    Ok(())
}
