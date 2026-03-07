//! The `copit update` command.
//!
//! Re-fetches specific tracked sources by path, optionally changing the
//! version ref. Always overwrites non-excluded files (no skip/overwrite
//! prompts unlike `add`).

use anyhow::{bail, Context, Result};
use std::path::PathBuf;

use crate::cli::UpdateCommand;
use crate::config::{self, SourceEntry};
use crate::sources::{self, Source};

use super::common;

/// Run the `update` command for the given paths.
pub async fn run(cmd: &UpdateCommand) -> Result<()> {
    if cmd.paths.is_empty() {
        bail!("No paths specified. Usage: copit update <path>...");
    }

    let cfg =
        config::load_config().context("Failed to load copit.toml. Run `copit init` first.")?;

    for path in &cmd.paths {
        let entry = cfg
            .sources
            .iter()
            .find(|e| e.path == *path)
            .ok_or_else(|| anyhow::anyhow!("Source not found in copit.toml: {path}"))?;

        update_source(entry, cmd.version_ref.as_deref(), cmd.backup).await?;
    }

    Ok(())
}

/// Re-fetch a single tracked source, optionally overriding the version ref.
///
/// Also used by [`super::sync`] to update each source during a full sync.
pub async fn update_source(
    entry: &SourceEntry,
    ref_override: Option<&str>,
    backup: bool,
) -> Result<()> {
    let source = sources::parse_source(&entry.source)?;

    let source = match ref_override {
        Some(new_ref) => source.with_version(new_ref),
        None => source,
    };

    let files = super::add::fetch_source(&source).await?;

    if files.is_empty() {
        println!("No files found for {}", source.to_source_string());
        return Ok(());
    }

    let track_path = PathBuf::from(&entry.path);
    let suggested = source.suggested_name();
    let is_single = files.len() == 1;
    let strip_prefix = common::compute_strip_prefix(&suggested, !is_single);

    // Determine the base target directory
    let base_target = track_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());

    for (relative_path, contents) in &files {
        let dest = if is_single {
            track_path.clone()
        } else {
            common::compute_dest(
                relative_path,
                &base_target,
                &suggested,
                &strip_prefix,
                false,
            )
        };

        // Check if this file is in exclude_modified
        if common::handle_exclude_modified(
            &dest,
            &track_path,
            &entry.exclude_modified,
            contents,
            backup,
        )? {
            continue;
        }

        // Always overwrite for update (no --skip/--overwrite prompts)
        common::write_file(&dest, contents)?;
        println!("Updated: {}", common::portable_display(&dest));
    }

    // Resolve version ref and commit for GitHub sources
    let (version_ref, commit) = match &source {
        Source::GitHub {
            owner,
            repo,
            version,
            ..
        } => {
            let sha = sources::github::resolve_commit_sha(owner, repo, version).await;
            (Some(version.clone()), sha)
        }
        _ => (None, None),
    };

    config::add_source_entry(
        &entry.path,
        &source.to_source_string(),
        version_ref.as_deref(),
        commit.as_deref(),
    )?;

    Ok(())
}
