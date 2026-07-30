//! Package installers, one per package manager.
//!
//! Components declare the packages they need; copit hands those to whatever manager the
//! project already uses. It never resolves versions itself: the real manager does that
//! far better, and owns the lockfile.
//!
//! The set is compiled in rather than pluggable: a registry index is downloaded, so
//! letting it name a command to run would hand every registry author a shell on their
//! users' machines.

use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// A package manager copit can add dependencies with.
///
/// Managers differ only in their name, ecosystem, subcommand and detection files, so
/// they are data rather than eight near-identical trait impls. Adding one is a row in
/// [`INSTALLERS`], which is also the detection order.
pub struct PackageInstaller {
    /// Identifier used in `copit.toml` and on the command line.
    pub name: &'static str,
    /// Ecosystem this manager belongs to, matching a registry's `ecosystem`.
    pub ecosystem: &'static str,
    /// Subcommand that adds dependencies, e.g. `add` or `install`.
    subcommand: &'static str,
    /// Lockfiles or manifests whose presence identifies this manager.
    lockfiles: &'static [&'static str],
    /// `pyproject.toml` table that identifies this manager, e.g. `[tool.uv`.
    pyproject_table: Option<&'static str>,
}

impl PackageInstaller {
    /// Whether this manager appears to be the one `root` uses.
    pub fn detect(&self, root: &Path) -> bool {
        if self.lockfiles.iter().any(|file| root.join(file).exists()) {
            return true;
        }
        match self.pyproject_table {
            Some(table) => pyproject_has_table(root, table),
            None => false,
        }
    }

    /// The command that adds `specs` as dependencies.
    pub fn add_command(&self, specs: &[String]) -> Vec<String> {
        let mut command = vec![self.name.to_string(), self.subcommand.to_string()];
        command.extend(specs.iter().cloned());
        command
    }
}

/// True when `pyproject.toml` in `root` contains `table`, e.g. `[tool.uv]`.
fn pyproject_has_table(root: &Path, table: &str) -> bool {
    std::fs::read_to_string(root.join("pyproject.toml"))
        .map(|contents| contents.contains(table))
        .unwrap_or(false)
}

/// Every known installer, in detection order.
///
/// Order matters: a project using uv may also keep a `requirements.txt` for other
/// tooling, so lockfile-based managers are checked before pip.
pub static INSTALLERS: &[PackageInstaller] = &[
    PackageInstaller {
        name: "uv",
        ecosystem: "python",
        subcommand: "add",
        lockfiles: &["uv.lock"],
        pyproject_table: Some("[tool.uv"),
    },
    PackageInstaller {
        name: "poetry",
        ecosystem: "python",
        subcommand: "add",
        lockfiles: &["poetry.lock"],
        pyproject_table: Some("[tool.poetry"),
    },
    PackageInstaller {
        name: "pdm",
        ecosystem: "python",
        subcommand: "add",
        lockfiles: &["pdm.lock"],
        pyproject_table: Some("[tool.pdm"),
    },
    PackageInstaller {
        // pip has no manifest to record into, so this only installs; the user still
        // has to write the specs into their requirements file.
        name: "pip",
        ecosystem: "python",
        subcommand: "install",
        lockfiles: &["requirements.txt", "requirements/base.txt"],
        pyproject_table: None,
    },
    PackageInstaller {
        name: "pnpm",
        ecosystem: "node",
        subcommand: "add",
        lockfiles: &["pnpm-lock.yaml"],
        pyproject_table: None,
    },
    PackageInstaller {
        name: "yarn",
        ecosystem: "node",
        subcommand: "add",
        lockfiles: &["yarn.lock"],
        pyproject_table: None,
    },
    PackageInstaller {
        name: "npm",
        ecosystem: "node",
        subcommand: "install",
        lockfiles: &["package-lock.json"],
        pyproject_table: None,
    },
    PackageInstaller {
        name: "cargo",
        ecosystem: "rust",
        subcommand: "add",
        lockfiles: &["Cargo.toml"],
        pyproject_table: None,
    },
];

/// Find an installer by its `copit.toml` name.
pub fn by_name(name: &str) -> Option<&'static PackageInstaller> {
    INSTALLERS.iter().find(|installer| installer.name == name)
}

/// Detect the installer `root` uses, restricted to `ecosystem` when given.
///
/// Returns `None` when nothing matches, so callers should print the specs and let the
/// user install them rather than guessing.
pub fn detect(root: &Path, ecosystem: Option<&str>) -> Option<&'static PackageInstaller> {
    INSTALLERS.iter().find(|installer| {
        let ecosystem_matches = ecosystem
            .map(|wanted| installer.ecosystem == wanted)
            .unwrap_or(true);
        ecosystem_matches && installer.detect(root)
    })
}

/// Run an installer's add command in `root`.
pub fn install(installer: &PackageInstaller, specs: &[String], root: &Path) -> Result<()> {
    if specs.is_empty() {
        return Ok(());
    }

    let argv = installer.add_command(specs);
    let (program, args) = argv
        .split_first()
        .context("Installer produced an empty command")?;

    println!("Running: {}", argv.join(" "));

    // Either failure mode leaves the files copied and tracked, so both must say what
    // is still outstanding, otherwise the user is left knowing only that something
    // went wrong, not what to install.
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .with_context(|| {
            format!(
                "Failed to run `{program}`. Is it installed and on PATH? \
                 The files were copied; install these yourself: {}",
                specs.join(" ")
            )
        })?;

    if !status.success() {
        bail!(
            "`{}` failed. The files were copied; install these yourself: {}",
            argv.join(" "),
            specs.join(" ")
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn project(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (name, contents) in files {
            let path = dir.path().join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, contents).unwrap();
        }
        dir
    }

    #[test]
    fn uv_is_detected_from_its_lockfile() {
        let dir = project(&[("uv.lock", "")]);

        let installer = detect(dir.path(), Some("python")).unwrap();

        assert_eq!(installer.name, "uv");
    }

    #[test]
    fn uv_is_detected_from_a_pyproject_table() {
        let dir = project(&[("pyproject.toml", "[tool.uv]\ndefault-groups = []\n")]);

        assert_eq!(detect(dir.path(), Some("python")).unwrap().name, "uv");
    }

    #[test]
    fn poetry_is_detected_from_its_lockfile() {
        let dir = project(&[("poetry.lock", "")]);

        assert_eq!(detect(dir.path(), Some("python")).unwrap().name, "poetry");
    }

    #[test]
    fn pdm_is_detected_from_a_pyproject_table() {
        let dir = project(&[("pyproject.toml", "[tool.pdm]\n")]);

        assert_eq!(detect(dir.path(), Some("python")).unwrap().name, "pdm");
    }

    #[test]
    fn pip_is_detected_from_a_requirements_file() {
        let dir = project(&[("requirements.txt", "")]);

        assert_eq!(detect(dir.path(), Some("python")).unwrap().name, "pip");
    }

    #[test]
    fn a_lockfile_manager_wins_over_a_stray_requirements_file() {
        // Projects on uv often keep requirements.txt around for other tooling; picking
        // pip there would install outside the lockfile and silently drift.
        let dir = project(&[("uv.lock", ""), ("requirements.txt", "")]);

        assert_eq!(detect(dir.path(), Some("python")).unwrap().name, "uv");
    }

    #[test]
    fn nothing_is_detected_in_an_empty_project() {
        let dir = project(&[]);

        assert!(detect(dir.path(), Some("python")).is_none());
    }

    #[test]
    fn detection_is_scoped_to_the_registrys_ecosystem() {
        // A Python registry must not install through npm just because the repo also
        // has a frontend.
        let dir = project(&[("package-lock.json", "")]);

        assert!(detect(dir.path(), Some("python")).is_none());
        assert_eq!(detect(dir.path(), Some("node")).unwrap().name, "npm");
    }

    #[test]
    fn node_managers_are_detected_by_lockfile() {
        let pnpm = project(&[("pnpm-lock.yaml", "")]);
        let yarn = project(&[("yarn.lock", "")]);

        assert_eq!(detect(pnpm.path(), Some("node")).unwrap().name, "pnpm");
        assert_eq!(detect(yarn.path(), Some("node")).unwrap().name, "yarn");
    }

    #[test]
    fn cargo_is_detected_from_its_manifest() {
        let dir = project(&[("Cargo.toml", "[package]\nname = \"x\"\n")]);

        assert_eq!(detect(dir.path(), Some("rust")).unwrap().name, "cargo");
    }

    #[test]
    fn add_commands_pass_specs_through_verbatim() {
        let specs = vec!["my-package>=0.1.0".to_string(), "redis>=5".to_string()];

        assert_eq!(
            by_name("uv").unwrap().add_command(&specs),
            vec!["uv", "add", "my-package>=0.1.0", "redis>=5"]
        );
        assert_eq!(
            by_name("poetry").unwrap().add_command(&specs),
            vec!["poetry", "add", "my-package>=0.1.0", "redis>=5"]
        );
        assert_eq!(
            by_name("pip").unwrap().add_command(&specs),
            vec!["pip", "install", "my-package>=0.1.0", "redis>=5"]
        );
    }

    #[test]
    fn an_installer_can_be_looked_up_by_name() {
        assert_eq!(by_name("poetry").unwrap().name, "poetry");
        assert!(by_name("conda").is_none());
    }

    #[test]
    fn installing_nothing_is_a_no_op() {
        let dir = project(&[]);

        assert!(install(by_name("uv").unwrap(), &[], dir.path()).is_ok());
    }

    #[test]
    fn a_missing_manager_reports_which_program_was_missing() {
        let missing = PackageInstaller {
            name: "copit-no-such-program",
            ecosystem: "python",
            subcommand: "add",
            lockfiles: &[],
            pyproject_table: None,
        };
        let dir = project(&[]);

        let error = install(&missing, &["x".to_string()], dir.path())
            .unwrap_err()
            .to_string();

        assert!(error.contains("copit-no-such-program"), "{error}");
        // The files are already on disk, so the user needs to know what is outstanding.
        assert!(error.contains("install these yourself"), "{error}");
        assert!(error.contains("x"), "{error}");
    }
}
