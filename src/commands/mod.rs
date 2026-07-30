//! CLI command implementations.
//!
//! Each submodule corresponds to a CLI subcommand:
//!
//! - [`init`] — Create a new `copit.toml`
//! - [`add`] — Fetch and copy source code into the project
//! - [`remove`] — Delete tracked files and their config entries
//! - [`update`] — Re-fetch specific tracked sources
//! - [`update_all`] — Re-fetch all tracked sources
//! - [`registry`]: Configure and inspect component registries
//! - [`registry_add`]: Install components from a registry, with their dependencies
//! - [`common`] — Shared utilities (path handling, file writing)

pub mod add;
pub mod common;
pub mod init;
pub mod licenses_sync;
pub mod registry;
pub mod registry_add;
pub mod remove;

pub mod update;

pub mod update_all;
