//! CLI command implementations.
//!
//! Each submodule corresponds to a CLI subcommand:
//!
//! - [`init`] — Create a new `copit.toml`
//! - [`add`] — Fetch and copy source code into the project
//! - [`remove`] — Delete tracked files and their config entries
//! - [`update`] — Re-fetch specific tracked sources
//! - [`sync`] — Re-fetch all tracked sources
//! - [`common`] — Shared utilities (path handling, file writing)

pub mod add;
pub mod common;
pub mod init;
pub mod remove;
pub mod sync;
pub mod update;
