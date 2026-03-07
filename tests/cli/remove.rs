use tempfile::TempDir;

use super::copit_cmd;

#[test]
fn no_args() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .arg("remove")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("No paths specified"));
}

#[test]
fn alias_rm() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args(["rm", "vendor/nonexistent.rs"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("not tracked"));
}

#[test]
fn all_empty() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        "[project]\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args(["remove", "--all"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("No tracked sources"));
}

#[test]
fn tracked_file() {
    let dir = TempDir::new().unwrap();
    let config_file = dir.path().join("copit.toml");
    std::fs::write(
        &config_file,
        r#"[project]
target = "vendor"

[[sources]]
path = "vendor/file.txt"
source = "https://example.com/file.txt"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/file.txt"), "hello").unwrap();

    copit_cmd()
        .args(["remove", "vendor/file.txt"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Removed: vendor/file.txt"));

    assert!(!dir.path().join("vendor/file.txt").exists());

    let loaded = copit::config::load_config_from(&config_file).unwrap();
    assert!(loaded.sources.is_empty());
}

#[test]
fn all_tracked() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"[project]
target = "vendor"

[[sources]]
path = "vendor/a.txt"
source = "https://example.com/a.txt"
copied_at = "2026-01-01T00:00:00Z"

[[sources]]
path = "vendor/b.txt"
source = "https://example.com/b.txt"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/a.txt"), "a").unwrap();
    std::fs::write(dir.path().join("vendor/b.txt"), "b").unwrap();

    copit_cmd()
        .args(["rm", "--all"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Removed 2 source(s)"));

    assert!(!dir.path().join("vendor/a.txt").exists());
    assert!(!dir.path().join("vendor/b.txt").exists());
}
