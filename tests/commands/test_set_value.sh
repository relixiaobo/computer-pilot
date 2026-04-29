#!/bin/bash
# Test: cu set-value
# Opens TextEdit, writes text via AXValue, verifies via AppleScript
source "$(dirname "$0")/helpers.sh"

# Helper: read TextEdit document, compare to expected via Python (NFC-normalized)
verify_doc() {
  local label="$1" expected="$2"
  local got
  got=$(osascript -e 'tell application "TextEdit" to get text of front document' 2>/dev/null)
  local match
  match=$(python3 -c "
import sys, unicodedata
got = unicodedata.normalize('NFC', sys.argv[1])
exp = unicodedata.normalize('NFC', sys.argv[2])
print('yes' if got == exp else f'no|{got!r}|{exp!r}')
" "$got" "$expected")
  if [[ "$match" == "yes" ]]; then
    _pass "$label"
  else
    _fail "$label" "$match"
  fi
}

# Open TextEdit and create a doc. Warm up the AX bridge before any write —
# on a fresh TextEdit instance the first AXValue write can be silently
# accepted but not propagate to the document until AX has been touched.
osascript -e 'tell application "TextEdit" to activate' 2>/dev/null
sleep 1
osascript -e 'tell application "TextEdit" to make new document' 2>/dev/null
"$CU" wait --ref 1 --app TextEdit --timeout 5 >/dev/null 2>&1 || true
"$CU" snapshot TextEdit --limit 5 >/dev/null 2>&1 || true
sleep 0.3

section "set-value — basic ASCII write"

cu_json set-value 1 "hello via AXValue" --app TextEdit --no-snapshot
assert_ok "set-value ref 1"
assert_json_field "method is ax-set-value" ".method" "ax-set-value"
assert_json_field "value echoed" ".value" "hello via AXValue"
assert_json_field "ax_value_written true" ".ax_value_written" "true"
assert_json_field_exists "effect_advice present" ".effect_advice"
verify_doc "TextEdit document contains the value" "hello via AXValue"

section "set-value — Unicode (Chinese)"

cu_json set-value 1 "你好世界" --app TextEdit --no-snapshot
assert_ok "set-value Chinese text"
verify_doc "TextEdit document contains Chinese text" "你好世界"

section "set-value — replaces previous value"

cu_json set-value 1 "overwrite" --app TextEdit --no-snapshot
assert_ok "set-value overwrite"
verify_doc "AXValue write replaces, not appends" "overwrite"

section "set-value — auto-snapshot"

cu_json set-value 1 "with snap" --app TextEdit
assert_ok "set-value with snapshot"
HAS_SNAP=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('yes' if 'snapshot' in d else 'no')
" 2>/dev/null || echo "error")
if [[ "$HAS_SNAP" == "yes" ]]; then
  _pass "auto-snapshot attached"
else
  _fail "auto-snapshot attached" "snapshot missing"
fi

section "set-value — error: ref 0"

cu_json set-value 0 "x" --app TextEdit --no-snapshot
assert_fail "ref 0 rejected"

section "set-value — error: non-existent ref"

cu_json set-value 9999 "x" --app TextEdit --no-snapshot
assert_fail "ref 9999 not found"

section "set-value — failure carries structured hint + suggested_next"

# Finder ref 1 is typically a non-settable container; failure JSON goes to stderr.
cu_json set-value 1 "x" --app Finder --no-snapshot
JSON_OUT="${OUT:-$ERR}"
PARSED=$(echo "$JSON_OUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
except Exception:
    print('malformed'); sys.exit(0)
print('|'.join([
    'ok=' + str(d.get('ok')),
    'has_hint=' + str('hint' in d),
    'has_next=' + str(isinstance(d.get('suggested_next'), list) and len(d['suggested_next']) > 0),
]))
" 2>/dev/null || echo "malformed")
[[ "$PARSED" == *"ok=False"* ]]      && _pass "ok=false on non-settable" || _fail "ok=false on non-settable" "$PARSED"
[[ "$PARSED" == *"has_hint=True"* ]] && _pass "hint field populated"     || _fail "hint field populated"     "$PARSED"
[[ "$PARSED" == *"has_next=True"* ]] && _pass "suggested_next populated" || _fail "suggested_next populated" "$PARSED"

section "set-value — human mode"

cu_human set-value 1 "testing" --app TextEdit
assert_exit_zero "set-value human exits 0"
assert_contains "shows write confirmation" "Set"

# Cleanup — `|| true` because quit may prompt to save (-128) and set -e would
# otherwise kill the script before summary().
osascript -e 'tell application "TextEdit" to close every document saving no' >/dev/null 2>&1 || true
osascript -e 'tell application "TextEdit" to quit' >/dev/null 2>&1 || true

summary
