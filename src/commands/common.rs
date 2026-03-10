//! Shared utilities for command implementations.
//!
//! Provides path computation, path traversal validation, `excludes`
//! handling, and file writing helpers used by the `add`, `update`, and `update-all`
//! commands.

use anyhow::{bail, Context, Result};
use std::path::{Component, Path, PathBuf};

/// Display a path using forward slashes on all platforms.
///
/// This ensures consistent output and config storage across Windows, macOS,
/// and Linux. Without this, `PathBuf::display()` uses `\` on Windows, which
/// breaks config lookups and produces inconsistent user-facing output.
pub fn portable_display(path: &Path) -> String {
    let s = path.to_string_lossy();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.into_owned()
    }
}

/// Compute the strip prefix for multi-file sources.
///
/// For multi-file sources, strips the source path prefix so only the last
/// folder name is preserved. E.g. if source path is `crates/prek-identify`
/// and a file is `crates/prek-identify/src/lib.rs`, the dest becomes
/// `{target}/prek-identify/src/lib.rs` instead of `{target}/crates/prek-identify/src/lib.rs`.
pub fn compute_strip_prefix(suggested: &str, is_multi_file: bool) -> Option<String> {
    if !is_multi_file {
        return None;
    }
    let suggested_path = Path::new(suggested);
    suggested_path.parent().and_then(|p| {
        let p_str = p.to_str().unwrap_or("");
        if p_str.is_empty() {
            None
        } else {
            Some(format!("{}/", p_str))
        }
    })
}

/// Compute the destination path for a file.
///
/// For single-file sources, joins the filename to `base_target`.
/// For multi-file sources, strips the common prefix and joins to `base_target`.
pub fn compute_dest(
    relative_path: &str,
    base_target: &str,
    suggested: &str,
    strip_prefix: &Option<String>,
    is_single_file: bool,
) -> PathBuf {
    if is_single_file {
        let filename = Path::new(relative_path)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| suggested.to_string());
        PathBuf::from(base_target).join(filename)
    } else {
        let stripped = match strip_prefix {
            Some(prefix) => relative_path.strip_prefix(prefix).unwrap_or(relative_path),
            None => relative_path,
        };
        PathBuf::from(base_target).join(stripped)
    }
}

/// Validate that the destination path does not escape the base target directory
/// via path traversal (e.g. `../`).
pub fn validate_no_path_traversal(dest: &Path, base_target: &str) -> Result<()> {
    // Logically resolve the path by processing `.` and `..` components
    let mut resolved = Vec::new();
    for component in dest.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if resolved.pop().is_none() {
                    bail!(
                        "Path traversal detected: {} escapes target directory {}",
                        dest.display(),
                        base_target
                    );
                }
            }
            c => resolved.push(c),
        }
    }

    let resolved_path: PathBuf = resolved.into_iter().collect();
    let base = Path::new(base_target);

    if !resolved_path.starts_with(base) {
        bail!(
            "Path traversal detected: {} is outside target directory {}",
            dest.display(),
            base_target
        );
    }

    Ok(())
}

/// Build a [`globset::GlobSet`] from the given exclude patterns.
///
/// Each pattern is compiled with `literal_separator(true)` so that `*` matches
/// anything except `/` (use `**` for recursive matching across directories).
fn build_glob_set(excludes: &[String]) -> Result<globset::GlobSet> {
    let mut builder = globset::GlobSetBuilder::new();
    for pattern in excludes {
        let glob = globset::GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .with_context(|| format!("Invalid glob pattern: {pattern}"))?;
        builder.add(glob);
    }
    builder
        .build()
        .context("Failed to build glob set for excludes")
}

/// Handle excludes logic: skip the file and optionally write a `.orig` backup.
///
/// Returns `true` if the file was skipped (excluded), `false` if it should be
/// written normally. Supports glob patterns (e.g. `*.toml`, `src/**`) via
/// [`globset`].
pub fn handle_excludes(
    dest: &Path,
    track_path: &Path,
    excludes: &[String],
    contents: &[u8],
    backup: bool,
) -> Result<bool> {
    if excludes.is_empty() {
        return Ok(false);
    }

    let rel_within_source = dest
        .strip_prefix(track_path)
        .ok()
        .map(|p| p.to_string_lossy().to_string());

    if let Some(ref rel_path) = rel_within_source {
        let glob_set = build_glob_set(excludes)?;
        if glob_set.is_match(rel_path) {
            if backup {
                let orig_path = PathBuf::from(format!("{}.orig", portable_display(dest)));
                if let Some(parent) = orig_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&orig_path, contents).with_context(|| {
                    format!("Failed to write backup: {}", portable_display(&orig_path))
                })?;
                println!(
                    "Skipped (modified): {} (backup: {})",
                    portable_display(dest),
                    portable_display(&orig_path)
                );
            } else {
                println!("Skipped (modified): {}", portable_display(dest));
            }
            return Ok(true);
        }
    }

    Ok(false)
}

/// Determine whether an existing file should be overwritten.
///
/// Returns `true` if the file should be written, `false` if it should be skipped.
/// When neither `overwrite` nor `skip` is set, prompts the user interactively.
pub fn should_write_existing(dest: &Path, overwrite: bool, skip: bool) -> Result<bool> {
    if !dest.exists() {
        return Ok(true);
    }
    if skip {
        println!("Skipping (already exists): {}", portable_display(dest));
        return Ok(false);
    }
    if overwrite {
        return Ok(true);
    }
    Ok(dialoguer::Confirm::new()
        .with_prompt(format!(
            "{} already exists. Overwrite?",
            portable_display(dest)
        ))
        .default(false)
        .interact()
        .unwrap_or(false))
}

/// Write file contents to dest, creating parent directories as needed.
pub fn write_file(dest: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    std::fs::write(dest, contents)
        .with_context(|| format!("Failed to write file: {}", dest.display()))?;
    Ok(())
}
