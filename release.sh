#!/usr/bin/env bash
# SERAPH release script
# Run from the seraph-releases repo root.
#
# Usage:
#   ./release.sh v1.0.0 [--dry-run]
#
# Prerequisites:
#   - gh CLI authenticated with repo write access
#   - Source dist built at SERAPH_DIST (default: D:/seraph-source/dist)

set -euo pipefail

VERSION="${1:?Usage: ./release.sh <version> [--dry-run]}"
DRY_RUN="${2:-}"
DIST="${SERAPH_DIST:-D:/seraph-source/dist}"

if [[ ! "$VERSION" =~ ^v[0-9] ]]; then
    echo "ERROR: version must start with 'v' (e.g. v1.0.0)" >&2
    exit 1
fi

# Verify dist has artifacts
for dir in ffi python cli; do
    if [ ! -d "$DIST/$dir" ]; then
        echo "ERROR: $DIST/$dir not found. Build first." >&2
        exit 1
    fi
done

echo "=== SERAPH Release: $VERSION ==="
echo "Source dist: $DIST"
echo ""

# --- 1. Sync docs from source dist into release repo ---
echo "Syncing docs..."
for doc in README.md PYTHON_API.md FFI_API.md; do
    if [ -f "$DIST/$doc" ]; then
        cp "$DIST/$doc" "./$doc"
        echo "  $doc"
    fi
done

# --- 2. Stage release archive ---
STAGING="staging/seraph-${VERSION}"
rm -rf staging/
mkdir -p "$STAGING"/{ffi,python,cli}

echo ""
echo "Staging artifacts..."
for platform_dir in "$DIST"/ffi/*/; do
    platform=$(basename "$platform_dir")
    mkdir -p "$STAGING/ffi/$platform"
    cp "$platform_dir"* "$STAGING/ffi/$platform/"
    echo "  ffi/$platform/$(ls "$platform_dir")"
done

for platform_dir in "$DIST"/python/*/; do
    platform=$(basename "$platform_dir")
    mkdir -p "$STAGING/python/$platform"
    cp "$platform_dir"* "$STAGING/python/$platform/"
    echo "  python/$platform/$(ls "$platform_dir")"
done

for platform_dir in "$DIST"/cli/*/; do
    platform=$(basename "$platform_dir")
    mkdir -p "$STAGING/cli/$platform"
    cp "$platform_dir"* "$STAGING/cli/$platform/"
    echo "  cli/$platform/$(ls "$platform_dir")"
done

# --- 3. Create archives ---
echo ""
echo "Creating archives..."
(cd staging && tar czf "../seraph-${VERSION}.tar.gz" "seraph-${VERSION}")
(cd staging && zip -qr "../seraph-${VERSION}.zip" "seraph-${VERSION}")
echo "  seraph-${VERSION}.tar.gz"
echo "  seraph-${VERSION}.zip"

# --- 4. Commit doc updates ---
echo ""
echo "Committing doc updates..."
git add README.md PYTHON_API.md FFI_API.md LICENSE.md .gitignore 2>/dev/null || true
if ! git diff --cached --quiet; then
    git commit -m "Update docs for ${VERSION}"
    echo "  Committed."
else
    echo "  No doc changes to commit."
fi

# --- 5. Tag ---
echo "Tagging ${VERSION}..."
git tag -a "$VERSION" -m "SERAPH ${VERSION}"

# --- 6. Push ---
if [ "$DRY_RUN" = "--dry-run" ]; then
    echo ""
    echo "[DRY RUN] Would push and create release. Skipping."
    echo "Archives ready in repo root. Run without --dry-run to publish."
    rm -rf staging/
    exit 0
fi

echo "Pushing to remote..."
git push origin main --tags

# --- 7. Create GitHub Release ---
echo ""
echo "Creating GitHub Release..."

# Collect all individual binaries as separate assets
ASSETS=("seraph-${VERSION}.tar.gz" "seraph-${VERSION}.zip")
while IFS= read -r f; do
    ASSETS+=("$f")
done < <(find "$STAGING" -type f)

gh release create "$VERSION" \
    --title "SERAPH ${VERSION}" \
    --generate-notes \
    "${ASSETS[@]}"

# --- 8. Cleanup ---
rm -rf staging/ "seraph-${VERSION}.tar.gz" "seraph-${VERSION}.zip"

echo ""
echo "=== Released SERAPH ${VERSION} ==="
echo "https://github.com/Tree-D-Interactive-LLC/seraph/releases/tag/${VERSION}"
