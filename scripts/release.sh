#!/usr/bin/env bash
# Quick local release script.
# Usage: ./scripts/release.sh <version>
# Example: ./scripts/release.sh 0.2.0
#
# This bumps the version in Cargo.toml, commits, tags, and pushes.
# The tag push triggers the release.yml workflow which builds and publishes.

set -euo pipefail

VERSION="${1:?Usage: $0 <version> (e.g. 0.2.0)}"
TAG="v${VERSION}"

# Ensure we're on main and up to date
BRANCH=$(git branch --show-current)
if [ "$BRANCH" != "main" ]; then
  echo "Error: must be on the main branch (currently on '$BRANCH')"
  exit 1
fi

git pull --ff-only

# Check for uncommitted changes
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Error: working tree is not clean. Commit or stash changes first."
  exit 1
fi

# Check tag doesn't already exist
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "Error: tag $TAG already exists"
  exit 1
fi

# Bump version in Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml
rm -f Cargo.toml.bak

# Update Cargo.lock
cargo check --quiet 2>/dev/null || true

# Commit and tag
git add Cargo.toml Cargo.lock
git commit -m "chore: release v${VERSION}"
git tag "$TAG"

# Push commit and tag
git push origin main
git push origin "$TAG"

echo "Released ${TAG} — release workflow will now build and publish."
