use predicates::prelude::*;
use tempfile::TempDir;

use super::{copit_cmd, create_zip};

#[test]
fn no_args() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .arg("update")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("No paths specified"));
}

#[test]
fn no_config() {
    let dir = TempDir::new().unwrap();

    copit_cmd()
        .args(["update", "vendor/lib.rs"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("copit.toml"));
}

#[test]
fn nonexistent_path() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args(["update", "vendor/nonexistent"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("Source not found"));
}

#[test]
fn help() {
    copit_cmd()
        .args(["update", "--help"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("--ref")
                .and(predicates::str::contains("--backup"))
                .and(predicates::str::contains("--overwrite"))
                .and(predicates::str::contains("--skip")),
        );
}

#[tokio::test]
async fn http_source() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/file.rs")
        .with_status(200)
        .with_body("updated v2")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    let source_url = format!("{}/file.rs", server.url());
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/file.rs"
source = "{source_url}"
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/file.rs"), "old v1").unwrap();

    copit_cmd()
        .args(["update", "vendor/file.rs", "--overwrite"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Updated: vendor/file.rs"));

    let content = std::fs::read_to_string(dir.path().join("vendor/file.rs")).unwrap();
    assert_eq!(content, "updated v2");

    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_ne!(config.sources[0].copied_at, "2026-01-01T00:00:00Z");

    mock.assert_async().await;
}

#[tokio::test]
async fn excludes_modified() {
    let zip_data = create_zip(&[
        ("mylib/src/lib.rs", b"new lib"),
        ("mylib/Cargo.toml", b"new cargo"),
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
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/mylib"
source = "{}/archive.zip#mylib"
copied_at = "2026-01-01T00:00:00Z"
excludes = ["Cargo.toml"]
"#,
            server.url()
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor/mylib/src")).unwrap();
    std::fs::write(dir.path().join("vendor/mylib/src/lib.rs"), "old lib").unwrap();
    std::fs::write(dir.path().join("vendor/mylib/Cargo.toml"), "modified cargo").unwrap();

    copit_cmd()
        .args(["update", "vendor/mylib", "--overwrite"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Updated:")
                .and(predicates::str::contains("Skipped (modified)")),
        );

    let lib = std::fs::read_to_string(dir.path().join("vendor/mylib/src/lib.rs")).unwrap();
    assert_eq!(lib, "new lib");

    let cargo = std::fs::read_to_string(dir.path().join("vendor/mylib/Cargo.toml")).unwrap();
    assert_eq!(cargo, "modified cargo");

    mock.assert_async().await;
}

#[tokio::test]
async fn excludes_modified_with_backup() {
    let zip_data = create_zip(&[
        ("mylib/Cargo.toml", b"new cargo"),
        ("mylib/src/lib.rs", b"new lib"),
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
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/mylib"
source = "{}/archive.zip#mylib"
copied_at = "2026-01-01T00:00:00Z"
excludes = ["Cargo.toml"]
"#,
            server.url()
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor/mylib/src")).unwrap();
    std::fs::write(
        dir.path().join("vendor/mylib/Cargo.toml"),
        "modified locally",
    )
    .unwrap();
    std::fs::write(dir.path().join("vendor/mylib/src/lib.rs"), "old lib").unwrap();

    copit_cmd()
        .args(["update", "vendor/mylib", "--backup", "--overwrite"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("backup"));

    let content = std::fs::read_to_string(dir.path().join("vendor/mylib/Cargo.toml")).unwrap();
    assert_eq!(content, "modified locally");

    let orig = dir.path().join("vendor/mylib/Cargo.toml.orig");
    assert!(orig.exists(), ".orig backup should exist");
    assert_eq!(std::fs::read_to_string(&orig).unwrap(), "new cargo");

    let lib = std::fs::read_to_string(dir.path().join("vendor/mylib/src/lib.rs")).unwrap();
    assert_eq!(lib, "new lib");

    mock.assert_async().await;
}

#[tokio::test]
async fn creates_parent_dirs() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/deep.rs")
        .with_status(200)
        .with_body("deep file")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    let source_url = format!("{}/deep.rs", server.url());
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/deep/nested/deep.rs"
source = "{source_url}"
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();

    copit_cmd()
        .args(["update", "vendor/deep/nested/deep.rs"])
        .current_dir(dir.path())
        .assert()
        .success();

    assert!(dir.path().join("vendor/deep/nested/deep.rs").exists());
    mock.assert_async().await;
}

#[tokio::test]
async fn fetch_failure() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/file.rs")
        .with_status(500)
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    let source_url = format!("{}/file.rs", server.url());
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/file.rs"
source = "{source_url}"
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();

    copit_cmd()
        .args(["update", "vendor/file.rs"])
        .current_dir(dir.path())
        .assert()
        .failure();

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
    let source_url = format!("{}/file.rs", server.url());
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/file.rs"
source = "{source_url}"
copied_at = "2026-01-01T00:00:00Z"
            "#
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/file.rs"), b"old content").unwrap();

    copit_cmd()
        .args(["update", "vendor/file.rs", "--skip"])
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
    let source_url = format!("{}/file.rs", server.url());
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/file.rs"
source = "{source_url}"
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/file.rs"), "old content").unwrap();

    copit_cmd()
        .args(["update", "vendor/file.rs", "--overwrite"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Updated: vendor/file.rs"));

    let content = std::fs::read_to_string(dir.path().join("vendor/file.rs")).unwrap();
    assert_eq!(content, "new content");

    mock.assert_async().await;
}

#[tokio::test]
async fn skips_frozen_source() {
    let mut server = mockito::Server::new_async().await;
    let source_url = format!("{}/file.rs", server.url());

    let mock = server
        .mock("GET", "/file.rs")
        .with_status(200)
        .with_body("new content")
        .expect(0)
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/file.rs"
source = "{source_url}"
frozen = true
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();

    copit_cmd()
        .args(["update", "vendor/file.rs"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Skipping frozen: vendor/file.rs"));

    mock.assert_async().await;
}

#[tokio::test]
async fn unfreeze_bypasses_frozen() {
    let mut server = mockito::Server::new_async().await;
    let source_url = format!("{}/file.rs", server.url());

    let mock = server
        .mock("GET", "/file.rs")
        .with_status(200)
        .with_body("new content")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/file.rs"
source = "{source_url}"
frozen = true
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/file.rs"), "old content").unwrap();

    copit_cmd()
        .args(["update", "vendor/file.rs", "--unfreeze", "--overwrite"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Updated: vendor/file.rs"));

    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.sources[0].frozen, None);

    mock.assert_async().await;
}

#[tokio::test]
async fn freeze_flag_sets_frozen() {
    let mut server = mockito::Server::new_async().await;
    let source_url = format!("{}/file.rs", server.url());

    let mock = server
        .mock("GET", "/file.rs")
        .with_status(200)
        .with_body("new content")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/file.rs"
source = "{source_url}"
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/file.rs"), "old content").unwrap();

    copit_cmd()
        .args(["update", "vendor/file.rs", "--freeze", "--overwrite"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Updated: vendor/file.rs"));

    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.sources[0].frozen, Some(true));

    mock.assert_async().await;
}

#[tokio::test]
async fn source_skip_overrides_project_overwrite() {
    let mut server = mockito::Server::new_async().await;
    let source_url = format!("{}/file.rs", server.url());

    let mock = server
        .mock("GET", "/file.rs")
        .with_status(200)
        .with_body("new content")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"
overwrite = true

[[sources]]
path = "vendor/file.rs"
source = "{source_url}"
skip = true
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/file.rs"), "old content").unwrap();

    copit_cmd()
        .args(["update", "vendor/file.rs"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Skipping (already exists)"));

    let content = std::fs::read_to_string(dir.path().join("vendor/file.rs")).unwrap();
    assert_eq!(content, "old content");

    mock.assert_async().await;
}
