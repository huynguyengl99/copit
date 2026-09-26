use predicates::prelude::*;
use std::path::Path;
use tempfile::TempDir;

use super::{copit_cmd, create_zip};

fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// A template in a working tree, with ignored files and ignore rules at two levels.
fn working_tree() -> TempDir {
    let repo = TempDir::new().unwrap();
    let root = repo.path();
    write(&root.join(".gitignore"), "dist/\n");

    let template = root.join("templates/app");
    write(&template.join("README.md"), "# app");
    write(&template.join("src/main.py"), "print('hi')");
    write(&template.join(".gitignore"), ".venv/\nnode_modules/\n");
    write(
        &template.join("copit.toml"),
        "target = \"vendor\"\n\n[registries.kit]\nsource = \"github:o/r@v1\"\n",
    );
    write(&template.join(".venv/lib/site.py"), "installed");
    write(&template.join("web/node_modules/pkg/index.js"), "installed");
    write(&template.join("web/dist/index.html"), "built");
    write(&template.join("web/src/app.ts"), "export {}");
    repo
}

#[test]
fn a_local_template_is_copied_without_what_git_ignores() {
    let repo = working_tree();
    let out = TempDir::new().unwrap();
    let project = out.path().join("my-app");

    copit_cmd()
        .args([
            "create",
            repo.path().join("templates/app").to_str().unwrap(),
        ])
        .arg(&project)
        .assert()
        .success()
        .stdout(predicates::str::contains("Created").and(predicates::str::contains("copit add")));

    for kept in [
        "README.md",
        "src/main.py",
        ".gitignore",
        "copit.toml",
        "web/src/app.ts",
    ] {
        assert!(project.join(kept).is_file(), "{kept} should be copied");
    }
    for skipped in [".venv", "web/node_modules", "web/dist"] {
        assert!(
            !project.join(skipped).exists(),
            "{skipped} should be skipped"
        );
    }
}

#[test]
fn the_copy_is_not_tracked_and_the_template_config_is_kept_as_is() {
    let repo = working_tree();
    let out = TempDir::new().unwrap();
    let project = out.path().join("my-app");

    copit_cmd()
        .args([
            "create",
            repo.path().join("templates/app").to_str().unwrap(),
        ])
        .arg(&project)
        .assert()
        .success();

    let config = copit::config::load_config_from(&project.join("copit.toml")).unwrap();
    assert!(config.sources.is_empty());
    assert!(config.registries.contains_key("kit"));
}

#[test]
fn an_empty_directory_is_accepted() {
    let repo = working_tree();
    let project = TempDir::new().unwrap();

    copit_cmd()
        .args([
            "create",
            repo.path().join("templates/app").to_str().unwrap(),
            ".",
        ])
        .current_dir(project.path())
        .assert()
        .success();

    assert!(project.path().join("README.md").is_file());
}

#[test]
fn a_non_empty_directory_is_refused_and_left_alone() {
    let repo = working_tree();
    let project = TempDir::new().unwrap();
    write(&project.path().join("README.md"), "mine");

    copit_cmd()
        .args([
            "create",
            repo.path().join("templates/app").to_str().unwrap(),
        ])
        .arg(project.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("not empty"));

    assert_eq!(
        std::fs::read_to_string(project.path().join("README.md")).unwrap(),
        "mine"
    );
    assert!(!project.path().join("src").exists());
}

#[test]
fn a_registry_component_is_pointed_at_copit_add() {
    let out = TempDir::new().unwrap();

    copit_cmd()
        .args(["create", "@kit/auth", "my-app"])
        .current_dir(out.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("copit add @kit/auth"));

    assert!(!out.path().join("my-app").exists());
}

#[test]
fn a_missing_local_folder_is_an_unknown_source() {
    let out = TempDir::new().unwrap();

    copit_cmd()
        .args(["create", "no/such/folder", "my-app"])
        .current_dir(out.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("Unknown source format"));
}

#[tokio::test]
async fn a_single_file_url_is_not_a_template() {
    let out = TempDir::new().unwrap();

    copit_cmd()
        .args(["create", "https://example.com/main.py", "my-app"])
        .current_dir(out.path())
        .assert()
        .failure()
        .stderr(predicates::str::contains("single file"));
}

#[tokio::test]
async fn a_zip_folder_lands_at_the_directory_root() {
    let mut server = mockito::Server::new_async().await;
    let archive = create_zip(&[
        ("templates/app/README.md", b"# app"),
        ("templates/app/src/main.py", b"print('hi')"),
        ("templates/other/README.md", b"# other"),
    ]);
    let mock = server
        .mock("GET", "/t.zip")
        .with_status(200)
        .with_body(archive)
        .create_async()
        .await;
    let out = TempDir::new().unwrap();

    copit_cmd()
        .args([
            "create",
            &format!("{}/t.zip#templates/app", server.url()),
            "my-app",
        ])
        .current_dir(out.path())
        .assert()
        .success();

    let project = out.path().join("my-app");
    assert_eq!(
        std::fs::read_to_string(project.join("README.md")).unwrap(),
        "# app"
    );
    assert!(project.join("src/main.py").is_file());
    assert!(!project.join("templates").exists());
    mock.assert_async().await;
}
