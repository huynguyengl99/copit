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

/// Compute the directory where license files should be placed for a source.
///
/// If `licenses_dir` is set, mirrors the target structure:
/// strips `target` prefix from `track_path` and joins the remainder onto `licenses_dir`.
/// For single-file sources, uses the filename without extension as the folder name
/// (e.g. `vendor/lib.rs` → `licenses/lib/`).
/// For directory sources, uses the directory name as-is
/// (e.g. `vendor/mylib` → `licenses/mylib/`).
///
/// Without `licenses_dir`, places licenses side-by-side:
/// single files get licenses in a stem-named subfolder (e.g. `vendor/lib.rs` → `vendor/lib/`),
/// directories get licenses inside.
pub fn license_dir_for(track_path: &Path, target: &str, licenses_dir: Option<&str>) -> PathBuf {
    if let Some(dir) = licenses_dir {
        let relative = track_path.strip_prefix(target).unwrap_or(track_path);
        // For single files, use the stem (filename without extension) as folder
        if track_path.extension().is_some() {
            let parent = relative.parent().unwrap_or(Path::new(""));
            let stem = relative.file_stem().unwrap_or(relative.as_os_str());
            PathBuf::from(dir).join(parent).join(stem)
        } else {
            PathBuf::from(dir).join(relative)
        }
    } else if track_path.extension().is_some() {
        // Single file — license in a stem-named subfolder next to it
        // e.g. vendor/lib.rs → vendor/lib/LICENSE
        // This avoids collisions when multiple single-file sources share a parent.
        let parent = track_path.parent().unwrap_or(Path::new("."));
        let stem = track_path.file_stem().unwrap_or(track_path.as_os_str());
        parent.join(stem)
    } else {
        // Directory — license inside it
        track_path.to_path_buf()
    }
}

/// Remove license files associated with a source.
///
/// Uses [`license_dir_for`] to find the license directory, then removes any
/// known license files. If the license directory becomes empty after removal
/// (and it's not the same as `track_path`), it is also deleted along with any
/// empty ancestors.
pub fn remove_license_files(
    track_path: &Path,
    target: &str,
    licenses_dir: Option<&str>,
) -> Result<()> {
    use crate::sources::github::LICENSE_NAMES;

    let license_dir = license_dir_for(track_path, target, licenses_dir);

    // For directory sources without licenses_dir, the license dir IS the track path.
    // Those license files get removed when the source directory itself is deleted,
    // so skip cleanup here.
    if license_dir == track_path {
        return Ok(());
    }

    if !license_dir.exists() {
        return Ok(());
    }

    let mut removed_any = false;
    for name in LICENSE_NAMES {
        let path = license_dir.join(name);
        if path.is_file() {
            std::fs::remove_file(&path)
                .with_context(|| format!("Failed to remove license: {}", path.display()))?;
            println!("Removed license: {}", portable_display(&path));
            removed_any = true;
        }
    }

    // Clean up empty license directory and ancestors
    if removed_any {
        let mut dir = Some(license_dir.as_path());
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
    }

    Ok(())
}

/// Write license files to disk alongside the copied source.
///
/// Uses [`license_dir_for`] to determine the destination directory.
pub fn write_license_files(
    license_files: &[(String, Vec<u8>)],
    track_path: &Path,
    target: &str,
    licenses_dir: Option<&str>,
) -> Result<()> {
    if license_files.is_empty() {
        return Ok(());
    }

    let dest_dir = license_dir_for(track_path, target, licenses_dir);

    for (name, contents) in license_files {
        let dest = dest_dir.join(name);
        write_file(&dest, contents)?;
        println!("License: {}", portable_display(&dest));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_dir_for_centralized_single_file() {
        let result = license_dir_for(Path::new("vendor/lib.rs"), "vendor", Some("licenses"));
        assert_eq!(result, PathBuf::from("licenses/lib"));
    }

    #[test]
    fn license_dir_for_centralized_directory() {
        let result = license_dir_for(Path::new("vendor/mylib"), "vendor", Some("licenses"));
        assert_eq!(result, PathBuf::from("licenses/mylib"));
    }

    #[test]
    fn license_dir_for_side_by_side_single_file() {
        let result = license_dir_for(Path::new("vendor/lib.rs"), "vendor", None);
        assert_eq!(result, PathBuf::from("vendor/lib"));
    }

    #[test]
    fn license_dir_for_side_by_side_directory() {
        let result = license_dir_for(Path::new("vendor/mylib"), "vendor", None);
        assert_eq!(result, PathBuf::from("vendor/mylib"));
    }

    #[test]
    fn license_dir_for_nested_target_single_file() {
        let result = license_dir_for(
            Path::new("my_proj/ext/utils/helpers.rs"),
            "my_proj/ext",
            Some("licenses"),
        );
        assert_eq!(result, PathBuf::from("licenses/utils/helpers"));
    }

    #[test]
    fn license_dir_for_nested_target_directory() {
        let result = license_dir_for(
            Path::new("my_proj/ext/commands"),
            "my_proj/ext",
            Some("licenses"),
        );
        assert_eq!(result, PathBuf::from("licenses/commands"));
    }

    #[test]
    fn license_dir_for_strip_prefix_not_matching() {
        // When track_path doesn't start with target, uses full path as relative
        let result = license_dir_for(Path::new("other/lib.rs"), "vendor", Some("licenses"));
        assert_eq!(result, PathBuf::from("licenses/other/lib"));
    }
}
