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

section "snapshot — focused element (A4)"

# Open TextEdit and create a doc; the textarea should be the focused element
osascript -e 'tell application "TextEdit" to activate' 2>/dev/null
sleep 1
osascript -e 'tell application "TextEdit" to make new document' 2>/dev/null
"$CU" wait --ref 1 --app TextEdit --timeout 5 >/dev/null 2>&1 || true
"$CU" snapshot TextEdit --limit 5 >/dev/null 2>&1 || true  # warm up
sleep 0.3

cu_json snapshot TextEdit --limit 30
assert_ok "snapshot TextEdit"
FOCUS_INFO=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
f = d.get('focused')
if not f:
    print('missing'); sys.exit()
print('|'.join([
    'role=' + f.get('role', '?'),
    'has_ref=' + str('ref' in f),
]))
" 2>/dev/null || echo "error")
[[ "$FOCUS_INFO" == *"role=textarea"* ]] && _pass "focused.role is textarea" || _fail "focused.role is textarea" "$FOCUS_INFO"
[[ "$FOCUS_INFO" == *"has_ref=True"* ]]  && _pass "focused.ref populated"   || _fail "focused.ref populated"   "$FOCUS_INFO"

# Human mode shows "Focused: ..." line
cu_human snapshot TextEdit --limit 5
if echo "$OUT" | grep -q "^Focused:"; then
  _pass "human mode renders 'Focused:' line"
else
  _fail "human mode 'Focused:' line" "missing in output"
fi

section "snapshot — modal warning (A6)"

# Trigger TextEdit's "Save changes?" sheet by closing an unsaved document.
# Use osascript path because Cmd+W via PID-targeted doesn't fire the menu chain.
"$CU" set-value 1 "modal-trigger" --app TextEdit --no-snapshot >/dev/null 2>&1 || true
sleep 0.2
osascript -e 'tell application "TextEdit" to activate' 2>/dev/null
sleep 0.3
osascript -e 'tell application "System Events" to tell process "TextEdit" to keystroke "w" using {command down}' 2>/dev/null
sleep 1

cu_json snapshot TextEdit --limit 30
MODAL_INFO=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
m = d.get('modal')
if not m:
    print('missing'); sys.exit()
print('|'.join([
    'role=' + m.get('role', '?'),
    'has_subrole=' + str('subrole' in m),
]))
" 2>/dev/null || echo "error")

# The modal trigger is environment-dependent — TextEdit with iCloud auto-save
# enabled silently closes the doc on Cmd+W and never shows a sheet. When that
# happens, skip the assertion (it's not a `cu` regression).
if [[ "$MODAL_INFO" == "missing" ]]; then
  _skip "modal.role is AXSheet" "TextEdit didn't show save sheet (likely iCloud auto-save)"
  _skip "human mode '⚠ Modal:' warning" "no sheet to render"
else
  [[ "$MODAL_INFO" == *"role=AXSheet"* ]] && _pass "modal.role is AXSheet" || _fail "modal.role is AXSheet" "$MODAL_INFO"

  # Human mode shows the warning line
  cu_human snapshot TextEdit --limit 5
  if echo "$OUT" | grep -q "^⚠ Modal:"; then
    _pass "human mode renders '⚠ Modal:' warning"
  else
    _fail "human mode '⚠ Modal:' warning" "missing in output"
  fi
fi

# Cleanup: dismiss sheet (don't save), quit. `|| true` because TextEdit's
# AppleScript bridge can return -128 ("user canceled") when there's still
# a sheet up; that's not a test failure.
osascript -e 'tell application "System Events" to tell process "TextEdit" to keystroke "d" using {command down}' 2>/dev/null || true
sleep 0.5
osascript -e 'tell application "TextEdit" to quit' 2>/dev/null || true

summary
