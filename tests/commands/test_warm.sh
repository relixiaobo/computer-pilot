#!/bin/bash
# Test: cu warm — AX bridge warm-up for already-running apps (D8)
source "$(dirname "$0")/helpers.sh"

# Use Finder — always running, never quits.
section "warm — running app"

cu_json warm Finder
assert_ok "warm Finder returns ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('|'.join([
    'ok=' + str(d.get('ok')),
    'app=' + str(d.get('app')),
    'pid_ok=' + str(isinstance(d.get('pid'), int) and d.get('pid', 0) > 0),
    'warmup_ok=' + str(isinstance(d.get('warmup_ms'), int) and d.get('warmup_ms', -1) >= 0),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"ok=True"* ]]         && _pass "ok=true"          || _fail "ok=true"          "$PARSED"
[[ "$PARSED" == *"app=Finder"* ]]      && _pass "app=Finder"       || _fail "app=Finder"       "$PARSED"
[[ "$PARSED" == *"pid_ok=True"* ]]     && _pass "pid populated"    || _fail "pid populated"    "$PARSED"
[[ "$PARSED" == *"warmup_ok=True"* ]]  && _pass "warmup_ms set"    || _fail "warmup_ms set"    "$PARSED"

section "warm — non-running app fails"

cu_json warm NonExistentApp987654
assert_fail "warm non-existent app fails"

section "warm — human mode"

cu_human warm Finder
assert_exit_zero "warm human exits 0"
if echo "$OUT" | grep -qE "Warmed AX bridge"; then
  _pass "human prints 'Warmed AX bridge ...'"
else
  _fail "human format" "${OUT:0:200}"
fi

summary
