#!/bin/bash
# Release script for computer-pilot.
#
# Usage:
#   bash scripts/release.sh <version>      # e.g. bash scripts/release.sh 0.2.1
#   bash scripts/release.sh <version> --dry-run
#
# What it does:
#   1. Verifies clean working tree, on main, in sync with origin
#   2. Updates Cargo.toml version
#   3. Runs cargo build --release
#   4. Runs all tests (must pass)
#   5. Commits the version bump
#   6. Pushes the commit to origin
#   7. Creates and pushes the v<version> tag
#   8. Builds binary, creates GitHub release with cu-arm64 asset
#
# Prerequisites:
#   - gh CLI authenticated (gh auth status)
#   - Clean working tree on main branch
#   - All tests passing
#   - CHANGELOG.md updated (optional but recommended)

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

VERSION="${1:-}"
DRY_RUN=""
[[ "${2:-}" == "--dry-run" ]] && DRY_RUN=1

if [[ -z "$VERSION" ]]; then
  echo "Usage: bash scripts/release.sh <version> [--dry-run]" >&2
  echo "  Example: bash scripts/release.sh 0.2.1" >&2
  exit 1
fi

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?$ ]]; then
  echo "Error: version must be semver (e.g., 0.2.0 or 1.0.0-beta1)" >&2
  exit 1
fi

TAG="v$VERSION"

# ── Pre-flight checks ──────────────────────────────────────────────────────

run() {
  if [[ -n "$DRY_RUN" ]]; then
    echo "[DRY-RUN] $*"
  else
    eval "$@"
  fi
}

echo "→ Pre-flight checks for $TAG"

# 1. Clean working tree
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Error: working tree has uncommitted changes." >&2
  git status --short >&2
  exit 1
fi

# 2. On main branch
BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "$BRANCH" != "main" ]]; then
  echo "Error: not on main branch (currently: $BRANCH)" >&2
  exit 1
fi

# 3. In sync with origin
git fetch origin main --quiet
LOCAL="$(git rev-parse @)"
REMOTE="$(git rev-parse @{u})"
if [[ "$LOCAL" != "$REMOTE" ]]; then
  echo "Error: local main is not in sync with origin/main." >&2
  echo "  Local:  $LOCAL" >&2
  echo "  Remote: $REMOTE" >&2
  exit 1
fi

# 4. Tag doesn't already exist
if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "Error: tag $TAG already exists." >&2
  exit 1
fi
if gh release view "$TAG" >/dev/null 2>&1; then
  echo "Error: GitHub release $TAG already exists." >&2
  exit 1
fi

# 5. gh CLI ready
if ! command -v gh >/dev/null; then
  echo "Error: gh CLI not installed." >&2
  exit 1
fi
if ! gh auth status >/dev/null 2>&1; then
  echo "Error: gh CLI not authenticated. Run: gh auth login" >&2
  exit 1
fi

echo "  ✓ Working tree clean"
echo "  ✓ On main branch"
echo "  ✓ In sync with origin"
echo "  ✓ Tag $TAG does not exist"
echo "  ✓ gh CLI ready"

# ── Version bump ───────────────────────────────────────────────────────────

echo ""
echo "→ Updating Cargo.toml to version $VERSION"
CURRENT="$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')"
echo "  $CURRENT → $VERSION"
run "sed -i.bak 's/^version = \"$CURRENT\"/version = \"$VERSION\"/' Cargo.toml && rm Cargo.toml.bak"

# ── Build & test ───────────────────────────────────────────────────────────

echo ""
echo "→ Building release binary"
run "cargo build --release"

echo ""
echo "→ Running test suite"
run "bash tests/commands/run_all.sh"

# Verify binary version matches
if [[ -z "$DRY_RUN" ]]; then
  BIN_VERSION="$(./target/release/cu --version | awk '{print $2}')"
  if [[ "$BIN_VERSION" != "$VERSION" ]]; then
    echo "Error: binary reports version $BIN_VERSION, expected $VERSION" >&2
    exit 1
  fi
  echo "  ✓ Binary reports cu $BIN_VERSION"
fi

# ── Commit, tag, push ──────────────────────────────────────────────────────

echo ""
echo "→ Committing version bump"
run "git add Cargo.toml Cargo.lock"
run "git commit -m 'Bump version to $VERSION'"

echo ""
echo "→ Pushing commit and tag"
run "git push origin main"
run "git tag -a $TAG -m '$TAG'"
run "git push origin $TAG"

# ── GitHub release ─────────────────────────────────────────────────────────

echo ""
echo "→ Creating GitHub release"
ASSET="/tmp/cu-arm64-$VERSION"
run "cp ./target/release/cu '$ASSET'"

# Generate notes from commits since last tag
LAST_TAG="$(git describe --tags --abbrev=0 HEAD^ 2>/dev/null || echo '')"
if [[ -n "$LAST_TAG" ]]; then
  NOTES_BODY="$(git log --oneline "$LAST_TAG"..HEAD | sed 's/^/- /')"
else
  NOTES_BODY="Initial release"
fi

NOTES_FILE="$(mktemp)"
cat > "$NOTES_FILE" <<EOF
## cu $VERSION

### Install

\`\`\`bash
sudo curl -Lo /usr/local/bin/cu https://github.com/relixiaobo/computer-pilot/releases/download/$TAG/cu-arm64 && sudo chmod +x /usr/local/bin/cu && cu setup
\`\`\`

### Changes since ${LAST_TAG:-start}

$NOTES_BODY

**Full diff**: https://github.com/relixiaobo/computer-pilot/compare/${LAST_TAG:-$TAG}...$TAG
EOF

if [[ -n "$DRY_RUN" ]]; then
  echo "[DRY-RUN] gh release create $TAG '$ASSET' --title 'cu $VERSION' --notes-file $NOTES_FILE"
  echo ""
  echo "[DRY-RUN] Notes:"
  cat "$NOTES_FILE"
else
  gh release create "$TAG" "$ASSET" --title "cu $VERSION" --notes-file "$NOTES_FILE"
  rm -f "$ASSET" "$NOTES_FILE"
fi

echo ""
echo "✓ Released $TAG"
echo "  https://github.com/relixiaobo/computer-pilot/releases/tag/$TAG"
