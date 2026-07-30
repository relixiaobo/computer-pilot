#!/bin/bash
# Test: action commands attach confidence + advice based on method (C4)
source "$(dirname "$0")/helpers.sh"

section "method meta — pid-targeted confidence follows observed effect"

cu_json key escape --app Finder
assert_ok "cu key escape --app Finder"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('|'.join([
    'method=' + str(d.get('method')),
    'conf=' + str(d.get('confidence')),
    'effect=' + str(d.get('effect_verified')),
    'has_advice=' + str('advice' in d),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"method=key-pid"* ]]    && _pass "method=key-pid"        || _fail "method=key-pid"        "$PARSED"
if [[ "$PARSED" == *"effect=True"* ]]; then
  [[ "$PARSED" == *"conf=high"* ]] && _pass "verified effect promotes confidence=high" || _fail "verified effect confidence" "$PARSED"
  [[ "$PARSED" == *"has_advice=False"* ]] && _pass "verified effect needs no dispatch advice" || _fail "verified effect advice" "$PARSED"
else
  [[ "$PARSED" == *"conf=medium"* ]] && _pass "unverified PID dispatch stays confidence=medium" || _fail "unverified PID confidence" "$PARSED"
  [[ "$PARSED" == *"has_advice=True"* ]] && _pass "unverified PID dispatch carries advice" || _fail "unverified PID advice" "$PARSED"
fi

section "method meta — global tap → confidence=low + advice"

if interactive_tests_enabled; then
  # --allow-global opts past the frontmost-app safety check. This intentionally
  # sends a real event to the user's foreground app and is opt-in only.
  cu_json key escape --no-snapshot --allow-global
  assert_ok "cu key escape (no --app)"

  PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('|'.join([
    'method=' + str(d.get('method')),
    'conf=' + str(d.get('confidence')),
    'advice_has_app=' + str('--app' in str(d.get('advice', ''))),
]))
" 2>/dev/null || echo "malformed")

  [[ "$PARSED" == *"method=key-global"* ]]     && _pass "method=key-global"               || _fail "method=key-global"               "$PARSED"
  [[ "$PARSED" == *"conf=low"* ]]              && _pass "confidence=low on global tap"    || _fail "confidence=low on global tap"    "$PARSED"
  [[ "$PARSED" == *"advice_has_app=True"* ]]   && _pass "advice mentions --app remediation" || _fail "advice mentions --app remediation" "$PARSED"
else
  _skip "cu key escape (no --app)" "interactive desktop tests disabled"
  _skip "method=key-global" "interactive desktop tests disabled"
  _skip "confidence=low on global tap" "interactive desktop tests disabled"
  _skip "advice mentions --app remediation" "interactive desktop tests disabled"
fi

section "method meta — set-value distinguishes AX acceptance from UI effect"

# Look for a textfield in whatever Finder window already exists. We deliberately
# do NOT activate Finder — if there's no textfield, the test SKIPs cleanly.
cu_json find --app Finder --role textfield --first
SET_REF=$(json_get '.match.ref' || echo "")

if [[ -n "$SET_REF" && "$SET_REF" != "__MISSING__" && "$SET_REF" =~ ^[0-9]+$ ]]; then
  cu_json set-value "$SET_REF" "test" --app Finder --no-snapshot
  if [[ "$EXIT" -eq 0 ]]; then
    PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('method=' + str(d.get('method')) + '|conf=' + str(d.get('confidence')))
" 2>/dev/null || echo "malformed")
    [[ "$PARSED" == *"method=ax-set-value"* ]] && _pass "method=ax-set-value"            || _fail "method=ax-set-value"            "$PARSED"
    if [[ "$PARSED" == *"conf=medium"* || "$PARSED" == *"conf=low"* ]]; then
      _pass "ax-set-value does not claim high business-effect confidence"
    else
      _fail "ax-set-value confidence" "$PARSED"
    fi
  else
    _skip "set-value method check" "set-value failed (env)"
  fi
else
  _skip "set-value method check" "no textfield in Finder window"
fi

summary
