//! The `copit init` command.
//!
//! Creates a new `copit.toml` in the current directory with default settings.

use anyhow::{bail, Result};

use crate::config::{self, CopitConfig};

/// Run the `init` command.
///
/// Creates a `copit.toml` with `target = "vendor"`. Fails if the file
/// already exists to avoid accidentally overwriting configuration.
pub fn run() -> Result<()> {
    if config::config_exists() {
        bail!("copit.toml already exists in the current directory");
    }

    let config = CopitConfig::default();
    config::save_config(&config)?;

    println!(
        "Created copit.toml with target directory: {}",
        config.project.target
    );
    Ok(())
}
