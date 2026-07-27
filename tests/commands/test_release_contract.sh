#!/bin/bash
# Test: release publication is gated to tags whose commit belongs to main.
source "$(dirname "$0")/helpers.sh"

WORKFLOW="$ROOT_DIR/.github/workflows/release.yml"
TAG_WORKFLOW="$ROOT_DIR/.github/workflows/tag-release.yml"

section "release contract — tag ancestry"

if grep -Fq 'git fetch --no-tags origin main' "$WORKFLOW"; then
  _pass "release fetches authoritative main"
else
  _fail "release fetches authoritative main" "missing explicit origin/main fetch"
fi

if grep -Fq 'TAG_COMMIT="$(git rev-list -n 1 "$TAG")"' "$WORKFLOW"; then
  _pass "release resolves the tagged commit"
else
  _fail "release resolves the tagged commit" "missing tag commit resolution"
fi

if grep -Fq 'git merge-base --is-ancestor "$TAG_COMMIT" origin/main' "$WORKFLOW"; then
  _pass "release rejects tags outside main history"
else
  _fail "release rejects tags outside main history" "missing main ancestry gate"
fi

section "release contract — workflow handoff"

if grep -Fq 'workflow_call:' "$WORKFLOW"; then
  _pass "publish workflow accepts a direct reusable call"
else
  _fail "reusable publish workflow" "release.yml has no workflow_call entry"
fi

if grep -Fq 'uses: ./.github/workflows/release.yml' "$TAG_WORKFLOW"; then
  _pass "tag workflow invokes publication directly"
else
  _fail "tag-to-publish handoff" "tag workflow relies only on a token-created push event"
fi

if grep -Fq 'ref: ${{ inputs.tag || github.ref }}' "$WORKFLOW"; then
  _pass "publication checks out the requested tag"
else
  _fail "release tag checkout" "publish workflow does not bind checkout to its tag input"
fi

section "permission contract — Apple Events are tell-only"

OSASCRIPT_SPAWNS=$(rg -n 'Command::new\("osascript"\)' "$ROOT_DIR/src" | wc -l | tr -d ' ')
if [[ "$OSASCRIPT_SPAWNS" == "1" ]] && rg -q 'fn run_applescript_capture' "$ROOT_DIR/src/system.rs"; then
  _pass "only the tell implementation can spawn osascript"
else
  _fail "tell-only osascript boundary" "found $OSASCRIPT_SPAWNS spawn sites"
fi

if rg -q 'ax::window_action' "$ROOT_DIR/src/main.rs" && rg -q 'ax::list_menu' "$ROOT_DIR/src/main.rs"; then
  _pass "window and menu commands route through native AX"
else
  _fail "native AX routing" "window/menu routing is missing"
fi

summary
