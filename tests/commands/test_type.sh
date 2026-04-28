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

section "type — --paste path (clipboard ⌘V, the proven CEF/chat-app path)"

# ⌘V routes to whichever app currently owns key focus. In a backgrounded
# TextEdit the text view often hasn't claimed key focus, so cmd+v is a no-op.
# `osascript activate` is heavier-handed than `cu window focus`, but here we
# need the textview to actually receive keystrokes.
osascript -e 'tell application "TextEdit" to activate' 2>/dev/null
sleep 0.5

# Snapshot the current clipboard so we can verify cu's restore step.
SAVED_CLIP=$(pbpaste 2>/dev/null || echo "")
echo -n "cu-test-saved-clipboard-$$" | pbcopy
sleep 0.1
ORIGINAL_CLIP=$(pbpaste)

# Clear TextEdit document.
"$CU" key cmd+a --app TextEdit --no-snapshot >/dev/null 2>&1 || true
"$CU" key delete --app TextEdit --no-snapshot >/dev/null 2>&1 || true
sleep 0.2

# Paste a string that contains CJK + emoji + ASCII. Method must report paste-pid.
cu_json type "你好世界 hi 🎉" --app TextEdit --paste --no-snapshot
assert_ok "type --paste returns ok"

METHOD=$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin).get('method',''))" 2>/dev/null || echo "")
if [[ "$METHOD" == "paste-pid" ]]; then
  _pass "method=paste-pid"
else
  _fail "method=paste-pid" "got: $METHOD"
fi

# Verify the doc actually contains all the characters (including CJK first char).
sleep 0.4
DOC=$(osascript -e 'tell application "TextEdit" to get text of front document' 2>/dev/null || echo "")
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
