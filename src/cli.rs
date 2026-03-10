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
