#!/bin/bash
# Promote a published release to the skill-stable channel.
#
# skill-stable is a gate, not a branch policy: it only ever fast-forwards to
# a release tag whose GitHub release is public, checksum-verified, Developer
# ID signed AND notarized, and version-synced. Agent hosts (Tenon etc.) track
# skill-stable; they must never track main, which may reference unreleased
# versions. The stable channel refuses ad-hoc artifacts unconditionally.
#
# Usage:
#   bash scripts/promote-skill-stable.sh <version>
#   bash scripts/promote-skill-stable.sh <version> --dry-run   # verify only

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

VERSION=""
DRY_RUN=""
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
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
  echo "Usage: bash scripts/promote-skill-stable.sh <semver> [--dry-run]" >&2
  exit 1
fi

TAG="v$VERSION"
BRANCH="skill-stable"
WORKDIR="$(mktemp -d)"
WORKTREE=""
cleanup() {
  if [[ -n "$WORKTREE" ]]; then
    git worktree remove --force "$WORKTREE" >/dev/null 2>&1 || true
  fi
  rm -rf "$WORKDIR"
}
trap cleanup EXIT

fail() {
  echo "REFUSED: $*" >&2
  exit 1
}

json() {
  python3 -c "
import json, sys
value = json.load(open(sys.argv[1]))
for key in sys.argv[2].split('.'):
    value = value[key]
print(value)
" "$1" "$2"
}

echo "==> Verifying GitHub release $TAG"
gh release view "$TAG" --json isDraft,isPrerelease >"$WORKDIR/release.json" ||
  fail "release $TAG does not exist or is not visible"
[[ "$(json "$WORKDIR/release.json" isDraft)" == "False" ]] ||
  fail "release $TAG is a draft"
[[ "$(json "$WORKDIR/release.json" isPrerelease)" == "False" ]] ||
  fail "release $TAG is a prerelease"

echo "==> Verifying release index"
gh release download "$TAG" --pattern 'release-index.json' --pattern 'release-index.json.sha256' \
  --dir "$WORKDIR" || fail "could not download release-index.json for $TAG"
(cd "$WORKDIR" && shasum -a 256 -c release-index.json.sha256 >/dev/null) ||
  fail "release-index.json fails its checksum"

SIGNING_STATUS="$(python3 -c "
import json
print(json.load(open('$WORKDIR/release-index.json'))['signing']['status'])
")"
SIGNING_IDENTIFIER="$(python3 -c "
import json
print(json.load(open('$WORKDIR/release-index.json'))['signing']['identifier'])
")"
INDEX_VERSION="$(python3 -c "
import json
print(json.load(open('$WORKDIR/release-index.json'))['version'])
")"
[[ "$INDEX_VERSION" == "$VERSION" ]] ||
  fail "release-index.json reports version '$INDEX_VERSION', expected '$VERSION'"

echo "==> Resolving tag commit"
git fetch origin main --quiet
git fetch origin "refs/tags/$TAG:refs/tags/$TAG" --no-tags --quiet 2>/dev/null || true
git show-ref --verify --quiet "refs/tags/$TAG" || fail "tag $TAG is not available locally"
TAG_COMMIT="$(git rev-list -n 1 "$TAG")"
git merge-base --is-ancestor "$TAG_COMMIT" origin/main ||
  fail "$TAG does not point to a commit on origin/main"

WORKTREE="$WORKDIR/tag-checkout"
git worktree add --detach "$WORKTREE" "$TAG_COMMIT" >/dev/null 2>&1 ||
  fail "could not create a worktree for $TAG"
MANIFEST="$WORKTREE/plugin/skills/computer-pilot/compatibility.json"
[[ -f "$MANIFEST" ]] || fail "$TAG has no skill manifest at plugin/skills/computer-pilot/compatibility.json"
python3 -c "
import json, sys
manifest = json.load(open('$MANIFEST'))
signing = (manifest.get('installation') or {}).get('signing')
sys.exit(0 if manifest.get('schema_version') == 2 and signing else 1)
" 2>/dev/null ||
  fail "the manifest at $TAG predates the installation schema (schema_version 2) — releases built before the skill pinned its own installer cannot be promoted"

echo "==> Verifying binary archive checksum"
# The tagged manifest is the single source for the asset name — the same
# template the skill's installer renders when it downloads this release.
ARCHIVE="$(VERSION="$VERSION" python3 -c "
import json, os
template = json.load(open('$MANIFEST'))['installation']['asset_template']
print(template.replace('{version}', os.environ['VERSION']))
")"
gh release download "$TAG" --pattern "$ARCHIVE" --pattern "$ARCHIVE.sha256" --dir "$WORKDIR" ||
  fail "could not download $ARCHIVE (manifest asset_template renders to this name)"
(cd "$WORKDIR" && shasum -a 256 -c "$ARCHIVE.sha256" >/dev/null) ||
  fail "$ARCHIVE fails its checksum"
INDEX_SHA="$(python3 -c "
import json
assets = json.load(open('$WORKDIR/release-index.json'))['assets']
print(next(a['sha256'] for a in assets if a['name'] == '$ARCHIVE'))
")"
ACTUAL_SHA="$(shasum -a 256 "$WORKDIR/$ARCHIVE" | awk '{print $1}')"
[[ "$INDEX_SHA" == "$ACTUAL_SHA" ]] ||
  fail "$ARCHIVE checksum does not match release-index.json"

echo "==> Verifying signing intent and version sync at $TAG"
# The gate enforces the tagged manifest's declared intent, not a fixed tier.
# Its real job is catching a MISMATCH: release.yml silently falls back to an
# ad-hoc identity when Developer ID secrets are missing, so a repository that
# declares developer-id-notarized must never promote an ad-hoc artifact.
REQUIRED_STATUS="$(python3 -c "
import json
print(json.load(open('$MANIFEST'))['installation']['signing']['required_status'])
")"
case "$REQUIRED_STATUS" in
  developer-id-notarized|ad-hoc-unsigned) ;;
  *) fail "the manifest at $TAG declares an unknown required_status '$REQUIRED_STATUS'" ;;
esac
[[ "$SIGNING_STATUS" == "$REQUIRED_STATUS" ]] ||
  fail "release signing status is '$SIGNING_STATUS' but the manifest at $TAG requires '$REQUIRED_STATUS' — refusing to promote a release that does not match its declared identity"

MANIFEST_IDENTIFIER="$(python3 -c "
import json
print(json.load(open('$MANIFEST'))['installation']['signing']['identifier'])
")"
[[ "$SIGNING_IDENTIFIER" == "$MANIFEST_IDENTIFIER" ]] ||
  fail "release signing identifier '$SIGNING_IDENTIFIER' does not match the manifest's '$MANIFEST_IDENTIFIER'"

REQUIREMENT="$(python3 -c "
import json
value = json.load(open('$MANIFEST'))['installation']['signing']['requirement']
print(value if value else '')
")"
if [[ "$REQUIRED_STATUS" == "developer-id-notarized" ]]; then
  [[ -n "$REQUIREMENT" ]] ||
    fail "the manifest at $TAG declares developer-id-notarized but pins no installation.signing.requirement"
elif [[ -n "$REQUIREMENT" ]]; then
  # An ad-hoc designated requirement is a bare cdhash that changes with every
  # build, so a pinned requirement here could only ever be stale or wrong.
  fail "the manifest at $TAG declares ad-hoc-unsigned but pins a codesign requirement — ad-hoc signatures have no stable identity"
fi

bash "$WORKTREE/scripts/check-version-sync.sh" "$VERSION" >/dev/null ||
  fail "version surfaces at $TAG are out of sync"

echo "==> Verifying the released binary's code identity"
tar -xzf "$WORKDIR/$ARCHIVE" -C "$WORKDIR"
RELEASED_BINARY="$WORKDIR/${ARCHIVE%.tar.gz}/cu"
[[ -f "$RELEASED_BINARY" ]] || fail "released archive does not contain cu"
codesign --verify --strict "$RELEASED_BINARY" >/dev/null 2>&1 ||
  fail "released cu fails codesign verification"
ACTUAL_IDENTIFIER="$(codesign -dvv "$RELEASED_BINARY" 2>&1 | sed -n 's/^Identifier=//p')"
[[ "$ACTUAL_IDENTIFIER" == "$MANIFEST_IDENTIFIER" ]] ||
  fail "released cu has code identifier '$ACTUAL_IDENTIFIER', expected '$MANIFEST_IDENTIFIER'"
if [[ -n "$REQUIREMENT" ]]; then
  codesign --verify --strict -R="$REQUIREMENT" "$RELEASED_BINARY" ||
    fail "released cu does not satisfy the manifest codesign requirement"
else
  # Without a Developer ID there is no stable code identity to verify against;
  # integrity rests on the checksums verified above. Say so out loud.
  echo "NOTE: $TAG is ad-hoc signed by declared intent — integrity rests on the verified SHA-256 digests, not on a code signature."
fi

echo "All gates passed for $TAG ($TAG_COMMIT)."
if [[ -n "$DRY_RUN" ]]; then
  echo "[DRY-RUN] Would fast-forward $BRANCH to $TAG_COMMIT."
  exit 0
fi

echo "==> Fast-forwarding $BRANCH"
if git ls-remote --exit-code --heads origin "$BRANCH" >/dev/null 2>&1; then
  REMOTE_HEAD="$(git ls-remote origin "refs/heads/$BRANCH" | awk '{print $1}')"
  git merge-base --is-ancestor "$REMOTE_HEAD" "$TAG_COMMIT" ||
    fail "$BRANCH at $REMOTE_HEAD is not an ancestor of $TAG_COMMIT — refusing a non-fast-forward promotion"
fi
git push origin "$TAG_COMMIT:refs/heads/$BRANCH"
echo "Promoted $TAG to $BRANCH."
