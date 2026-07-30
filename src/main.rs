mod cli;
mod commands;
mod config;
mod installers;
mod registry;
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
        Command::Registry(cmd) => match cmd {
            cli::RegistryCommand::Add(cmd) => commands::registry::add(&cmd).await?,
            cli::RegistryCommand::List => commands::registry::list()?,
        },
        Command::Search(cmd) => commands::registry::search(&cmd).await?,
        Command::Info(cmd) => commands::registry::info(&cmd).await?,
    }

    Ok(())
}
