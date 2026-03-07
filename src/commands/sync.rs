//! The `copit sync` command.
//!
//! Re-fetches all tracked sources in `copit.toml`. Delegates to
//! [`super::update::update_source`] for each entry.

use anyhow::{bail, Context, Result};

use crate::cli::SyncCommand;
use crate::config;

/// Run the `sync` command, re-fetching all tracked sources.
///
/// If `--ref` is specified, it must be used with exactly one tracked source
/// (otherwise it's ambiguous which source to apply it to).
pub async fn run(cmd: &SyncCommand) -> Result<()> {
    let cfg =
        config::load_config().context("Failed to load copit.toml. Run `copit init` first.")?;

    if cfg.sources.is_empty() {
        println!("No tracked sources to sync.");
        return Ok(());
    }

    if cmd.version_ref.is_some() && cfg.sources.len() > 1 {
        bail!(
            "--ref is ambiguous with multiple sources. Use `copit update <path> --ref <version>` instead."
        );
    }

    for entry in &cfg.sources {
        println!("Syncing {}...", entry.path);
        super::update::update_source(entry, cmd.version_ref.as_deref(), cmd.backup).await?;
    }

    Ok(())
}
