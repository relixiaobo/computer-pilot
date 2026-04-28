#!/bin/bash
# Test: action commands attach confidence + advice based on method (C4)
source "$(dirname "$0")/helpers.sh"

section "method meta — pid-targeted → confidence=high, no advice"

cu_json key escape --app Finder
assert_ok "cu key escape --app Finder"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('|'.join([
    'method=' + str(d.get('method')),
    'conf=' + str(d.get('confidence')),
    'has_advice=' + str('advice' in d),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"method=key-pid"* ]]    && _pass "method=key-pid"        || _fail "method=key-pid"        "$PARSED"
[[ "$PARSED" == *"conf=high"* ]]         && _pass "confidence=high"       || _fail "confidence=high"       "$PARSED"
[[ "$PARSED" == *"has_advice=False"* ]]  && _pass "no advice on best path" || _fail "no advice on best path" "$PARSED"

section "method meta — global tap → confidence=low + advice"

# --allow-global opts past the frontmost-app safety check (test runs from a terminal,
# which is now refused by default). The intent here is to exercise the global tap
# path itself and verify it carries low confidence + remediation advice.
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

section "method meta — set-value (ax-set-value) → high"

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
    [[ "$PARSED" == *"conf=high"* ]]           && _pass "ax-set-value confidence=high"   || _fail "ax-set-value confidence=high"   "$PARSED"
  else
    _skip "set-value method check" "set-value failed (env)"
  fi
else
  _skip "set-value method check" "no textfield in Finder window"
fi

summary
