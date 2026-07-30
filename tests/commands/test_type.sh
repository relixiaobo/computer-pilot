#!/bin/bash
# Test: cu type
# Opens TextEdit, types text, verifies
source "$(dirname "$0")/helpers.sh"
trap 'textedit_cleanup; cleanup_run' EXIT

# Remove documents leaked by interrupted/manual test runs, then create exactly
# one target document. We do NOT activate — `cu type --app
# TextEdit` is PID-targeted, so TextEdit doesn't need to be frontmost as long
# as the document's textarea is its focused element.
textedit_reset
sleep 0.5

section "type — basic text"

cu_json "type hello --app TextEdit --no-snapshot"
assert_ok "type 'hello'"
assert_json_field "text echoed" ".text" "hello"
assert_json_field "type dispatched" ".dispatched" "true"
assert_json_field "type effect verified" ".effect_verified" "true"

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

# Clear the internally focused textarea. TextEdit stays in the background so
# real user keystrokes cannot contaminate this disposable document.
"$CU" snapshot TextEdit --limit 5 >/dev/null
cu_json set-value 1 "" --app TextEdit --no-snapshot
assert_ok "emoji fixture cleared through AX"
cu_json type "ab 😀 🎉 cd" --app TextEdit
assert_ok "type emoji + ASCII"
sleep 0.3
EMOJI_DOC=$(_run_with_timeout 10 osascript -e 'tell application "TextEdit" to get text of front document' 2>/dev/null || true)
if [[ "$EMOJI_DOC" == *"😀"* && "$EMOJI_DOC" == *"🎉"* ]]; then
  _pass "non-BMP emoji round-tripped via TextEdit"
else
  _fail "emoji round-trip" "emoji not in TextEdit document — got: $EMOJI_DOC"
fi

section "type — background PID paste fails loud"

# Snapshot the current clipboard so we can verify cu's restore step.
SAVED_CLIP=$(pbpaste 2>/dev/null || echo "")
echo -n "cu-test-saved-clipboard-$$" | pbcopy
sleep 0.1
ORIGINAL_CLIP=$(pbpaste)

# Clear through AX instead of relying on a pre-existing keyboard selection.
"$CU" snapshot TextEdit --limit 5 >/dev/null
cu_json set-value 1 "" --app TextEdit --no-snapshot
assert_ok "paste fixture cleared through AX"

cu_json state TextEdit --no-screenshot
TEXTEDIT_FRONTMOST=$(json_get '.frontmost' 2>/dev/null || echo "true")
if [[ "$TEXTEDIT_FRONTMOST" == "false" ]]; then
  cu_json type "SHOULD_NOT_LAND" --app TextEdit --paste --no-snapshot
  assert_fail "background TextEdit paste is not reported as success"
  BACKGROUND_CODE=$(echo "$ERR" | python3 -c "import sys,json;print(json.load(sys.stdin).get('code',''))" 2>/dev/null || echo "")
  [[ "$BACKGROUND_CODE" == "unknown_outcome" || "$BACKGROUND_CODE" == "verification_failed" ]] \
    && _pass "background paste has a recoverable result code" \
    || _fail "background paste result code" "got: $BACKGROUND_CODE"
  BACKGROUND_DOC=$(_run_with_timeout 10 osascript -e 'tell application "TextEdit" to get text of front document' 2>/dev/null || true)
  [[ -z "$BACKGROUND_DOC" ]] && _pass "background paste did not mutate TextEdit" || _fail "background paste isolation" "got: $BACKGROUND_DOC"
else
  _skip "background TextEdit paste is not reported as success" "TextEdit unexpectedly became frontmost"
  _skip "background paste has a recoverable result code" "background precondition unavailable"
  _skip "background paste did not mutate TextEdit" "background precondition unavailable"
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

section "type — focused input preflight"

# Dock has no text input. The command must fail before dispatch instead of
# sending Unicode events and reporting a false success.
cu_json type "must-not-dispatch" --app Dock --no-snapshot
assert_fail "type refuses a target with no focused text input"
TYPE_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
TYPE_DISPATCHED=$(echo "$ERR" | python3 -c 'import json,sys; print((json.load(sys.stdin).get("diagnostics") or {}).get("dispatched", ""))' 2>/dev/null || true)
[[ "$TYPE_CODE" == "verification_failed" ]] && _pass "missing focus has verification_failed code" || _fail "missing focus code" "got '$TYPE_CODE'"
[[ "$TYPE_DISPATCHED" == "False" ]] && _pass "missing focus fails before dispatch" || _fail "missing focus dispatch guard" "got '$TYPE_DISPATCHED'"

section "type — human mode"

cu_human "type test123 --app TextEdit"
assert_exit_zero "type human exits 0"
assert_contains "shows typed text" "Typed"

summary
