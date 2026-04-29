#!/bin/bash
# Release script for computer-pilot.
#
# Usage:
#   bash scripts/release.sh <version>                      # full release
#   bash scripts/release.sh <version> --dry-run            # walk through, run nothing
#   bash scripts/release.sh <version> --skip-tests         # skip the L1 test run
#                                                          # (use only when you JUST ran
#                                                          #  run_all.sh manually and got 0 failures)
#   bash scripts/release.sh <version> --skip-agent         # skip the L2 agent E2E run
#                                                          # (default: L2 runs if .env has an
#                                                          #  ANTHROPIC_API_KEY or OPENAI_API_KEY;
#                                                          #  no key → silently skipped, not an error)
#   Flags can combine: --dry-run --skip-tests --skip-agent
#
# What it does:
#   1. Verifies clean working tree, on main, in sync with origin
#   2. Updates Cargo.toml version
#   3. Runs cargo build --release
#   4. Runs L1 command tests (must pass) — skipped with --skip-tests
#   5. Runs L2 agent E2E (must pass) — skipped with --skip-agent or when no API key
#   6. Commits the version bump
#   7. Pushes the commit to origin
#   8. Creates and pushes the v<version> tag
#   9. Builds binary, creates GitHub release with cu-arm64 asset
#
# Prerequisites:
#   - gh CLI authenticated (gh auth status)
#   - Clean working tree on main branch
#   - All tests passing
#   - CHANGELOG.md updated (optional but recommended)

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

VERSION=""
DRY_RUN=""
SKIP_TESTS=""
SKIP_AGENT=""

# Accept flags in any position; first positional arg is version.
for arg in "$@"; do
  case "$arg" in
    --dry-run)    DRY_RUN=1 ;;
    --skip-tests) SKIP_TESTS=1 ;;
    --skip-agent) SKIP_AGENT=1 ;;
    --*)
      echo "Error: unknown flag '$arg'" >&2
      echo "Usage: bash scripts/release.sh <version> [--dry-run] [--skip-tests] [--skip-agent]" >&2
      exit 1
      ;;
    *)
      if [[ -z "$VERSION" ]]; then
        VERSION="$arg"
      else
        echo "Error: unexpected positional argument '$arg'" >&2
        exit 1
      fi
      ;;
  esac
done

if [[ -z "$VERSION" ]]; then
  echo "Usage: bash scripts/release.sh <version> [--dry-run] [--skip-tests] [--skip-agent]" >&2
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
echo "→ Updating version to $VERSION in:"
CURRENT="$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')"
echo "  Cargo.toml ($CURRENT → $VERSION)"
echo "  plugin/.claude-plugin/plugin.json"
echo "  .claude-plugin/marketplace.json"
run "sed -i.bak 's/^version = \"$CURRENT\"/version = \"$VERSION\"/' Cargo.toml && rm Cargo.toml.bak"
run "sed -i.bak 's/\"version\": \"$CURRENT\"/\"version\": \"$VERSION\"/' plugin/.claude-plugin/plugin.json && rm plugin/.claude-plugin/plugin.json.bak"
run "sed -i.bak 's/\"version\": \"$CURRENT\"/\"version\": \"$VERSION\"/' .claude-plugin/marketplace.json && rm .claude-plugin/marketplace.json.bak"

# ── Build & test ───────────────────────────────────────────────────────────

echo ""
echo "→ Building release binary"
run "cargo build --release"

if [[ -n "$SKIP_TESTS" ]]; then
  echo ""
  echo "→ Skipping test suite (--skip-tests)"
  echo "  Caller is responsible for having run \`bash tests/commands/run_all.sh\` recently."
else
  echo ""
  echo "→ Running test suite"
  run "bash tests/commands/run_all.sh"
fi

# Verify binary version matches
if [[ -z "$DRY_RUN" ]]; then
  BIN_VERSION="$(./target/release/cu --version | awk '{print $2}')"
  if [[ "$BIN_VERSION" != "$VERSION" ]]; then
    echo "Error: binary reports version $BIN_VERSION, expected $VERSION" >&2
    exit 1
  fi
  echo "  ✓ Binary reports cu $BIN_VERSION"
fi

# ── L2 agent E2E ───────────────────────────────────────────────────────────
# L1 (above) catches "did the protocol regress". L2 catches "does an LLM
# agent, given just SKILL.md and the cu CLI, actually complete a task".
# These are different failure classes — L1 cannot catch agent training-prior
# regressions (e.g. agent invents flags, falls back to osascript, ignores
# verify_advice). L2 needs an API key; missing key is "skip", not "fail".

echo ""
if [[ -n "$SKIP_AGENT" ]]; then
  echo "→ Skipping agent E2E (--skip-agent)"
elif [[ ! -f tests/agent/run.py ]]; then
  echo "→ Skipping agent E2E (tests/agent/run.py not found)"
else
  HAS_KEY=""
  # Check shell env first, then .env file (run.py reads both).
  if [[ -n "${ANTHROPIC_API_KEY:-}" || -n "${OPENAI_API_KEY:-}" ]]; then
    HAS_KEY=1
  elif [[ -f .env ]] && grep -qE '^(ANTHROPIC_API_KEY|OPENAI_API_KEY)=' .env; then
    HAS_KEY=1
  fi

  if [[ -z "$HAS_KEY" ]]; then
    echo "→ Skipping agent E2E (no ANTHROPIC_API_KEY / OPENAI_API_KEY in env or .env)"
    echo "  Set one to enable real-agent verification on every release."
  else
    echo "→ Running agent E2E (catches training-prior regressions L1 cannot see)"
    run "python3 tests/agent/run.py"
  fi
fi

# ── Commit, tag, push ──────────────────────────────────────────────────────

echo ""
echo "→ Committing version bump"
run "git add Cargo.toml Cargo.lock plugin/.claude-plugin/plugin.json .claude-plugin/marketplace.json"
run "git commit -m 'Bump version to $VERSION'"

echo ""
echo "→ Pushing commit and tag"
run "git push origin main"
run "git tag -a $TAG -m '$TAG'"
run "git push origin $TAG"

# ── GitHub release ─────────────────────────────────────────────────────────

echo ""
echo "→ Creating GitHub release"
# Two binary assets per release:
#   cu-arm64-$VERSION  versioned (provenance / pinning)
#   cu-arm64           unversioned alias (so the README's
#                       /releases/latest/download/cu-arm64 URL resolves)
ASSET="/tmp/cu-arm64-$VERSION"
ASSET_ALIAS="/tmp/cu-arm64"
run "cp ./target/release/cu '$ASSET'"
run "cp ./target/release/cu '$ASSET_ALIAS'"

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
  echo "[DRY-RUN] gh release create $TAG '$ASSET' '$ASSET_ALIAS' --title 'cu $VERSION' --notes-file $NOTES_FILE"
  echo ""
  echo "[DRY-RUN] Notes:"
  cat "$NOTES_FILE"
else
  gh release create "$TAG" "$ASSET" "$ASSET_ALIAS" --title "cu $VERSION" --notes-file "$NOTES_FILE"
  rm -f "$ASSET" "$ASSET_ALIAS" "$NOTES_FILE"
fi

echo ""
echo "✓ Released $TAG"
echo "  https://github.com/relixiaobo/computer-pilot/releases/tag/$TAG"
