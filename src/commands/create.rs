//! The `copit create` command.
//!
//! Copies a template folder into a new directory as untracked user code. Nothing is run.

use anyhow::{bail, Context, Result};
use std::path::{Component, Path, PathBuf};

use crate::cli::CreateCommand;
use crate::sources::{self, Source};

use super::common::{self, portable_display};

pub async fn run(cmd: &CreateCommand) -> Result<()> {
    let target = Path::new(&cmd.dir);
    ensure_empty_target(target)?;

    let files = fetch_template(&cmd.source).await?;
    if files.is_empty() {
        bail!("No files found in template {}", cmd.source);
    }

    for (relative, _) in &files {
        ensure_inside(relative)?;
    }
    for (relative, contents) in &files {
        common::write_file(&target.join(relative), contents)?;
    }

    println!(
        "Created {} from {} ({} files)",
        portable_display(target),
        cmd.source,
        files.len()
    );
    print_next_steps(target, &files);
    Ok(())
}

/// The target must be new or empty, so nothing is overwritten.
fn ensure_empty_target(target: &Path) -> Result<()> {
    if !target.exists() {
        return Ok(());
    }
    if !target.is_dir() {
        bail!("{} exists and is not a directory", target.display());
    }
    let mut entries = std::fs::read_dir(target)
        .with_context(|| format!("Failed to read {}", target.display()))?;
    if entries.next().is_some() {
        bail!(
            "{} is not empty. copit create only writes into a new or empty directory",
            target.display()
        );
    }
    Ok(())
}

/// Only plain segments, so `target.join(path)` cannot escape the target.
fn ensure_inside(relative: &str) -> Result<()> {
    let plain = Path::new(relative)
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    if !plain {
        bail!("Refusing template path {relative}: it would be written outside the new directory");
    }
    Ok(())
}

/// The template's files, relative to the template folder.
async fn fetch_template(raw: &str) -> Result<Vec<(String, Vec<u8>)>> {
    if let Some(local) = local_directory(raw) {
        return read_local_template(&local);
    }

    match sources::parse_source(raw)? {
        Source::GitHub {
            owner,
            repo,
            version,
            path,
        } => {
            let fetched = sources::github::fetch_github(&owner, &repo, &version, &path).await?;
            folder_contents(fetched.files, &path, raw)
        }
        Source::Zip { url, inner_path } => {
            let bytes = sources::http::fetch_url(&url).await?;
            let files = sources::zip::extract_from_bytes(&bytes, inner_path.as_deref(), None)?;
            match inner_path {
                Some(inner) => folder_contents(files, &inner, raw),
                None => Ok(sorted(files)),
            }
        }
        Source::Http { .. } => {
            bail!("A template is a folder, but {raw} is a single file. Use a GitHub path or url.zip#folder")
        }
        Source::Registry { .. } => {
            bail!("{raw} is a registry component. Use `copit add {raw}` inside a project instead")
        }
    }
}

fn local_directory(raw: &str) -> Option<PathBuf> {
    if raw.starts_with('@')
        || raw.contains("://")
        || raw.starts_with("github:")
        || raw.starts_with("gh:")
    {
        return None;
    }
    let path = PathBuf::from(raw);
    path.is_dir().then_some(path)
}

/// Re-root archive paths at `folder`.
fn folder_contents(
    files: impl IntoIterator<Item = (String, Vec<u8>)>,
    folder: &str,
    raw: &str,
) -> Result<Vec<(String, Vec<u8>)>> {
    let prefix = format!("{}/", folder.trim_end_matches('/'));
    let mut found = Vec::new();
    for (name, contents) in files {
        if name == folder.trim_end_matches('/') {
            bail!("A template is a folder, but {raw} is a single file");
        }
        if let Some(relative) = name.strip_prefix(&prefix) {
            found.push((relative.to_string(), contents));
        }
    }
    Ok(sorted(found))
}

/// Read a local template, skipping what git ignores (`.venv`, `node_modules`, ...).
fn read_local_template(dir: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let walker = ignore::WalkBuilder::new(dir)
        .hidden(false)
        .git_global(false)
        .require_git(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();

    let mut files = Vec::new();
    for entry in walker {
        let entry = entry.with_context(|| format!("Failed to read {}", dir.display()))?;
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.path();
        let relative = portable_display(path.strip_prefix(dir).expect("walked path is under dir"));
        let contents =
            std::fs::read(path).with_context(|| format!("Failed to read {}", path.display()))?;
        files.push((relative, contents));
    }
    Ok(sorted(files))
}

fn sorted(files: impl IntoIterator<Item = (String, Vec<u8>)>) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<_> = files.into_iter().collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn print_next_steps(target: &Path, files: &[(String, Vec<u8>)]) {
    let has = |name: &str| files.iter().any(|(path, _)| path == name);

    println!();
    println!("Next:");
    println!("  cd {}", portable_display(target));
    if has("README.md") {
        println!("  then follow README.md");
    }
    if has("copit.toml") {
        println!();
        println!("copit.toml came with the template, so `copit add` works from there.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> (String, Vec<u8>) {
        (name.to_string(), name.as_bytes().to_vec())
    }

    #[test]
    fn folder_contents_are_re_rooted_at_the_folder() {
        let files = vec![
            file("templates/app/README.md"),
            file("templates/app/src/main.py"),
            file("templates/app-other/README.md"),
        ];

        let found = folder_contents(files, "templates/app", "src").unwrap();
        let names: Vec<_> = found.iter().map(|(name, _)| name.as_str()).collect();

        assert_eq!(names, ["README.md", "src/main.py"]);
    }

    #[test]
    fn a_trailing_slash_on_the_folder_is_accepted() {
        let found = folder_contents(vec![file("t/a.txt")], "t/", "src").unwrap();
        assert_eq!(found[0].0, "a.txt");
    }

    #[test]
    fn a_single_file_is_not_a_template() {
        let error = folder_contents(vec![file("t/a.txt")], "t/a.txt", "src").unwrap_err();
        assert!(error.to_string().contains("single file"));
    }

    #[test]
    fn paths_that_leave_the_directory_are_refused() {
        for bad in ["../x", "a/../../x", "/etc/passwd", "./a"] {
            assert!(ensure_inside(bad).is_err(), "{bad}");
        }
        assert!(ensure_inside("web/src/app.ts").is_ok());
    }

    #[test]
    fn remote_and_registry_sources_are_not_local() {
        for raw in [
            "@kit/auth",
            "github:o/r@v1/p",
            "gh:o/r@v1/p",
            "https://x/a.zip",
        ] {
            assert!(local_directory(raw).is_none(), "{raw}");
        }
    }
}
