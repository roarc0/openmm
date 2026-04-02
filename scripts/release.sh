#!/usr/bin/env bash
# release.sh — bump version, tag, and push to trigger the release CI.
#
# Usage:
#   ./scripts/release.sh 0.2.0
#
# What it does:
#   1. Validates the version argument (semver x.y.z)
#   2. Updates [workspace.package] version in Cargo.toml
#   3. Runs `cargo check` to refresh Cargo.lock
#   4. Commits the version bump on main
#   5. Creates and pushes an annotated tag v<VERSION>
#
# The GitHub Actions release.yml workflow triggers on the tag and does
# the actual cross-platform build + GitHub Release creation.

set -euo pipefail

VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
    echo "Usage: $0 <version>  (e.g. $0 0.2.0)" >&2
    exit 1
fi

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: version must be semver (x.y.z), got: $VERSION" >&2
    exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Ensure working tree is clean
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "Error: working tree has uncommitted changes. Commit or stash first." >&2
    exit 1
fi

# Update version in workspace Cargo.toml
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml

# Refresh Cargo.lock
cargo check --workspace -q

git add Cargo.toml Cargo.lock
git commit --no-gpg-sign -m "chore: release v${VERSION}"

git tag -a "v${VERSION}" -m "Release v${VERSION}"

git push
git push origin "v${VERSION}"

echo "Released v${VERSION} — GitHub Actions will build and publish the release."
