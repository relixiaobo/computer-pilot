#!/bin/bash
# Test: verify_advice does NOT recommend --allow-global (G3).
#
# Why this exists: the original advice told agents to recover from a
# verified=false silent click via `cu window focus --app X && cu click ...
# --allow-global`. In a real agent session that recovery hit the agent's
# OWN terminal because focus drifted between bash invocations. The advice
# now points at cu primitives with --app (ax-action retry, perform AXPress,
# single osascript activate then retry --app cu) — never --allow-global.
source "$(dirname "$0")/helpers.sh"

section "verify_advice — coord-click silent failure"

# Click at coordinates inside Finder window but on dead space. CGEvent path
# fires, AX tree won't change → triggers cgevent-pid silent-failure advice.
cu_json click 500 500 --app Finder
assert_ok "click coord ok"

VERIFIED=$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin).get('verified'))" 2>/dev/null || echo "?")
ADVICE=$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin).get('verify_advice',''))" 2>/dev/null || echo "")

if [[ "$VERIFIED" == "False" ]]; then
  if [[ -n "$ADVICE" ]]; then
    _pass "verify_advice attached on cgevent-pid silent failure"
  else
    _fail "verify_advice attached" "missing on verified=false"
    summary
    exit 0
  fi

  # The critical assertion — advice should NOT push the agent toward the
  # global-tap recovery that races with bash-interval focus drift.
  if echo "$ADVICE" | grep -q -- "--allow-global"; then
    _fail "advice does NOT suggest --allow-global" "found '--allow-global' in advice: $ADVICE"
  else
    _pass "advice does NOT suggest --allow-global"
  fi

  # Positive check — advice should redirect to a cu primitive that stays
  # on --app-targeted delivery.
  if echo "$ADVICE" | grep -qE "(ref-based|--ax-path|AXPress|cu perform|--app)"; then
    _pass "advice points to cu primitive with --app"
  else
    _fail "advice points to cu primitive with --app" "advice was: $ADVICE"
  fi
else
  _skip "advice content checks" "this click verified=$VERIFIED — couldn't trigger silent-failure path"
fi

summary
