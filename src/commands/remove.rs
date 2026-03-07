//! The `copit remove` command.
//!
//! Removes tracked source entries from `copit.toml` and deletes the
//! corresponding files from disk.

use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::cli::RemoveCommand;
use crate::config;

/// Run the `remove` command.
///
/// Removes config entries first (so files remain if config write fails),
/// then deletes files from disk. Only files that were actually tracked
/// in `copit.toml` are deleted.
pub fn run(cmd: &RemoveCommand) -> Result<()> {
    if !cmd.all && cmd.paths.is_empty() {
        bail!("No paths specified. Usage: copit remove <path>... or copit remove --all");
    }

    let cfg =
        config::load_config().context("Failed to load copit.toml. Run `copit init` first.")?;

    let paths_to_remove: Vec<String> = if cmd.all {
        cfg.sources.iter().map(|s| s.path.clone()).collect()
    } else {
        cmd.paths.clone()
    };

    if paths_to_remove.is_empty() {
        println!("No tracked sources to remove.");
        return Ok(());
    }

    // Remove entries from config first (safer: if this fails, files remain intact)
    let removed = config::remove_source_entries(&paths_to_remove)?;

    let not_in_config: Vec<_> = paths_to_remove
        .iter()
        .filter(|p| !removed.contains(p))
        .collect();

    for path in &not_in_config {
        println!("Warning: {path} was not tracked in copit.toml");
    }

    if !removed.is_empty() {
        println!("Removed {} source(s) from copit.toml", removed.len());
    }

    // Remove files from disk (only for paths that were actually tracked)
    for path in &removed {
        let file_path = Path::new(path);
        if file_path.exists() {
            if file_path.is_dir() {
                std::fs::remove_dir_all(file_path)
                    .with_context(|| format!("Failed to remove directory: {path}"))?;
            } else {
                std::fs::remove_file(file_path)
                    .with_context(|| format!("Failed to remove file: {path}"))?;
            }
            println!("Removed: {path}");
        } else {
            println!("Not found on disk (already removed): {path}");
        }
    }

    Ok(())
}
