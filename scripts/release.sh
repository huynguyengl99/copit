#!/usr/bin/env bash
# Quick local release script.
# Usage: ./scripts/release.sh [version]
# Example: ./scripts/release.sh 0.2.0
#
# Uses release-plz to update the changelog and determine the next version.
# If a version argument is provided, it overrides the auto-detected version.
# The tag push triggers the release.yml workflow which builds and publishes.
#
# Requires: cargo install release-plz

set -euo pipefail

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

# Check release-plz is installed
if ! command -v release-plz &>/dev/null; then
  echo "Error: release-plz not found. Install with: cargo install --locked release-plz"
  exit 1
fi

# Run release-plz to update changelog and bump version
echo "Running release-plz update..."
release-plz update --allow-dirty

# If a version was explicitly provided, override what release-plz set
if [ -n "${1:-}" ]; then
  VERSION="$1"
  echo "Overriding version to ${VERSION}..."
  sed -i.bak "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml
  rm -f Cargo.toml.bak
  cargo check --quiet 2>/dev/null || true
fi

# Extract version from Cargo.toml
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
TAG="v${VERSION}"

# Check if there are any changes to commit
if git diff --quiet && git diff --cached --quiet; then
  echo "No changes to release."
  exit 0
fi

# Check tag doesn't already exist
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "Error: tag $TAG already exists"
  exit 1
fi

# Commit and tag
git add -A
git commit -m "chore: release v${VERSION}"
git tag "$TAG"

# Push commit and tag
git push origin main
git push origin "$TAG"

echo "Released ${TAG} — release workflow will now build and publish."
