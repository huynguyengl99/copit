# copit

Copy reusable source code from GitHub repos, HTTP URLs, and ZIP archives into your project.

Inspired by [shadcn/ui](https://ui.shadcn.com/) — instead of installing opaque packages, copit copies
source code directly into your codebase. The code is yours: readable, modifiable, and fully owned.
No hidden abstractions, no dependency lock-in. Override anything, keep what you need.

## Features

- **GitHub sources** — Copy files or directories from any GitHub repo at a specific ref
- **HTTP URLs** — Fetch files from any raw URL
- **ZIP archives** — Extract files from ZIP archives with inner path selection
- **Tracking** — `copit.toml` tracks all copied sources for easy updates
- **Update & sync** — Re-fetch individual sources or sync all at once
- **Exclude modified** — Skip files you've customized during updates, with optional `.orig` backups

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

# Sync all tracked sources
copit sync
```

## Installation

See the [Getting Started](getting-started.md) guide for installation options and a walkthrough.
