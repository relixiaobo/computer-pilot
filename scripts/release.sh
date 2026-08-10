#!/bin/bash
# Prepare a release pull request. This script never pushes or tags main.
#
# Usage:
#   bash scripts/release.sh <version>
#   bash scripts/release.sh <version> --dry-run
#   bash scripts/release.sh <version> --skip-tests --skip-agent

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

VERSION=""
DRY_RUN=""
SKIP_TESTS=""
SKIP_AGENT=""

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    --skip-tests) SKIP_TESTS=1 ;;
    --skip-agent) SKIP_AGENT=1 ;;
    --*) echo "Error: unknown flag '$arg'" >&2; exit 1 ;;
    *)
      if [[ -n "$VERSION" ]]; then
        echo "Error: unexpected argument '$arg'" >&2
        exit 1
      fi
      VERSION="$arg"
      ;;
  esac
done

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-z0-9.]+)?$ ]]; then
  echo "Usage: bash scripts/release.sh <semver> [--dry-run] [--skip-tests] [--skip-agent]" >&2
  exit 1
fi

TAG="v$VERSION"
BRANCH="release/$TAG"

run() {
  if [[ -n "$DRY_RUN" ]]; then
    echo "[DRY-RUN] $*"
  else
    "$@"
  fi
}

# The version surfaces this script edits, in the order it edits them. Also the
# exact set its own recovery is allowed to touch.
VERSION_FILES=(
  Cargo.toml
  Cargo.lock
  plugin/.claude-plugin/plugin.json
  plugin/package.json
  .claude-plugin/marketplace.json
  plugin/skills/computer-pilot/compatibility.json
)

# An interrupted attempt leaves exactly two traces: the version surfaces edited
# in the working tree, and an empty release branch. Both then fail this run's
# own preflight, so a retry's first obstacle is the wreckage of the last one —
# and the tests never get a chance to run. A trap alone cannot fix this: a
# killed process never runs one, so recovery has to happen on the way in.
#
# Only provably-ours state is touched: the branch must carry no commits and
# must not exist on the remote, and no file may be dirty outside VERSION_FILES.
# Anything else is left alone and still fails the checks below.
dirty_outside_version_files() {
  local path
  while read -r path; do
    [[ -z "$path" ]] && continue
    local known=""
    for vf in "${VERSION_FILES[@]}"; do
      [[ "$path" == "$vf" ]] && known=1 && break
    done
    [[ -z "$known" ]] && return 0
  done < <(git status --porcelain | grep -v '^??' | awk '{print $2}')
  return 1
}

branch_is_an_abandoned_attempt() {
  git show-ref --verify --quiet "refs/heads/$BRANCH" || return 1
  git ls-remote --exit-code --heads origin "$BRANCH" >/dev/null 2>&1 && return 1
  [[ "$(git rev-list --count "origin/main..$BRANCH" 2>/dev/null || echo 1)" == "0" ]]
}

git fetch origin main --quiet

if ! dirty_outside_version_files && [[ -n "$(git status --porcelain | grep -v '^??' || true)" ]] \
   || branch_is_an_abandoned_attempt; then
  if [[ -n "$DRY_RUN" ]]; then
    echo "[DRY-RUN] discard leftover version edits and delete abandoned branch $BRANCH"
  else
    echo "Recovering from an interrupted attempt: discarding leftover version edits"
    git checkout -- "${VERSION_FILES[@]}" 2>/dev/null || true
    if branch_is_an_abandoned_attempt; then
      echo "Recovering from an interrupted attempt: deleting empty branch $BRANCH"
      [[ "$(git branch --show-current)" == "$BRANCH" ]] && git switch --quiet main
      git branch -D "$BRANCH" >/dev/null
    fi
  fi
fi

# Untracked files are not an obstacle: the release commit adds six named paths
# and never `git add -A`, so nothing untracked can reach it. Counting them as
# dirty forced callers to stash their own work first — and a run killed
# mid-flight then left that work stranded in a stash, which is a far worse
# failure than the one the check was guarding against.
if [[ -n "$(git status --porcelain | grep -v '^??' || true)" ]]; then
  echo "Error: release preparation requires no uncommitted changes to tracked files." >&2
  git status --short | grep -v '^??' >&2
  exit 1
fi
UNTRACKED_COUNT=$(git status --porcelain | grep -c '^??' || true)
if [[ "$UNTRACKED_COUNT" -gt 0 ]]; then
  echo "Note: $UNTRACKED_COUNT untracked path(s) present; they are not part of the release commit."
fi
if [[ "$(git branch --show-current)" != "main" ]]; then
  echo "Error: start release preparation from main." >&2
  exit 1
fi

git fetch origin main --quiet
if [[ "$(git rev-parse HEAD)" != "$(git rev-parse origin/main)" ]]; then
  echo "Error: local main must match origin/main." >&2
  exit 1
fi
if git show-ref --verify --quiet "refs/tags/$TAG"; then
  echo "Error: tag $TAG already exists." >&2
  exit 1
fi
if git show-ref --verify --quiet "refs/heads/$BRANCH" || git ls-remote --exit-code --heads origin "$BRANCH" >/dev/null 2>&1; then
  echo "Error: release branch $BRANCH already exists." >&2
  exit 1
fi

CURRENT="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)"
if [[ -z "$CURRENT" ]]; then
  echo "Error: could not read current version." >&2
  exit 1
fi

run git switch -c "$BRANCH"
if [[ -n "$DRY_RUN" ]]; then
  echo "[DRY-RUN] update version $CURRENT -> $VERSION in Cargo.toml, plugin manifests, marketplace, and compatibility.json"
else
  sed -i.bak "s/^version = \"$CURRENT\"/version = \"$VERSION\"/" Cargo.toml
  sed -i.bak "s/\"version\": \"$CURRENT\"/\"version\": \"$VERSION\"/" plugin/.claude-plugin/plugin.json
  sed -i.bak "s/\"version\": \"$CURRENT\"/\"version\": \"$VERSION\"/" plugin/package.json
  sed -i.bak "s/\"version\": \"$CURRENT\"/\"version\": \"$VERSION\"/" .claude-plugin/marketplace.json
  sed -i.bak "s/\"version\": \"$CURRENT\"/\"version\": \"$VERSION\"/g" plugin/skills/computer-pilot/compatibility.json
  # Exact-pin support policy: tested_version and minimum_version move with the
  # release version until cross-version compatibility tests justify a range.
  sed -i.bak "s/\"tested_version\": \"$CURRENT\"/\"tested_version\": \"$VERSION\"/" plugin/skills/computer-pilot/compatibility.json
  sed -i.bak "s/\"minimum_version\": \"$CURRENT\"/\"minimum_version\": \"$VERSION\"/" plugin/skills/computer-pilot/compatibility.json
  rm Cargo.toml.bak plugin/.claude-plugin/plugin.json.bak plugin/package.json.bak
  rm .claude-plugin/marketplace.json.bak plugin/skills/computer-pilot/compatibility.json.bak
fi

run bash scripts/check-version-sync.sh "$VERSION"
# One entry point for every gate, so the release cannot pass on a narrower
# set of checks than CI runs. --skip-tests still covers fmt/clippy/cargo
# test/build; only the desktop command suite is dropped.
if [[ -n "$SKIP_TESTS" ]]; then
  run bash scripts/verify.sh --skip-desktop
else
  run bash scripts/verify.sh
fi

if [[ -z "$SKIP_AGENT" && -f tests/agent/run.py ]]; then
  HAS_KEY=""
  if [[ -n "${ANTHROPIC_API_KEY:-}" || -n "${OPENAI_API_KEY:-}" ]]; then
    HAS_KEY=1
  elif [[ -f .env ]] && grep -qE '^(ANTHROPIC_API_KEY|OPENAI_API_KEY)=' .env; then
    HAS_KEY=1
  fi
  if [[ -n "$HAS_KEY" ]]; then
    run python3 tests/agent/run.py
  fi
fi

run git add Cargo.toml Cargo.lock plugin/.claude-plugin/plugin.json plugin/package.json
run git add .claude-plugin/marketplace.json plugin/skills/computer-pilot/compatibility.json
run git commit -m "Prepare $TAG"
run git push -u origin "$BRANCH"
run gh pr create --draft --base main --head "$BRANCH" --title "Release $TAG" --body "Prepare Computer Pilot $TAG. CI must pass before merge. After merge, run the Tag Release workflow for $VERSION."

echo "Prepared release PR for $TAG. This script did not modify or tag main."
