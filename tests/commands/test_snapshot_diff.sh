#!/bin/bash
# Test: cu snapshot --diff (C1)
source "$(dirname "$0")/helpers.sh"

# Resolve a stable target: open one TextEdit doc, lock its position so
# identity-by-position stays stable across calls.
osascript -e 'tell application "TextEdit" to close every document saving no' >/dev/null 2>&1 || true
osascript -e 'tell application "TextEdit" to quit saving no' >/dev/null 2>&1 || true
sleep 1
osascript -e 'tell application "TextEdit" to activate' >/dev/null 2>&1
sleep 1
osascript -e 'tell application "TextEdit" to make new document' >/dev/null 2>&1
sleep 1
"$CU" wait --ref 1 --app TextEdit --timeout 5 >/dev/null 2>&1 || true
"$CU" snapshot TextEdit --limit 5 >/dev/null 2>&1 || true   # warm AX bridge
sleep 0.3

# Move + resize the window to a known location so it doesn't drift between calls
"$CU" window move 100 100 --app TextEdit >/dev/null 2>&1 || true
"$CU" window resize 800 600 --app TextEdit >/dev/null 2>&1 || true
sleep 0.3

# Confirm TextEdit is available before running the behavior checks.
PID=$("$CU" apps 2>/dev/null | python3 -c "
import sys, json
d = json.load(sys.stdin)
for a in d.get('apps', []):
    if a.get('name') == 'TextEdit':
        print(a.get('pid', ''))
        break
" 2>/dev/null || true)

if [[ -z "$PID" ]]; then
  echo "Cannot resolve TextEdit pid — skipping diff suite"
  summary
  exit 0
fi

# Start the diff assertions in a fresh client namespace. Warm-up calls above
# deliberately used the default test client and cannot populate this cache.
export COMPUTER_PILOT_CLIENT_KEY="test.snapshot-diff.$$"

section "snapshot --diff — first call (no cache)"

cu_json snapshot TextEdit --limit 30 --diff
assert_ok "first --diff call ok"
FIRST=$(json_get '.first_snapshot' || echo "")
if [[ "$FIRST" == "true" ]]; then
  _pass "first_snapshot:true on cold cache"
else
  _fail "first_snapshot:true" "got: $FIRST"
fi
# First call returns full elements (snapshot shape), not diff
assert_json_field_exists "elements present on first call" ".elements"

section "snapshot --diff — no UI change → empty diff"

cu_json snapshot TextEdit --limit 30 --diff
assert_ok "second --diff call ok"
NO_FIRST=$(json_get '.first_snapshot' || echo "absent")
if [[ "$NO_FIRST" == "absent" || "$NO_FIRST" == "__MISSING__" ]]; then
  _pass "first_snapshot flag absent on warm cache"
else
  _fail "first_snapshot absent" "got: $NO_FIRST"
fi
assert_json_field_exists "diff field present" ".diff"

EMPTY=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin).get('diff', {})
print('|'.join([
    'added=' + str(len(d.get('added', []))),
    'changed=' + str(len(d.get('changed', []))),
    'removed=' + str(len(d.get('removed', []))),
    'unchanged=' + str(d.get('unchanged_count', -1)),
]))
" 2>/dev/null || echo "error")
[[ "$EMPTY" == *"added=0"* ]]    && _pass "added=0 with no UI change"   || _fail "added=0" "$EMPTY"
[[ "$EMPTY" == *"changed=0"* ]]  && _pass "changed=0 with no UI change" || _fail "changed=0" "$EMPTY"
[[ "$EMPTY" == *"removed=0"* ]]  && _pass "removed=0 with no UI change" || _fail "removed=0" "$EMPTY"
# unchanged should be > 0 (TextEdit window has elements)
UNCHANGED=$(echo "$EMPTY" | grep -oE 'unchanged=[0-9]+' | cut -d= -f2)
if [[ "$UNCHANGED" -ge 1 ]]; then
  _pass "unchanged_count > 0 ($UNCHANGED)"
else
  _fail "unchanged_count > 0" "got: $UNCHANGED"
fi

section "snapshot --diff — content change → ~ on textarea"

# Mutate the front document independently of the AX snapshot implementation,
# then verify the fixture state before asking snapshot --diff to observe it.
osascript -e 'tell application "TextEdit" to set text of front document to "diff sentinel value"' >/dev/null 2>&1
DOC_TEXT=$(osascript -e 'tell application "TextEdit" to get text of front document' 2>/dev/null || true)
if [[ "$DOC_TEXT" == "diff sentinel value" ]]; then
  _pass "TextEdit fixture content changed"
else
  _fail "TextEdit fixture content changed" "got: ${DOC_TEXT:0:100}"
fi
sleep 0.5

cu_json snapshot TextEdit --limit 30 --diff
assert_ok "post-write --diff ok"
DELTA=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin).get('diff', {})
changed = d.get('changed', [])
# Look for a textarea whose value contains the sentinel
hit = any(
    el.get('role') == 'textarea' and 'diff sentinel value' in (el.get('value') or '')
    for el in changed
)
print('|'.join([
    'changed=' + str(len(changed)),
    'textarea_changed=' + str(hit),
    'added=' + str(len(d.get('added', []))),
    'removed=' + str(len(d.get('removed', []))),
]))
" 2>/dev/null || echo "error")
[[ "$DELTA" == *"textarea_changed=True"* ]] && _pass "textarea content change captured" || _fail "textarea changed" "$DELTA"
# Should NOT show massive added/removed (window didn't move)
ADDED=$(echo "$DELTA" | grep -oE 'added=[0-9]+' | cut -d= -f2)
REMOVED=$(echo "$DELTA" | grep -oE 'removed=[0-9]+' | cut -d= -f2)
if [[ "$ADDED" -le 2 && "$REMOVED" -le 2 ]]; then
  _pass "diff is targeted (added=$ADDED removed=$REMOVED)"
else
  _fail "diff targeted" "added=$ADDED removed=$REMOVED — window may have moved"
fi

section "snapshot --diff — human mode renders +/~/-"

cu_human snapshot TextEdit --limit 30 --diff
assert_exit_zero "human --diff exits 0"
# Should contain "(diff)" header marker AND a Summary line
if echo "$OUT" | grep -q "(diff)"; then
  _pass "human header marks (diff)"
else
  _fail "human (diff) marker" "${OUT:0:200}"
fi
if echo "$OUT" | grep -qE "(no changes|^Summary:)"; then
  _pass "human emits Summary or no-changes line"
else
  _fail "human summary line" "${OUT:0:200}"
fi

section "snapshot --diff — diff and non-diff coexist"

# Plain snapshot (no --diff) should still work and not corrupt the cache
cu_json snapshot TextEdit --limit 30
assert_ok "plain snapshot still ok"
assert_json_field_exists ".elements present on plain snapshot" ".elements"

# Following --diff should still see the same baseline
cu_json snapshot TextEdit --limit 30 --diff
assert_ok "post-plain --diff ok"
assert_json_field_exists "diff field present after plain snapshot" ".diff"

# Cleanup
osascript -e 'tell application "TextEdit" to close every document saving no' 2>/dev/null || true
osascript -e 'tell application "TextEdit" to quit' 2>/dev/null || true

summary
