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

section "type — non-BMP emoji (UTF-16 surrogate pairs)"

# Clear any prior content via select-all + delete (PID-targeted, no focus theft).
"$CU" key cmd+a --app TextEdit --no-snapshot >/dev/null 2>&1 || true
"$CU" key delete --app TextEdit --no-snapshot >/dev/null 2>&1 || true
sleep 0.2

# 😀 (U+1F600) and 🎉 (U+1F389) are non-BMP — each encodes to a UTF-16
# surrogate pair, which the previous "one code unit per event" loop would
# have split. Both must round-trip whole.
cu_json type "ab 😀 🎉 cd" --app TextEdit --no-snapshot
assert_ok "type emoji + ASCII"

sleep 0.5
"$CU" snapshot TextEdit --limit 30 > /tmp/cu-emoji-snap.json 2>/dev/null
EMOJI_FOUND=$(python3 -c "
import json
d = json.load(open('/tmp/cu-emoji-snap.json'))
for e in d.get('elements', []):
    v = (e.get('value') or '').strip()
    if '😀' in v and '🎉' in v:
        print('yes'); break
else:
    print('no')
" 2>/dev/null || echo "error")

if [[ "$EMOJI_FOUND" == "yes" ]]; then
  _pass "non-BMP emoji round-tripped via TextEdit"
else
  _fail "emoji round-trip" "emoji not in TextEdit document — surrogate pairs may have split"
fi

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
