//! The `copit remove` command.
//!
//! Removes tracked source entries from `copit.toml` and deletes the
//! corresponding files from disk.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::cli::RemoveCommand;
use crate::config;

use super::common;

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

    // Collect entries that will be removed (need entry data for license cleanup)
    let entries_to_remove: Vec<_> = cfg
        .sources
        .iter()
        .filter(|e| paths_to_remove.contains(&e.path))
        .cloned()
        .collect();

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

    // Remove files and associated licenses from disk
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

        // Clean up associated license files
        if let Some(entry) = entries_to_remove.iter().find(|e| &e.path == path) {
            if entry.no_license != Some(true) {
                let track_path = PathBuf::from(&entry.path);
                common::remove_license_files(
                    &track_path,
                    &cfg.target,
                    cfg.licenses_dir.as_deref(),
                )?;
            }
        }
    }

    Ok(())
}
