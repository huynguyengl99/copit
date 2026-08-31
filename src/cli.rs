//! Command-line argument definitions.
//!
//! Uses [clap](https://docs.rs/clap) with derive macros to define the CLI
//! interface. See each command struct for available options and examples.

use clap::{Parser, Subcommand};

/// Top-level CLI arguments.
#[derive(Parser)]
#[command(
    name = "copit",
    version,
    about = "Copy reusable source code into your project"
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new copit.toml config file
    Init,
    /// Add source code from GitHub, HTTP URLs, or ZIP archives
    Add(AddCommand),
    /// Remove previously copied source files
    #[command(alias = "rm")]
    Remove(RemoveCommand),
    /// Re-fetch specific tracked source(s) by path
    Update(UpdateCommand),
    /// Re-fetch all tracked sources
    UpdateAll(UpdateAllCommand),
    /// Reorganize license files (centralize or restore side-by-side)
    LicensesSync(LicensesSyncCommand),
    /// Configure and inspect component registries
    #[command(subcommand)]
    Registry(RegistryCommand),
    /// Search a registry's components
    Search(SearchCommand),
    /// Show what a component installs
    Info(InfoCommand),
}

#[derive(Subcommand)]
pub enum RegistryCommand {
    /// Configure a registry so its components can be installed by id
    Add(RegistryAddCommand),
    /// List configured registries
    List,
}

#[derive(Parser)]
#[command(after_help = "\
Examples:
  # Configure a registry, then install by id
  copit registry add my-kit github:owner/repo@v1.0.0 --to app/components --variant postgres
  copit add @my-kit/auth

  # Develop a registry locally, before publishing it
  copit registry add my-kit ../my-registry
")]
pub struct RegistryAddCommand {
    /// Name used in `@name/component`
    pub name: String,

    /// Where the registry lives: `github:owner/repo@ref`, or a local directory
    pub source: String,

    /// Target directory for this registry's components
    #[arg(long = "to")]
    pub to: Option<String>,

    /// Index filename within the source; defaults to `copit-registry.json`
    #[arg(long)]
    pub index: Option<String>,

    /// Registry-defined variant to select (repeatable)
    #[arg(long = "variant")]
    pub variants: Vec<String>,

    /// Optional file group every component of this registry installs, e.g. `--with tests` (repeatable). `--with` on `add` overrides it
    #[arg(long = "with")]
    pub with: Vec<String>,

    /// Package manager to use; detected when omitted. Use `none` to never install.
    #[arg(long)]
    pub package_manager: Option<String>,
}

#[derive(Parser)]
#[command(after_help = "\
Examples:
  copit search @my-kit cache
  copit search @my-kit          # list everything
")]
pub struct SearchCommand {
    /// Registry to search, as `@name`
    pub registry: String,

    /// Text matched against ids, titles, descriptions and tags
    pub query: Option<String>,
}

#[derive(Parser)]
#[command(after_help = "\
Examples:
  copit info @my-kit/auth
")]
pub struct InfoCommand {
    /// Component to describe, as `@registry/component`
    pub component: String,
}

#[derive(Parser)]
#[command(after_help = "\
Examples:
  # Remove a specific file
  copit remove vendor/lib.rs

  # Remove multiple files
  copit rm vendor/lib.rs vendor/utils.rs

  # Remove all tracked sources
  copit rm --all
")]
pub struct RemoveCommand {
    /// Path(s) to remove (as shown in copit.toml)
    pub paths: Vec<String>,

    /// Remove all tracked sources
    #[arg(long)]
    pub all: bool,
}

#[derive(Parser)]
#[command(after_help = "\
Examples:
  # Copy a single file from GitHub
  copit add github:owner/repo@v1.0.0/src/utils.rs

  # Same, using the short alias
  copit add gh:owner/repo@v1.0.0/src/utils.rs

  # Copy an entire folder from GitHub
  copit add gh:owner/repo@main/src/helpers

  # Copy a raw file from a URL
  copit add https://example.com/LICENSE-MIT

  # Copy a file from a ZIP archive
  copit add https://example.com/archive.zip#src/utils.rs

  # Copy to a specific directory
  copit add gh:owner/repo@v1.0.0/src/lib.rs --to vendor/

  # Install a component from a configured registry, with its dependencies
  copit add @my-kit/auth
  copit add @my-kit/cache -y
")]
pub struct AddCommand {
    /// Source(s) to add
    ///
    /// Supported formats:
    ///   `github:owner/repo@ref/path`  - file or folder from a GitHub repo (alias: `gh:`)
    ///   `https://example.com/file.txt` - raw file from a URL
    ///   `https://...archive.zip#path`  - file or folder inside a ZIP archive
    pub sources: Vec<String>,

    /// Target directory to copy files into
    #[arg(long)]
    pub to: Option<String>,

    /// Overwrite existing files without prompting
    #[arg(long)]
    pub overwrite: bool,

    /// Skip existing files without prompting
    #[arg(long, conflicts_with = "overwrite")]
    pub skip: bool,

    /// Save .orig copy of new version for excluded modified files
    #[arg(long)]
    pub backup: bool,

    /// Pin this source so update and update-all skip it
    #[arg(long)]
    pub freeze: bool,

    /// Skip copying license files
    #[arg(long)]
    pub no_license: bool,

    /// Accept the install plan without prompting
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Show what would be installed and exit
    #[arg(long)]
    pub dry_run: bool,

    /// Install only the named components, not what they require
    #[arg(long)]
    pub no_deps: bool,

    /// Do not install package dependencies
    #[arg(long)]
    pub no_packages: bool,

    /// Registry variant to select, overriding copit.toml (repeatable)
    #[arg(long = "variant")]
    pub variants: Vec<String>,

    /// Also copy an optional file group, e.g. `--with tests` (repeatable)
    #[arg(long = "with")]
    pub with: Vec<String>,

    /// Install no optional groups, ignoring the registry's configured `optional`
    #[arg(long, conflicts_with = "with")]
    pub no_optional: bool,
}

#[derive(Parser)]
#[command(after_help = "\
Examples:
  # Re-fetch a specific tracked source
  copit update vendor/mylib

  # Re-fetch with a new version
  copit update vendor/mylib --ref v2.0

  # Re-fetch with backup for excluded modified files
  copit update vendor/mylib --backup
")]
pub struct UpdateCommand {
    /// Path(s) to update (as shown in copit.toml)
    pub paths: Vec<String>,

    /// Override the version ref for this update
    #[arg(long = "ref")]
    pub version_ref: Option<String>,

    /// Save .orig copy of new version for excluded modified files
    #[arg(long)]
    pub backup: bool,

    /// Overwrite existing files without prompting
    #[arg(long)]
    pub overwrite: bool,

    /// Skip existing files without prompting
    #[arg(long, conflicts_with = "overwrite")]
    pub skip: bool,

    /// Pin this source so update and update-all skip it
    #[arg(long)]
    pub freeze: bool,

    /// Unpin this source so it can be updated again
    #[arg(long, conflicts_with = "freeze")]
    pub unfreeze: bool,
}

#[derive(Parser)]
#[command(after_help = "\
Examples:
  # Re-fetch all tracked sources
  copit update-all

  # Re-fetch all with backup for excluded modified files
  copit update-all --backup
")]
pub struct UpdateAllCommand {
    /// Override the version ref (only valid with a single source)
    #[arg(long = "ref")]
    pub version_ref: Option<String>,

    /// Save .orig copy of new version for excluded modified files
    #[arg(long)]
    pub backup: bool,

    /// Overwrite existing files without prompting
    #[arg(long)]
    pub overwrite: bool,

    /// Skip existing files without prompting
    #[arg(long, conflicts_with = "overwrite")]
    pub skip: bool,
}

#[derive(Parser)]
#[command(after_help = "\
Examples:
  # Move licenses into a centralized directory
  copit licenses-sync --licenses-dir licenses

  # Move licenses back to side-by-side (next to each source)
  copit licenses-sync --no-dir

  # Re-sync based on current config
  copit licenses-sync
")]
pub struct LicensesSyncCommand {
    /// Move licenses back to side-by-side (remove licenses_dir)
    #[arg(long, conflicts_with = "licenses_dir")]
    pub no_dir: bool,

    /// Move licenses into a centralized directory
    #[arg(short = 'l', long)]
    pub licenses_dir: Option<String>,

    /// Preview what would be moved without making changes
    #[arg(long)]
    pub dry_run: bool,
}
