//! The `copit licenses-sync` command.
//!
//! Moves license files between side-by-side and centralized layouts,
//! or re-syncs them to match the current configuration.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::cli::LicensesSyncCommand;
use crate::config;
use crate::sources::github::LICENSE_NAMES;

use super::common::{license_dir_for, portable_display};

/// Find license files that exist in a directory.
fn find_license_files(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    if !dir.exists() {
        return found;
    }
    for name in LICENSE_NAMES {
        let path = dir.join(name);
        if path.is_file() {
            found.push(name.to_string());
        }
    }
    found
}

/// Move a file from old_path to new_path. Creates parent dirs, removes old file,
/// and cleans up empty ancestor directories left behind.
///
/// Tries `fs::rename` first (fast, same-filesystem), falls back to read+write+delete
/// for cross-device moves.
fn move_file(old_path: &Path, new_path: &Path) -> Result<()> {
    if let Some(parent) = new_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Try atomic rename first; fall back to copy+delete on cross-device errors.
    if std::fs::rename(old_path, new_path).is_err() {
        let contents = std::fs::read(old_path)
            .with_context(|| format!("Failed to read {}", old_path.display()))?;
        std::fs::write(new_path, &contents)
            .with_context(|| format!("Failed to write {}", new_path.display()))?;
        std::fs::remove_file(old_path)
            .with_context(|| format!("Failed to remove {}", old_path.display()))?;
    }

    // Clean up empty ancestor directories
    let mut dir = old_path.parent();
    while let Some(d) = dir {
        if d == Path::new("") || d == Path::new(".") {
            break;
        }
        match std::fs::read_dir(d) {
            Ok(mut entries) => {
                if entries.next().is_none() {
                    let _ = std::fs::remove_dir(d);
                    dir = d.parent();
                } else {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    Ok(())
}

/// Run the `licenses-sync` command.
pub fn run(cmd: &LicensesSyncCommand) -> Result<()> {
    let cfg =
        config::load_config().context("Failed to load copit.toml. Run `copit init` first.")?;

    // Determine the target licenses_dir
    let target_licenses_dir: Option<&str> = if cmd.no_dir {
        None
    } else if let Some(ref dir) = cmd.licenses_dir {
        Some(dir.as_str())
    } else {
        cfg.licenses_dir.as_deref()
    };

    // Current licenses_dir from config (where files are now)
    let current_licenses_dir = cfg.licenses_dir.as_deref();

    let mut moved_count = 0;

    for entry in &cfg.sources {
        // Skip entries with no_license set
        if entry.no_license == Some(true) {
            continue;
        }

        let track_path = PathBuf::from(&entry.path);

        let current_dir = license_dir_for(&track_path, &cfg.target, current_licenses_dir);
        let target_dir = license_dir_for(&track_path, &cfg.target, target_licenses_dir);

        if current_dir == target_dir {
            continue;
        }

        let license_files = find_license_files(&current_dir);

        for filename in &license_files {
            let old_path = current_dir.join(filename);
            let new_path = target_dir.join(filename);

            if cmd.dry_run {
                println!(
                    "Would move: {} -> {}",
                    portable_display(&old_path),
                    portable_display(&new_path)
                );
            } else {
                move_file(&old_path, &new_path)?;
                println!(
                    "Moved: {} -> {}",
                    portable_display(&old_path),
                    portable_display(&new_path)
                );
            }
            moved_count += 1;
        }
    }

    // Update licenses_dir in config (skip in dry-run mode)
    if !cmd.dry_run {
        if cmd.no_dir {
            config::update_licenses_dir(None)?;
        } else if let Some(ref dir) = cmd.licenses_dir {
            config::update_licenses_dir(Some(dir))?;
        }
    }

    if moved_count == 0 {
        println!("All license files are already in sync.");
    } else if cmd.dry_run {
        println!("Would move {} license file(s).", moved_count);
    } else {
        println!("Moved {} license file(s).", moved_count);
    }

    Ok(())
}
