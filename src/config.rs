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
//! [project]
//! target = "vendor"
//!
//! [[sources]]
//! path = "vendor/lib.rs"
//! source = "github:owner/repo@v1.0/src/lib.rs"
//! ref = "v1.0"
//! commit = "abc123..."
//! copied_at = "2026-03-07T08:46:51Z"
//! exclude_modified = ["Cargo.toml"]
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "copit.toml";

/// Project-level configuration (the `[project]` table).
#[derive(Debug, Deserialize, Serialize)]
pub struct ProjectConfig {
    /// Default target directory for copied files (e.g., `"vendor"`).
    pub target: String,
}

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
    pub exclude_modified: Vec<String>,
}

/// Top-level `copit.toml` configuration.
#[derive(Debug, Deserialize, Serialize)]
pub struct CopitConfig {
    /// Project settings.
    pub project: ProjectConfig,
    /// List of tracked source entries.
    #[serde(default, rename = "sources")]
    pub sources: Vec<SourceEntry>,
}

impl Default for CopitConfig {
    fn default() -> Self {
        Self {
            project: ProjectConfig {
                target: "vendor".to_string(),
            },
            sources: Vec::new(),
        }
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

    let mut project = toml_edit::Table::new();
    project["target"] = toml_edit::value(&config.project.target);
    doc["project"] = toml_edit::Item::Table(project);

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
            table["copied_at"] = toml_edit::value(&entry.copied_at);
            if !entry.exclude_modified.is_empty() {
                let mut arr = toml_edit::Array::new();
                for item in &entry.exclude_modified {
                    arr.push(item.as_str());
                }
                table["exclude_modified"] = toml_edit::value(arr);
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
/// (preserving `exclude_modified`). Otherwise a new entry is appended.
pub fn add_source_entry(
    path: &str,
    source: &str,
    version_ref: Option<&str>,
    commit: Option<&str>,
) -> Result<()> {
    add_source_entry_to(&config_path(), path, source, version_ref, commit)
}

pub fn add_source_entry_to(
    config_file: &Path,
    path: &str,
    source: &str,
    version_ref: Option<&str>,
    commit: Option<&str>,
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
            // Preserve existing exclude_modified — don't touch it on update
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
        assert_eq!(loaded.project.target, "vendor");
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
        )
        .unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/lib.rs",
            "github:a/b@v2/path",
            Some("v2"),
            Some("sha2"),
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
        add_source_entry_to(&config_file, "vendor/a.rs", "github:a/b@v1/a", None, None).unwrap();
        add_source_entry_to(&config_file, "vendor/b.rs", "github:a/b@v1/b", None, None).unwrap();

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
        add_source_entry_to(&config_file, "vendor/a.rs", "github:a/b@v1/a", None, None).unwrap();
        add_source_entry_to(&config_file, "vendor/b.rs", "github:a/b@v1/b", None, None).unwrap();

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
        )
        .unwrap();

        let entry = get_source_entry_from(&config_file, "vendor/lib.rs");
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.version_ref, Some("v1".to_string()));

        assert!(get_source_entry_from(&config_file, "vendor/nonexistent.rs").is_none());
    }

    #[test]
    fn roundtrip_with_exclude_modified() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        let config = CopitConfig {
            project: ProjectConfig {
                target: "vendor".to_string(),
            },
            sources: vec![SourceEntry {
                path: "vendor/prek-identify".to_string(),
                source: "github:j178/prek@master/crates/prek-identify".to_string(),
                version_ref: Some("master".to_string()),
                commit: Some("abc123def456".to_string()),
                copied_at: "2026-03-07T08:46:51Z".to_string(),
                exclude_modified: vec!["Cargo.toml".to_string(), "src/lib.rs".to_string()],
            }],
        };
        save_config_to(&config, &config_file).unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(
            loaded.sources[0].exclude_modified,
            vec!["Cargo.toml", "src/lib.rs"]
        );
    }

    #[test]
    fn exclude_modified_parsing() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        std::fs::write(
            &config_file,
            r#"
[project]
target = "vendor"

[[sources]]
path = "vendor/mylib"
source = "github:owner/repo@main/src/mylib"
ref = "main"
commit = "deadbeef"
copied_at = "2026-03-07T00:00:00Z"
exclude_modified = ["README.md", "config.yaml"]
"#,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(
            loaded.sources[0].exclude_modified,
            vec!["README.md", "config.yaml"]
        );
    }

    #[test]
    fn optional_fields_absent() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        std::fs::write(
            &config_file,
            r#"
[project]
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
        assert!(loaded.sources[0].exclude_modified.is_empty());
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
        )
        .unwrap();
        add_source_entry_to(
            &config_file,
            "vendor/b.rs",
            "https://example.com/b.rs",
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
            project: ProjectConfig {
                target: "libs".to_string(),
            },
            sources: vec![],
        };
        save_config_to(&config, &config_file).unwrap();
        add_source_entry_to(
            &config_file,
            "libs/util.rs",
            "github:x/y@main/util.rs",
            Some("main"),
            None,
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.project.target, "libs");
        assert_eq!(loaded.sources[0].path, "libs/util.rs");
    }

    #[test]
    fn update_preserves_exclude_modified() {
        let dir = TempDir::new().unwrap();
        let config_file = config_path_in(dir.path());

        let config = CopitConfig {
            project: ProjectConfig {
                target: "vendor".to_string(),
            },
            sources: vec![SourceEntry {
                path: "vendor/mylib".to_string(),
                source: "github:owner/repo@v1/src/mylib".to_string(),
                version_ref: Some("v1".to_string()),
                commit: Some("sha1".to_string()),
                copied_at: "2026-01-01T00:00:00Z".to_string(),
                exclude_modified: vec!["Cargo.toml".to_string()],
            }],
        };
        save_config_to(&config, &config_file).unwrap();

        add_source_entry_to(
            &config_file,
            "vendor/mylib",
            "github:owner/repo@v2/src/mylib",
            Some("v2"),
            Some("sha2"),
        )
        .unwrap();

        let loaded = load_config_from(&config_file).unwrap();
        assert_eq!(loaded.sources[0].exclude_modified, vec!["Cargo.toml"]);
        assert_eq!(loaded.sources[0].version_ref, Some("v2".to_string()));
    }
}
