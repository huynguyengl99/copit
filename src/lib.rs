//! # copit
//!
//! Copy reusable source code from GitHub repos, HTTP URLs, and ZIP archives
//! into your project.
//!
//! Inspired by [shadcn/ui](https://ui.shadcn.com/) — instead of installing
//! opaque packages, copit copies source code directly into your codebase.
//! The code is yours: readable, modifiable, and fully owned.
//!
//! ## Overview
//!
//! copit is a CLI tool that helps you:
//!
//! - **Copy** files or folders from GitHub repositories, HTTP URLs, or ZIP archives
//! - **Track** what you copied and where it came from in a `copit.toml` manifest
//! - **Update** previously copied sources to newer versions
//! - **Sync** all tracked sources at once
//!
//! ## Quick start
//!
//! ```bash
//! # Initialize a copit.toml in your project
//! copit init
//!
//! # Copy a file from a GitHub repo
//! copit add github:serde-rs/serde@v1.0.219/serde/src/lib.rs
//!
//! # Copy from an HTTP URL
//! copit add https://example.com/utils.rs
//!
//! # Copy from a ZIP archive
//! copit add https://example.com/archive.zip#src/utils.rs
//!
//! # Re-fetch a tracked source with a new version
//! copit update vendor/lib.rs --ref v2.0
//!
//! # Re-fetch all tracked sources
//! copit sync
//! ```
//!
//! ## Source formats
//!
//! | Format | Example |
//! |--------|---------|
//! | GitHub | `github:owner/repo@ref/path` (alias: `gh:`) |
//! | HTTP   | `https://example.com/file.txt` |
//! | ZIP    | `https://example.com/archive.zip#inner/path` |
//!
//! ## Architecture
//!
//! - [`sources`] — Source parsing, fetching (GitHub, HTTP, ZIP)
//! - [`config`] — `copit.toml` loading, saving, and manipulation
//! - [`commands`] — CLI command implementations (init, add, remove, update, sync)
//! - [`cli`] — Clap argument definitions

pub mod cli;
pub mod commands;
pub mod config;
pub mod sources;
