use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

use super::copit_cmd;

/// A local registry with two components, one depending on the other.
///
/// `auth` lives at `components/auth_core`, so its id and its directory differ, and
/// `base.py` is listed under *both* variants, so a shared file is exercised.
fn write_registry(root: &Path) {
    write_registry_at(root, INDEX_FILE);
}

/// Filename copit looks for when a registry does not override it.
const INDEX_FILE: &str = "copit-registry.json";

/// Same registry, with the index written at `index_path` relative to `root`.
fn write_registry_at(root: &Path, index_path: &str) {
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
    let index_path = root.join(index_path);
    std::fs::create_dir_all(index_path.parent().unwrap()).unwrap();
    std::fs::write(index_path, index).unwrap();
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
    let index = std::fs::read_to_string(registry.path().join(INDEX_FILE))
        .unwrap()
        .replace("\"app/components\"", "\"/tmp/copit-escape\"");
    std::fs::write(registry.path().join(INDEX_FILE), index).unwrap();

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
fn an_index_override_is_recorded_and_used_for_later_installs() {
    let registry = TempDir::new().unwrap();
    write_registry_at(registry.path(), "registry/index.json");

    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    copit_cmd()
        .args([
            "registry",
            "add",
            "my-kit",
            &registry.path().to_string_lossy(),
            "--index",
            "registry/index.json",
        ])
        .current_dir(project.path())
        .assert()
        .success();

    let config = std::fs::read_to_string(project.path().join("copit.toml")).unwrap();
    assert!(
        config.contains("index = \"registry/index.json\""),
        "{config}"
    );

    // The override has to survive into `add`, which reloads the index from config.
    copit_cmd()
        .args(["add", "@my-kit/logger", "-y"])
        .current_dir(project.path())
        .assert()
        .success();

    assert!(project
        .path()
        .join("app/components/logger/__init__.py")
        .exists());
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

/// A registry where `pg_admin` only installs under postgres, and `reports` needs it.
///
/// Kept separate from [`write_registry`] so the shared fixture stays unrestricted.
fn write_restricted_registry(root: &Path, only_variants: &str) {
    let index = format!(
        r#"{{
  "version": 1,
  "name": "my-kit",
  "ecosystem": "python",
  "variants": ["sqlite", "postgres"],
  "install": {{ "target": "app/components" }},
  "components": {{
    "pg_admin": {{
      "name": "pg_admin", "path": "components/pg_admin", "version": "0.1.0",
      "only_variants": [{only_variants}],
      "files": ["__init__.py"]
    }},
    "reports": {{
      "name": "reports", "path": "components/reports", "version": "0.1.0",
      "requires": ["pg_admin"],
      "files": ["__init__.py"]
    }}
  }}
}}"#
    );
    std::fs::create_dir_all(root.join("components/pg_admin")).unwrap();
    std::fs::create_dir_all(root.join("components/reports")).unwrap();
    std::fs::write(root.join(INDEX_FILE), index).unwrap();
    std::fs::write(root.join("LICENSE"), "MIT").unwrap();
    std::fs::write(root.join("components/pg_admin/__init__.py"), "pg").unwrap();
    std::fs::write(root.join("components/reports/__init__.py"), "reports").unwrap();
}

/// Project with the restricted registry configured under `variants`.
fn project_with_restricted_registry(variants: &[&str], only_variants: &str) -> (TempDir, TempDir) {
    let registry = TempDir::new().unwrap();
    write_restricted_registry(registry.path(), only_variants);

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
fn a_component_restricted_to_a_variant_is_refused_without_it() {
    let (project, _registry) = project_with_restricted_registry(&["sqlite"], r#""postgres""#);

    copit_cmd()
        .args(["add", "@my-kit/pg_admin", "-y"])
        .current_dir(project.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("requires variant 'postgres'"))
        .stderr(predicates::str::contains("This project selects: sqlite"))
        .stderr(predicates::str::contains("--variant postgres"));

    assert!(!project.path().join("app/components/pg_admin").exists());
}

#[test]
fn a_component_restricted_to_a_variant_installs_with_it() {
    let (project, _registry) = project_with_restricted_registry(&["postgres"], r#""postgres""#);

    copit_cmd()
        .args(["add", "@my-kit/pg_admin", "-y"])
        .current_dir(project.path())
        .assert()
        .success();

    assert!(project
        .path()
        .join("app/components/pg_admin/__init__.py")
        .exists());
}

#[test]
fn a_restricted_dependency_fails_the_install() {
    // `reports` itself is unrestricted, so only the transitive check catches this. Without
    // it the dependency would be skipped and `reports` would land importing nothing.
    let (project, _registry) = project_with_restricted_registry(&["sqlite"], r#""postgres""#);

    copit_cmd()
        .args(["add", "@my-kit/reports", "-y"])
        .current_dir(project.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("'@my-kit/pg_admin'"))
        .stderr(predicates::str::contains("required by 'reports'"));

    assert!(!project.path().join("app/components/reports").exists());
}

#[test]
fn restricting_to_an_undeclared_variant_is_rejected() {
    // Otherwise the typo reads as a copit bug: the component installs nowhere, and the
    // error blames whichever variant the user did select.
    let registry = TempDir::new().unwrap();
    write_restricted_registry(registry.path(), r#""postgres", "mysql""#);

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
        .stderr(predicates::str::contains("does not declare"))
        .stderr(predicates::str::contains("mysql"));
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

/// A registry whose optional group needs another component to be usable: `notify`
/// publishes its tests, and those tests import the shared `testing` harness.
///
/// `group` is the JSON for that group, so one fixture covers both forms.
fn write_group_registry(root: &Path, group: &str) {
    let index = format!(
        r#"{{
  "version": 1,
  "name": "my-kit",
  "ecosystem": "python",
  "install": {{ "target": "app/components" }},
  "components": {{
    "testing": {{
      "name": "testing", "path": "components/testing", "version": "0.1.0", "tier": "core",
      "files": ["__init__.py"]
    }},
    "notify": {{
      "name": "notify", "path": "components/notify", "version": "0.2.0", "tier": "core",
      "files": ["__init__.py"],
      "optional": {{ "tests": {group} }}
    }}
  }}
}}"#
    );
    std::fs::create_dir_all(root.join("components/testing")).unwrap();
    std::fs::create_dir_all(root.join("components/notify/tests")).unwrap();
    std::fs::write(root.join(INDEX_FILE), index).unwrap();
    std::fs::write(root.join("components/testing/__init__.py"), "harness").unwrap();
    std::fs::write(root.join("components/notify/__init__.py"), "notify").unwrap();
    std::fs::write(
        root.join("components/notify/tests/test_notify.py"),
        "import testing",
    )
    .unwrap();
}

const TESTS_GROUP: &str = r#"{
        "include": ["tests/test_notify.py"],
        "requires": ["testing"],
        "dependencies": ["pytest>=8"]
      }"#;

fn project_with_group_registry(group: &str) -> (TempDir, TempDir) {
    configure_group_registry(group, &[])
}

/// The same fixture, with groups configured on the registry itself.
fn configure_group_registry(group: &str, with: &[&str]) -> (TempDir, TempDir) {
    let registry = TempDir::new().unwrap();
    write_group_registry(registry.path(), group);

    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    let mut args = vec![
        "registry".to_string(),
        "add".to_string(),
        "my-kit".to_string(),
        registry.path().to_string_lossy().to_string(),
    ];
    for group in with {
        args.push("--with".to_string());
        args.push((*group).to_string());
    }

    copit_cmd()
        .args(&args)
        .current_dir(project.path())
        .assert()
        .success();

    (project, registry)
}

#[test]
fn info_names_the_groups_a_component_publishes_and_what_they_bring() {
    let (project, _registry) = project_with_group_registry(TESTS_GROUP);

    copit_cmd()
        .args(["info", "@my-kit/notify"])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("--with"))
        .stdout(predicates::str::contains(
            "tests: 1 file, requires testing, packages pytest>=8",
        ));
}

#[test]
fn a_registry_can_default_every_component_to_a_group() {
    let (project, _registry) = configure_group_registry(TESTS_GROUP, &["tests"]);
    let root = project.path();

    let config = std::fs::read_to_string(root.join("copit.toml")).unwrap();
    assert!(config.contains("optional = [\"tests\"]"), "{config}");

    // No --with on the command line, so the registry's groups apply.
    copit_cmd()
        .args(["add", "@my-kit/notify", "-y"])
        .current_dir(root)
        .assert()
        .success();

    assert!(root
        .join("app/components/notify/tests/test_notify.py")
        .exists());
    assert!(root.join("app/components/testing/__init__.py").exists());
}

#[test]
fn add_with_overrides_the_registry_default_rather_than_adding_to_it() {
    // `--with docs` on a registry that defaults to tests must install docs only, or
    // there would be no way to opt out of a default for one component.
    let group = r#"{
        "include": ["tests/test_notify.py"],
        "requires": ["testing"]
      },
      "docs": ["tests/test_notify.py"]"#;
    let (project, _registry) = configure_group_registry(group, &["tests"]);
    let root = project.path();

    copit_cmd()
        .args(["add", "@my-kit/notify", "--with", "docs", "-y"])
        .current_dir(root)
        .assert()
        .success();

    assert!(!root.join("app/components/testing").exists());

    let config = std::fs::read_to_string(root.join("copit.toml")).unwrap();
    assert!(config.contains("optional = [\"docs\"]"), "{config}");
}

#[test]
fn no_optional_opts_one_component_out_of_a_registry_default() {
    let (project, _registry) = configure_group_registry(TESTS_GROUP, &["tests"]);
    let root = project.path();

    copit_cmd()
        .args(["add", "@my-kit/notify", "--no-optional", "-y"])
        .current_dir(root)
        .assert()
        .success();

    assert!(!root.join("app/components/notify/tests").exists());
    assert!(!root.join("app/components/testing").exists());

    // The empty list is what distinguishes "deliberately none" from "never recorded",
    // so an update must not quietly re-apply the registry's groups.
    let config = std::fs::read_to_string(root.join("copit.toml")).unwrap();
    assert!(config.contains("optional = []"), "{config}");

    copit_cmd()
        .args(["update", "app/components/notify"])
        .current_dir(root)
        .assert()
        .success();

    assert!(!root.join("app/components/notify/tests").exists());
    assert!(!root.join("app/components/testing").exists());
}

#[test]
fn no_optional_is_rejected_on_a_plain_source() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    copit_cmd()
        .args(["add", "https://example.com/x.txt", "--no-optional"])
        .current_dir(dir.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("only applies to registry"));
}

#[test]
fn a_registry_group_no_component_publishes_is_rejected_when_configured() {
    let registry = TempDir::new().unwrap();
    write_group_registry(registry.path(), TESTS_GROUP);

    let project = TempDir::new().unwrap();
    std::fs::write(project.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    copit_cmd()
        .args([
            "registry",
            "add",
            "my-kit",
            &registry.path().to_string_lossy(),
            "--with",
            "docs",
        ])
        .current_dir(project.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("no optional group 'docs'"));
}

#[test]
fn a_registry_group_applies_to_an_entry_that_recorded_none() {
    // Turning the default on later has to reach components installed before it, the
    // same way adding a variant does.
    let (project, registry) = project_with_group_registry(TESTS_GROUP);
    let root = project.path();

    copit_cmd()
        .args(["add", "@my-kit/notify", "-y"])
        .current_dir(root)
        .assert()
        .success();
    assert!(!root.join("app/components/testing").exists());

    copit_cmd()
        .args([
            "registry",
            "add",
            "my-kit",
            &registry.path().to_string_lossy(),
            "--with",
            "tests",
        ])
        .current_dir(root)
        .assert()
        .success();

    copit_cmd()
        .args(["update", "app/components/notify"])
        .current_dir(root)
        .assert()
        .success();

    assert!(root
        .join("app/components/notify/tests/test_notify.py")
        .exists());
    assert!(root.join("app/components/testing/__init__.py").exists());
}

#[test]
fn a_group_requirement_stays_out_without_the_group() {
    // Putting the harness in the component's own `requires` instead would copy it into
    // every project, for tests that were never installed.
    let (project, _registry) = project_with_group_registry(TESTS_GROUP);

    copit_cmd()
        .args(["add", "@my-kit/notify", "-y"])
        .current_dir(project.path())
        .assert()
        .success();

    assert!(project.path().join("app/components/notify").exists());
    assert!(!project.path().join("app/components/testing").exists());
}

#[test]
fn a_group_requirement_is_installed_with_the_group() {
    let (project, _registry) = project_with_group_registry(TESTS_GROUP);

    copit_cmd()
        .args(["add", "@my-kit/notify", "--with", "tests", "-y"])
        .current_dir(project.path())
        .assert()
        .success();

    let root = project.path();
    assert!(root
        .join("app/components/notify/tests/test_notify.py")
        .exists());
    assert!(root.join("app/components/testing/__init__.py").exists());

    let config = std::fs::read_to_string(root.join("copit.toml")).unwrap();
    assert!(config.contains("optional = [\"tests\"]"), "{config}");
    assert!(config.contains("component_version = \"0.2.0\""), "{config}");
}

#[test]
fn no_deps_leaves_a_group_requirement_out() {
    let (project, _registry) = project_with_group_registry(TESTS_GROUP);

    copit_cmd()
        .args([
            "add",
            "@my-kit/notify",
            "--with",
            "tests",
            "--no-deps",
            "-y",
        ])
        .current_dir(project.path())
        .assert()
        .success();

    assert!(project
        .path()
        .join("app/components/notify/tests/test_notify.py")
        .exists());
    assert!(!project.path().join("app/components/testing").exists());
}

#[test]
fn a_group_package_is_listed_only_when_the_group_is_selected() {
    let (project, _registry) = project_with_group_registry(TESTS_GROUP);

    copit_cmd()
        .args(["add", "@my-kit/notify", "--dry-run"])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("pytest>=8").not());

    copit_cmd()
        .args(["add", "@my-kit/notify", "--with", "tests", "--dry-run"])
        .current_dir(project.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("pytest>=8"));
}

#[test]
fn a_group_written_as_a_bare_file_list_still_installs() {
    let (project, _registry) = project_with_group_registry(r#"["tests/test_notify.py"]"#);

    copit_cmd()
        .args(["add", "@my-kit/notify", "--with", "tests", "-y"])
        .current_dir(project.path())
        .assert()
        .success();

    assert!(project
        .path()
        .join("app/components/notify/tests/test_notify.py")
        .exists());
    assert!(!project.path().join("app/components/testing").exists());
}

#[test]
fn an_update_restores_a_group_file_deleted_locally() {
    let (project, _registry) = project_with_group_registry(TESTS_GROUP);
    let root = project.path();

    copit_cmd()
        .args(["add", "@my-kit/notify", "--with", "tests", "-y"])
        .current_dir(root)
        .assert()
        .success();

    let test_file = root.join("app/components/notify/tests/test_notify.py");
    std::fs::remove_file(&test_file).unwrap();

    copit_cmd()
        .args(["update", "app/components/notify"])
        .current_dir(root)
        .assert()
        .success();

    // Inferring the selection from disk lost the group the moment its files went away.
    assert!(test_file.exists());
}

#[test]
fn an_entry_without_a_recorded_group_still_refreshes_what_is_on_disk() {
    // Entries predating `optional` have no record of the selection, so they fall back
    // to refreshing the optional files already present.
    let (project, _registry) = project_with_group_registry(TESTS_GROUP);
    let root = project.path();

    copit_cmd()
        .args(["add", "@my-kit/notify", "--with", "tests", "-y"])
        .current_dir(root)
        .assert()
        .success();

    let config = std::fs::read_to_string(root.join("copit.toml"))
        .unwrap()
        .replace("optional = [\"tests\"]\n", "");
    std::fs::write(root.join("copit.toml"), config).unwrap();

    let test_file = root.join("app/components/notify/tests/test_notify.py");
    std::fs::write(&test_file, "stale").unwrap();

    copit_cmd()
        .args(["update", "app/components/notify", "--overwrite"])
        .current_dir(root)
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(&test_file).unwrap(),
        "import testing"
    );
}

#[test]
fn a_group_requirement_added_in_a_later_index_is_installed_on_update() {
    // The component was complete when it was installed and is not after the registry
    // moves its tests onto a shared harness.
    let (project, registry) = project_with_group_registry(r#"["tests/test_notify.py"]"#);
    let root = project.path();

    copit_cmd()
        .args(["add", "@my-kit/notify", "--with", "tests", "-y"])
        .current_dir(root)
        .assert()
        .success();
    assert!(!root.join("app/components/testing").exists());

    write_group_registry(registry.path(), TESTS_GROUP);

    copit_cmd()
        .args(["update", "app/components/notify"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicates::str::contains("now requires: testing"));

    assert!(root.join("app/components/testing/__init__.py").exists());

    let config = std::fs::read_to_string(root.join("copit.toml")).unwrap();
    assert!(
        config.contains("component = \"my-kit:testing\""),
        "{config}"
    );
}

#[test]
fn an_update_reports_the_component_version_it_moved_to() {
    let (project, registry) = project_with_group_registry(TESTS_GROUP);
    let root = project.path();

    copit_cmd()
        .args(["add", "@my-kit/notify", "-y"])
        .current_dir(root)
        .assert()
        .success();

    let index = std::fs::read_to_string(registry.path().join(INDEX_FILE))
        .unwrap()
        .replace("\"version\": \"0.2.0\"", "\"version\": \"0.3.0\"");
    std::fs::write(registry.path().join(INDEX_FILE), index).unwrap();

    copit_cmd()
        .args(["update", "app/components/notify"])
        .current_dir(root)
        .assert()
        .success()
        .stdout(predicates::str::contains("0.2.0 -> 0.3.0"));

    let config = std::fs::read_to_string(root.join("copit.toml")).unwrap();
    assert!(config.contains("component_version = \"0.3.0\""), "{config}");
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

#[test]
fn updating_a_component_keeps_its_license_under_its_own_name() {
    // A component's own directory is not a target. Passing it collapsed every
    // component's license onto one path, so each update overwrote the last.
    let (project, _registry) = project_with_registry(&["sqlite"]);
    let root = project.path();

    copit_cmd()
        .args(["add", "@my-kit/auth", "-y"])
        .current_dir(root)
        .assert()
        .success();

    copit_cmd()
        .args(["licenses-sync", "--licenses-dir", "licenses"])
        .current_dir(root)
        .assert()
        .success();

    assert!(root.join("licenses/auth_core/LICENSE").exists());
    assert!(root.join("licenses/logger/LICENSE").exists());

    copit_cmd()
        .args(["update", "app/components/auth_core"])
        .current_dir(root)
        .assert()
        .success();

    // The stray licenses/LICENSE is what the collapse produced.
    assert!(
        !root.join("licenses/LICENSE").exists(),
        "update wrote the license to the licenses root"
    );
    assert!(root.join("licenses/auth_core/LICENSE").exists());
    assert!(root.join("licenses/logger/LICENSE").exists());
}
