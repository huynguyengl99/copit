# copit

Copy reusable source code from GitHub repos, HTTP URLs, and ZIP archives into your project.

Inspired by [shadcn/ui](https://ui.shadcn.com/) — instead of installing opaque packages, copit copies
source code directly into your codebase. The code is yours: readable, modifiable, and fully owned.
No hidden abstractions, no dependency lock-in. Override anything, keep what you need.

## Installation

### Standalone (recommended)

```bash
# macOS / Linux
curl -fsSL https://raw.githubusercontent.com/huynguyengl99/copit/main/install.sh | bash
```

You can specify a version or install directory:

```bash
COPIT_VERSION=v0.1.0 curl -fsSL https://raw.githubusercontent.com/huynguyengl99/copit/main/install.sh | bash

# Custom install location
INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/huynguyengl99/copit/main/install.sh | bash
```

### From PyPI

```bash
pip install copit
# or
uv pip install copit
```

### From Cargo

```bash
cargo install copit
```

## Quick start

```bash
# Initialize a copit.toml in your project
copit init

# Copy a file from a GitHub repo
copit add github:serde-rs/serde@v1.0.219/serde/src/lib.rs

# Copy a file from a raw URL
copit add https://raw.githubusercontent.com/serde-rs/serde/refs/heads/master/LICENSE-MIT

# Copy from a ZIP archive
copit add https://example.com/archive.zip#src/utils.rs
```

## Usage

### `copit init`

Creates a `copit.toml` config file in the current directory with a default target directory (`vendor`).

### `copit add <source>...`

Fetches source code and copies it into your project.

```
Usage: copit add [OPTIONS] [SOURCES]...

Arguments:
  [SOURCES]...  Source(s) to add (e.g., github:owner/repo@ref/path, https://...)

Options:
      --to <TO>    Target directory to copy files into
      --overwrite  Overwrite existing files without prompting
      --skip       Skip existing files without prompting
      --backup     Save .orig copy of new version for excluded modified files
```

### Source formats

| Format | Example |
|---|---|
| GitHub | `github:owner/repo@ref/path/to/file` (alias: `gh:`) |
| HTTP URL | `https://example.com/file.txt` |
| ZIP archive | `https://example.com/archive.zip#inner/path` |

### `copit update <path>...`

Re-fetches specific tracked source(s) by path (as shown in `copit.toml`). Always overwrites non-excluded files.

```bash
# Re-fetch a specific tracked source
copit update vendor/mylib

# Re-fetch with a new version
copit update vendor/mylib --ref v2.0

# Re-fetch with backup for excluded modified files
copit update vendor/mylib --backup
```

Options:
- `--ref <version>` — Override the version ref for this update (updates the source string and ref field)
- `--backup` — Save `.orig` copy of new version for excluded modified files

### `copit sync`

Re-fetches **all** tracked sources in `copit.toml`.

```bash
# Re-fetch all tracked sources
copit sync

# Re-fetch all with backup for excluded modified files
copit sync --backup
```

Options:
- `--ref <version>` — Override the version ref (errors if multiple sources are tracked)
- `--backup` — Save `.orig` copy of new version for excluded modified files

### `copit remove <path>...` (alias: `rm`)

Removes previously copied files from disk and their entries from `copit.toml`.

```bash
# Remove a specific file
copit remove vendor/lib.rs

# Remove multiple files
copit rm vendor/lib.rs vendor/utils.rs

# Remove all tracked sources
copit rm --all
```

### Config file

`copit.toml` tracks your project's target directory and all copied sources:

```toml
[project]
target = "vendor"

[[sources]]
path = "vendor/prek-identify"
source = "github:j178/prek@master/crates/prek-identify"
ref = "master"
commit = "abc123def456..."
copied_at = "2026-03-07T08:46:51Z"
exclude_modified = ["Cargo.toml", "src/lib.rs"]
```

- `ref`: The user-specified version string (branch/tag/sha)
- `commit`: Resolved commit SHA from GitHub API (optional, GitHub sources only)
- `exclude_modified`: List of relative paths (within source folder) to skip on re-add. With `--backup`, the new version is saved as `<file>.orig`.

## License

MIT
