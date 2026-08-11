//! GitHub source fetching.
//!
//! Downloads files from GitHub repositories by fetching the repository's
//! ZIP archive and extracting the requested path. Supports authentication
//! via `GITHUB_TOKEN` or `GH_TOKEN` environment variables for private
//! repositories and higher rate limits.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;

use super::zip::{extract_from_bytes, ExtractedFiles};

const GITHUB_ARCHIVE_BASE: &str = "https://github.com";
const GITHUB_API_BASE: &str = "https://api.github.com";

pub const LICENSE_NAMES: &[&str] = &[
    "LICENSE",
    "LICENSE.md",
    "LICENSE.txt",
    "LICENSE-MIT",
    "LICENSE-APACHE",
    "LICENCE",
    "LICENCE.md",
    "LICENCE.txt",
    "COPYING",
    "COPYING.md",
];

/// Result of fetching from a GitHub repository, separating requested source
/// files from license files found at the repository root.
pub struct GitHubFetchResult {
    /// The files matching the requested path inside the repository.
    pub files: ExtractedFiles,
    /// License files (e.g. `LICENSE`, `LICENSE-MIT`) found at the repo root.
    pub license_files: Vec<(String, Vec<u8>)>,
}

/// Build a reqwest client with optional GitHub token authentication.
/// Checks `GITHUB_TOKEN` and `GH_TOKEN` environment variables.
fn github_client() -> Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("User-Agent", "copit".parse().unwrap());

    if let Ok(token) = std::env::var("GITHUB_TOKEN").or_else(|_| std::env::var("GH_TOKEN")) {
        if !token.is_empty() {
            headers.insert(
                "Authorization",
                format!("Bearer {token}")
                    .parse()
                    .context("Invalid GITHUB_TOKEN value")?,
            );
        }
    }

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .context("Failed to build HTTP client")
}

/// Archives already downloaded during this run, keyed by URL.
///
/// One repository archive serves the registry index and every component in it, so
/// without this a five-component install downloads the same zip six times. Scoped to
/// the process, so each command still sees fresh data.
static ARCHIVE_CACHE: std::sync::OnceLock<tokio::sync::Mutex<HashMap<String, Arc<Vec<u8>>>>> =
    std::sync::OnceLock::new();

async fn archive_bytes(url: &str, owner: &str, repo: &str, version: &str) -> Result<Arc<Vec<u8>>> {
    let cache = ARCHIVE_CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()));
    let mut cache = cache.lock().await;

    if let Some(bytes) = cache.get(url) {
        return Ok(bytes.clone());
    }

    println!("Downloading {url}...");
    let client = github_client()?;
    let response = client.get(url).send().await.with_context(|| {
        format!("Failed to download GitHub archive for {owner}/{repo}@{version}")
    })?;

    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("HTTP {status} when fetching {url}");
    }

    let bytes = Arc::new(
        response
            .bytes()
            .await
            .with_context(|| format!("Failed to read response body from {url}"))?
            .to_vec(),
    );

    cache.insert(url.to_string(), Arc::clone(&bytes));
    Ok(bytes)
}

/// Fetch files from a GitHub repository by downloading the ZIP archive.
pub async fn fetch_github(
    owner: &str,
    repo: &str,
    version: &str,
    path: &str,
) -> Result<GitHubFetchResult> {
    fetch_github_from(GITHUB_ARCHIVE_BASE, owner, repo, version, path).await
}

async fn fetch_github_from(
    base_url: &str,
    owner: &str,
    repo: &str,
    version: &str,
    path: &str,
) -> Result<GitHubFetchResult> {
    let url = format!("{base_url}/{owner}/{repo}/archive/{version}.zip");
    let bytes = archive_bytes(&url, owner, repo, version).await?;

    // GitHub archives have a top-level directory like `repo-version/`
    // We need to find this prefix to strip it
    let strip_prefix = find_archive_prefix(&bytes)?;

    let files = extract_from_bytes(&bytes, Some(path), Some(&strip_prefix))?;

    let mut license_files = Vec::new();
    let all_root = extract_from_bytes(&bytes, None, Some(&strip_prefix))?;
    for (name, content) in all_root {
        if name.contains('/') {
            continue;
        }
        if LICENSE_NAMES.iter().any(|l| l.eq_ignore_ascii_case(&name)) {
            license_files.push((name, content))
        }
    }
    Ok(GitHubFetchResult {
        files,
        license_files,
    })
}

/// Resolve the commit SHA for a given version ref (branch/tag/sha) via the GitHub API.
/// Returns None if the API call fails (e.g. rate limit, network error).
pub async fn resolve_commit_sha(owner: &str, repo: &str, version: &str) -> Option<String> {
    // Every component of a registry resolves the same owner/repo/ref, and the answer
    // cannot change mid-command. Without this an N-component install spends N of the
    // 60/hour unauthenticated budget to learn one SHA.
    static SHA_CACHE: std::sync::OnceLock<tokio::sync::Mutex<HashMap<String, Option<String>>>> =
        std::sync::OnceLock::new();

    let key = format!("{owner}/{repo}@{version}");
    let cache = SHA_CACHE.get_or_init(|| tokio::sync::Mutex::new(HashMap::new()));
    let mut cache = cache.lock().await;

    if let Some(sha) = cache.get(&key) {
        return sha.clone();
    }

    let sha = resolve_commit_sha_from(GITHUB_API_BASE, owner, repo, version).await;
    cache.insert(key, sha.clone());
    sha
}

async fn resolve_commit_sha_from(
    api_base: &str,
    owner: &str,
    repo: &str,
    version: &str,
) -> Option<String> {
    let url = format!("{api_base}/repos/{owner}/{repo}/commits/{version}");

    let client = github_client().ok()?;
    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await
        .ok()?;

    if !response.status().is_success() {
        return None;
    }

    let json: serde_json::Value = response.json().await.ok()?;
    json.get("sha")?.as_str().map(|s| s.to_string())
}

/// Find the common top-level directory prefix in a ZIP archive.
fn find_archive_prefix(bytes: &[u8]) -> Result<String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).context("Failed to open ZIP archive")?;

    if archive.is_empty() {
        anyhow::bail!("ZIP archive is empty");
    }

    let first = archive
        .by_index(0)
        .context("Failed to read first ZIP entry")?;
    let name = first.name().to_string();

    // GitHub archives always have a top-level dir like `repo-ref/`
    let prefix = match name.find('/') {
        Some(idx) => &name[..=idx],
        None => &name,
    };

    Ok(prefix.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_github_zip(prefix: &str, files: &[(&str, &[u8])]) -> Vec<u8> {
        let buf = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, contents) in files {
            let full_name = format!("{prefix}{name}");
            zip.start_file(&full_name, options).unwrap();
            zip.write_all(contents).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn test_find_archive_prefix() {
        let data = create_github_zip("repo-v1.0/", &[("src/lib.rs", b"code")]);
        let prefix = find_archive_prefix(&data).unwrap();
        assert_eq!(prefix, "repo-v1.0/");
    }

    #[test]
    fn test_find_archive_prefix_empty_zip() {
        let buf = std::io::Cursor::new(Vec::new());
        let zip = zip::ZipWriter::new(buf);
        let data = zip.finish().unwrap().into_inner();
        assert!(find_archive_prefix(&data).is_err());
    }

    #[test]
    fn test_find_archive_prefix_no_slash() {
        let buf = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(buf);
        let options = zip::write::SimpleFileOptions::default();
        zip.start_file("single-file", options).unwrap();
        zip.write_all(b"data").unwrap();
        let data = zip.finish().unwrap().into_inner();
        let prefix = find_archive_prefix(&data).unwrap();
        assert_eq!(prefix, "single-file");
    }

    #[tokio::test]
    async fn one_repository_archive_is_downloaded_once_per_run() {
        // A registry install fetches the index and then every component from the same
        // archive. Without the cache that was one full repo download each time.
        let mut server = mockito::Server::new_async().await;
        // The archive cache is keyed by URL and mockito reuses server ports, so every
        // test here needs a version no other test uses or it may be served a cached
        // zip from another test instead of hitting its own mock.
        let zip = create_github_zip(
            "repo-v1-cached/",
            &[
                ("registry.json", b"{}"),
                ("components/a/mod.rs", b"a"),
                ("components/b/mod.rs", b"b"),
            ],
        );
        let archive = server
            .mock("GET", "/owner/repo/archive/v1-cached.zip")
            .with_status(200)
            .with_body(zip)
            .expect(1)
            .create_async()
            .await;

        for path in ["registry.json", "components/a", "components/b"] {
            fetch_github_from(&server.url(), "owner", "repo", "v1-cached", path)
                .await
                .unwrap();
        }

        archive.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_commit_sha_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/repos/owner/repo/commits/main")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"sha": "abc123def456"}"#)
            .create_async()
            .await;

        let sha = resolve_commit_sha_from(&server.url(), "owner", "repo", "main").await;
        assert_eq!(sha, Some("abc123def456".to_string()));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_commit_sha_404() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/repos/owner/repo/commits/nonexistent")
            .with_status(404)
            .create_async()
            .await;

        let sha = resolve_commit_sha_from(&server.url(), "owner", "repo", "nonexistent").await;
        assert_eq!(sha, None);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_commit_sha_invalid_json() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/repos/owner/repo/commits/main")
            .with_status(200)
            .with_body("not json")
            .create_async()
            .await;

        let sha = resolve_commit_sha_from(&server.url(), "owner", "repo", "main").await;
        assert_eq!(sha, None);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_resolve_commit_sha_missing_sha_field() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/repos/owner/repo/commits/main")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"message": "no sha here"}"#)
            .create_async()
            .await;

        let sha = resolve_commit_sha_from(&server.url(), "owner", "repo", "main").await;
        assert_eq!(sha, None);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_github_from_mock() {
        let zip_data = create_github_zip(
            "repo-main/",
            &[
                ("src/lib.rs", b"pub fn hello() {}"),
                ("src/util.rs", b"pub fn util() {}"),
            ],
        );

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/owner/repo/archive/main.zip")
            .with_status(200)
            .with_body(&zip_data)
            .create_async()
            .await;

        let result = fetch_github_from(&server.url(), "owner", "repo", "main", "src")
            .await
            .unwrap();
        assert_eq!(result.files.len(), 2);
        assert_eq!(result.files["src/lib.rs"], b"pub fn hello() {}");
        assert_eq!(result.files["src/util.rs"], b"pub fn util() {}");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_github_single_file() {
        let zip_data = create_github_zip(
            "repo-v1/",
            &[("src/lib.rs", b"code"), ("README.md", b"readme")],
        );

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/owner/repo/archive/v1.zip")
            .with_status(200)
            .with_body(&zip_data)
            .create_async()
            .await;

        let result = fetch_github_from(&server.url(), "owner", "repo", "v1", "README.md")
            .await
            .unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files["README.md"], b"readme");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_github_extracts_license_files() {
        let zip_data = create_github_zip(
            "repo-main-license/",
            &[
                ("src/lib.rs", b"pub fn hello() {}"),
                ("LICENSE", b"MIT License"),
                ("LICENSE-APACHE", b"Apache License"),
            ],
        );

        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/owner/repo/archive/main-license.zip")
            .with_status(200)
            .with_body(&zip_data)
            .create_async()
            .await;

        let result = fetch_github_from(&server.url(), "owner", "repo", "main-license", "src")
            .await
            .unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.license_files.len(), 2);

        let license_names: Vec<&str> = result
            .license_files
            .iter()
            .map(|(n, _)| n.as_str())
            .collect();
        assert!(license_names.contains(&"LICENSE"));
        assert!(license_names.contains(&"LICENSE-APACHE"));

        let mit = result
            .license_files
            .iter()
            .find(|(n, _)| n == "LICENSE")
            .unwrap();
        assert_eq!(mit.1, b"MIT License");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetch_github_404() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/owner/repo/archive/nonexistent.zip")
            .with_status(404)
            .create_async()
            .await;

        let result = fetch_github_from(&server.url(), "owner", "repo", "nonexistent", "src").await;
        assert!(result.is_err());
        mock.assert_async().await;
    }
}
