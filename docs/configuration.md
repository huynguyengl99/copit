# Configuration

copit uses a `copit.toml` file to track your project's target directory and all copied sources.

## File format

```toml
target = "vendor"

[[sources]]
path = "vendor/prek-identify"
source = "github:j178/prek@master/crates/prek-identify"
ref = "master"
commit = "abc123def456..."
copied_at = "2026-03-07T08:46:51Z"
excludes = ["Cargo.toml", "src/lib.rs"]
```

## Fields

### Root-level fields

| Field | Description |
|---|---|
| `target` | Default directory where source files are copied into |
| `overwrite` | Default: overwrite existing files without prompting |
| `skip` | Default: skip existing files without prompting |
| `backup` | Default: save `.orig` backup for excluded modified files |
| `licenses_dir` | Centralized directory for license files. When set, licenses are stored in `{licenses_dir}/{owner}-{repo}/` instead of next to the source files |

### `[[sources]]`

Each entry in the `sources` array represents one copied source:

| Field | Description |
|---|---|
| `path` | Local path where the source was copied |
| `source` | Original source string used to fetch the file |
| `ref` | User-specified version string (branch, tag, or SHA) |
| `commit` | Resolved commit SHA from GitHub API (GitHub sources only) |
| `copied_at` | Timestamp of when the source was last copied |
| `excludes` | List of relative paths within the source to skip on re-add. With `--backup`, the new version is saved as `<file>.orig` |
| `frozen` | Pin this source so it's skipped during updates |
| `overwrite` | Per-source override: overwrite existing files without prompting |
| `skip` | Per-source override: skip existing files without prompting |
| `backup` | Per-source override: save `.orig` backup for excluded modified files |
| `no_license` | Skip copying license files for this source (set via `--no-license` on `add`) |

Settings priority: CLI flags > per-source config > root-level config > default (`false`).

## License auto-copy

When adding or updating GitHub sources, copit automatically copies LICENSE files from the repository root alongside your source files. By default, licenses are placed side-by-side with the copied source:

- **Single file** — license is placed in the same directory as the file
- **Directory** — license is placed inside the copied directory

To skip license copying, use `--no-license` with `add`. This sets `no_license = true` on the source entry, so subsequent `update` and `update-all` calls also skip licenses for that source.

To centralize all license files in one directory, set `licenses_dir` in `copit.toml`:

```toml
target = "vendor"
licenses_dir = "licenses"
```

This stores licenses in `licenses/{owner}-{repo}/` (e.g., `licenses/serde-rs-serde/LICENSE`).
