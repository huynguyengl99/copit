use predicates::prelude::*;
use tempfile::TempDir;

use super::copit_cmd;

#[test]
fn no_args() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

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
    std::fs::write(dir.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

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
    std::fs::write(dir.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

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
        r#"target = "vendor"

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
        r#"target = "vendor"

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

#[test]
fn removes_license_files_centralized_single_file() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "vendor"
licenses_dir = "licenses"

[[sources]]
path = "vendor/lib.rs"
source = "github:owner/repo@v1/src/lib.rs"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/lib.rs"), "code").unwrap();
    std::fs::create_dir_all(dir.path().join("licenses/lib")).unwrap();
    std::fs::write(dir.path().join("licenses/lib/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .args(["remove", "vendor/lib.rs"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Removed: vendor/lib.rs").and(
            predicates::str::contains("Removed license: licenses/lib/LICENSE"),
        ));

    assert!(!dir.path().join("vendor/lib.rs").exists());
    assert!(!dir.path().join("licenses/lib/LICENSE").exists());
    // Empty licenses/lib/ dir should be cleaned up
    assert!(!dir.path().join("licenses/lib").exists());
}

#[test]
fn removes_license_files_side_by_side_single_file() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "vendor"

[[sources]]
path = "vendor/lib.rs"
source = "github:owner/repo@v1/src/lib.rs"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/lib.rs"), "code").unwrap();
    std::fs::create_dir_all(dir.path().join("vendor/lib")).unwrap();
    std::fs::write(dir.path().join("vendor/lib/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .args(["remove", "vendor/lib.rs"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Removed license: vendor/lib/LICENSE",
        ));

    assert!(!dir.path().join("vendor/lib.rs").exists());
    assert!(!dir.path().join("vendor/lib/LICENSE").exists());
    assert!(!dir.path().join("vendor/lib").exists());
}

#[test]
fn removes_license_files_centralized_directory() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "vendor"
licenses_dir = "licenses"

[[sources]]
path = "vendor/mylib"
source = "github:owner/repo@v1/src/mylib"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("vendor/mylib")).unwrap();
    std::fs::write(dir.path().join("vendor/mylib/main.rs"), "code").unwrap();
    std::fs::create_dir_all(dir.path().join("licenses/mylib")).unwrap();
    std::fs::write(dir.path().join("licenses/mylib/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .args(["remove", "vendor/mylib"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Removed: vendor/mylib").and(predicates::str::contains(
                "Removed license: licenses/mylib/LICENSE",
            )),
        );

    assert!(!dir.path().join("vendor/mylib").exists());
    assert!(!dir.path().join("licenses/mylib").exists());
}

#[test]
fn skips_license_cleanup_for_no_license_entry() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "vendor"
licenses_dir = "licenses"

[[sources]]
path = "vendor/lib.rs"
source = "github:owner/repo@v1/src/lib.rs"
ref = "v1"
no_license = true
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("vendor")).unwrap();
    std::fs::write(dir.path().join("vendor/lib.rs"), "code").unwrap();

    copit_cmd()
        .args(["remove", "vendor/lib.rs"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Removed: vendor/lib.rs")
                .and(predicates::str::contains("Removed license").not()),
        );
}
