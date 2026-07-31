# Registries

A **registry** is a repository that publishes named **components**: folders of source
code a user installs by id rather than by path.

```bash
copit registry add my-kit github:owner/repo@v1.0.0 --to app/components
copit add @my-kit/auth
```

copit resolves what that component requires, copies each one, and hands any package
dependencies to whatever package manager the project already uses.

Nothing about the format is tied to a language or framework. A registry declares its
`ecosystem` and copit maps that to a package manager.

!!! note "Data, never code"
    An index is fetched over the network, so it is treated as **data**. Components
    declare files, dependencies and text; they cannot declare commands for copit to run.
    Anything else would hand every registry author a shell on their users' machines.

---

## For users

### Configure a registry

```bash
copit registry add <name> <source> [--to <dir>] [--index <file>] [--variant <name>]
                  [--package-manager <name>]
```

`<source>` is where the registry lives: `github:owner/repo@ref`, or a **local
directory** while you are developing one. The index is read immediately, so a bad source
fails here rather than at first install.

If the index suggests an `install.target` and you did not pass `--to`, that suggestion is
recorded as `registries.<name>.target` in your `copit.toml`. A registry never picks the
destination at install time: the recorded value is yours to edit, and any target that is
absolute or escapes the project is rejected.

A registry name is used in `@name/component` and recorded against every component it
installs, so it cannot contain `:`, `/`, `@` or whitespace.

```bash
copit registry list
```

### Install components

```bash
copit add @my-kit/auth            # with a confirmation prompt
copit add @my-kit/cache -y        # accept the plan
copit add @my-kit/cache --dry-run # show the plan and stop
```

| Flag | Effect |
|---|---|
| `-y`, `--yes` | Accept the plan without prompting |
| `--dry-run` | Print the plan and exit |
| `--no-deps` | Install only what was named, not what it requires |
| `--no-packages` | Copy files but do not install package dependencies |
| `--variant <name>` | Override the configured variants |
| `--with <group>` | Also copy an optional file group, e.g. `--with tests` |
| `--to <dir>` | Override the target directory |

Dependencies are installed **before** the components that need them, so a component can
import one that is already on disk. A component already tracked in `copit.toml` is
skipped unless you ask for it by name again, and copit refuses to install over a path
that another source already owns.

License files from the registry's root are copied alongside the component, exactly as
they are for a plain `copit add`. Pass `--no-license` to skip them.

Without a terminal there is nothing to confirm, so `copit add @my-kit/auth` fails rather
than doing nothing. In CI, pass `-y`.

### Inspect

```bash
copit search @my-kit         # list every component
copit search @my-kit cache   # match ids, titles, descriptions, tags
copit info @my-kit/auth      # deps, packages, files
```

### Update

```bash
copit update app/components/auth
copit update app/components/auth --ref v1.1.0
```

A component is re-fetched through its registry, so only the files the index publishes are
written, so metadata and excluded paths stay out. `--ref` moves the **registry** ref, since
the index and the component have to come from the same commit; it applies to GitHub
registries only and errors rather than being ignored elsewhere. The variants selected at
install time are recorded per entry and reused here, so `--variant` at install time keeps
working on every later update. Files listed in `excludes` are preserved, and `--backup`
keeps the incoming version as `.orig`. `copit update-all` takes the same path.

---

## For registry authors

Publish a generated `copit-registry.json` at the root of your repository. copit reads that one
file, so an install is a single request and a tag is self-describing.

The name is namespaced deliberately: `registry.json` is used by other tools (shadcn/ui
among them), and a repository can plausibly be more than one kind of registry. Publish
it elsewhere if you prefer, and point copit at it:

```toml
[registries.my-kit]
source = "github:owner/repo@v1.0.0"
index = "registry/index.json"    # defaults to copit-registry.json
```

### `copit-registry.json`

```json
{
  "version": 1,
  "name": "my-kit",
  "title": "My Kit",
  "description": "Composable components for …",
  "source": "github:owner/repo",
  "homepage": "https://example.com",
  "ecosystem": "python",
  "variants": ["sqlite", "postgres"],
  "install": {
    "target": "app/components",
    "package_marker": "__init__.py",
    "exclude": ["component.yaml"],
    "optional": { "tests": ["tests/**"] }
  },
  "components": {
    "auth": {
      "name": "auth",
      "title": "Auth",
      "description": "Session handling with a pluggable store.",
      "tier": "core",
      "version": "0.1.0",
      "path": "components/auth",
      "tags": ["auth", "sessions"],
      "requires": ["logger"],
      "dependencies": ["my-runtime>=0.1"],
      "variants": {
        "sqlite": { "dependencies": ["sqlite-driver>=1.0"], "include": ["stores/sqlite.py"] }
      },
      "files": ["__init__.py", "sessions.py", "mixins.py"],
      "optional": { "tests": ["tests/test_auth.py"] }
    }
  }
}
```

Validate yours against [`registry.schema.json`](registry.schema.json) in CI.

### Top-level fields

| Field | Required | Meaning |
|---|---|---|
| `version` | yes | Index format version. copit refuses versions it does not know |
| `name` | yes | Registry id, used as `@name/component` |
| `title` | | Human-readable name |
| `source` | | Informational. copit fetches from the source *you* configured, at the ref you pinned |
| `ecosystem` | | `python`, `node`, `rust`. Selects the package manager |
| `variants` | | Names a component may specialise on |
| `install` | | Defaults: `target` and `package_marker` are used; `exclude` and `optional` are generator-side |
| `components` | yes | Map of id to component |

### Component fields

| Field | Required | Meaning |
|---|---|---|
| `name` | yes | Must equal the key this component is listed under |
| `path` | yes | Directory within the repository |
| `files` | | Exactly what a normal install copies, excludes already applied |
| `requires` | | Other components, resolved transitively |
| `dependencies` | | Packages for the project's package manager |
| `variants` | | Per-variant extra `dependencies` and `include` files |
| `only_variants` | | Variants this component requires. Empty installs anywhere |
| `optional` | | Named groups excluded by default, installed with `--with`. Must be materialised per component |
| `tier` | | Free-form label such as `core`. copit treats it as opaque |
| `version`, `title`, `description`, `tags`, `authors` | | Shown by `search` and `info` |

!!! warning "Optional groups are per component"
    `install.optional` is a default for your generator. copit copies only what a
    component lists in its own `optional` map, so materialise the group into every
    component that ships it. `--with <group>` errors if no component publishes it.

### Restricting a component to a variant

`variants` is additive: it adds files and packages when one is selected, and the
component installs either way. Some components cannot work that way — one carrying a
framework's models, migrations and admin has nothing to fall back to. Those declare
`only_variants`:

```json
"django-message-store": {
  "only_variants": ["django"],
  "requires": ["room-chat"]
}
```

copit then refuses to install it unless the project selects one of those variants,
naming what to pass:

```
Component '@my-kit/django-message-store' requires variant 'django'.
  This project selects: fastapi
  Pass --variant django, or set `variants` for this registry in copit.toml.
```

The check covers components pulled in through `requires` as well, so a restricted
dependency fails the install rather than landing unusable. Every name must appear in the
registry's top-level `variants`; copit rejects an index where it does not, since that
would make the component uninstallable everywhere.

### Materialise `files` at build time

`files` is the contract: copit copies exactly those paths and nothing else. Resolving
globs when you generate the index rather than at install time means

- the CLI can show a plan before touching disk,
- a diff of `copit-registry.json` makes added or removed files reviewable in a pull request,
- and metadata or tests cannot leak into a user's project by accident.

### Three rules worth knowing

**A component's key and its `name` must match.** Lookups go through the key, but the
installed entry records `name`, so a generator that lets them drift produces components
that install once and can then never be updated. copit rejects an index where they
disagree.

**Component directory names are the names users import.** copit copies each component to
`<target>/<basename of path>`, so the directory name in your repository becomes the
module name in theirs. If components import each other, that name has to be valid in
your language.

**Keep the repository layout identical to the installed layout.** copit installs
components side by side in one target directory. If your repository nests them, say
under `core/` and `contrib/`, then a relative import between two components needs a
different depth in your repository than in a user's project, and will break on install.
Use metadata for grouping instead of directories.

### Variants

A variant is a second axis, usually a target framework:

```json
"variants": {
  "sqlite":   { "dependencies": ["sqlite-driver>=1.0"], "include": ["stores/sqlite.py"] },
  "postgres": { "dependencies": ["pg-driver>=2.0"],     "include": ["stores/postgres.py"] }
}
```

Variant files are additive: a file listed under a variant that is not selected is not
copied, unless a selected variant lists it too, so a shared base module can appear under
several variants. Selection comes from `copit.toml` or `--variant`, and whatever was used
is recorded on the installed entry.

### Local development

Point a registry at a directory to test it before publishing:

```bash
copit registry add my-kit ../my-registry
copit add @my-kit/auth --dry-run
```

Same code path users get, minus the network.

---

## `copit.toml`

```toml
target = "vendor"

[registries.my-kit]
source = "github:owner/repo@v1.0.0"
index = "copit-registry.json"  # optional; this is the default
target = "app/components"
variants = ["postgres"]
package_manager = "uv"        # omit to detect; "none" to never install

[[sources]]
path = "app/components/auth"
source = "github:owner/repo@v1.0.0/components/auth"
component = "my-kit:auth"
variants = ["postgres"]
ref = "v1.0.0"
copied_at = "2026-07-30T00:00:00Z"
```

Components are tracked as ordinary `[[sources]]` entries with a `component` backlink, so
`update`, `remove`, `excludes`, `--freeze` and `--backup` all work on them unchanged.
The backlink is also how copit knows which components are already installed when
resolving dependencies.

## Package managers

Detected from the project, restricted to the registry's `ecosystem` so a Python registry
never installs through npm just because the repository also has a frontend.

| Ecosystem | Detected by | Command |
|---|---|---|
| `python` | `uv.lock` / `[tool.uv]` | `uv add` |
| `python` | `poetry.lock` / `[tool.poetry]` | `poetry add` |
| `python` | `pdm.lock` / `[tool.pdm]` | `pdm add` |
| `python` | `requirements.txt` | `pip install` |
| `node` | `pnpm-lock.yaml` | `pnpm add` |
| `node` | `yarn.lock` | `yarn add` |
| `node` | `package-lock.json` | `npm install` |
| `rust` | `Cargo.toml` | `cargo add` |

Lockfile-based managers are checked before `pip`, because a project on uv often keeps a
`requirements.txt` for other tooling and installing outside the lockfile would drift.

copit never resolves versions itself, it shells out. If the command fails, the files are
still copied and the failing specs are printed. When nothing is detected, the specs are
printed for you to install.
