# Getting Started

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

### 1. Initialize your project

```bash
copit init
```

This creates a `copit.toml` file in the current directory with a default target directory (`vendor`).

### 2. Add a source

```bash
# Copy a file from a GitHub repo
copit add github:serde-rs/serde@v1.0.219/serde/src/lib.rs

# Copy a file from a raw URL
copit add https://raw.githubusercontent.com/serde-rs/serde/refs/heads/master/LICENSE-MIT

# Copy from a ZIP archive
copit add https://example.com/archive.zip#src/utils.rs
```

### 3. Update and sync

```bash
# Re-fetch a specific tracked source
copit update vendor/mylib

# Re-fetch all tracked sources
copit sync
```

### 4. Remove sources

```bash
# Remove a specific file
copit remove vendor/lib.rs

# Remove all tracked sources
copit rm --all
```
