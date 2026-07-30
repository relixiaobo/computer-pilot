#!/bin/bash
# Test: cu type selects Unicode vs clipboard paste by target capability.
#
# Native controls support CJK Unicode events. Chromium/CEF inputs and known
# chat apps use PID paste instead; the Electron behavior fixture verifies the
# controlled-editor state change for that route.
source "$(dirname "$0")/helpers.sh"
trap 'textedit_cleanup; cleanup_run' EXIT

textedit_reset
sleep 0.4

section "paste auto-detect — native CJK stays on Unicode"

cu_json type "你好世界" --app TextEdit --no-snapshot
assert_ok "type CJK ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('method=' + str(d.get('method')) + '|reason=' + str(d.get('paste_reason','')))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"method=unicode-pid"* ]] && _pass "native CJK uses unicode-pid" || _fail "native CJK method" "$PARSED"
[[ "$PARSED" == *"reason=" ]] && _pass "native CJK has no paste reason" || _fail "native CJK paste reason" "$PARSED"
CJK_DOC=$(_run_with_timeout 10 osascript -e 'tell application "TextEdit" to get text of front document' 2>/dev/null || true)
[[ "$CJK_DOC" == *"你好世界"* ]] && _pass "native CJK changed TextEdit" || _fail "native CJK TextEdit state" "got '$CJK_DOC'"

section "paste auto-detect — ASCII content stays on unicode events"

cu_json type "hello world" --app TextEdit --no-snapshot
assert_ok "type ASCII ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('method=' + str(d.get('method')) + '|has_reason=' + str('paste_reason' in d))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"method=unicode-pid"* ]] && _pass "ASCII stays on unicode-pid"          || _fail "ASCII stays on unicode-pid"          "$PARSED"
[[ "$PARSED" == *"has_reason=False"* ]]   && _pass "paste_reason absent on non-paste path" || _fail "paste_reason absent on non-paste"  "$PARSED"

section "paste auto-detect — explicit --no-paste keeps native Unicode"

cu_json type "你好" --app TextEdit --no-snapshot --no-paste
assert_ok "type CJK --no-paste ok"

METHOD=$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin).get('method',''))" 2>/dev/null || echo "")
[[ "$METHOD" == "unicode-pid" ]] && _pass "--no-paste overrides CJK auto-detection" || _fail "--no-paste overrides" "method=$METHOD"

section "paste auto-detect — unobservable explicit paste fails loud"

if interactive_tests_enabled; then
  "$CU" snapshot TextEdit --limit 5 >/dev/null
  cu_json set-value 1 "" --app TextEdit --no-snapshot
  assert_ok "explicit paste fixture cleared"
  cu_json window focus --app Finder
  if [[ "$EXIT" -eq 0 ]]; then
    cu_json type "hello" --app TextEdit --no-snapshot --paste
    assert_fail "unobservable TextEdit paste is not success"

    PARSED=$(echo "$ERR" | python3 -c "
import sys, json
d = json.load(sys.stdin)
diagnostics = d.get('diagnostics') or {}
print('code=' + str(d.get('code')) + '|method=' + str(diagnostics.get('method')) + '|reason=' + str(diagnostics.get('paste_reason','')))
" 2>/dev/null || echo "malformed")

    [[ "$PARSED" == *"code=unknown_outcome"* ]] && _pass "unobservable paste returns unknown_outcome" || _fail "unobservable paste code" "$PARSED"
    [[ "$PARSED" == *"method=paste-pid"* ]] && _pass "unknown outcome identifies paste-pid" || _fail "unknown outcome method" "$PARSED"
    [[ "$PARSED" == *"reason="*"explicit"* ]] && _pass "unknown outcome retains paste_reason" || _fail "unknown paste reason" "$PARSED"
  else
    _skip "unobservable TextEdit paste is not success" "external foreground contention prevented Finder focus"
    _skip "unobservable paste returns unknown_outcome" "background precondition unavailable"
    _skip "unknown outcome identifies paste-pid" "background precondition unavailable"
    _skip "unknown outcome retains paste_reason" "background precondition unavailable"
  fi
else
  _skip "explicit paste fixture cleared" "interactive desktop tests disabled"
  _skip "unobservable TextEdit paste is not success" "interactive desktop tests disabled"
  _skip "unobservable paste returns unknown_outcome" "interactive desktop tests disabled"
  _skip "unknown outcome identifies paste-pid" "interactive desktop tests disabled"
  _skip "unknown outcome retains paste_reason" "interactive desktop tests disabled"
fi

section "paste — clipboard restored with full fidelity (image survives)"

# These tests overwrite the clipboard. pbpaste can only save a text clipboard,
# so if the runner's clipboard currently holds non-text content (image, files,
# rich text), skip rather than destroy it — the exact failure mode the feature
# under test exists to prevent.
CLIP_INFO=$(_run_with_timeout 10 osascript -e 'clipboard info' 2>/dev/null || echo "")
CLIP_NONTEXT=$(echo "$CLIP_INFO" | python3 -c "
import sys
info = sys.stdin.read().strip()
# 'clipboard info' emits 'type, size' pairs: 'Unicode text, 12, string, 12'
tokens = [t.strip().lower() for t in info.split(',')]
types = tokens[::2]
text_ok = {'', 'string', 'unicode text', 'utf8 text', 'utf16 text',
           '«class utf8»', '«class ut16»', 'ctxt'}
print(';'.join(t for t in types if t not in text_ok))
" 2>/dev/null || echo "")

if [[ -n "$CLIP_NONTEXT" ]]; then
  _skip "PNG clipboard survives the paste round-trip" "runner clipboard holds non-text content ($CLIP_NONTEXT); not willing to destroy it"
  _skip "concurrent clipboard write preserved" "runner clipboard holds non-text content"
  _skip "clipboard_hint emitted on mid-paste write" "runner clipboard holds non-text content"
  summary
  exit
fi

# pbcopy/pbpaste could never round-trip a non-text clipboard; the NSPasteboard
# save/restore must. Put a PNG on the clipboard, run a paste-routed type, and
# require the identical PNG back afterwards. The paste may fail verification
# (background TextEdit) — the save → set → ⌘V → restore path runs either way,
# as long as the text was dispatched.
SAVED_TEXT_CLIP=$(pbpaste 2>/dev/null || true) # text-only clipboard confirmed above
CLIP_PNG="/tmp/cu-test-clip-$$.png"
python3 -c "
import base64
open('$CLIP_PNG', 'wb').write(base64.b64decode(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=='))
" 2>/dev/null || true
_run_with_timeout 10 osascript -e "set the clipboard to (read (POSIX file \"$CLIP_PNG\") as «class PNGf»)" >/dev/null 2>&1
BEFORE_PNG=$(_run_with_timeout 10 osascript -e 'get the clipboard as «class PNGf»' 2>/dev/null || echo "setup-failed")

if [[ "$BEFORE_PNG" == "setup-failed" || -z "$BEFORE_PNG" ]]; then
  _skip "PNG clipboard survives the paste round-trip" "could not place a PNG on the clipboard"
else
  textedit_reset
  cu_json type "clip-fidelity-probe" --app TextEdit --paste --no-snapshot
  DISPATCHED=$(echo "$OUT$ERR" | python3 -c "
import sys, json
try:
    d = json.loads(sys.stdin.read())
except Exception:
    print('parse-error'); raise SystemExit
diag = d.get('diagnostics') or {}
print(d.get('dispatched') or diag.get('dispatched') or False)
" 2>/dev/null || echo "parse-error")
  AFTER_PNG=$(_run_with_timeout 10 osascript -e 'get the clipboard as «class PNGf»' 2>/dev/null || echo "png-gone")
  if [[ "$DISPATCHED" != "True" ]]; then
    _skip "PNG clipboard survives the paste round-trip" "paste was not dispatched (dispatched=$DISPATCHED)"
  elif [[ "$AFTER_PNG" == "$BEFORE_PNG" ]]; then
    _pass "PNG clipboard survives the paste round-trip"
  else
    _fail "PNG clipboard survives the paste round-trip" "clipboard after: ${AFTER_PNG:0:80}"
  fi
fi
rm -f "$CLIP_PNG"

section "paste — mid-paste clipboard write: preserved + clipboard_hint"

# The changeCount protection: when the user writes to the clipboard during the
# paste window, cu must NOT restore over their new content, and must say so
# via the clipboard_hint advisory. Hammer the clipboard with a marker for the
# whole cu invocation so at least one write lands inside the window.
printf 'original-fixture-text' | pbcopy 2>/dev/null || true
(
  for _ in $(seq 1 600); do
    printf 'concurrent-write-marker' | pbcopy 2>/dev/null || true
    sleep 0.02
  done
) &
HAMMER_PID=$!
cu_json type "hint-probe" --app TextEdit --paste --no-snapshot
kill "$HAMMER_PID" 2>/dev/null || true
wait "$HAMMER_PID" 2>/dev/null || true

PARSED=$(echo "$OUT$ERR" | python3 -c "
import sys, json
try:
    d = json.loads(sys.stdin.read())
except Exception:
    print('parse-error'); raise SystemExit
diag = d.get('diagnostics') or {}
hint = d.get('clipboard_hint') or diag.get('clipboard_hint') or ''
dispatched = d.get('dispatched') or diag.get('dispatched') or False
print('dispatched=' + str(dispatched) + '|hint=' + hint)
" 2>/dev/null || echo "parse-error")
FINAL_CLIP=$(pbpaste 2>/dev/null || echo "")

if [[ "$PARSED" != *"dispatched=True"* ]]; then
  _skip "concurrent clipboard write preserved" "paste was not dispatched ($PARSED)"
  _skip "clipboard_hint emitted on mid-paste write" "paste was not dispatched"
else
  if [[ "$FINAL_CLIP" == "concurrent-write-marker" ]]; then
    _pass "concurrent clipboard write preserved (restore was skipped)"
  else
    _fail "concurrent clipboard write preserved" "clipboard holds '${FINAL_CLIP:0:60}' instead of the marker"
  fi
  if [[ "$PARSED" == *"mid-paste"* ]]; then
    _pass "clipboard_hint emitted on mid-paste write"
  else
    _fail "clipboard_hint emitted on mid-paste write" "$PARSED"
  fi
fi

printf '%s' "$SAVED_TEXT_CLIP" | pbcopy 2>/dev/null || true

summary
