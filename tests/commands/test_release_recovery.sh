#!/bin/bash
# Test: release.sh recovers from its own interrupted attempt.
#
# An interrupted run leaves the version surfaces edited and an empty release
# branch behind. Both then fail the next run's preflight, so a retry's first
# obstacle is the wreckage of the last one and the tests never get to run —
# three consecutive release attempts died this way. A trap cannot fix it: a
# killed process never runs one, so recovery has to happen on the way in.
#
# Every case runs in a throwaway clone; nothing here touches the real repo.
source "$(dirname "$0")/helpers.sh"

section "release.sh — recovery from an interrupted attempt"

SANDBOX=$(mktemp -d "${TMPDIR:-/tmp}/cu-release-recovery.XXXXXX")
cleanup_sandbox() { rm -rf "$SANDBOX"; }
trap cleanup_sandbox EXIT

# L1 must exercise the script on disk, not the one at HEAD — otherwise a
# change is only ever tested after it is already merged. So: mirror the repo
# into a bare origin we own, land the working-tree release.sh there, and clone
# test copies from that. Committing inside each clone instead would leave main
# ahead of its origin, which release.sh rejects long before the checks under
# test — and the assertions would then pass on the wrong error message.
#
# The sandbox must not inherit whatever branch the host repo happens to be on.
# release.sh runs this suite from inside its own release branch, so a bare
# clone's HEAD points at release/vX.Y.Z and every sandbox then fails with
# "start release preparation from main" — the suite measuring its environment
# instead of the thing under test.
ORIGIN="$SANDBOX/origin.git"
git clone --quiet --bare --local --no-hardlinks "$ROOT_DIR" "$ORIGIN" 2>"$SANDBOX/clone.err"
git -C "$ORIGIN" symbolic-ref HEAD refs/heads/main
git clone --quiet --local --branch main "$ORIGIN" "$SANDBOX/prime" 2>>"$SANDBOX/clone.err"
cp "$ROOT_DIR/scripts/release.sh" "$SANDBOX/prime/scripts/release.sh"
(cd "$SANDBOX/prime" \
  && git -c user.email=t@t -c user.name=t commit --quiet -am "use working-tree release.sh" 2>/dev/null \
  && git push --quiet origin main) 2>>"$SANDBOX/clone.err" || true
rm -rf "$SANDBOX/prime"

make_clone() {
  git clone --quiet --local --branch main "$ORIGIN" "$1" 2>>"$SANDBOX/clone.err"
}

CLONE="$SANDBOX/repo"
if ! make_clone "$CLONE"; then
  _fail "sandbox clone" "$(head -c 200 "$SANDBOX/clone.err")"
  summary
fi

# The clone's HEAD must be on main and match its origin, which is what
# release.sh checks. Pick a version above whatever the clone carries.
NEXT_VERSION=$(cd "$CLONE" && python3 -c "
import re
v = re.search(r'^version = \"([^\"]+)\"', open('Cargo.toml').read(), re.M).group(1)
major, minor, patch = (int(x) for x in v.split('-')[0].split('.'))
print(f'{major}.{minor}.{patch + 99}')
")
BRANCH="release/v$NEXT_VERSION"

release_dry_run() {
  (cd "$CLONE" && bash scripts/release.sh "$NEXT_VERSION" --dry-run --skip-tests --skip-agent 2>&1)
}

# --- baseline: a clean clone must get past preflight ------------------------
OUT=$(release_dry_run || true)
# Must reach the version-bump step, not merely avoid one particular error.
if [[ "$OUT" == *"[DRY-RUN] update version"* ]]; then
  _pass "a clean tree passes preflight"
else
  _fail "a clean tree passes preflight" "got: ${OUT:0:250}"
fi

# --- untracked files must not block a release ------------------------------
# The release commit adds six named paths and never `git add -A`, so nothing
# untracked can reach it. Rejecting them forced callers to stash their own
# work, and a killed run then stranded that work in a stash.
echo "scratch" >"$CLONE/my-unrelated-notes.md"
mkdir -p "$CLONE/my-unrelated-dir" && echo "x" >"$CLONE/my-unrelated-dir/f.txt"
OUT=$(release_dry_run || true)
if [[ "$OUT" == *"[DRY-RUN] update version"* ]]; then
  _pass "untracked files do not block preflight"
else
  _fail "untracked files do not block preflight" "got: ${OUT:0:200}"
fi
if [[ "$OUT" == *"untracked path"* ]]; then
  _pass "untracked files are reported rather than silently ignored"
else
  _fail "untracked files are reported rather than silently ignored" "no note in output"
fi

# --- the exact wreckage of an interrupted run ------------------------------
# Version surfaces edited, empty branch created, nothing committed or pushed.
(cd "$CLONE" \
  && git switch --quiet -c "$BRANCH" \
  && sed -i.bak "s/^version = \"/version = \"9.9.9\" # /" Cargo.toml \
  && rm -f Cargo.toml.bak)

OUT=$(release_dry_run || true)
if [[ "$OUT" == *"[DRY-RUN] discard leftover version edits"* ]]; then
  _pass "an interrupted attempt is recognized instead of blocking the retry"
else
  _fail "an interrupted attempt is recognized instead of blocking the retry" "got: ${OUT:0:300}"
fi

# Now let it actually recover (not a dry run, but still --skip-tests so no
# build happens) and confirm the leftovers are gone.
(cd "$CLONE" && bash scripts/release.sh "$NEXT_VERSION" --skip-tests --skip-agent >"$SANDBOX/recover.log" 2>&1 || true)
LEFTOVER_DIRTY=$(cd "$CLONE" && git status --porcelain | grep -v '^??' | grep -c 'Cargo.toml' || true)
if [[ "$LEFTOVER_DIRTY" == "0" ]] || (cd "$CLONE" && git log --oneline -1 | grep -q "Prepare"); then
  _pass "recovery leaves no stale version edit behind"
else
  _fail "recovery leaves no stale version edit behind" "$(cd "$CLONE" && git status --short | head -3)"
fi

# --- state that is NOT ours must still be refused --------------------------
# A branch carrying real commits, or an unrelated dirty file, is not wreckage.
CLONE2="$SANDBOX/repo2"
make_clone "$CLONE2"
(cd "$CLONE2" && echo "// deliberate local edit" >>src/main.rs)
OUT2=$(cd "$CLONE2" && bash scripts/release.sh "$NEXT_VERSION" --dry-run --skip-tests --skip-agent 2>&1 || true)
if [[ "$OUT2" == *"requires no uncommitted changes"* ]]; then
  _pass "an unrelated tracked edit is still refused"
else
  _fail "an unrelated tracked edit is still refused" "got: ${OUT2:0:200}"
fi
if [[ "$OUT2" == *"src/main.rs"* ]]; then
  _pass "the refusal names the file that is in the way"
else
  _fail "the refusal names the file that is in the way" "got: ${OUT2:0:200}"
fi

CLONE3="$SANDBOX/repo3"
make_clone "$CLONE3"
(cd "$CLONE3" && git switch --quiet -c "$BRANCH" \
  && echo "x" >real-work.txt && git add real-work.txt \
  && git -c user.email=t@t -c user.name=t commit --quiet -m "real work" \
  && git switch --quiet main)
OUT3=$(cd "$CLONE3" && bash scripts/release.sh "$NEXT_VERSION" --dry-run --skip-tests --skip-agent 2>&1 || true)
if [[ "$OUT3" == *"already exists"* ]]; then
  _pass "a release branch carrying commits is never deleted"
else
  _fail "a release branch carrying commits is never deleted" "got: ${OUT3:0:200}"
fi

summary
