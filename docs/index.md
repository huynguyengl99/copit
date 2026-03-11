# copit

Copy reusable source code from GitHub repos, HTTP URLs, and ZIP archives into your project.

Inspired by [shadcn/ui](https://ui.shadcn.com/) — instead of installing opaque packages, copit copies
source code directly into your codebase. The code is yours: readable, modifiable, and fully owned.
No hidden abstractions, no dependency lock-in. Override anything, keep what you need.

## Use cases

- **Quickly copy and own code** — Pull files from GitHub repos, HTTP URLs, or ZIP archives directly into your project. No forks, no submodules — just your own copy to read, modify, and maintain.

- **Build frameworks with injectable components** — Create a core library as a traditional package, then offer optional components that users copy into their projects via copit. Think of how shadcn/ui is built on top of Tailwind and Radix UI: the base libraries are installed as dependencies, while UI components are copied in and fully owned. Apply the same pattern to any ecosystem — a LangChain-style core as a library, with community integrations (OpenAI, Anthropic, etc.) as injectable source code that users can customize freely.

## Features

- **GitHub sources** — Copy files or directories from any GitHub repo at a specific ref
- **HTTP URLs** — Fetch files from any raw URL
- **ZIP archives** — Extract files from ZIP archives with inner path selection
- **Tracking** — `copit.toml` tracks all copied sources for easy updates
- **Update & update-all** — Re-fetch individual sources or update all at once
- **Freeze** — Pin sources with `--freeze` to skip them during updates; unpin with `--unfreeze`
- **Exclude modified** — Skip files you've customized during updates, with optional `.orig` backups
- **Per-source settings** — Override `overwrite`, `skip`, and `backup` per source, with CLI flags taking highest priority
- **License auto-copy** — Automatically copies LICENSE files from GitHub repos alongside your source files
- **License organization** — Centralize licenses in a dedicated directory or keep them side-by-side, and reorganize anytime with `licenses-sync`

## Quick example

```bash
# Initialize a copit.toml in your project
copit init

# Copy a file from a GitHub repo
copit add github:serde-rs/serde@v1.0.219/serde/src/lib.rs

# Copy a file from a raw URL
copit add https://raw.githubusercontent.com/serde-rs/serde/refs/heads/master/LICENSE-MIT

# Update a tracked source
copit update vendor/serde

# Update all tracked sources
copit update-all

# Remove a tracked source
copit remove vendor/serde

# Reorganize license files into a centralized directory
copit licenses-sync --licenses-dir licenses
```

## Installation

See the [Getting Started](getting-started.md) guide for installation options and a walkthrough.
