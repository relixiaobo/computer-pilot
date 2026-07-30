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

summary
