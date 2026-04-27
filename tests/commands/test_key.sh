#!/bin/bash
# Test: cu key
source "$(dirname "$0")/helpers.sh"

# Make sure TextEdit has a document. We do NOT activate — `cu key --app TextEdit`
# is PID-targeted, so TextEdit doesn't need to be frontmost.
osascript -e 'tell application "TextEdit" to make new document' 2>/dev/null
sleep 0.5

section "key — basic keys with --app"

cu_json "key enter --app TextEdit --no-snapshot"
assert_ok "key enter"
assert_json_field "combo echoed" ".combo" "enter"

cu_json "key tab --app TextEdit --no-snapshot"
assert_ok "key tab"

cu_json "key escape --app TextEdit --no-snapshot"
assert_ok "key escape"

cu_json "key space --app TextEdit --no-snapshot"
assert_ok "key space"

section "key — modifier combos with --app"

cu_json "key cmd+a --app TextEdit --no-snapshot"
assert_ok "key cmd+a (select all)"

cu_json "key cmd+c --app TextEdit --no-snapshot"
assert_ok "key cmd+c (copy)"

cu_json "key cmd+z --app TextEdit --no-snapshot"
assert_ok "key cmd+z (undo)"

cu_json "key cmd+shift+z --app TextEdit --no-snapshot"
assert_ok "key cmd+shift+z (redo)"

section "key — without --app (CGEvent to frontmost)"

cu_json "key escape --no-snapshot"
assert_ok "key escape without --app"

section "key — with auto-snapshot"

cu_json "key escape --app TextEdit"
assert_ok "key with auto-snapshot"
HAS_SNAP=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('yes' if 'snapshot' in d else 'no')
" 2>/dev/null || echo "error")
if [[ "$HAS_SNAP" == "yes" ]]; then
  _pass "auto-snapshot attached"
else
  _fail "auto-snapshot attached" "snapshot missing"
fi

section "key — arrow keys"

cu_json "key up --app TextEdit --no-snapshot"
assert_ok "key up"

cu_json "key down --app TextEdit --no-snapshot"
assert_ok "key down"

cu_json "key left --app TextEdit --no-snapshot"
assert_ok "key left"

cu_json "key right --app TextEdit --no-snapshot"
assert_ok "key right"

section "key — function keys"

cu_json "key f1 --no-snapshot"
assert_ok "key f1"

section "key — human mode"

cu_human "key escape --app TextEdit"
assert_exit_zero "key human exits 0"
assert_contains "shows key info" "Sent key"

# Cleanup
osascript -e 'tell application "TextEdit" to close every document saving no' 2>/dev/null
osascript -e 'tell application "TextEdit" to quit' 2>/dev/null

summary
