//! The `copit update-all` command.
//!
//! Re-fetches all tracked sources in `copit.toml`. Delegates to
//! [`super::update::update_source`] for each entry.

use anyhow::{bail, Context, Result};

use crate::cli::UpdateAllCommand;
use crate::config;
use crate::config::ResolvedSettings;

/// Run the `update-all` command, re-fetching all tracked sources.
///
/// Frozen entries (`frozen = true` in `copit.toml`) are skipped automatically.
/// If `--ref` is specified, it must be used with exactly one tracked source
/// (otherwise it's ambiguous which source to apply it to).
pub async fn run(cmd: &UpdateAllCommand) -> Result<()> {
    let cfg =
        config::load_config().context("Failed to load copit.toml. Run `copit init` first.")?;

    if cfg.sources.is_empty() {
        println!("No tracked sources to update.");
        return Ok(());
    }

    if cmd.version_ref.is_some() && cfg.sources.len() > 1 {
        bail!(
            "--ref is ambiguous with multiple sources. Use `copit update <path> --ref <version>` instead."
        );
    }

    for entry in &cfg.sources {
        if entry.frozen == Some(true) {
            println!("Skipping frozen: {}", entry.path);
            continue;
        }
        println!("Updating all {}...", entry.path);

        let settings = ResolvedSettings::resolve(
            cmd.overwrite,
            cmd.skip,
            cmd.backup,
            Some(entry),
            &cfg.project,
        );

        super::update::update_source(entry, cmd.version_ref.as_deref(), settings, None).await?;
    }

    Ok(())
}
