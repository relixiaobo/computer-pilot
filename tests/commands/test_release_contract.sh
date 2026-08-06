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

section "release contract — signing provenance"

if grep -Fq "HAS_DEVELOPER_ID" "$WORKFLOW" && grep -Fq "ad-hoc-unsigned" "$WORKFLOW"; then
  _pass "release falls back explicitly when Developer ID secrets are absent"
else
  _fail "unsigned release fallback" "workflow still requires unavailable signing secrets"
fi

if grep -Fq 'COMPUTER_PILOT_SIGNING_STATUS' "$ROOT_DIR/scripts/build-release-assets.sh" \
  && grep -Fq '"signing"' "$ROOT_DIR/scripts/build-release-assets.sh"; then
  _pass "release index records signing provenance"
else
  _fail "release signing provenance" "release-index.json omits signing status"
fi

section "release contract — skill-stable promotion gate"

PROMOTE="$ROOT_DIR/scripts/promote-skill-stable.sh"

# The gate enforces the tagged manifest's declared signing intent rather than
# a fixed tier, because release.yml silently falls back to an ad-hoc identity
# when Developer ID secrets are absent — promoting that mismatch is the risk.
if [[ -f "$PROMOTE" ]] && grep -Fq '"$SIGNING_STATUS" == "$REQUIRED_STATUS"' "$PROMOTE" \
  && grep -Fq 'does not match its declared identity' "$PROMOTE"; then
  _pass "skill-stable promotion refuses a release that contradicts its declared identity"
else
  _fail "skill-stable promotion refuses a release that contradicts its declared identity" \
    "promote script no longer compares release signing status against the manifest"
fi

if grep -Fq 'ACTUAL_IDENTIFIER' "$PROMOTE" && grep -Fq 'MANIFEST_IDENTIFIER' "$PROMOTE"; then
  _pass "promotion pins the released binary's code identifier"
else
  _fail "promotion pins the released binary's code identifier" \
    "promote script does not verify the codesign identifier"
fi

# An ad-hoc designated requirement is a bare cdhash that changes every build,
# so pinning one alongside ad-hoc-unsigned could only ever be wrong.
if grep -Fq 'ad-hoc signatures have no stable identity' "$PROMOTE"; then
  _pass "promotion rejects a pinned requirement under an ad-hoc declaration"
else
  _fail "promotion rejects a pinned requirement under an ad-hoc declaration" \
    "promote script accepts a requirement that ad-hoc signing cannot satisfy"
fi

# The manifest's own declaration must be one the gate understands.
MANIFEST_STATUS=$(python3 -c "
import json
print(json.load(open('$ROOT_DIR/plugin/skills/computer-pilot/compatibility.json'))['installation']['signing']['required_status'])
")
MANIFEST_REQUIREMENT=$(python3 -c "
import json
value = json.load(open('$ROOT_DIR/plugin/skills/computer-pilot/compatibility.json'))['installation']['signing']['requirement']
print(value if value else '')
")
case "$MANIFEST_STATUS" in
  developer-id-notarized)
    if [[ -n "$MANIFEST_REQUIREMENT" ]]; then
      _pass "signed declaration pins a codesign requirement"
    else
      _fail "signed declaration pins a codesign requirement" "required_status is signed but requirement is null"
    fi
    ;;
  ad-hoc-unsigned)
    if [[ -z "$MANIFEST_REQUIREMENT" ]]; then
      _pass "ad-hoc declaration pins no codesign requirement"
    else
      _fail "ad-hoc declaration pins no codesign requirement" "requirement '$MANIFEST_REQUIREMENT' cannot be satisfied by an ad-hoc signature"
    fi
    ;;
  *)
    _fail "manifest declares a known signing tier" "got '$MANIFEST_STATUS'"
    ;;
esac

if grep -Fq 'merge-base --is-ancestor' "$PROMOTE" \
  && grep -Fq 'check-version-sync.sh' "$PROMOTE" \
  && grep -Fq 'codesign --verify' "$PROMOTE"; then
  _pass "promotion verifies ancestry, version sync, and codesign requirement"
else
  _fail "promotion verification gates" "promote script is missing a required gate"
fi

if grep -Fq 'tested_version' "$ROOT_DIR/scripts/release.sh" \
  && grep -Fq 'tested_version' "$ROOT_DIR/scripts/check-version-sync.sh"; then
  _pass "release tooling moves and enforces the manifest version pins"
else
  _fail "manifest version pin tooling" "release.sh or check-version-sync.sh does not handle tested_version"
fi

section "release contract — one source for the binary asset name"

# The publisher, the promotion gate, and the skill's installer must agree on
# what the release asset is called; a rename that only lands in one of them
# breaks every install with no CI signal.
MANIFEST_TEMPLATE=$(python3 -c "
import json
print(json.load(open('$ROOT_DIR/plugin/skills/computer-pilot/compatibility.json'))['installation']['asset_template'])
")
if [[ -n "$MANIFEST_TEMPLATE" && "$MANIFEST_TEMPLATE" == *"{version}"* ]]; then
  _pass "manifest declares a versioned asset template"
else
  _fail "manifest declares a versioned asset template" "got '$MANIFEST_TEMPLATE'"
fi

for script in build-release-assets.sh promote-skill-stable.sh; do
  if grep -Fq "asset_template" "$ROOT_DIR/scripts/$script"; then
    _pass "$script reads the asset name from the manifest"
  else
    _fail "$script reads the asset name from the manifest" \
      "no asset_template lookup — the archive name is hardcoded again"
  fi
done

INSTALLER_DEFAULT=$(sed -n "s/^DEFAULT_ASSET_TEMPLATE='\(.*\)'$/\1/p" \
  "$ROOT_DIR/plugin/skills/computer-pilot/scripts/install-native.sh")
if [[ "$INSTALLER_DEFAULT" == "$MANIFEST_TEMPLATE" ]]; then
  _pass "installer default asset template matches the manifest"
else
  _fail "installer default asset template matches the manifest" \
    "manifest='$MANIFEST_TEMPLATE' installer='$INSTALLER_DEFAULT'"
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
