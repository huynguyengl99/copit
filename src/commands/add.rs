//! The `copit add` command.
//!
//! Fetches source code from GitHub, HTTP, or ZIP sources and copies it into
//! the project. Registers each new source in `copit.toml` for tracking.

use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::cli::AddCommand;
use crate::config;
use crate::sources::{self, Source};

use super::common::{self, portable_display, should_write_existing};

/// Run the `add` command, fetching and copying each specified source.
pub async fn run(cmd: &AddCommand) -> Result<()> {
    if cmd.sources.is_empty() {
        bail!("No sources specified. Usage: copit add <source>...");
    }

    let cfg =
        config::load_config().context("Failed to load copit.toml. Run `copit init` first.")?;

    let base_target = cmd.to.as_deref().unwrap_or(&cfg.project.target);

    for source_str in &cmd.sources {
        let source = sources::parse_source(source_str)?;
        add_source(&source, base_target, cmd).await?;
    }

    Ok(())
}

async fn add_source(source: &Source, base_target: &str, cmd: &AddCommand) -> Result<()> {
    let files = fetch_source(source).await?;

    if files.is_empty() {
        println!("No files found for {}", source.to_source_string());
        return Ok(());
    }

    let suggested = source.suggested_name();
    let is_single = files.len() == 1;
    let strip_prefix = common::compute_strip_prefix(&suggested, !is_single);

    // Compute the track path early so we can look up excludes
    let track_path = if is_single {
        let (relative_path, _) = &files[0];
        let filename = Path::new(relative_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| suggested.clone());
        PathBuf::from(base_target).join(filename)
    } else {
        let folder_name = Path::new(&suggested)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or(suggested.clone());
        PathBuf::from(base_target).join(folder_name)
    };

    let track_key = portable_display(&track_path);

    // Refuse to re-add an already tracked source
    if config::get_source_entry(&track_key).is_some() {
        println!(
            "Already tracked: {} (use `copit update` to re-fetch)",
            track_key
        );
        return Ok(());
    }

    let mut any_written = false;

    for (relative_path, contents) in &files {
        let dest = common::compute_dest(
            relative_path,
            base_target,
            &suggested,
            &strip_prefix,
            is_single,
        );

        // Validate no path traversal
        common::validate_no_path_traversal(&dest, base_target)?;

        if !should_write_existing(&dest, cmd.overwrite, cmd.skip)? {
            continue;
        }

        common::write_file(&dest, contents)?;
        println!("Copied: {}", portable_display(&dest));
        any_written = true;
    }

    if !any_written {
        return Ok(());
    }

    // Resolve version ref and commit for GitHub sources
    let (version_ref, commit) = match source {
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

    let frozen = if cmd.freeze { Some(true) } else { None };

    config::add_source_entry(
        &track_key,
        &source.to_source_string(),
        version_ref.as_deref(),
        commit.as_deref(),
        frozen,
    )?;

    Ok(())
}

/// Fetch files from a source, returning `(relative_path, contents)` pairs.
///
/// Dispatches to the appropriate fetcher based on the source type.
pub async fn fetch_source(source: &Source) -> Result<Vec<(String, Vec<u8>)>> {
    match source {
        Source::GitHub {
            owner,
            repo,
            version,
            path,
        } => {
            let files = sources::github::fetch_github(owner, repo, version, path).await?;
            Ok(files.into_iter().collect())
        }
        Source::Http { url } => {
            let bytes = sources::http::fetch_url(url).await?;
            let filename = url.rsplit('/').next().unwrap_or("downloaded").to_string();
            Ok(vec![(filename, bytes)])
        }
        Source::Zip { url, inner_path } => {
            let bytes = sources::http::fetch_url(url).await?;
            let files = sources::zip::extract_from_bytes(&bytes, inner_path.as_deref(), None)?;
            Ok(files.into_iter().collect())
        }
    }
}
