# Configuration

copit uses a `copit.toml` file to track your project's target directory and all copied sources.

## File format

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

## Fields

### `[project]`

| Field | Description |
|---|---|
| `target` | Default directory where source files are copied into |

### `[[sources]]`

Each entry in the `sources` array represents one copied source:

| Field | Description |
|---|---|
| `path` | Local path where the source was copied |
| `source` | Original source string used to fetch the file |
| `ref` | User-specified version string (branch, tag, or SHA) |
| `commit` | Resolved commit SHA from GitHub API (GitHub sources only) |
| `copied_at` | Timestamp of when the source was last copied |
| `exclude_modified` | List of relative paths within the source to skip on re-add. With `--backup`, the new version is saved as `<file>.orig` |
