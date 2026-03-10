use predicates::prelude::*;
use tempfile::TempDir;

use super::{copit_cmd, create_zip};

// ---------------------------------------------------------------------------
// Error cases (no network)
// ---------------------------------------------------------------------------

#[test]
fn no_sources() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .arg("add")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("No sources specified"));
}

#[test]
fn no_config() {
    let dir = TempDir::new().unwrap();

    copit_cmd()
        .args(["add", "github:owner/repo@v1/path"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("copit.toml"));
}

// ---------------------------------------------------------------------------
// HTTP source — mock server
// ---------------------------------------------------------------------------

#[tokio::test]
async fn http_source_full_flow() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/utils.rs")
        .with_status(200)
        .with_body("fn helper() {}")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args(["add", &format!("{}/utils.rs", server.url()), "--overwrite"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Copied: vendor/utils.rs"));

    let content = std::fs::read_to_string(dir.path().join("vendor/utils.rs")).unwrap();
    assert_eq!(content, "fn helper() {}");

    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.sources.len(), 1);
    assert_eq!(config.sources[0].path, "vendor/utils.rs");

    mock.assert_async().await;
}

#[tokio::test]
async fn skip_existing() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/file.rs")
        .with_status(200)
        .with_body("new content")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/file.rs"), "old content").unwrap();

    copit_cmd()
        .args(["add", &format!("{}/file.rs", server.url()), "--skip"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Skipping (already exists)"));

    let content = std::fs::read_to_string(dir.path().join("vendor/file.rs")).unwrap();
    assert_eq!(content, "old content");

    mock.assert_async().await;
}

#[tokio::test]
async fn overwrite_existing() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/file.rs")
        .with_status(200)
        .with_body("new content")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/file.rs"), "old content").unwrap();

    copit_cmd()
        .args(["add", &format!("{}/file.rs", server.url()), "--overwrite"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Copied: vendor/file.rs"));

    let content = std::fs::read_to_string(dir.path().join("vendor/file.rs")).unwrap();
    assert_eq!(content, "new content");

    mock.assert_async().await;
}

#[tokio::test]
async fn custom_target() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/lib.rs")
        .with_status(200)
        .with_body("pub fn run() {}")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args([
            "add",
            &format!("{}/lib.rs", server.url()),
            "--to",
            "libs",
            "--overwrite",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Copied: libs/lib.rs"));

    assert!(dir.path().join("libs/lib.rs").exists());
    mock.assert_async().await;
}

#[tokio::test]
async fn multiple_sources() {
    let mut server = mockito::Server::new_async().await;
    let mock_a = server
        .mock("GET", "/a.txt")
        .with_status(200)
        .with_body("file a")
        .create_async()
        .await;
    let mock_b = server
        .mock("GET", "/b.txt")
        .with_status(200)
        .with_body("file b")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args([
            "add",
            &format!("{}/a.txt", server.url()),
            &format!("{}/b.txt", server.url()),
            "--overwrite",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Copied: vendor/a.txt")
                .and(predicates::str::contains("Copied: vendor/b.txt")),
        );

    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.sources.len(), 2);

    mock_a.assert_async().await;
    mock_b.assert_async().await;
}

#[tokio::test]
async fn fetch_failure() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/fail.rs")
        .with_status(500)
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args(["add", &format!("{}/fail.rs", server.url())])
        .current_dir(dir.path())
        .assert()
        .failure();

    mock.assert_async().await;
}

// ---------------------------------------------------------------------------
// ZIP source
// ---------------------------------------------------------------------------

#[tokio::test]
async fn zip_source() {
    let zip_data = create_zip(&[
        ("src/lib.rs", b"pub fn add() {}"),
        ("src/util.rs", b"pub fn util() {}"),
    ]);

    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/archive.zip")
        .with_status(200)
        .with_body(&zip_data)
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args([
            "add",
            &format!("{}/archive.zip#src/lib.rs", server.url()),
            "--overwrite",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Copied: vendor/lib.rs"));

    let content = std::fs::read_to_string(dir.path().join("vendor/lib.rs")).unwrap();
    assert_eq!(content, "pub fn add() {}");

    mock.assert_async().await;
}

// ---------------------------------------------------------------------------
// already tracked
// ---------------------------------------------------------------------------

#[tokio::test]
async fn already_tracked_skips() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/file.rs")
        .with_status(200)
        .with_body("new content")
        .expect(0)
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"[project]
target = "vendor"

[[sources]]
path = "vendor/file.rs"
source = "https://old-server/file.rs"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/file.rs"), "original").unwrap();

    copit_cmd()
        .args(["add", &format!("{}/file.rs", server.url())])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Already tracked"));

    // File unchanged
    let content = std::fs::read_to_string(dir.path().join("vendor/file.rs")).unwrap();
    assert_eq!(content, "original");

    // Config unchanged — still 1 source
    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.sources.len(), 1);
    assert_eq!(config.sources[0].source, "https://old-server/file.rs");
}

#[tokio::test]
async fn add_with_freeze_flag() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/lib.rs")
        .with_status(200)
        .with_body("fn lib() {}")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args(["add", &format!("{}/lib.rs", server.url()), "--freeze"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Copied: vendor/lib.rs"));

    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.sources[0].frozen, Some(true));

    mock.assert_async().await;
}

#[tokio::test]
async fn add_without_freeze_flag() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/lib.rs")
        .with_status(200)
        .with_body("fn lib() {}")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args(["add", &format!("{}/lib.rs", server.url())])
        .current_dir(dir.path())
        .assert()
        .success();

    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.sources[0].frozen, None);

    mock.assert_async().await;
}
