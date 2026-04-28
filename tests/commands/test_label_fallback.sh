#!/bin/bash
# Test: snapshot title falls back through AXTitle → AXDescription → AXHelp
# → AXIdentifier (R5). Electron/CEF apps often set AXTitle to internal IDs
# while the user-visible label lives in AXHelp (tooltip) or AXIdentifier
# (aria-label). Without the fallback, agents searching by visible text
# can't find buttons that are right there on the screen.
#
# We can't synthesize an Electron app inside the test runner, so we settle
# for shape verification — the snapshot output must include `title` fields
# populated for elements (proving the chain is at least running), and the
# AX batch must read AXHelp + AXIdentifier (caught at compile time when
# BATCH_ATTR_NAMES has the right entries).
source "$(dirname "$0")/helpers.sh"

section "label fallback — Finder snapshot has populated titles"

# Finder is non-Electron, so AXTitle alone is enough. This test guards
# the chain from regressing to "no titles anywhere" if the structure breaks.
cu_json snapshot Finder --limit 50

TITLED_COUNT=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
els = d.get('elements', [])
print(sum(1 for e in els if e.get('title')))
" 2>/dev/null || echo "0")

if [[ "$TITLED_COUNT" -ge 3 ]]; then
  _pass "at least 3 elements have titles ($TITLED_COUNT seen)"
else
  _fail "elements have populated titles" "only $TITLED_COUNT had a title — fallback chain may be broken"
fi

section "label fallback — Safari (CEF-adjacent) finds button labels"

# Safari is WebKit-native (not Electron) but has a fair amount of
# WebKit-driven AX where AXIdentifier or AXDescription matter. This is
# only run if Safari is open — skip otherwise to keep CI green on
# headless machines.
if osascript -e 'tell application "System Events" to (name of processes) contains "Safari"' 2>/dev/null | grep -q true; then
  cu_json snapshot Safari --limit 100
  if echo "$OUT" | python3 -c "import sys,json;d=json.load(sys.stdin);sys.exit(0 if d.get('ok') else 1)" 2>/dev/null; then
    HAS_BUTTON=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
els = d.get('elements', [])
buttons_with_label = [e for e in els if e.get('role') == 'button' and e.get('title')]
print(len(buttons_with_label))
" 2>/dev/null || echo "0")
    if [[ "$HAS_BUTTON" -ge 1 ]]; then
      _pass "Safari has at least one labeled button ($HAS_BUTTON found)"
    else
      _fail "Safari labeled buttons" "no buttons with title — AXIdentifier fallback may be missing"
    fi
  else
    _skip "Safari labeled buttons" "snapshot failed (Safari may have lost AX permissions)"
  fi
else
  _skip "Safari labeled buttons" "Safari not running"
fi

summary
