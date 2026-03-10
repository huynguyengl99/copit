use predicates::prelude::*;
use tempfile::TempDir;

use super::copit_cmd;

#[test]
fn no_config() {
    let dir = TempDir::new().unwrap();

    copit_cmd()
        .arg("update-all")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("copit.toml"));
}

#[test]
fn no_sources() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .arg("update-all")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("No tracked sources"));
}

#[test]
fn ref_with_multiple_sources_errors() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"[project]
target = "vendor"

[[sources]]
path = "vendor/a"
source = "github:owner/repo@v1/src/a"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"

[[sources]]
path = "vendor/b"
source = "github:owner/repo@v1/src/b"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    copit_cmd()
        .args(["update-all", "--ref", "v2"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("ambiguous"));
}

#[test]
fn ref_with_single_source_accepted() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"[project]
target = "vendor"

[[sources]]
path = "vendor/a"
source = "github:owner/repo@v1/src/a"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    let output = copit_cmd()
        .args(["update-all", "--ref", "v2"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("ambiguous"),
        "Should accept --ref with single source, got: {stderr}"
    );
}

#[test]
fn help() {
    copit_cmd()
        .args(["update-all", "--help"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("--ref")
                .and(predicates::str::contains("--backup"))
                .and(predicates::str::contains("--skip"))
                .and(predicates::str::contains("--overwrite")),
        );
}

#[tokio::test]
async fn single_http_source() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/lib.rs")
        .with_status(200)
        .with_body("updated all content")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    let source_url = format!("{}/lib.rs", server.url());
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/lib.rs"
source = "{source_url}"
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/lib.rs"), "old").unwrap();

    copit_cmd()
        .args(["update-all", "--overwrite"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Updating all vendor/lib.rs")
                .and(predicates::str::contains("Updated: vendor/lib.rs")),
        );

    let content = std::fs::read_to_string(dir.path().join("vendor/lib.rs")).unwrap();
    assert_eq!(content, "updated all content");

    mock.assert_async().await;
}

#[tokio::test]
async fn multiple_sources() {
    let mut server = mockito::Server::new_async().await;
    let mock_a = server
        .mock("GET", "/a.rs")
        .with_status(200)
        .with_body("content a")
        .create_async()
        .await;
    let mock_b = server
        .mock("GET", "/b.rs")
        .with_status(200)
        .with_body("content b")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/a.rs"
source = "{url}/a.rs"
copied_at = "2026-01-01T00:00:00Z"

[[sources]]
path = "vendor/b.rs"
source = "{url}/b.rs"
copied_at = "2026-01-01T00:00:00Z"
"#,
            url = server.url()
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();

    copit_cmd()
        .arg("update-all")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Updating all vendor/a.rs")
                .and(predicates::str::contains("Updating all vendor/b.rs")),
        );

    assert_eq!(
        std::fs::read_to_string(dir.path().join("vendor/a.rs")).unwrap(),
        "content a"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("vendor/b.rs")).unwrap(),
        "content b"
    );

    mock_a.assert_async().await;
    mock_b.assert_async().await;
}

#[tokio::test]
async fn skip_existing() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/lib.rs")
        .with_status(200)
        .with_body("new content")
        .create_async()
        .await;

    let dir = TempDir::new().unwrap();
    let source_url = format!("{}/lib.rs", server.url());
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"[project]
target = "vendor"

[[sources]]
path = "vendor/lib.rs"
source = "{source_url}"
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/lib.rs"), "old content").unwrap();

    copit_cmd()
        .args(["update-all", "--skip"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Skipping (already exists)"));

    let content = std::fs::read_to_string(dir.path().join("vendor/lib.rs")).unwrap();
    assert_eq!(content, "old content");

    mock.assert_async().await;
}

#[tokio::test]
async fn skips_frozen_sources() {
    let mut server = mockito::Server::new_async().await;
    let source_url = format!("{}/utils.rs", server.url());

    // This mock should NOT be called — frozen source should be skipped
    let mock = server
        .mock("GET", "/utils.rs")
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
path = "vendor/utils.rs"
source = "{source_url}"
frozen = true
copied_at = "2026-01-01T00:00:00Z"
"#
        ),
    )
    .unwrap();

    copit_cmd()
        .arg("update-all")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Skipping frozen: vendor/utils.rs",
        ));

    mock.assert_async().await;
}
