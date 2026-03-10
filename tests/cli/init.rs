use tempfile::TempDir;

use super::copit_cmd;

#[test]
fn creates_config() {
    let dir = TempDir::new().unwrap();
    copit_cmd()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .success();

    assert!(dir.path().join("copit.toml").exists());
    let content = std::fs::read_to_string(dir.path().join("copit.toml")).unwrap();
    assert!(content.contains("target"));
}

#[test]
fn fails_if_already_exists() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("copit.toml"), "target = \"vendor\"\n").unwrap();

    copit_cmd()
        .arg("init")
        .current_dir(dir.path())
        .assert()
        .failure();
}
