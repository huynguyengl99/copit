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

/// Validate that a target directory stays inside the project.
///
/// [`validate_no_path_traversal`] compares a destination against its own base, so it
/// cannot judge the base itself. An absolute or escaping target would pass that check
/// while writing anywhere on disk.
///
/// Judged on the string rather than via [`Path`], whose notion of "absolute" is platform
/// specific: Windows does not count a leading `/` as absolute, and Unix treats `C:\\x`
/// as an ordinary filename. A `copit.toml` is committed and shared across machines, so
/// the same target has to be accepted or rejected identically everywhere.
pub fn validate_install_target(target: &str) -> Result<()> {
    if target.is_empty() {
        bail!("Target directory cannot be empty");
    }

    if target.starts_with('/') || target.starts_with('\\') {
        bail!("Target directory must be relative to the project: {target}");
    }

    // A drive specifier, absolute (`C:\\x`) or drive-relative (`C:x`); both leave the
    // project.
    let mut chars = target.chars();
    if let (Some(letter), Some(':')) = (chars.next(), chars.next()) {
        if letter.is_ascii_alphabetic() {
            bail!("Target directory must be relative to the project: {target}");
        }
    }

    let mut depth: i32 = 0;
    for part in target.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                depth -= 1;
                if depth < 0 {
                    bail!("Target directory escapes the project: {target}");
                }
            }
            _ => depth += 1,
        }
    }

    Ok(())
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
    match dialoguer::Confirm::new()
        .with_prompt(format!(
            "{} already exists. Overwrite?",
            portable_display(dest)
        ))
        .default(false)
        .interact()
    {
        Ok(answer) => Ok(answer),
        // No terminal to ask at: keeping the file is the safe answer, but saying
        // nothing makes a whole update look like it succeeded having written
        // nothing. See `--overwrite` and `--skip`.
        Err(_) => {
            println!(
                "Keeping (exists, nothing to prompt with): {} \u{2014} pass --overwrite to replace it",
                portable_display(dest)
            );
            Ok(false)
        }
    }
}

/// Whether the file on disk already holds exactly these bytes.
///
/// Worth asking before prompting about an overwrite: a component whose version moved
/// without its files changing has nothing to write, and neither a question nor a
/// warning is warranted.
pub fn is_unchanged(dest: &Path, contents: &[u8]) -> bool {
    std::fs::read(dest).is_ok_and(|existing| existing == contents)
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
    project_target: &str,
    licenses_dir: Option<&str>,
) -> Result<()> {
    for license_dir in license_dir_candidates(track_path, target, project_target, licenses_dir) {
        remove_license_files_in(&license_dir, track_path)?;
    }

    Ok(())
}

fn remove_license_files_in(license_dir: &Path, track_path: &Path) -> Result<()> {
    use crate::sources::github::LICENSE_NAMES;

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
        let mut dir = Some(license_dir);
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

/// The directory a source was copied into.
///
/// A component installed from a registry lives under that registry's own `target`,
/// which is usually not the project-wide one. Using the project target for it makes
/// [`license_dir_for`] fail to strip the prefix and keep the whole path.
pub fn target_for_entry(
    cfg: &crate::config::CopitConfig,
    entry: &crate::config::SourceEntry,
) -> String {
    entry
        .component
        .as_deref()
        .and_then(|backlink| backlink.rsplit_once(':'))
        .and_then(|(registry_name, _)| cfg.registries.get(registry_name))
        .and_then(|registry| registry.target.as_deref())
        .unwrap_or(&cfg.target)
        .to_string()
}

/// Whether a directory holds any recognised license file.
pub fn has_license_files(dir: &Path) -> bool {
    crate::sources::github::LICENSE_NAMES
        .iter()
        .any(|name| dir.join(name).is_file())
}

/// Where a source's license files actually are.
///
/// `licenses_dir` records where they were put, but it can be edited by hand, and
/// then it states an intention rather than a fact. Trusting it would make "current"
/// and "target" agree and the move silently do nothing, so prefer whichever layout
/// has files on disk.
pub fn existing_license_dir(
    track_path: &Path,
    target: &str,
    project_target: &str,
    licenses_dir: Option<&str>,
) -> PathBuf {
    let candidates = license_dir_candidates(track_path, target, project_target, licenses_dir);
    candidates
        .iter()
        .find(|dir| has_license_files(dir))
        .cloned()
        .unwrap_or_else(|| candidates[0].clone())
}

/// Every directory a source's licenses may occupy, current layout first.
///
/// Two of these are historical. Before registry targets were honoured, centralised
/// licenses were placed using the project target, which kept the whole path
/// (`licenses/app/ws_kits/ag_ui` rather than `licenses/ag_ui`). And `licenses_dir`
/// may have been set after the files were written, leaving them side by side. Both
/// have to be found, or upgrading orphans licenses already on disk.
pub fn license_dir_candidates(
    track_path: &Path,
    target: &str,
    project_target: &str,
    licenses_dir: Option<&str>,
) -> Vec<PathBuf> {
    let mut candidates = vec![license_dir_for(track_path, target, licenses_dir)];

    for legacy in [
        license_dir_for(track_path, project_target, licenses_dir),
        license_dir_for(track_path, target, None),
    ] {
        if !candidates.contains(&legacy) {
            candidates.push(legacy);
        }
    }

    candidates
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

    #[test]
    fn an_install_target_must_stay_inside_the_project() {
        // validate_no_path_traversal compares a dest against its own base, so an
        // absolute base passes it while writing anywhere on disk.
        assert!(validate_install_target("vendor").is_ok());
        assert!(validate_install_target("app/components").is_ok());
        assert!(validate_install_target("app/../components").is_ok());
        assert!(validate_install_target("./app/components").is_ok());

        assert!(validate_install_target("").is_err());
        assert!(validate_install_target("../../shared").is_err());
        assert!(validate_install_target("app/../../shared").is_err());
    }

    #[test]
    fn an_install_target_is_judged_the_same_on_every_platform() {
        // Path::is_absolute is platform specific: Windows does not count a leading `/`,
        // and Unix reads `C:\\x` as an ordinary filename. copit.toml is shared across
        // machines, so these must be rejected everywhere, not just where they are
        // locally "absolute".
        for target in [
            "/etc/cron.d",
            "\\etc\\cron.d",
            "C:\\Windows\\Temp",
            "C:temp",
            "..\\..\\shared",
        ] {
            assert!(
                validate_install_target(target).is_err(),
                "{target} should be rejected"
            );
        }
    }

    #[test]
    fn a_component_file_resolves_under_its_target() {
        // Mirrors what copy_component does: the index lists inner paths with forward
        // slashes, and they are joined onto the target on every platform.
        let target = "app/components";
        let track_path = PathBuf::from(target).join("auth_core");

        for within in ["__init__.py", "stores/sqlite.py", "a/b/c/deep.py"] {
            let dest = track_path.join(within);
            assert!(
                validate_no_path_traversal(&dest, target).is_ok(),
                "{within} should resolve inside {target}"
            );
        }
    }

    #[test]
    fn a_component_file_cannot_climb_out_of_its_target() {
        let target = "app/components";
        let track_path = PathBuf::from(target).join("auth_core");

        for within in ["../../../etc/passwd", "../../../../outside.py"] {
            let dest = track_path.join(within);
            assert!(
                validate_no_path_traversal(&dest, target).is_err(),
                "{within} should be rejected"
            );
        }
    }

    fn config_with_registry(registry_target: Option<&str>) -> crate::config::CopitConfig {
        let mut registries = std::collections::BTreeMap::new();
        registries.insert(
            "my-kit".to_string(),
            crate::config::RegistryConfig {
                source: "github:owner/repo@v1".to_string(),
                index: None,
                target: registry_target.map(str::to_string),
                variants: Vec::new(),
                optional: Vec::new(),
                package_manager: None,
            },
        );
        crate::config::CopitConfig {
            target: "vendor".to_string(),
            registries,
            ..Default::default()
        }
    }

    fn entry(path: &str, component: Option<&str>) -> crate::config::SourceEntry {
        crate::config::SourceEntry {
            path: path.to_string(),
            component: component.map(str::to_string),
            ..Default::default()
        }
    }

    #[test]
    fn target_for_entry_prefers_the_registrys_own_target() {
        let cfg = config_with_registry(Some("app/ws_kits"));
        let entry = entry("app/ws_kits/ag_ui", Some("my-kit:ag-ui"));

        assert_eq!(target_for_entry(&cfg, &entry), "app/ws_kits");
    }

    #[test]
    fn target_for_entry_falls_back_to_the_project_target() {
        let cfg = config_with_registry(None);

        assert_eq!(
            target_for_entry(&cfg, &entry("vendor/ag_ui", Some("my-kit:ag-ui"))),
            "vendor"
        );
        assert_eq!(
            target_for_entry(&cfg, &entry("vendor/lib.rs", None)),
            "vendor"
        );
    }

    #[test]
    fn target_for_entry_ignores_a_registry_that_is_gone() {
        let cfg = config_with_registry(Some("app/ws_kits"));
        let entry = entry("vendor/thing", Some("removed-kit:thing"));

        assert_eq!(target_for_entry(&cfg, &entry), "vendor");
    }

    #[test]
    fn a_registry_component_keeps_its_name_under_a_licenses_dir() {
        // The bug this guards: passing the project target leaves the whole path in
        // place, giving licenses/app/ws_kits/ag_ui instead of licenses/ag_ui.
        let cfg = config_with_registry(Some("app/ws_kits"));
        let entry = entry("app/ws_kits/ag_ui", Some("my-kit:ag-ui"));
        let target = target_for_entry(&cfg, &entry);

        let dir = license_dir_for(Path::new(&entry.path), &target, Some("licenses"));

        assert_eq!(dir, PathBuf::from("licenses/ag_ui"));
    }

    #[test]
    fn existing_license_dir_finds_files_the_config_does_not_know_about() {
        // licenses_dir edited by hand: the setting says "licenses", the files are
        // still side by side, and reporting the configured path would move nothing.
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let track_path = root.join("vendor/mylib");
        std::fs::create_dir_all(&track_path).expect("create");
        std::fs::write(track_path.join("LICENSE"), b"MIT").expect("write");

        let target = portable_display(&root.join("vendor"));
        let found = existing_license_dir(&track_path, &target, &target, Some("licenses"));

        assert_eq!(found, track_path);
    }

    #[test]
    fn existing_license_dir_finds_the_layout_written_before_registry_targets() {
        // Upgrade path: an older copit centralised using the project target, so the
        // file sits at licenses/app/ws_kits/ag_ui rather than licenses/ag_ui.
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let legacy = root.join("licenses/app/ws_kits/ag_ui");
        std::fs::create_dir_all(&legacy).expect("create");
        std::fs::write(legacy.join("LICENSE"), b"MIT").expect("write");

        let track_path = root.join("app/ws_kits/ag_ui");
        let licenses_dir = portable_display(&root.join("licenses"));
        let found = existing_license_dir(
            &track_path,
            &portable_display(&root.join("app/ws_kits")),
            &portable_display(root),
            Some(&licenses_dir),
        );

        assert_eq!(found, legacy);
    }

    #[test]
    fn existing_license_dir_uses_the_configured_layout_when_it_holds_the_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path();
        let centralized = root.join("licenses/mylib");
        std::fs::create_dir_all(&centralized).expect("create");
        std::fs::write(centralized.join("LICENSE"), b"MIT").expect("write");

        let track_path = root.join("vendor/mylib");
        let target = portable_display(&root.join("vendor"));
        let licenses_dir = portable_display(&root.join("licenses"));
        let found = existing_license_dir(&track_path, &target, &target, Some(&licenses_dir));

        assert_eq!(found, centralized);
    }
}
