#!/bin/bash
# Test: cu type
# Opens TextEdit, types text, verifies
source "$(dirname "$0")/helpers.sh"

# Remove documents leaked by interrupted/manual test runs, then create exactly
# one target document. We do NOT activate — `cu type --app
# TextEdit` is PID-targeted, so TextEdit doesn't need to be frontmost as long
# as the document's textarea is its focused element.
osascript -e 'tell application "TextEdit" to close every document saving no' 2>/dev/null || true
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

# Clear and focus the textarea deterministically before testing unicode events.
cu_json set-value 1 "" --app TextEdit --no-snapshot
assert_ok "emoji fixture cleared through AX"
cu_json click 1 --app TextEdit --no-snapshot --no-verify
assert_ok "emoji fixture textarea focused"
sleep 0.2

# 😀 (U+1F600) and 🎉 (U+1F389) are non-BMP — each encodes to a UTF-16
# surrogate pair, which the previous "one code unit per event" loop would
# have split. Both must round-trip whole.
cu_json type "ab 😀 🎉 cd" --app TextEdit
assert_ok "type emoji + ASCII"

EMOJI_FOUND=$(echo "$OUT" | python3 -c "
import json, sys
d = json.load(sys.stdin).get('snapshot', {})
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

section "type — --paste path (clipboard ⌘V, the proven CEF/chat-app path)"

# Snapshot the current clipboard so we can verify cu's restore step.
SAVED_CLIP=$(pbpaste 2>/dev/null || echo "")
echo -n "cu-test-saved-clipboard-$$" | pbcopy
sleep 0.1
ORIGINAL_CLIP=$(pbpaste)

# Clear through AX instead of relying on a pre-existing keyboard selection.
cu_json set-value 1 "" --app TextEdit --no-snapshot
assert_ok "paste fixture cleared through AX"

# Run the global paste while TextEdit remains frontmost for the whole
# AppleScript transaction. TextEdit does not reliably dispatch background
# PID-targeted menu shortcuts; that routing is covered in test_paste_auto.sh.
EXIT=0
OUT=$(osascript \
  -e 'on run argv' \
  -e 'set toolPath to item 1 of argv' \
  -e 'set inputText to item 2 of argv' \
  -e 'tell application "TextEdit" to activate' \
  -e 'delay 0.5' \
  -e 'return do shell script (quoted form of toolPath & " type " & quoted form of inputText & " --paste --no-snapshot --allow-global")' \
  -e 'end run' \
  "$CU" "你好世界 hi 🎉" 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_ok "type --paste returns ok"

METHOD=$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin).get('method',''))" 2>/dev/null || echo "")
if [[ "$METHOD" == "paste-global" ]]; then
  _pass "method=paste-global"
else
  _fail "method=paste-global" "got: $METHOD"
fi

# Verify the AX-visible document contains all characters, including the first
# CJK character. This avoids an unrelated TextEdit AppleScript response race.
sleep 0.4
DOC=$("$CU" snapshot TextEdit --limit 5 2>/dev/null | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(next((e.get('value', '') for e in d.get('elements', []) if e.get('role') == 'textarea'), ''))
" 2>/dev/null || true)
if [[ "$DOC" == *"你好世界 hi 🎉"* ]]; then
  _pass "paste delivered full string (CJK + emoji + ASCII)"
else
  _fail "paste delivered full string" "got: $DOC"
fi

# Verify clipboard was restored (not left set to the typed text).
sleep 0.1
RESTORED_CLIP=$(pbpaste)
if [[ "$RESTORED_CLIP" == "$ORIGINAL_CLIP" ]]; then
  _pass "clipboard restored after paste"
else
  _fail "clipboard restored after paste" "expected '$ORIGINAL_CLIP', got '$RESTORED_CLIP'"
fi

# Restore the user's original clipboard (best-effort).
printf '%s' "$SAVED_CLIP" | pbcopy 2>/dev/null || true

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

section "type — frontmost-app safety check"

# Use the test seam to inject a known-dangerous frontmost (deterministic across
# environments). Without --app, cu should refuse to dump text into a terminal.
OUT=$(CU_TEST_FRONTMOST_OVERRIDE=Terminal "$CU" type "rm -rf /" --no-snapshot 2>&1) || true
if echo "$OUT" | grep -q "refusing to type"; then
  _pass "refuses type without --app when frontmost is dangerous"
else
  _fail "refuses type without --app when frontmost is dangerous" "expected refusal, got: $OUT"
fi

# --app bypasses the check entirely (target is explicit)
cu_json "type harmless --app TextEdit --no-snapshot"
assert_ok "--app bypasses safety check"

section "type — human mode"

cu_human "type test123 --app TextEdit"
assert_exit_zero "type human exits 0"
assert_contains "shows typed text" "Typed"

# Cleanup: close TextEdit without saving (`|| true` — see test_key.sh comment)
osascript -e 'tell application "TextEdit" to close every document saving no' >/dev/null 2>&1 || true
osascript -e 'tell application "TextEdit" to quit' >/dev/null 2>&1 || true

summary
