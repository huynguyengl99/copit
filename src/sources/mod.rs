//! Source parsing and fetching.
//!
//! This module defines the [`Source`] enum representing where code can be
//! copied from, and provides [`parse_source`] to convert user-provided
//! strings into structured source descriptors.
//!
//! Submodules handle the actual fetching:
//! - [`github`] — Download files from GitHub repository archives
//! - [`http`] — Download raw files from HTTP/HTTPS URLs
//! - [`zip`] — Extract files from ZIP archives

pub mod github;
pub mod http;
pub mod zip;

use anyhow::{bail, Result};

/// A parsed source location from which files can be fetched.
///
/// Created by [`parse_source`] from a user-provided string like
/// `github:owner/repo@v1.0/src/lib.rs` or `https://example.com/file.txt`.
#[derive(Debug, Clone)]
pub enum Source {
    GitHub {
        owner: String,
        repo: String,
        version: String,
        path: String,
    },
    Http {
        url: String,
    },
    Zip {
        url: String,
        inner_path: Option<String>,
    },
}

impl Source {
    /// Returns a human-readable source string for config tracking.
    pub fn to_source_string(&self) -> String {
        match self {
            Source::GitHub {
                owner,
                repo,
                version,
                path,
            } => format!("github:{owner}/{repo}@{version}/{path}"),
            Source::Http { url } => url.clone(),
            Source::Zip { url, inner_path } => match inner_path {
                Some(p) => format!("{url}#{p}"),
                None => url.clone(),
            },
        }
    }

    /// Returns a new Source with a different version string.
    /// For GitHub sources, updates the version field and the source string.
    /// For non-GitHub sources, returns the source unchanged.
    pub fn with_version(&self, new_version: &str) -> Source {
        match self {
            Source::GitHub {
                owner, repo, path, ..
            } => Source::GitHub {
                owner: owner.clone(),
                repo: repo.clone(),
                version: new_version.to_string(),
                path: path.clone(),
            },
            other => other.clone(),
        }
    }

    /// Returns a suggested filename/path from this source.
    pub fn suggested_name(&self) -> String {
        match self {
            Source::GitHub { path, .. } => path.clone(),
            Source::Http { url } => url.rsplit('/').next().unwrap_or("downloaded").to_string(),
            Source::Zip {
                inner_path, url, ..
            } => inner_path
                .as_deref()
                .unwrap_or_else(|| url.rsplit('/').next().unwrap_or("archive"))
                .to_string(),
        }
    }
}

/// Parse a source string into a [`Source`] enum.
///
/// # Supported formats
///
/// | Input | Parsed as |
/// |-------|-----------|
/// | `github:owner/repo@version/path` | [`Source::GitHub`] |
/// | `gh:owner/repo@version/path` | [`Source::GitHub`] (alias) |
/// | `https://example.com/file.txt` | [`Source::Http`] |
/// | `https://example.com/a.zip` | [`Source::Zip`] (no inner path) |
/// | `https://example.com/a.zip#path` | [`Source::Zip`] (with inner path) |
///
/// # Errors
///
/// Returns an error if the input doesn't match any known format or has
/// missing/empty components (e.g., empty owner, repo, or version).
pub fn parse_source(input: &str) -> Result<Source> {
    if let Some(rest) = input
        .strip_prefix("github:")
        .or_else(|| input.strip_prefix("gh:"))
    {
        return parse_github_source(rest);
    }

    if input.starts_with("http://") || input.starts_with("https://") {
        if input.starts_with("http://") {
            eprintln!("Warning: Using insecure HTTP connection for {input}. Consider using HTTPS instead.");
        }
        // Check for zip URL with inner path
        if let Some((url, inner_path)) = input.split_once('#') {
            if url.ends_with(".zip") {
                return Ok(Source::Zip {
                    url: url.to_string(),
                    inner_path: Some(inner_path.to_string()),
                });
            }
        }

        if input.ends_with(".zip") {
            return Ok(Source::Zip {
                url: input.to_string(),
                inner_path: None,
            });
        }

        return Ok(Source::Http {
            url: input.to_string(),
        });
    }

    bail!("Unknown source format: {input}\nExpected: github:owner/repo@version/path (or gh:), https://..., or url.zip#path")
}

fn parse_github_source(input: &str) -> Result<Source> {
    // Format: owner/repo@version/path
    let (owner_repo, rest) = input
        .split_once('@')
        .ok_or_else(|| anyhow::anyhow!("GitHub source must contain @version: {input}"))?;

    let (owner, repo) = owner_repo
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("GitHub source must be owner/repo: {owner_repo}"))?;

    // rest is version/path/to/file
    let (version, path) = rest.split_once('/').ok_or_else(|| {
        anyhow::anyhow!("GitHub source must contain a path after version: {rest}")
    })?;

    if owner.is_empty() || repo.is_empty() || version.is_empty() || path.is_empty() {
        bail!("GitHub source has empty components: github:{input}");
    }

    Ok(Source::GitHub {
        owner: owner.to_string(),
        repo: repo.to_string(),
        version: version.to_string(),
        path: path.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- parse_source: GitHub --

    #[test]
    fn parse_github_full() {
        let s = parse_source("github:serde-rs/serde@v1.0.219/serde/src/lib.rs").unwrap();
        match s {
            Source::GitHub {
                owner,
                repo,
                version,
                path,
            } => {
                assert_eq!(owner, "serde-rs");
                assert_eq!(repo, "serde");
                assert_eq!(version, "v1.0.219");
                assert_eq!(path, "serde/src/lib.rs");
            }
            _ => panic!("Expected GitHub"),
        }
    }

    #[test]
    fn parse_github_gh_alias() {
        let s = parse_source("gh:owner/repo@main/src/utils.rs").unwrap();
        match s {
            Source::GitHub {
                owner,
                repo,
                version,
                path,
            } => {
                assert_eq!(owner, "owner");
                assert_eq!(repo, "repo");
                assert_eq!(version, "main");
                assert_eq!(path, "src/utils.rs");
            }
            _ => panic!("Expected GitHub"),
        }
    }

    #[test]
    fn parse_github_empty_components() {
        assert!(parse_source("github:/repo@v1/path").is_err());
        assert!(parse_source("github:owner/@v1/path").is_err());
        assert!(parse_source("github:owner/repo@/path").is_err());
        assert!(parse_source("github:owner/repo@v1/").is_err());
    }

    #[test]
    fn parse_github_no_path() {
        assert!(parse_source("github:owner/repo@v1").is_err());
    }

    #[test]
    fn parse_github_no_at() {
        assert!(parse_source("github:missing-at").is_err());
    }

    // -- parse_source: HTTP --

    #[test]
    fn parse_http() {
        match parse_source("https://example.com/file.txt").unwrap() {
            Source::Http { url } => assert_eq!(url, "https://example.com/file.txt"),
            _ => panic!("Expected Http"),
        }
    }

    #[test]
    fn parse_http_scheme() {
        match parse_source("http://example.com/file.txt").unwrap() {
            Source::Http { url } => assert_eq!(url, "http://example.com/file.txt"),
            _ => panic!("Expected Http"),
        }
    }

    // -- parse_source: Zip --

    #[test]
    fn parse_zip_with_inner() {
        match parse_source("https://example.com/archive.zip#src/main.rs").unwrap() {
            Source::Zip { url, inner_path } => {
                assert_eq!(url, "https://example.com/archive.zip");
                assert_eq!(inner_path, Some("src/main.rs".to_string()));
            }
            _ => panic!("Expected Zip"),
        }
    }

    #[test]
    fn parse_zip_without_inner() {
        match parse_source("https://example.com/archive.zip").unwrap() {
            Source::Zip { url, inner_path } => {
                assert_eq!(url, "https://example.com/archive.zip");
                assert_eq!(inner_path, None);
            }
            _ => panic!("Expected Zip"),
        }
    }

    // -- parse_source: invalid --

    #[test]
    fn parse_invalid() {
        assert!(parse_source("ftp://example.com").is_err());
        assert!(parse_source("random-string").is_err());
    }

    // -- to_source_string --

    #[test]
    fn to_source_string_all_variants() {
        let gh = parse_source("github:owner/repo@main/src/lib.rs").unwrap();
        assert_eq!(gh.to_source_string(), "github:owner/repo@main/src/lib.rs");

        let http = parse_source("https://example.com/file.txt").unwrap();
        assert_eq!(http.to_source_string(), "https://example.com/file.txt");

        let zip_with = parse_source("https://example.com/a.zip#inner/path").unwrap();
        assert_eq!(
            zip_with.to_source_string(),
            "https://example.com/a.zip#inner/path"
        );

        let zip_none = parse_source("https://example.com/a.zip").unwrap();
        assert_eq!(zip_none.to_source_string(), "https://example.com/a.zip");
    }

    // -- with_version --

    #[test]
    fn with_version_github() {
        let s = parse_source("github:owner/repo@v1.0/src/lib.rs").unwrap();
        let updated = s.with_version("v2.0");
        match &updated {
            Source::GitHub { version, path, .. } => {
                assert_eq!(version, "v2.0");
                assert_eq!(path, "src/lib.rs");
            }
            _ => panic!("Expected GitHub"),
        }
        assert_eq!(
            updated.to_source_string(),
            "github:owner/repo@v2.0/src/lib.rs"
        );
    }

    #[test]
    fn with_version_http_unchanged() {
        let s = parse_source("https://example.com/file.txt").unwrap();
        let updated = s.with_version("v2.0");
        match &updated {
            Source::Http { url } => assert_eq!(url, "https://example.com/file.txt"),
            _ => panic!("Expected Http"),
        }
    }

    #[test]
    fn with_version_zip_unchanged() {
        let s = parse_source("https://example.com/a.zip#src/main.rs").unwrap();
        let updated = s.with_version("v3.0");
        match &updated {
            Source::Zip { url, inner_path } => {
                assert_eq!(url, "https://example.com/a.zip");
                assert_eq!(inner_path.as_deref(), Some("src/main.rs"));
            }
            _ => panic!("Expected Zip"),
        }
    }

    #[test]
    fn with_version_roundtrip() {
        let s = parse_source("gh:owner/repo@v1/path/to/file.rs").unwrap();
        let updated = s.with_version("v2");
        let reparsed = parse_source(&updated.to_source_string()).unwrap();
        match reparsed {
            Source::GitHub { version, path, .. } => {
                assert_eq!(version, "v2");
                assert_eq!(path, "path/to/file.rs");
            }
            _ => panic!("Expected GitHub"),
        }
    }

    // -- suggested_name --

    #[test]
    fn suggested_name_all_variants() {
        let gh = parse_source("github:owner/repo@v1/src/lib.rs").unwrap();
        assert_eq!(gh.suggested_name(), "src/lib.rs");

        let http = parse_source("https://example.com/file.txt").unwrap();
        assert_eq!(http.suggested_name(), "file.txt");

        let zip_with = parse_source("https://example.com/a.zip#src/utils.rs").unwrap();
        assert_eq!(zip_with.suggested_name(), "src/utils.rs");

        let zip_none = parse_source("https://example.com/archive.zip").unwrap();
        assert_eq!(zip_none.suggested_name(), "archive.zip");
    }
}
