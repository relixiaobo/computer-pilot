#!/bin/bash
# Test: cu click (ref mode, coordinate mode, modifiers)
# Uses Finder as target — always available on macOS
source "$(dirname "$0")/helpers.sh"

section "click — coordinate mode"

cu_json "click 100 100"
assert_ok "click coords (100, 100)"
assert_json_field "x in response" ".x" "100.0"
assert_json_field "y in response" ".y" "100.0"

cu_json "click 500 300 --no-snapshot"
assert_ok "click coords with --no-snapshot"
# Should NOT have a snapshot field
NO_SNAP=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('absent' if 'snapshot' not in d else 'present')
" 2>/dev/null || echo "error")
if [[ "$NO_SNAP" == "absent" ]]; then
  _pass "--no-snapshot omits snapshot"
else
  _fail "--no-snapshot omits snapshot" "snapshot field was present"
fi

section "click — coordinate mode with modifiers"

cu_json "click 100 100 --right --no-snapshot"
assert_ok "right-click coords"
assert_json_field "right flag in response" ".right" "true"

cu_json "click 100 100 --no-snapshot"
assert_ok "double-click coords"

section "click — ref mode (Finder)"

# First snapshot Finder to get a valid ref
cu_json "snapshot Finder --limit 20"
FIRST_REF=$(json_get '.elements[0].ref' 2>/dev/null || echo "")

if [[ -n "$FIRST_REF" && "$FIRST_REF" != "__MISSING__" ]]; then
  cu_json "click $FIRST_REF --app Finder --no-snapshot"
  assert_ok "click ref [$FIRST_REF] in Finder"
  assert_json_field "ref in response" ".ref" "$FIRST_REF"
  assert_json_field "app in response" ".app" "Finder"
  assert_json_field_exists "method in response" ".method"

  METHOD=$(json_get '.method' || echo "")
  if [[ "$METHOD" == "ax-action" || "$METHOD" == "cgevent-pid" ]]; then
    _pass "method is ax-action or cgevent-pid ($METHOD)"
  else
    _fail "method type" "got: $METHOD"
  fi
else
  _skip "click ref in Finder" "no elements found in Finder snapshot"
fi

section "click — ref with auto-snapshot"

if [[ -n "$FIRST_REF" && "$FIRST_REF" != "__MISSING__" ]]; then
  # Without --no-snapshot, JSON output should include snapshot
  cu_json "click $FIRST_REF --app Finder"
  assert_ok "click ref with auto-snapshot"
  HAS_SNAP=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('yes' if 'snapshot' in d else 'no')
" 2>/dev/null || echo "error")
  if [[ "$HAS_SNAP" == "yes" ]]; then
    _pass "auto-snapshot present in response"
  else
    _fail "auto-snapshot present" "snapshot field missing"
  fi
else
  _skip "click ref with auto-snapshot" "no elements"
fi

section "click — error cases"

cu_json "click 9999 --app Finder --no-snapshot"
assert_fail "invalid ref 9999 fails"

cu_json "click 0 --app Finder --no-snapshot"
assert_fail "ref 0 fails (must be >= 1)"

cu_json "click abc"
assert_fail "non-numeric target fails"

cu_json "click 5 --app NonExistentApp98765 --no-snapshot"
assert_fail "click with non-existent app fails"

section "click — text mode (OCR)"

# Ensure Finder has a clean window with the sidebar (Applications row visible).
# OCR via CGWindowListCreateImage captures behind other windows, so Finder
# does not have to be frontmost — but `make new Finder window` may briefly
# pull it forward when it has to spawn a window.
osascript -e 'tell application "Finder"
  close every window
  make new Finder window
  set target of front Finder window to home
end tell' 2>/dev/null
sleep 1

cu_json click --text "Applications" --app Finder --no-snapshot
assert_ok "click --text finds visible text"
assert_json_field "method is ocr-text-pid" ".method" "ocr-text-pid"
assert_json_field_exists "matched text" ".text"

cu_json click --text "ZZZZNONEXISTENT99" --app Finder --no-snapshot
assert_fail "click --text with non-existent text fails"

section "click — human mode"

cu_human "click 100 100"
assert_exit_zero "click human exits 0"
assert_contains "shows click info" "Clicked"

summary
