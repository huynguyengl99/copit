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
        Command::Sync(cmd) => commands::sync::run(&cmd).await?,
    }

    Ok(())
}
