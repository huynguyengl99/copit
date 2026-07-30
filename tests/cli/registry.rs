use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

use super::copit_cmd;

/// A local registry with two components, one depending on the other.
///
/// `auth` lives at `components/auth_core`, so its id and its directory differ, and
/// `base.py` is listed under *both* variants, so a shared file is exercised.
fn write_registry(root: &Path) {
    let index = r#"{
  "version": 1,
  "name": "my-kit",
  "ecosystem": "python",
  "variants": ["sqlite", "postgres"],
  "install": { "target": "app/components" },
  "components": {
    "logger": {
      "name": "logger", "path": "components/logger", "version": "0.1.0", "tier": "core",
      "description": "Structured logging", "tags": ["logs"],
      "files": ["__init__.py"]
    },
    "auth": {
      "name": "auth", "path": "components/auth_core", "version": "0.1.0", "tier": "core",
      "requires": ["logger"],
      "files": ["__init__.py", "base.py", "stores/sqlite.py", "stores/postgres.py"],
      "variants": {
        "sqlite":   { "include": ["base.py", "stores/sqlite.py"] },
        "postgres": { "include": ["base.py", "stores/postgres.py"] }
      }
    }
  }
}"#;
    std::fs::create_dir_all(root.join("components/logger")).unwrap();
    std::fs::create_dir_all(root.join("components/auth_core/stores")).unwrap();
    std::fs::write(root.join("registry.json"), index).unwrap();
    std::fs::write(root.join("LICENSE"), "MIT").unwrap();
    std::fs::write(root.join("components/logger/__init__.py"), "logger").unwrap();
    std::fs::write(root.join("components/auth_core/__init__.py"), "auth").unwrap();
    std::fs::write(root.join("components/auth_core/base.py"), "shared base").unwrap();
    std::fs::write(root.join("components/auth_core/stores/sqlite.py"), "sqlite").unwrap();
    std::fs::write(root.join("components/auth_core/stores/postgres.py"), "pg").unwrap();
}

/// A project with the registry configured, returning (project, registry) dirs.
fn project_with_registry(variants: &[&str]) -> (TempDir, TempDir) {
    let registry = TempDir::new().unwrap();
    write_registry(registry.path());

    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    let mut args = vec![
        "registry".to_string(),
        "add".to_string(),
        "my-kit".to_string(),
        registry.path().to_string_lossy().to_string(),
    ];
    for variant in variants {
        args.push("--variant".to_string());
        args.push((*variant).to_string());
    }

    copit_cmd()
        .args(&args)
        .current_dir(project.path())
        .assert()
        .success();

    (project, registry)
}

#[test]
fn configuring_a_registry_preserves_comments_in_copit_toml() {
    let registry = TempDir::new().unwrap();
    write_registry(registry.path());
    let project = TempDir::new().unwrap();
    std::fs::write(
        project.path().join("copit.toml"),
        "# hand written, keep me\ntarget = \"vendor\"\n",
    )
    .unwrap();

    copit_cmd()
        .args([
            "registry",
            "add",
            "my-kit",
            &registry.path().to_string_lossy(),
        ])
        .current_dir(project.path())
        .assert()
        .success();

    let config = std::fs::read_to_string(project.path().join("copit.toml")).unwrap();
    assert!(config.contains("# hand written, keep me"), "{config}");
    assert!(config.contains("[registries.my-kit]"), "{config}");
}

#[test]
fn the_index_target_suggestion_is_recorded_not_applied_silently() {
    let (project, _registry) = project_with_registry(&[]);

    let config = std::fs::read_to_string(project.path().join("copit.toml")).unwrap();
    assert!(config.contains("target = \"app/components\""), "{config}");
}

#[test]
fn an_absolute_index_target_is_rejected() {
    let registry = TempDir::new().unwrap();
    write_registry(registry.path());
    let index = std::fs::read_to_string(registry.path().join("registry.json"))
        .unwrap()
        .replace("\"app/components\"", "\"/tmp/copit-escape\"");
    std::fs::write(registry.path().join("registry.json"), index).unwrap();

    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    copit_cmd()
        .args([
            "registry",
            "add",
            "my-kit",
            &registry.path().to_string_lossy(),
        ])
        .current_dir(project.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("must be relative"));
}

#[test]
fn installing_resolves_dependencies_and_keeps_shared_variant_files() {
    let (project, _registry) = project_with_registry(&["sqlite"]);

    copit_cmd()
        .args(["add", "@my-kit/auth", "-y"])
        .current_dir(project.path())
        .assert()
        .success();

    let root = project.path();
    // logger is pulled in by `requires`.
    assert!(root.join("app/components/logger/__init__.py").exists());
    // base.py is listed under both variants, so selecting one must not drop it.
    assert!(root.join("app/components/auth_core/base.py").exists());
    assert!(root
        .join("app/components/auth_core/stores/sqlite.py")
        .exists());
    assert!(!root
        .join("app/components/auth_core/stores/postgres.py")
        .exists());
    // Licenses travel with registry components, as they do for a plain add.
    assert!(root.join("app/components/auth_core/LICENSE").exists());
}

#[test]
fn the_plan_names_the_directory_that_is_actually_created() {
    let (project, _registry) = project_with_registry(&["sqlite"]);

    // `auth` lives at components/auth_core, so the plan must say auth_core.
    copit_cmd()
        .args(["add", "@my-kit/auth", "--dry-run"])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("auth_core/"));

    assert!(!project.path().join("app").exists(), "dry run wrote files");
}

#[test]
fn a_component_is_tracked_even_when_every_file_already_exists() {
    let (project, _registry) = project_with_registry(&[]);
    let root = project.path();
    std::fs::create_dir_all(root.join("app/components/logger")).unwrap();
    std::fs::write(root.join("app/components/logger/__init__.py"), "mine").unwrap();

    copit_cmd()
        .args(["add", "@my-kit/logger", "--skip", "-y"])
        .current_dir(root)
        .assert()
        .success();

    let config = std::fs::read_to_string(root.join("copit.toml")).unwrap();
    assert!(config.contains("component = \"my-kit:logger\""), "{config}");

    // An untracked component could never be updated.
    copit_cmd()
        .args(["update", "app/components/logger"])
        .current_dir(root)
        .assert()
        .success();
}

#[test]
fn installing_over_a_path_owned_by_another_source_is_refused() {
    let (project, _registry) = project_with_registry(&[]);
    let root = project.path();

    let mut config = std::fs::read_to_string(root.join("copit.toml")).unwrap();
    config.push_str(
        "\n[[sources]]\npath = \"app/components/logger\"\n\
         source = \"github:me/my-fork@v3/src/logger\"\ncopied_at = \"2026-01-01T00:00:00Z\"\n",
    );
    std::fs::write(root.join("copit.toml"), config).unwrap();

    copit_cmd()
        .args(["add", "@my-kit/logger", "-y", "--overwrite"])
        .current_dir(root)
        .assert()
        .failure()
        .stderr(predicates::str::contains("already tracked"));

    let config = std::fs::read_to_string(root.join("copit.toml")).unwrap();
    assert!(
        config.contains("github:me/my-fork@v3/src/logger"),
        "{config}"
    );
}

#[test]
fn selected_variants_survive_an_update() {
    let (project, _registry) = project_with_registry(&["sqlite"]);
    let root = project.path();

    // Override the configured variant on the command line.
    copit_cmd()
        .args(["add", "@my-kit/auth", "--variant", "postgres", "-y"])
        .current_dir(root)
        .assert()
        .success();

    copit_cmd()
        .args(["update", "app/components/auth_core"])
        .current_dir(root)
        .assert()
        .success();

    // The adapter installed must be the one refreshed, not the configured one.
    assert!(root
        .join("app/components/auth_core/stores/postgres.py")
        .exists());
    assert!(!root
        .join("app/components/auth_core/stores/sqlite.py")
        .exists());
}

#[test]
fn update_all_does_not_re_copy_files_the_index_withheld() {
    let (project, _registry) = project_with_registry(&["sqlite"]);
    let root = project.path();

    copit_cmd()
        .args(["add", "@my-kit/auth", "-y"])
        .current_dir(root)
        .assert()
        .success();

    copit_cmd()
        .arg("update-all")
        .current_dir(root)
        .assert()
        .success();

    assert!(!root
        .join("app/components/auth_core/stores/postgres.py")
        .exists());
}

#[test]
fn a_ref_override_is_rejected_for_a_local_registry() {
    let (project, _registry) = project_with_registry(&[]);
    let root = project.path();

    copit_cmd()
        .args(["add", "@my-kit/logger", "-y"])
        .current_dir(root)
        .assert()
        .success();

    // Silently ignoring this reported a successful update of the old ref.
    copit_cmd()
        .args(["update", "app/components/logger", "--ref", "v9.9.9"])
        .current_dir(root)
        .assert()
        .failure()
        .stderr(predicates::str::contains("--ref"));
}

#[test]
fn no_deps_still_validates_the_selected_variant() {
    let (project, _registry) = project_with_registry(&[]);

    copit_cmd()
        .args([
            "add",
            "@my-kit/auth",
            "--no-deps",
            "--variant",
            "postgresql",
            "-y",
        ])
        .current_dir(project.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("no variant 'postgresql'"));
}

#[test]
fn an_optional_group_no_component_publishes_is_rejected() {
    let (project, _registry) = project_with_registry(&[]);

    copit_cmd()
        .args(["add", "@my-kit/logger", "--with", "tests", "-y"])
        .current_dir(project.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("no optional group 'tests'"));
}

#[test]
fn registry_only_flags_are_rejected_on_a_plain_source() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    copit_cmd()
        .args(["add", "https://example.com/x.txt", "--no-deps"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("only applies to registry"));
}

#[test]
fn search_and_info_read_the_index() {
    let (project, _registry) = project_with_registry(&[]);

    copit_cmd()
        .args(["search", "@my-kit", "logs"])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("logger"));

    copit_cmd()
        .args(["info", "@my-kit/auth"])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("requires:").and(predicates::str::contains("logger")));

    copit_cmd()
        .args(["registry", "list"])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("@my-kit"));
}

#[test]
fn an_unknown_component_suggests_alternatives() {
    let (project, _registry) = project_with_registry(&[]);

    copit_cmd()
        .args(["add", "@my-kit/logg", "-y"])
        .current_dir(project.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("logger"));
}

#[test]
fn a_registry_name_that_breaks_the_backlink_is_rejected() {
    let registry = TempDir::new().unwrap();
    write_registry(registry.path());
    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    // `gh:internal` used to install fine and then fail every update with
    // "registry 'gh' is no longer configured".
    copit_cmd()
        .args([
            "registry",
            "add",
            "gh:internal",
            &registry.path().to_string_lossy(),
        ])
        .current_dir(project.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("cannot contain ':'"));
}

#[test]
fn a_component_reference_is_not_a_valid_registry_location() {
    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    // This used to surface copit's internal "must be resolved before fetching".
    copit_cmd()
        .args(["registry", "add", "kit", "@other"])
        .current_dir(project.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("component reference"));
}
