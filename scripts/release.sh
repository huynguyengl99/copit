#!/usr/bin/env bash
set -euo pipefail

# Release script for copit
# Usage: ./scripts/release.sh [version]
#   version  Optional semver (e.g. 0.2.0). If omitted, auto-bumps from conventional commits.

# --- Prerequisites ---
for cmd in git-cliff cargo; do
    if ! command -v "$cmd" &>/dev/null; then
        echo "Error: '$cmd' is not installed." >&2
        exit 1
    fi
done

if ! cargo set-version --help &>/dev/null; then
    echo "Error: 'cargo set-version' is not available. Install with: cargo install cargo-edit" >&2
    exit 1
fi

# --- Clean working tree ---
if [ -n "$(git status --porcelain)" ]; then
    echo "Error: working tree is not clean. Commit or stash changes first." >&2
    exit 1
fi

# --- Must be on main branch ---
BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$BRANCH" != "main" ]; then
    echo "Error: must be on 'main' branch (currently on '$BRANCH')." >&2
    exit 1
fi

# --- Determine version ---
if [ $# -ge 1 ]; then
    VERSION="$1"
else
    BUMPED=$(git-cliff --bumped-version 2>/dev/null || true)
    if [ -z "$BUMPED" ]; then
        echo "Error: git-cliff could not determine next version. Provide version explicitly." >&2
        echo "Usage: $0 [version]" >&2
        exit 1
    fi
    # git-cliff returns e.g. "0.2.0" or "v0.2.0"
    VERSION="${BUMPED#v}"
fi

TAG="v${VERSION}"

echo "Releasing ${TAG}..."

# --- Generate changelog ---
git-cliff --tag "$TAG" -o CHANGELOG.md
echo "  Updated CHANGELOG.md"

# --- Bump version in Cargo.toml ---
cargo set-version "$VERSION"
echo "  Updated Cargo.toml to ${VERSION}"

# --- Update Cargo.lock ---
cargo check --quiet
echo "  Updated Cargo.lock"

# --- Commit and tag ---
git add CHANGELOG.md Cargo.toml Cargo.lock
# Pre-commit hooks may fix files (e.g. trailing newline). If commit fails, re-stage and retry.
if ! git commit -m "chore: release ${TAG}"; then
    echo "  Pre-commit hooks modified files, retrying commit..."
    git add CHANGELOG.md Cargo.toml Cargo.lock
    git commit -m "chore: release ${TAG}"
fi
git tag -a "$TAG" -m "Release ${TAG}"

echo ""
echo "Done! Now push the commit and tag:"
echo "  git push && git push --tags"
