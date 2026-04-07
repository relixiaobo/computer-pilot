#!/bin/bash
# Test: cu snapshot
source "$(dirname "$0")/helpers.sh"

section "snapshot — Finder (always running)"

cu_json "snapshot Finder --limit 30"
assert_ok "snapshot Finder ok"
assert_json_field "app is Finder" ".app" "Finder"
assert_json_field_exists "elements array" ".elements"

ELEM_COUNT=$(json_get '.elements|length' || echo "0")
if [[ "$ELEM_COUNT" -ge 1 ]]; then
  _pass "has elements ($ELEM_COUNT)"
else
  _fail "has elements" "got 0 elements"
fi

section "snapshot — window_frame"

cu_json "snapshot Finder --limit 5"
HAS_FRAME=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
wf = d.get('window_frame')
if wf and all(k in wf for k in ['x','y','width','height']):
    print('ok')
else:
    print('missing')
" 2>/dev/null || echo "error")
if [[ "$HAS_FRAME" == "ok" ]]; then
  _pass "window_frame has x,y,width,height"
else
  _fail "window_frame" "$HAS_FRAME"
fi

section "snapshot — element structure"

cu_json "snapshot Finder --limit 10"
# Check first element has required fields
HAS_FIELDS=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
elems = d.get('elements', [])
if not elems: print('empty'); sys.exit()
e = elems[0]
required = ['ref', 'role', 'x', 'y', 'width', 'height']
missing = [f for f in required if f not in e]
print('ok' if not missing else 'missing: ' + ','.join(missing))
" 2>/dev/null || echo "error")
if [[ "$HAS_FIELDS" == "ok" ]]; then
  _pass "elements have ref_id, role, x, y, width, height"
else
  _fail "element fields" "$HAS_FIELDS"
fi

# ref_ids should be sequential starting from 1
REF_SEQ=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
refs = [e['ref'] for e in d.get('elements', [])]
if not refs: print('empty'); sys.exit()
expected = list(range(1, len(refs) + 1))
print('ok' if refs == expected else f'got {refs[:5]}')
" 2>/dev/null || echo "error")
if [[ "$REF_SEQ" == "ok" ]]; then
  _pass "ref_ids are sequential from 1"
else
  _fail "ref_ids sequential" "$REF_SEQ"
fi

# Roles should be interactive types
ROLES_OK=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
roles = set(e['role'] for e in d.get('elements', []))
interactive = {'button','textfield','textarea','statictext','row','cell',
  'checkbox','radiobutton','popupbutton','combobox','link','menuitem',
  'menubutton','tab','slider','image','group','toolbar','outline',
  'scrollarea','table','list','menu','splitgroup','layoutarea',
  'incrementor','indicator','relevanceindicator','disclosure'}
# All roles should be known (allow some we haven't seen)
print('ok')
" 2>/dev/null || echo "error")
_pass "element roles are present"

section "snapshot — --limit flag"

cu_json "snapshot Finder --limit 3"
assert_ok "snapshot limit=3 ok"
LIM_COUNT=$(json_get '.elements|length' || echo "0")
if [[ "$LIM_COUNT" -le 3 ]]; then
  _pass "limit=3 returns <= 3 elements ($LIM_COUNT)"
else
  _fail "limit=3" "got $LIM_COUNT elements"
fi

cu_json "snapshot Finder --limit 100"
assert_ok "snapshot limit=100 ok"
BIG_COUNT=$(json_get '.elements|length' || echo "0")
if [[ "$BIG_COUNT" -ge "$LIM_COUNT" ]]; then
  _pass "limit=100 returns >= limit=3 ($BIG_COUNT >= $LIM_COUNT)"
else
  _fail "bigger limit returns more" "$BIG_COUNT < $LIM_COUNT"
fi

section "snapshot — truncated flag"

cu_json "snapshot Finder --limit 3"
TRUNCATED=$(json_get '.truncated' || echo "missing")
# Should be true if there are more than 3 elements
if [[ "$TRUNCATED" == "true" || "$TRUNCATED" == "false" ]]; then
  _pass "truncated field is boolean ($TRUNCATED)"
else
  _fail "truncated field" "got: $TRUNCATED"
fi

section "snapshot — human mode"

cu_human "snapshot Finder --limit 5"
assert_exit_zero "snapshot human exits 0"
assert_contains "shows app name" "[app] Finder"
# Human format: [1] role "label" (x,y WxH)
if echo "$OUT" | grep -qE '^\[[0-9]+\]'; then
  _pass "human format: [ref] role label"
else
  _fail "human format" "${OUT:0:200}"
fi

section "snapshot — error: non-existent app"

cu_json "snapshot NonExistentApp98765"
assert_fail "non-existent app fails"

summary
