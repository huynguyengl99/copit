//! Configuration file (`copit.toml`) management.
//!
//! This module handles loading, saving, and modifying the `copit.toml` manifest
//! that tracks all copied sources. The config file lives in the project root and
//! records where each file came from, which version was used, and which files
//! should be skipped on updates.
//!
//! # Config format
//!
//! ```toml
//! target = "vendor"
//!
//! [[sources]]
//! path = "vendor/lib.rs"
//! source = "github:owner/repo@v1.0/src/lib.rs"
//! ref = "v1.0"
//! commit = "abc123..."
//! copied_at = "2026-03-07T08:46:51Z"
//! excludes = ["Cargo.toml"]
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "copit.toml";

/// A single tracked source entry (one `[[sources]]` table).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SourceEntry {
    /// Local path where the source was copied to (e.g., `"vendor/lib.rs"`).
    pub path: String,
    /// Original source string (e.g., `"github:owner/repo@v1.0/src/lib.rs"`).
    pub source: String,
    /// User-specified version ref (branch, tag, or SHA). GitHub sources only.
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref")]
    pub version_ref: Option<String>,
    /// Resolved commit SHA from the GitHub API. GitHub sources only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// ISO 8601 timestamp of when the source was last copied/updated.
    pub copied_at: String,
    /// Relative paths within the source to skip on update (user-modified files).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excludes: Vec<String>,
    /// Pin this source so it's skipped during updates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen: Option<bool>,
    /// Per-source override: overwrite existing files without prompting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overwrite: Option<bool>,
    /// Per-source override: skip existing files without prompting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip: Option<bool>,
    /// Per-source override: save `.orig` backup for excluded modified files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,
}

/// Top-level `copit.toml` configuration.
#[derive(Debug, Deserialize, Serialize)]
pub struct CopitConfig {
    /// Default target directory for copied files (e.g., `"vendor"`).
    pub target: String,
    /// Default: overwrite existing files without prompting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overwrite: Option<bool>,
    /// Default: skip existing files without prompting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip: Option<bool>,
    /// Default: save `.orig` backup for excluded modified files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<bool>,
    /// List of tracked source entries.
    #[serde(default, rename = "sources")]
    pub sources: Vec<SourceEntry>,
}

impl Default for CopitConfig {
    fn default() -> Self {
        Self {
            target: "vendor".to_string(),
            overwrite: None,
            skip: None,
            backup: None,
            sources: Vec::new(),
        }
    }
}

/// Resolved file-handling settings after applying the priority chain.
///
/// Priority: CLI flag (if `true`) > per-source config > root-level config > `false`.
#[derive(Debug)]
pub struct ResolvedSettings {
    pub overwrite: bool,
    pub skip: bool,
    pub backup: bool,
}

impl ResolvedSettings {
    /// Resolve settings from CLI flags, per-source overrides, and config defaults.
    ///
    /// Each setting follows the same priority chain:
    /// 1. CLI flag — wins if `true` (explicit user intent)
    /// 2. Per-source config (`[[sources]]` entry)
    /// 3. Config defaults (root-level fields)
    /// 4. Default: `false`
    pub fn resolve(
        cli_overwrite: bool,
        cli_skip: bool,
        cli_backup: bool,
        source: Option<&SourceEntry>,
        config: &CopitConfig,
    ) -> Self {
        Self {
            overwrite: Self::resolve_flag(
                cli_overwrite,
                source.and_then(|s| s.overwrite),
                config.overwrite,
            ),
            skip: Self::resolve_flag(cli_skip, source.and_then(|s| s.skip), config.skip),
            backup: Self::resolve_flag(cli_backup, source.and_then(|s| s.backup), config.backup),
        }
    }

    fn resolve_flag(cli: bool, source: Option<bool>, config: Option<bool>) -> bool {
        if cli {
            return true;
        }
        if let Some(v) = source {
            return v;
        }
        config.unwrap_or(false)
    }
}

#[cfg(test)]
pub fn config_path_in(dir: &Path) -> PathBuf {
    dir.join(CONFIG_FILE)
}

/// Returns the path to `copit.toml` in the current directory.
pub fn config_path() -> PathBuf {
    PathBuf::from(CONFIG_FILE)
}

/// Returns `true` if `copit.toml` exists in the current directory.
pub fn config_exists() -> bool {
    config_path().exists()
}

#[cfg(test)]
pub fn config_exists_in(dir: &Path) -> bool {
    config_path_in(dir).exists()
}

/// Load `copit.toml` from the current directory.
pub fn load_config() -> Result<CopitConfig> {
    load_config_from(&config_path())
}

/// Load a copit config from the given path.
pub fn load_config_from(path: &Path) -> Result<CopitConfig> {
    let content = std::fs::read_to_string(path).context("Failed to read copit.toml")?;
    let config: CopitConfig = toml::from_str(&content).context("Failed to parse copit.toml")?;
    Ok(config)
}

/// Look up an existing source entry by path.
pub fn get_source_entry(path: &str) -> Option<SourceEntry> {
    let config = load_config().ok()?;
    config.sources.into_iter().find(|e| e.path == path)
}

#[cfg(test)]
pub fn get_source_entry_from(config_file: &Path, path: &str) -> Option<SourceEntry> {
    let config = load_config_from(config_file).ok()?;
    config.sources.into_iter().find(|e| e.path == path)
}

/// Save config to `copit.toml` in the current directory.
pub fn save_config(config: &CopitConfig) -> Result<()> {
    save_config_to(config, &config_path())
}

/// Save config to the given path, using `toml_edit` to produce clean TOML output.
pub fn save_config_to(config: &CopitConfig, path: &Path) -> Result<()> {
    let mut doc = toml_edit::DocumentMut::new();

    doc["target"] = toml_edit::value(&config.target);
    if let Some(overwrite) = config.overwrite {
        doc["overwrite"] = toml_edit::value(overwrite);
    }
    if let Some(skip) = config.skip {
        doc["skip"] = toml_edit::value(skip);
    }
    if let Some(backup) = config.backup {
        doc["backup"] = toml_edit::value(backup);
    }

    if !config.sources.is_empty() {
        let mut sources = toml_edit::ArrayOfTables::new();
        for entry in &config.sources {
            let mut table = toml_edit::Table::new();
            table["path"] = toml_edit::value(&entry.path);
            table["source"] = toml_edit::value(&entry.source);
            if let Some(ref version_ref) = entry.version_ref {
                table["ref"] = toml_edit::value(version_ref);
            }
            if let Some(ref commit) = entry.commit {
                table["commit"] = toml_edit::value(commit);
            }
            if let Some(frozen) = entry.frozen {
                table["frozen"] = toml_edit::value(frozen);
            }
            if let Some(overwrite) = entry.overwrite {
                table["overwrite"] = toml_edit::value(overwrite);
            }
            if let Some(skip) = entry.skip {
                table["skip"] = toml_edit::value(skip);
            }
            if let Some(backup) = entry.backup {
                table["backup"] = toml_edit::value(backup);
            }
            table["copied_at"] = toml_edit::value(&entry.copied_at);
            if !entry.excludes.is_empty() {
                let mut arr = toml_edit::Array::new();
                for item in &entry.excludes {
                    arr.push(item.as_str());
                }
                table["excludes"] = toml_edit::value(arr);
            }
            sources.push(table);
        }
        doc["sources"] = toml_edit::Item::ArrayOfTables(sources);
    }

    std::fs::write(path, doc.to_string()).context("Failed to write copit.toml")?;
    Ok(())
}

/// Add or update a source entry in `copit.toml` in the current directory.
///
/// If an entry with the same `path` already exists, it is updated in place
/// (preserving `excludes`). Otherwise a new entry is appended.
pub fn add_source_entry(
    path: &str,
    source: &str,
    version_ref: Option<&str>,
    commit: Option<&str>,
    frozen: Option<bool>,
) -> Result<()> {
    add_source_entry_to(&config_path(), path, source, version_ref, commit, frozen)
}

pub fn add_source_entry_to(
    config_file: &Path,
    path: &str,
    source: &str,
    version_ref: Option<&str>,
    commit: Option<&str>,
    frozen: Option<bool>,
) -> Result<()> {
    let content = std::fs::read_to_string(config_file).context("Failed to read copit.toml")?;
    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .context("Failed to parse copit.toml for editing")?;

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // Get or create the [[sources]] array
    if doc.get("sources").is_none() {
        doc["sources"] = toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
    }

    let sources = doc["sources"]
        .as_array_of_tables_mut()
        .context("sources should be an array of tables")?;

    // Check if entry already exists and update it
    let mut found = false;
    for table in sources.iter_mut() {
        if table.get("path").and_then(|v| v.as_str()) == Some(path) {
            table["source"] = toml_edit::value(source);

            if let Some(f) = frozen {
                if f {
                    table["frozen"] = toml_edit::value(true);
                } else {
                    table.remove("frozen");
                }
            }

            if let Some(r) = version_ref {
                table["ref"] = toml_edit::value(r);
            } else {
                table.remove("ref");
            }
            if let Some(c) = commit {
                table["commit"] = toml_edit::value(c);
            } else {
                table.remove("commit");
            }
            table["copied_at"] = toml_edit::value(&now);
            // Preserve existing excludes — don't touch it on update
            found = true;
            break;
        }
    }

    if !found {
        let mut table = toml_edit::Table::new();
        table["path"] = toml_edit::value(path);
        table["source"] = toml_edit::value(source);
        if let Some(r) = version_ref {
            table["ref"] = toml_edit::value(r);
        }
        if let Some(c) = commit {
            table["commit"] = toml_edit::value(c);
        }
        if frozen == Some(true) {
            table["frozen"] = toml_edit::value(true);
        }
        table["copied_at"] = toml_edit::value(&now);
        sources.push(table);
    }

    std::fs::write(config_file, doc.to_string()).context("Failed to write copit.toml")?;
    Ok(())
}

/// Remove source entries by path from `copit.toml` in the current directory.
///
/// Returns the list of paths that were actually found and removed.
pub fn remove_source_entries(paths: &[String]) -> Result<Vec<String>> {
    remove_source_entries_from(&config_path(), paths)
}

pub fn remove_source_entries_from(config_file: &Path, paths: &[String]) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(config_file).context("Failed to read copit.toml")?;
    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .context("Failed to parse copit.toml for editing")?;

    let mut removed = Vec::new();

    let sources = match doc
        .get_mut("sources")
        .and_then(|s| s.as_array_of_tables_mut())
    {
        Some(s) => s,
        None => return Ok(removed),
    };

    // Collect indices to remove (in reverse order to avoid shifting)
    let mut indices_to_remove: Vec<usize> = Vec::new();
    for (i, table) in sources.iter().enumerate() {
        if let Some(path) = table.get("path").and_then(|v| v.as_str()) {
            if paths.iter().any(|p| p == path) {
                indices_to_remove.push(i);
                removed.push(path.to_string());
            }
        }
    }

    for i in indices_to_remove.into_iter().rev() {
        sources.remove(i);
    }

    // Remove empty sources array
    if sources.is_empty() {
        doc.remove("sources");
    }

    std::fs::write(config_file, doc.to_string()).context("Failed to write copit.toml")?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn init_creates_config() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        let config = CopitConfig::default();
        save_config_to(&config, &config_file).unwrap();

        assert!(config_exists_in(dir.path()));

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.target, "vendor");
        assert!(loaded.sources.is_empty());
    }

    #[test]
    fn roundtrip() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        let config = CopitConfig::default();
        save_config_to(&config, &config_file).unwrap();

        add_source_entry_to(
            &config_file,
            "vendor/lib.rs",
            "github:serde-rs/serde@v1.0.219/serde/src/lib.rs",
            Some("v1.0.219"),
            Some("abc123"),
            None,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].path, "vendor/lib.rs");
        assert_eq!(loaded.sources[0].version_ref, Some("v1.0.219".to_string()));
        assert_eq!(loaded.sources[0].commit, Some("abc123".to_string()));
        assert!(!loaded.sources[0].copied_at.is_empty());
    }

    #[test]
    fn add_source_updates_existing() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        save_config_to(&CopitConfig::default(), &config_file).unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/lib.rs",
            "github:a/b@v1/path",
            Some("v1"),
            Some("sha1"),
            None,
        )
        .unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/lib.rs",
            "github:a/b@v2/path",
            Some("v2"),
            Some("sha2"),
            None,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].source, "github:a/b@v2/path");
        assert_eq!(loaded.sources[0].version_ref, Some("v2".to_string()));
        assert_eq!(loaded.sources[0].commit, Some("sha2".to_string()));
    }

    #[test]
    fn add_source_without_ref_commit() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        save_config_to(&CopitConfig::default(), &config_file).unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/file.txt",
            "https://example.com/file.txt",
            None,
            None,
            None,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources[0].version_ref, None);
        assert_eq!(loaded.sources[0].commit, None);
    }

    #[test]
    fn remove_source_entry() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        save_config_to(&CopitConfig::default(), &config_file).unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/a.rs",
            "github:a/b@v1/a",
            None,
            None,
            None,
        )
        .unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/b.rs",
            "github:a/b@v1/b",
            None,
            None,
            None,
        )
        .unwrap();

        let removed =
            remove_source_entries_from(&config_file, &["vendor/a.rs".to_string()]).unwrap();
        assert_eq!(removed, vec!["vendor/a.rs"]);

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources.len(), 1);
        assert_eq!(loaded.sources[0].path, "vendor/b.rs");
    }

    #[test]
    fn remove_all_sources() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        save_config_to(&CopitConfig::default(), &config_file).unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/a.rs",
            "github:a/b@v1/a",
            None,
            None,
            None,
        )
        .unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/b.rs",
            "github:a/b@v1/b",
            None,
            None,
            None,
        )
        .unwrap();

        let removed = remove_source_entries_from(
            &config_file,
            &["vendor/a.rs".to_string(), "vendor/b.rs".to_string()],
        )
        .unwrap();
        assert_eq!(removed.len(), 2);

        let loaded = load_config_from(&config_file).unwrap();
        assert!(loaded.sources.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_empty() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        save_config_to(&CopitConfig::default(), &config_file).unwrap();
        let removed =
            remove_source_entries_from(&config_file, &["vendor/x.rs".to_string()]).unwrap();
        assert!(removed.is_empty());
    }

    #[test]
    fn get_source_entry() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        save_config_to(&CopitConfig::default(), &config_file).unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/lib.rs",
            "github:a/b@v1/lib.rs",
            Some("v1"),
            Some("sha1"),
            None,
        )
        .unwrap();

        let entry = get_source_entry_from(&config_file, "vendor/lib.rs");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.version_ref, Some("v1".to_string()));

        assert!(get_source_entry_from(&config_file, "vendor/nonexistent.rs").is_none());
    }

    #[test]
    fn roundtrip_with_excludes() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        let config = CopitConfig {
            target: "vendor".to_string(),
            overwrite: None,
            skip: None,
            backup: None,
            sources: vec![SourceEntry {
                path: "vendor/prek-identify".to_string(),
                source: "github:j178/prek@master/crates/prek-identify".to_string(),
                version_ref: Some("master".to_string()),
                commit: Some("abc123def456".to_string()),
                copied_at: "2026-03-07T08:46:51Z".to_string(),
                excludes: vec!["Cargo.toml".to_string(), "src/lib.rs".to_string()],
                frozen: None,
                overwrite: None,
                skip: None,
                backup: None,
            }],
        };
        save_config_to(&config, &config_file).unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources[0].excludes, vec!["Cargo.toml", "src/lib.rs"]);
    }

    #[test]
    fn excludes_parsing() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        std::fs::write(
            &config_file,
            r#"
target = "vendor"

[[sources]]
path = "vendor/mylib"
source = "github:owner/repo@main/src/mylib"
ref = "main"
commit = "deadbeef"
copied_at = "2026-03-07T00:00:00Z"
excludes = ["README.md", "config.yaml"]
"#,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources[0].excludes, vec!["README.md", "config.yaml"]);
    }

    #[test]
    fn optional_fields_absent() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        std::fs::write(
            &config_file,
            r#"
target = "vendor"

[[sources]]
path = "vendor/file.txt"
source = "https://example.com/file.txt"
copied_at = "2026-03-07T00:00:00Z"
"#,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources[0].version_ref, None);
        assert_eq!(loaded.sources[0].commit, None);
        assert!(loaded.sources[0].excludes.is_empty());
    }

    #[test]
    fn load_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let config_file = dir.path().join("copit.toml");
        std::fs::write(&config_file, "this is not valid toml [[[").unwrap();
        assert!(load_config_from(&config_file).is_err());
    }

    #[test]
    fn load_missing_file() {
        let dir = TempDir::new().unwrap();
        assert!(load_config_from(&dir.path().join("copit.toml")).is_err());
    }

    #[test]
    fn multiple_sources_independent() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        save_config_to(&CopitConfig::default(), &config_file).unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/a.rs",
            "github:a/b@v1/a.rs",
            Some("v1"),
            Some("sha1"),
            None,
        )
        .unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/b.rs",
            "https://example.com/b.rs",
            None,
            None,
            None,
        )
        .unwrap();

        // Update only a.rs
        add_source_entry_to(
            &config_file,
            "vendor/a.rs",
            "github:a/b@v2/a.rs",
            Some("v2"),
            Some("sha2"),
            None,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources.len(), 2);
        assert_eq!(loaded.sources[0].version_ref, Some("v2".to_string()));
        assert_eq!(loaded.sources[1].version_ref, None);
    }

    #[test]
    fn save_then_add_preserves_target() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        let config = CopitConfig {
            target: "libs".to_string(),
            overwrite: None,
            skip: None,
            backup: None,
            sources: vec![],
        };
        save_config_to(&config, &config_file).unwrap();
        add_source_entry_to(
            &config_file,
            "libs/util.rs",
            "github:x/y@main/util.rs",
            Some("main"),
            None,
            None,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.target, "libs");
        assert_eq!(loaded.sources[0].path, "libs/util.rs");
    }

    #[test]
    fn update_preserves_excludes() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        let config = CopitConfig {
            target: "vendor".to_string(),
            overwrite: None,
            skip: None,
            backup: None,
            sources: vec![SourceEntry {
                path: "vendor/mylib".to_string(),
                source: "github:owner/repo@v1/src/mylib".to_string(),
                version_ref: Some("v1".to_string()),
                commit: Some("sha1".to_string()),
                copied_at: "2026-01-01T00:00:00Z".to_string(),
                excludes: vec!["Cargo.toml".to_string()],
                frozen: None,
                overwrite: None,
                skip: None,
                backup: None,
            }],
        };
        save_config_to(&config, &config_file).unwrap();

        add_source_entry_to(
            &config_file,
            "vendor/mylib",
            "github:owner/repo@v2/src/mylib",
            Some("v2"),
            Some("sha2"),
            None,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources[0].excludes, vec!["Cargo.toml"]);
        assert_eq!(loaded.sources[0].version_ref, Some("v2".to_string()));
    }

    #[test]
    fn frozen_roundtrip() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        let config = CopitConfig {
            target: "vendor".to_string(),
            overwrite: None,
            skip: None,
            backup: None,
            sources: vec![SourceEntry {
                path: "vendor/lib.rs".to_string(),
                source: "github:a/b@v1/lib.rs".to_string(),
                version_ref: Some("v1".to_string()),
                commit: Some("sha1".to_string()),
                copied_at: "2026-01-01T00:00:00Z".to_string(),
                excludes: vec![],
                frozen: Some(true),
                overwrite: None,
                skip: None,
                backup: None,
            }],
        };
        save_config_to(&config, &config_file).unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources[0].frozen, Some(true));
    }

    #[test]
    fn frozen_not_serialized_when_none() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        save_config_to(&CopitConfig::default(), &config_file).unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/lib.rs",
            "https://example.com/lib.rs",
            None,
            None,
            None,
        )
        .unwrap();

        let raw = std::fs::read_to_string(&config_file).unwrap();
        assert!(!raw.contains("frozen"));
    }

    #[test]
    fn freeze_then_unfreeze() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        save_config_to(&CopitConfig::default(), &config_file).unwrap();

        // Freeze
        add_source_entry_to(
            &config_file,
            "vendor/lib.rs",
            "github:a/b@v1/lib.rs",
            Some("v1"),
            Some("sha1"),
            Some(true),
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources[0].frozen, Some(true));

        // Unfreeze
        add_source_entry_to(
            &config_file,
            "vendor/lib.rs",
            "github:a/b@v2/lib.rs",
            Some("v2"),
            Some("sha2"),
            Some(false),
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources[0].frozen, None);

        let raw = std::fs::read_to_string(&config_file).unwrap();
        assert!(!raw.contains("frozen"));
    }

    #[test]
    fn update_with_none_preserves_frozen() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        save_config_to(&CopitConfig::default(), &config_file).unwrap();

        // Add with frozen
        add_source_entry_to(
            &config_file,
            "vendor/lib.rs",
            "github:a/b@v1/lib.rs",
            Some("v1"),
            Some("sha1"),
            Some(true),
        )
        .unwrap();

        // Update with frozen=None — should preserve existing frozen
        add_source_entry_to(
            &config_file,
            "vendor/lib.rs",
            "github:a/b@v2/lib.rs",
            Some("v2"),
            Some("sha2"),
            None,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources[0].frozen, Some(true));
        assert_eq!(loaded.sources[0].version_ref, Some("v2".to_string()));
    }

    fn make_config(
        overwrite: Option<bool>,
        skip: Option<bool>,
        backup: Option<bool>,
    ) -> CopitConfig {
        CopitConfig {
            target: "vendor".to_string(),
            overwrite,
            skip,
            backup,
            sources: vec![],
        }
    }

    fn make_entry(
        overwrite: Option<bool>,
        skip: Option<bool>,
        backup: Option<bool>,
    ) -> SourceEntry {
        SourceEntry {
            path: "vendor/lib.rs".to_string(),
            source: "https://example.com/lib.rs".to_string(),
            version_ref: None,
            commit: None,
            copied_at: "2026-01-01T00:00:00Z".to_string(),
            excludes: vec![],
            frozen: None,
            overwrite,
            skip,
            backup,
        }
    }

    #[test]
    fn resolved_settings_default_all_false() {
        let cfg = make_config(None, None, None);
        let s = ResolvedSettings::resolve(false, false, false, None, &cfg);
        assert!(!s.overwrite);
        assert!(!s.skip);
        assert!(!s.backup);
    }

    #[test]
    fn resolved_settings_cli_wins() {
        let cfg = make_config(None, None, None);
        let s = ResolvedSettings::resolve(true, true, true, None, &cfg);
        assert!(s.overwrite);
        assert!(s.skip);
        assert!(s.backup);
    }

    #[test]
    fn resolved_settings_config_defaults() {
        let cfg = make_config(Some(true), Some(true), Some(true));
        let s = ResolvedSettings::resolve(false, false, false, None, &cfg);
        assert!(s.overwrite);
        assert!(s.skip);
        assert!(s.backup);
    }

    #[test]
    fn resolved_settings_source_overrides_config() {
        let cfg = make_config(Some(true), Some(true), Some(true));
        let entry = make_entry(Some(false), Some(false), Some(false));
        let s = ResolvedSettings::resolve(false, false, false, Some(&entry), &cfg);
        assert!(!s.overwrite);
        assert!(!s.skip);
        assert!(!s.backup);
    }

    #[test]
    fn resolved_settings_cli_overrides_source() {
        let cfg = make_config(None, None, None);
        let entry = make_entry(Some(false), Some(false), Some(false));
        let s = ResolvedSettings::resolve(true, true, true, Some(&entry), &cfg);
        assert!(s.overwrite);
        assert!(s.skip);
        assert!(s.backup);
    }

    #[test]
    fn resolved_settings_source_none_falls_through_to_config() {
        let cfg = make_config(Some(true), None, Some(true));
        let entry = make_entry(None, None, None);
        let s = ResolvedSettings::resolve(false, false, false, Some(&entry), &cfg);
        assert!(s.overwrite);
        assert!(!s.skip);
        assert!(s.backup);
    }
}
