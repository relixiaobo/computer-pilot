#!/bin/bash
# Behavior test: stale_state_advice fires when a ref-based click sees a
# different element at that ref than the previous snapshot saw.
#
# Why: ref [N] is a DFS index into the AX tree. When the tree shifts between
# the agent's snapshot and the agent's next click (user activity, async UI
# update), [N] now points to a different element. cu's stale-state guard
# compares the cached previous snapshot with the fresh pre-action AX walk
# and surfaces an advisory the agent can read and react to.
#
# Per CLAUDE.md Rule 1: this test constructs the actual drift scenario by
# mutating the cache between snapshot and click, not just asserting that
# the field name appears in the schema. It also exercises the no-drift
# case to confirm the advisory is silent when state is consistent.
source "$(dirname "$0")/helpers.sh"

CACHE_DIR="/tmp/cu-snapshot-cache"

section "stale-state guard — drift detected fires advisory"

# Open Finder home so we have a stable, snapshot-able UI to work with.
osascript -e 'tell application "Finder" to open home' >/dev/null 2>&1
sleep 1

cu_json snapshot Finder --limit 30
if ! is_json; then
  _skip "drift detected" "snapshot Finder did not return JSON: ${OUT:0:120}"
  summary
  exit 0
fi

# Resolve Finder's pid the same way cu does so we mutate the right cache file.
PID=$(pgrep -x Finder | head -1)
if [[ -z "$PID" ]]; then
  _skip "drift detected" "no Finder pid found"
  summary
  exit 0
fi

CACHE_FILE="$CACHE_DIR/${PID}.json"
if [[ ! -f "$CACHE_FILE" ]]; then
  _fail "drift detected" "expected cache file at $CACHE_FILE after snapshot, but it does not exist"
  summary
  exit 1
fi

# Pick a ref that lives in the snapshot, then rewrite the cached entry's role
# so the fresh AX walk at click time will see a different identity at that
# ref. (id_of() in diff.rs uses (role, round(x), round(y)) — flipping role
# alone is enough to trigger drift detection.)
TARGET_REF=$(python3 -c "
import json, sys
with open('$CACHE_FILE') as f:
    d = json.load(f)
els = d.get('elements', [])
if not els:
    print('NONE')
    sys.exit()
# Prefer a ref past the first 5 — sidebar items tend to be stable; main-pane
# refs are more representative of the staleness scenario.
target = els[min(len(els)-1, 10)] if len(els) > 5 else els[-1]
print(target['ref'])
")

if [[ "$TARGET_REF" == "NONE" || -z "$TARGET_REF" ]]; then
  _skip "drift detected" "Finder snapshot returned no elements"
  summary
  exit 0
fi

# Mutate the cache: tag the chosen ref with a sentinel role that cannot match
# any real AX role. This guarantees id_of() differs at click time.
python3 -c "
import json
with open('$CACHE_FILE') as f:
    d = json.load(f)
for e in d['elements']:
    if e['ref'] == $TARGET_REF:
        e['role'] = 'STALE_SENTINEL_ROLE'
        e['title'] = 'stale-test-marker'
        break
with open('$CACHE_FILE', 'w') as f:
    json.dump(d, f)
"

cu_json click "$TARGET_REF" --app Finder
if ! is_json; then
  _fail "drift detected" "click did not return JSON: ${OUT:0:200}"
  summary
  exit 1
fi

assert_json_field_exists "stale_state_advice present" ".stale_state_advice"

ADVICE=$(json_get '.stale_state_advice' 2>/dev/null || echo "")
if [[ "$ADVICE" == *"STALE_SENTINEL_ROLE"* ]]; then
  _pass "advice quotes the previous (mutated) role"
else
  _fail "advice quotes the previous (mutated) role" "expected 'STALE_SENTINEL_ROLE' in advice, got: ${ADVICE:0:200}"
fi

if [[ "$ADVICE" == *"re-snapshot"* ]]; then
  _pass "advice tells agent to re-snapshot"
else
  _fail "advice tells agent to re-snapshot" "expected 're-snapshot' in advice, got: ${ADVICE:0:200}"
fi

section "stale-state guard — no drift stays silent"

# Take a fresh snapshot so the cache matches reality, then click. The
# advice must be absent — false positives would train agents to ignore it.
cu_json snapshot Finder --limit 30
sleep 0.2

# Use a small ref that's almost certainly stable (top of DFS = sidebar/header).
cu_json click 1 --app Finder --no-verify
if ! is_json; then
  _skip "no-drift silence" "click did not return JSON (Finder may have closed): ${OUT:0:120}"
  summary
  exit 0
fi

# With --no-verify, pre_state is None, so stale_state_advice is intentionally
# not computed — but it also must not appear. This guards against future
# refactors that wire the advice in independent of pre_state.
ADVICE_NO_DRIFT=$(json_get '.stale_state_advice' 2>/dev/null || echo "")
if [[ -z "$ADVICE_NO_DRIFT" || "$ADVICE_NO_DRIFT" == "__MISSING__" ]]; then
  _pass "no advice when --no-verify (drift check piggybacks on pre_state)"
else
  _fail "no advice when --no-verify" "expected no stale_state_advice, got: ${ADVICE_NO_DRIFT:0:200}"
fi

# Now repeat with verify on, immediately after a fresh snapshot — drift
# should be absent because the cache matches the fresh AX walk.
cu_json snapshot Finder --limit 30
sleep 0.2
cu_json click 1 --app Finder
if ! is_json; then
  _skip "no-drift silence with verify" "click did not return JSON: ${OUT:0:120}"
  summary
  exit 0
fi

ADVICE_FRESH=$(json_get '.stale_state_advice' 2>/dev/null || echo "")
if [[ -z "$ADVICE_FRESH" || "$ADVICE_FRESH" == "__MISSING__" ]]; then
  _pass "no advice when cache matches fresh AX walk"
else
  _fail "no advice when fresh" "expected silent advice for matching cache, got: ${ADVICE_FRESH:0:200}"
fi

summary
