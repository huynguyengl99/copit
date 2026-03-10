# Contributing to copit

## Getting started

### Prerequisites

- [Rust](https://rustup.rs/) (see `rust-toolchain.toml` for the pinned version)
- [uv](https://docs.astral.sh/uv/) (recommended for Python tooling)
- [prek](https://github.com/j178/prek) (pre-commit hook runner)

### Setup

```bash
git clone https://github.com/huynguyengl99/copit.git
cd copit

# Build
cargo build

# Run tests
cargo test

# Run the CLI
cargo run -- --help
```

### Pre-commit hooks

We use [prek](https://github.com/j178/prek) (a fast pre-commit replacement written in Rust) to run checks before each commit.

#### Installing prek

prek is a standalone binary with no dependencies. Install it via any of these methods:

```bash
# Homebrew (macOS)
brew install prek

# PyPI (via uv, pip, or pipx)
uv tool install prek

# cargo-binstall
cargo binstall prek

# Build from source
cargo install --locked prek
```

See the [prek installation docs](https://prek.j178.dev/installation/) for more options (npm, conda, nix, etc.).

#### Setting up hooks

```bash
# Set up git hooks (run once after cloning)
prek install

# Run all hooks manually
prek run --all-files
```

The hooks include: rustfmt, typos, prettier (YAML), convco (conventional commits), zizmor (GitHub Actions security), and more. See `.pre-commit-config.yaml` for the full list.

## Development workflow

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

### Testing

Tests are organized into two categories:

- **Unit tests** — inline `#[cfg(test)]` modules inside source files (`src/**/*.rs`), testing individual functions and parsing logic.
- **CLI integration tests** — in `tests/cli/`, one file per command, testing the full binary end-to-end with `assert_cmd` and `mockito` for HTTP mocking.

```bash
# Run all tests (unit + integration)
cargo test

# Run only unit tests
cargo test --lib

# Run only CLI integration tests
cargo test --test cli

# Run a specific test by name
cargo test parse_github_full
cargo test cli::update::http_source
```

### Code coverage

Install [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) to measure test coverage:

```bash
# Install
cargo install cargo-llvm-cov

# Run coverage report
cargo llvm-cov

# Generate HTML report
cargo llvm-cov --html
# Open target/llvm-cov/html/index.html
```

### Testing the Python package locally

```bash
# Build and install with maturin
uv pip install -e .
copit --help

# Or install into another project
uv add --editable /path/to/copit
uv run copit --help
```

### CLI docs

The CLI reference at `docs/commands.md` is auto-generated from clap definitions in `src/cli.rs`. After changing CLI arguments, regenerate it:

```bash
cargo run --example generate_cli_docs
```

CI will fail if `docs/commands.md` is out of date.

### Linting

```bash
# Clippy
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Format check
cargo fmt --all --check
```

## Project structure

```
copit/
├── Cargo.toml
├── pyproject.toml               # maturin binary distribution
├── python/copit/__init__.py     # Python binary locator
├── examples/
│   └── generate_cli_docs.rs     # Auto-generate docs/commands.md
├── src/
│   ├── main.rs                  # Entry point
│   ├── lib.rs                   # Library exports
│   ├── cli.rs                   # Clap CLI definitions
│   ├── config.rs                # copit.toml parsing & writing
│   ├── commands/
│   │   ├── mod.rs               # Command module registry
│   │   ├── init.rs              # copit init
│   │   ├── add.rs               # copit add
│   │   ├── remove.rs            # copit remove
│   │   ├── update.rs            # copit update
│   │   └── update_all.rs        # copit update-all
│   └── sources/
│       ├── mod.rs               # Source enum + parsing
│       ├── github.rs            # GitHub ZIP archive fetching
│       ├── http.rs              # Raw HTTP file fetching
│       └── zip.rs               # ZIP extraction
└── tests/
    └── cli/
        ├── main.rs              # Shared helpers (copit_cmd, create_zip)
        ├── init.rs              # copit init tests
        ├── add.rs               # copit add tests
        ├── remove.rs            # copit remove tests
        ├── update.rs            # copit update tests
        └── update_all.rs        # copit update-all tests
```

## Releasing

### Prerequisites

- [git-cliff](https://git-cliff.org/) — changelog generator
- [cargo-edit](https://github.com/killercup/cargo-edit) — provides `cargo set-version`

```bash
# Install via cargo
cargo install git-cliff
cargo install cargo-edit

# Or via Homebrew (macOS)
brew install git-cliff
```

### Creating a release

```bash
# Auto-bump version from conventional commits (feat: → minor, fix: → patch)
./scripts/release.sh

# Or specify an explicit version
./scripts/release.sh 0.2.0

# Then push to trigger the release workflow
git push && git push --tags
```

The script updates `CHANGELOG.md`, bumps `Cargo.toml`, commits, and creates a git tag. Pushing the tag triggers the GitHub Actions release workflow which builds binaries, publishes to PyPI, and publishes to crates.io.

## Submitting changes

1. Fork the repo and create a feature branch
2. Make your changes
3. Run `cargo test` and `prek run --all-files`
4. Submit a pull request
