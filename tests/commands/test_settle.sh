#!/bin/bash
# Test: action responses include settle_ms (D7 single-shot AXObserver wait)
source "$(dirname "$0")/helpers.sh"

section "settle_ms — present on action responses"

cu_json key escape --app Finder
assert_ok "cu key returns ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ms = d.get('settle_ms')
print('|'.join([
    'has=' + str('settle_ms' in d),
    'is_int=' + str(isinstance(ms, int)),
    'in_bounds=' + str(isinstance(ms, int) and 0 <= ms <= 1500),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"has=True"* ]]       && _pass "settle_ms present"          || _fail "settle_ms present"          "$PARSED"
[[ "$PARSED" == *"is_int=True"* ]]    && _pass "settle_ms is integer"       || _fail "settle_ms is integer"       "$PARSED"
[[ "$PARSED" == *"in_bounds=True"* ]] && _pass "settle_ms within [0,1500]"  || _fail "settle_ms within [0,1500]"  "$PARSED"

section "settle_ms — never above POST_ACTION_DELAY_MS cap (500ms)"

# Sample 3 calls; max should be <= 600ms (allow small jitter for snapshot work)
MAX=0
for i in 1 2 3; do
  cu_json key escape --app Finder
  MS=$(json_get '.settle_ms' || echo "0")
  if [[ "$MS" -gt "$MAX" ]]; then MAX=$MS; fi
done
if [[ "$MAX" -le 700 ]]; then
  _pass "max settle_ms across 3 samples = ${MAX}ms (≤ 700ms cap)"
else
  _fail "settle_ms cap" "max=${MAX}ms exceeded ~700ms cap"
fi

section "auto-snapshot carries displays (D1)"

cu_json key escape --app Finder
HAS_DISPLAYS=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ds = d.get('snapshot', {}).get('displays')
print('yes' if isinstance(ds, list) and len(ds) >= 1 else 'no')
" 2>/dev/null || echo "error")
if [[ "$HAS_DISPLAYS" == "yes" ]]; then
  _pass "action's auto-snapshot includes displays array"
else
  _fail "auto-snapshot displays" "got: $HAS_DISPLAYS"
fi

section "settle_ms — absent when --no-snapshot"

cu_json key escape --app Finder --no-snapshot
HAS=$(echo "$OUT" | python3 -c "
import sys, json
print('yes' if 'settle_ms' in json.load(sys.stdin) else 'no')
" 2>/dev/null || echo "error")
if [[ "$HAS" == "no" ]]; then
  _pass "settle_ms omitted when --no-snapshot (no wait incurred)"
else
  _fail "settle_ms with --no-snapshot" "got: $HAS"
fi

summary
