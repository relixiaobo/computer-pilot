#!/bin/bash
# Test: cu window (list/move/resize/focus/minimize/close)
source "$(dirname "$0")/helpers.sh"

# Ensure Finder has a window. We do NOT activate — `cu window list / move /
# resize / focus` work over AX without requiring Finder to be frontmost. The
# `cu window focus` test below explicitly verifies focus theft only when asked.
osascript -e 'tell application "Finder"
  if (count of Finder windows) is 0 then make new Finder window
end tell' 2>/dev/null
sleep 0.3

section "window list"

cu_json window list
assert_ok "window list ok"
assert_json_field_exists "windows array" ".windows"

# Should have at least Finder
HAS_FINDER=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('yes' if any(w['app'] == 'Finder' for w in d['windows']) else 'no')
" 2>/dev/null || echo "no")
if [[ "$HAS_FINDER" == "yes" ]]; then
  _pass "Finder window listed"
else
  _fail "Finder window" "not found in list"
fi

# Check window fields
HAS_FIELDS=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
w = next((w for w in d['windows'] if w['app'] == 'Finder'), None)
if not w: print('missing'); sys.exit()
required = ['app','index','title','x','y','width','height','minimized','focused']
miss = [f for f in required if f not in w]
print('ok' if not miss else f'missing: {miss}')
" 2>/dev/null || echo "error")
if [[ "$HAS_FIELDS" == "ok" ]]; then
  _pass "window has all fields"
else
  _fail "window fields" "$HAS_FIELDS"
fi

section "window list --app Finder"

cu_json window list --app Finder
assert_ok "window list --app ok"
# Should only have Finder windows
ONLY_FINDER=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('yes' if all(w['app']=='Finder' for w in d['windows']) else 'no')
" 2>/dev/null || echo "no")
if [[ "$ONLY_FINDER" == "yes" ]]; then
  _pass "filtered to Finder only"
else
  _fail "app filter" "got other apps"
fi

section "window move + verify"

cu_json window move 250 150 --app Finder
assert_ok "window move ok"
sleep 0.3

# Verify new position
cu_json window list --app Finder
NEW_X=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print(d['windows'][0]['x'])
" 2>/dev/null || echo "0")
NEW_Y=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print(d['windows'][0]['y'])
" 2>/dev/null || echo "0")
if [[ "$NEW_X" == "250" && "$NEW_Y" == "150" ]]; then
  _pass "move verified (${NEW_X},${NEW_Y})"
else
  _fail "move verification" "got ($NEW_X,$NEW_Y)"
fi

section "window resize + verify"

cu_json window resize 900 600 --app Finder
assert_ok "window resize ok"
sleep 0.3

cu_json window list --app Finder
NEW_W=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print(d['windows'][0]['width'])
" 2>/dev/null || echo "0")
NEW_H=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print(d['windows'][0]['height'])
" 2>/dev/null || echo "0")
if [[ "$NEW_W" == "900" && "$NEW_H" == "600" ]]; then
  _pass "resize verified (${NEW_W}×${NEW_H})"
else
  _fail "resize verification" "got ${NEW_W}×${NEW_H}"
fi

section "window focus"

cu_json window focus --app Finder
assert_ok "window focus ok"
# B6: focus should report method=ax-raise (direct AX, no global activate)
METHOD=$(json_get '.method' || echo "")
if [[ "$METHOD" == "ax-raise" ]]; then
  _pass "focus uses method=ax-raise (B6)"
else
  _fail "focus method" "expected ax-raise, got '$METHOD'"
fi

section "window — error handling"

cu_json window move 100 100
assert_fail "move without --app fails"

cu_json window badaction --app Finder
assert_fail "unknown action fails"

section "window — human mode"

cu_human window list --app Finder
assert_exit_zero "window human exits 0"
assert_contains "shows window info" "Finder"

summary
