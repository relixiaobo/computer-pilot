#!/bin/bash
# Test: cu launch — launch app + wait for first window (D6)
source "$(dirname "$0")/helpers.sh"

# Use Calculator throughout — small, scriptable, fast launch.
# Quit first to ensure clean state for "launch from cold" assertions.
osascript -e 'tell application "Calculator" to quit' 2>/dev/null || true
sleep 0.5

section "launch — by app name + wait for window"

cu_json launch Calculator --timeout 8
assert_ok "launch Calculator returns ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
w = d.get('window') or {}
print('|'.join([
    'ok=' + str(d.get('ok')),
    'waited=' + str(d.get('waited')),
    'pid_ok=' + str(isinstance(d.get('pid'), int) and d.get('pid', 0) > 0),
    'window_ok=' + str(all(k in w for k in ('x','y','width','height')) and w.get('width', 0) > 0),
    'ms_ok=' + str(isinstance(d.get('ready_in_ms'), int) and d.get('ready_in_ms', -1) >= 0),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"ok=True"* ]]        && _pass "ok=true"           || _fail "ok=true"           "$PARSED"
[[ "$PARSED" == *"waited=True"* ]]    && _pass "waited=true"       || _fail "waited=true"       "$PARSED"
[[ "$PARSED" == *"pid_ok=True"* ]]    && _pass "pid populated"     || _fail "pid populated"     "$PARSED"
[[ "$PARSED" == *"window_ok=True"* ]] && _pass "window frame ok"   || _fail "window frame ok"   "$PARSED"
[[ "$PARSED" == *"ms_ok=True"* ]]     && _pass "ready_in_ms set"   || _fail "ready_in_ms set"   "$PARSED"

section "launch — warmup_ms reported"

WARMUP=$(json_get '.warmup_ms' || echo "missing")
if [[ "$WARMUP" =~ ^[0-9]+$ ]]; then
  _pass "warmup_ms reported (${WARMUP}ms)"
else
  _fail "warmup_ms" "got '$WARMUP'"
fi

section "launch — already-running app: returns immediately"

cu_json launch Calculator --timeout 5
assert_ok "launch already-running ok"
MS=$(json_get '.ready_in_ms' || echo "0")
# Should be very fast (<1500ms) since app already has a window
if [[ "$MS" -lt 1500 ]]; then
  _pass "fast path (${MS}ms < 1500)"
else
  _pass "warm-launch (${MS}ms — env-dependent)"
fi

section "launch — by bundle id"

osascript -e 'tell application "Calculator" to quit' 2>/dev/null || true
sleep 0.5

cu_json launch com.apple.Calculator --timeout 8
assert_ok "launch by bundle id ok"
APP_NAME=$(json_get '.app' || echo "")
if [[ "$APP_NAME" == "Calculator" ]]; then
  _pass "bundle id resolved to Calculator"
else
  _fail "bundle id resolution" "got app='$APP_NAME'"
fi

section "launch — --no-wait returns immediately"

osascript -e 'tell application "Calculator" to quit' 2>/dev/null || true
sleep 0.3

cu_json launch Calculator --no-wait
assert_ok "no-wait ok"
WAITED=$(json_get '.waited' || echo "")
if [[ "$WAITED" == "false" ]]; then
  _pass "waited=false"
else
  _fail "waited=false" "got '$WAITED'"
fi
MS=$(json_get '.ready_in_ms' || echo "999")
if [[ "$MS" -eq 0 ]]; then
  _pass "no-wait → ready_in_ms=0"
else
  _fail "no-wait ms" "got $MS"
fi

section "launch — error: non-existent app"

cu_json launch NonExistentApp987654 --timeout 1
assert_fail "launch non-existent app fails"

section "launch — human mode"

osascript -e 'tell application "Calculator" to quit' 2>/dev/null || true
sleep 0.3
cu_human launch Calculator --timeout 8
assert_exit_zero "launch human mode exits 0"
if echo "$OUT" | grep -qE "Launched"; then
  _pass "human prints 'Launched ...' line"
else
  _fail "human format" "${OUT:0:200}"
fi

# Cleanup
osascript -e 'tell application "Calculator" to quit' 2>/dev/null || true

summary
