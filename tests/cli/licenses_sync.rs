use predicates::prelude::*;
use tempfile::TempDir;

use super::copit_cmd;

#[test]
fn help() {
    copit_cmd()
        .args(["licenses-sync", "--help"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("--no-dir")
                .and(predicates::str::contains("--licenses-dir"))
                .and(predicates::str::contains("--dry-run")),
        );
}

#[test]
fn no_config() {
    let dir = TempDir::new().unwrap();

    copit_cmd()
        .arg("licenses-sync")
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("copit.toml"));
}

#[test]
fn conflicting_flags() {
    copit_cmd()
        .args(["licenses-sync", "--no-dir", "--licenses-dir", "licenses"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("cannot be used with"));
}

#[test]
fn no_entries_already_in_sync() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    copit_cmd()
        .arg("licenses-sync")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("already in sync"));
}

#[test]
fn move_side_by_side_to_centralized_single_file() {
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

    // Side-by-side: license in stem-named subfolder (vendor/lib/)
    std::fs::create_dir_all(dir.path().join("vendor/lib")).unwrap();
    std::fs::write(dir.path().join("vendor/lib/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .args(["licenses-sync", "--licenses-dir", "licenses"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Moved: vendor/lib/LICENSE -> licenses/lib/LICENSE",
        ));

    assert!(!dir.path().join("vendor/lib/LICENSE").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("licenses/lib/LICENSE")).unwrap(),
        "MIT License"
    );

    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.licenses_dir, Some("licenses".to_string()));
}

#[test]
fn move_side_by_side_to_centralized_directory() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "vendor"

[[sources]]
path = "vendor/mylib"
source = "github:owner/repo@v1/src/mylib"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    // Side-by-side for directory: license inside the directory
    std::fs::create_dir_all(dir.path().join("vendor/mylib")).unwrap();
    std::fs::write(dir.path().join("vendor/mylib/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .args(["licenses-sync", "--licenses-dir", "licenses"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Moved: vendor/mylib/LICENSE -> licenses/mylib/LICENSE",
        ));

    assert!(!dir.path().join("vendor/mylib/LICENSE").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("licenses/mylib/LICENSE")).unwrap(),
        "MIT License"
    );
}

#[test]
fn move_centralized_to_side_by_side() {
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

    std::fs::create_dir_all(dir.path().join("licenses/lib")).unwrap();
    std::fs::write(dir.path().join("licenses/lib/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .args(["licenses-sync", "--no-dir"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Moved:"));

    // Single file — license goes in stem-named subfolder (vendor/lib/)
    assert!(!dir.path().join("licenses/lib/LICENSE").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("vendor/lib/LICENSE")).unwrap(),
        "MIT License"
    );

    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.licenses_dir, None);
}

#[test]
fn already_in_sync_no_move() {
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

    std::fs::create_dir_all(dir.path().join("licenses/lib")).unwrap();
    std::fs::write(dir.path().join("licenses/lib/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .arg("licenses-sync")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("already in sync"));
}

#[test]
fn dry_run_previews_without_moving() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "vendor"

[[sources]]
path = "vendor/mylib"
source = "github:owner/repo@v1/src/mylib"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("vendor/mylib")).unwrap();
    std::fs::write(dir.path().join("vendor/mylib/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .args(["licenses-sync", "--licenses-dir", "licenses", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Would move:")
                .and(predicates::str::contains("Would move 1 license file(s)")),
        );

    // File should NOT have been moved
    assert!(dir.path().join("vendor/mylib/LICENSE").exists());
    assert!(!dir.path().join("licenses/mylib/LICENSE").exists());

    // Config should NOT have been updated
    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.licenses_dir, None);
}

#[test]
fn skips_no_license_entries() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "vendor"

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
    std::fs::write(dir.path().join("vendor/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .args(["licenses-sync", "--licenses-dir", "licenses"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("already in sync"));

    // License should NOT have been moved
    assert!(dir.path().join("vendor/LICENSE").exists());
}

#[test]
fn directory_source_centralized_to_side_by_side() {
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

    std::fs::create_dir_all(dir.path().join("licenses/mylib")).unwrap();
    std::fs::write(dir.path().join("licenses/mylib/LICENSE"), "MIT License").unwrap();
    std::fs::create_dir_all(dir.path().join("vendor/mylib")).unwrap();

    copit_cmd()
        .args(["licenses-sync", "--no-dir"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Moved:"));

    // For directory sources, license goes inside the directory
    assert_eq!(
        std::fs::read_to_string(dir.path().join("vendor/mylib/LICENSE")).unwrap(),
        "MIT License"
    );
}

#[test]
fn move_between_centralized_dirs() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "vendor"
licenses_dir = "old-licenses"

[[sources]]
path = "vendor/lib.rs"
source = "github:owner/repo@v1/src/lib.rs"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("old-licenses/lib")).unwrap();
    std::fs::write(dir.path().join("old-licenses/lib/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .args(["licenses-sync", "--licenses-dir", "new-licenses"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Moved:"));

    assert!(!dir.path().join("old-licenses/lib/LICENSE").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("new-licenses/lib/LICENSE")).unwrap(),
        "MIT License"
    );

    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.licenses_dir, Some("new-licenses".to_string()));
}

#[test]
fn multiple_license_files() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "vendor"

[[sources]]
path = "vendor/mylib"
source = "github:owner/repo@v1/src/mylib"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("vendor/mylib")).unwrap();
    std::fs::write(dir.path().join("vendor/mylib/LICENSE"), "MIT License").unwrap();
    std::fs::write(
        dir.path().join("vendor/mylib/LICENSE-APACHE"),
        "Apache License",
    )
    .unwrap();

    copit_cmd()
        .args(["licenses-sync", "--licenses-dir", "licenses"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("Moved 2 license file(s)"));

    assert!(!dir.path().join("vendor/mylib/LICENSE").exists());
    assert!(!dir.path().join("vendor/mylib/LICENSE-APACHE").exists());
    assert!(dir.path().join("licenses/mylib/LICENSE").exists());
    assert!(dir.path().join("licenses/mylib/LICENSE-APACHE").exists());
}

#[test]
fn no_license_files_at_source_location() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "vendor"

[[sources]]
path = "vendor/mylib"
source = "github:owner/repo@v1/src/mylib"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    // Create the directory but no license files inside
    std::fs::create_dir_all(dir.path().join("vendor/mylib")).unwrap();

    copit_cmd()
        .args(["licenses-sync", "--licenses-dir", "licenses"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("already in sync"));

    // No licenses dir should have been created
    assert!(!dir.path().join("licenses").exists());

    // But config should still be updated since --licenses-dir was passed
    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.licenses_dir, Some("licenses".to_string()));
}

#[test]
fn dry_run_no_dir() {
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

    std::fs::create_dir_all(dir.path().join("licenses/lib")).unwrap();
    std::fs::write(dir.path().join("licenses/lib/LICENSE"), "MIT License").unwrap();

    copit_cmd()
        .args(["licenses-sync", "--no-dir", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(
            predicates::str::contains("Would move:")
                .and(predicates::str::contains("Would move 1 license file(s)")),
        );

    // File should NOT have been moved
    assert!(dir.path().join("licenses/lib/LICENSE").exists());

    // Config should NOT have been changed
    let config = copit::config::load_config_from(&dir.path().join("copit.toml")).unwrap();
    assert_eq!(config.licenses_dir, Some("licenses".to_string()));
}

#[test]
fn custom_target_structure() {
    let dir = TempDir::new().unwrap();
    std::fs::write(
        dir.path().join("copit.toml"),
        r#"target = "my_proj/external"

[[sources]]
path = "my_proj/external/commands"
source = "github:astral-sh/ruff@main/crates/ruff/src/commands"
ref = "main"
copied_at = "2026-01-01T00:00:00Z"
"#,
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("my_proj/external/commands")).unwrap();
    std::fs::write(
        dir.path().join("my_proj/external/commands/LICENSE"),
        "MIT License",
    )
    .unwrap();

    copit_cmd()
        .args([
            "licenses-sync",
            "--licenses-dir",
            "my_proj/external_licenses",
        ])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Moved: my_proj/external/commands/LICENSE -> my_proj/external_licenses/commands/LICENSE",
        ));

    assert!(!dir
        .path()
        .join("my_proj/external/commands/LICENSE")
        .exists());
    assert_eq!(
        std::fs::read_to_string(
            dir.path()
                .join("my_proj/external_licenses/commands/LICENSE")
        )
        .unwrap(),
        "MIT License"
    );
}

/// A project holding one registry component, with its own target and a license
/// already written beside it. Mirrors what `copit add @kit/thing` leaves behind.
fn project_with_registry_component(licenses_dir: Option<&str>) -> TempDir {
    let dir = TempDir::new().unwrap();
    let licenses_line = licenses_dir
        .map(|d| format!("licenses_dir = \"{d}\"\n"))
        .unwrap_or_default();
    std::fs::write(
        dir.path().join("copit.toml"),
        format!(
            r#"target = "vendor"
{licenses_line}
[registries.my-kit]
source = "github:owner/repo@v1"
target = "app/ws_kits"

[[sources]]
path = "app/ws_kits/ag_ui"
source = "github:owner/repo@v1/kits/ag_ui"
ref = "v1"
copied_at = "2026-01-01T00:00:00Z"
component = "my-kit:ag-ui"
"#
        ),
    )
    .unwrap();

    std::fs::create_dir_all(dir.path().join("app/ws_kits/ag_ui")).unwrap();
    std::fs::write(dir.path().join("app/ws_kits/ag_ui/LICENSE"), "MIT License").unwrap();
    dir
}

#[test]
fn a_registry_component_centralizes_under_its_own_name() {
    // The registry's target is stripped, not the project's, so the component keeps
    // its name instead of dragging the whole path along.
    let dir = project_with_registry_component(None);

    copit_cmd()
        .args(["licenses-sync", "--licenses-dir", "licenses"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Moved: app/ws_kits/ag_ui/LICENSE -> licenses/ag_ui/LICENSE",
        ));

    assert!(dir.path().join("licenses/ag_ui/LICENSE").exists());
    assert!(!dir
        .path()
        .join("licenses/app/ws_kits/ag_ui/LICENSE")
        .exists());
}

#[test]
fn a_hand_edited_licenses_dir_still_moves_the_files() {
    // `licenses_dir` set by hand states an intention, not a fact: the files are
    // still side by side, and trusting config would report "already in sync".
    let dir = project_with_registry_component(Some("licenses"));

    copit_cmd()
        .arg("licenses-sync")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Moved: app/ws_kits/ag_ui/LICENSE -> licenses/ag_ui/LICENSE",
        ));

    assert!(dir.path().join("licenses/ag_ui/LICENSE").exists());
}

#[test]
fn a_layout_written_before_registry_targets_is_migrated() {
    // Upgrade path: an older copit centralized using the project target, leaving
    // licenses/app/ws_kits/ag_ui/LICENSE. Those must be found, not orphaned.
    let dir = project_with_registry_component(Some("licenses"));
    std::fs::remove_file(dir.path().join("app/ws_kits/ag_ui/LICENSE")).unwrap();
    std::fs::create_dir_all(dir.path().join("licenses/app/ws_kits/ag_ui")).unwrap();
    std::fs::write(
        dir.path().join("licenses/app/ws_kits/ag_ui/LICENSE"),
        "MIT License",
    )
    .unwrap();

    copit_cmd()
        .arg("licenses-sync")
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("-> licenses/ag_ui/LICENSE"));

    assert!(dir.path().join("licenses/ag_ui/LICENSE").exists());
    assert!(!dir
        .path()
        .join("licenses/app/ws_kits/ag_ui/LICENSE")
        .exists());
}
