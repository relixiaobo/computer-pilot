#!/bin/bash
# Test: cu type auto-routes through clipboard paste for CJK / chat apps (R7).
#
# Why: WeChat/Slack/Discord/Telegram/Lark/QQ/DingTalk drop the first
# character of unicode CGEvents (their text input is a webview that
# initialises after the first event lands). Auto-paste makes the agent
# "just work" without remembering --paste; the alternative is silent
# truncation that the agent has no easy way to detect.
source "$(dirname "$0")/helpers.sh"

# Activate TextEdit so the type events have a target window with focus.
osascript -e 'tell application "TextEdit" to make new document' >/dev/null 2>&1
osascript -e 'tell application "TextEdit" to activate' >/dev/null 2>&1
sleep 0.4

section "paste auto-detect — CJK content routes via paste"

cu_json type "你好世界" --app TextEdit --no-snapshot
assert_ok "type CJK ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('method=' + str(d.get('method')) + '|reason=' + str(d.get('paste_reason','')))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"method=paste-pid"* ]]   && _pass "CJK auto-routed to paste-pid"        || _fail "CJK auto-routed to paste-pid"        "$PARSED"
[[ "$PARSED" == *"reason="*"CJK"* ]]      && _pass "paste_reason names CJK as cause"     || _fail "paste_reason names CJK"             "$PARSED"

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

section "paste auto-detect — explicit --no-paste forces unicode even with CJK"

cu_json type "你好" --app TextEdit --no-snapshot --no-paste
assert_ok "type CJK --no-paste ok"

METHOD=$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin).get('method',''))" 2>/dev/null || echo "")
[[ "$METHOD" == "unicode-pid" ]] && _pass "--no-paste overrides CJK auto-detection" || _fail "--no-paste overrides" "method=$METHOD"

section "paste auto-detect — explicit --paste forces paste even on ASCII"

cu_json type "hello" --app TextEdit --no-snapshot --paste
assert_ok "type ASCII --paste ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('method=' + str(d.get('method')) + '|reason=' + str(d.get('paste_reason','')))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"method=paste-pid"* ]]    && _pass "--paste forces paste-pid"            || _fail "--paste forces paste-pid"   "$PARSED"
[[ "$PARSED" == *"reason="*"explicit"* ]]  && _pass "paste_reason cites explicit flag"   || _fail "paste_reason cites explicit" "$PARSED"

# Cleanup
osascript -e 'tell application "TextEdit" to close every document saving no' >/dev/null 2>&1 || true
osascript -e 'tell application "TextEdit" to quit' >/dev/null 2>&1 || true

summary
