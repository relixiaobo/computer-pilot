#!/bin/bash
# Test: cu type
# Opens TextEdit, types text, verifies
source "$(dirname "$0")/helpers.sh"

# Make sure TextEdit has a document. We do NOT activate — `cu type --app
# TextEdit` is PID-targeted, so TextEdit doesn't need to be frontmost as long
# as the document's textarea is its focused element.
osascript -e 'tell application "TextEdit" to make new document' 2>/dev/null
sleep 0.5

section "type — basic text"

cu_json "type hello --app TextEdit --no-snapshot"
assert_ok "type 'hello'"
assert_json_field "text echoed" ".text" "hello"

section "type — with auto-snapshot"

cu_json "type world --app TextEdit"
assert_ok "type 'world' with snapshot"
HAS_SNAP=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('yes' if 'snapshot' in d else 'no')
" 2>/dev/null || echo "error")
if [[ "$HAS_SNAP" == "yes" ]]; then
  _pass "auto-snapshot attached"
else
  _fail "auto-snapshot attached" "snapshot missing"
fi

section "type — special characters"

cu_json "type 'a@b#c\$d' --app TextEdit --no-snapshot"
assert_ok "type special chars @#\$"

section "type — spaces and punctuation"

cu_json type "hello, world!" --app TextEdit --no-snapshot
assert_ok "type with spaces and punctuation"

section "type — --no-snapshot flag"

cu_json "type test --app TextEdit --no-snapshot"
assert_ok "type with --no-snapshot"
NO_SNAP=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('absent' if 'snapshot' not in d else 'present')
" 2>/dev/null || echo "error")
if [[ "$NO_SNAP" == "absent" ]]; then
  _pass "--no-snapshot omits snapshot"
else
  _fail "--no-snapshot" "snapshot was present"
fi

section "type — human mode"

cu_human "type test123 --app TextEdit"
assert_exit_zero "type human exits 0"
assert_contains "shows typed text" "Typed"

# Cleanup: close TextEdit without saving
osascript -e 'tell application "TextEdit" to close every document saving no' 2>/dev/null
osascript -e 'tell application "TextEdit" to quit' 2>/dev/null

summary
