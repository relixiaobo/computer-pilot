#!/bin/bash
# Test: cu scroll
source "$(dirname "$0")/helpers.sh"

section "scroll — all directions"

cu_json "scroll down 3 --x 400 --y 300"
assert_ok "scroll down"
assert_json_field "direction" ".direction" "down"
assert_json_field "amount" ".amount" "3"

cu_json "scroll up 3 --x 400 --y 300"
assert_ok "scroll up"
assert_json_field "direction" ".direction" "up"

cu_json "scroll left 2 --x 400 --y 300"
assert_ok "scroll left"
assert_json_field "direction" ".direction" "left"

cu_json "scroll right 2 --x 400 --y 300"
assert_ok "scroll right"
assert_json_field "direction" ".direction" "right"

section "scroll — default amount"

cu_json "scroll down --x 400 --y 300"
assert_ok "scroll with default amount"
assert_json_field "default amount is 3" ".amount" "3"

section "scroll — coordinates in response"

cu_json "scroll down 5 --x 123 --y 456"
assert_ok "scroll with specific coords"
assert_json_field "x coord" ".x" "123.0"
assert_json_field "y coord" ".y" "456.0"

section "scroll — error cases"

cu_json "scroll diagonal 3 --x 400 --y 300"
assert_fail "invalid direction 'diagonal'"

# Missing --x or --y
EXIT=0
OUT=$($CU scroll down 3 --y 300 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_nonzero "missing --x fails"

EXIT=0
OUT=$($CU scroll down 3 --x 400 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_nonzero "missing --y fails"

section "scroll — human mode"

cu_human "scroll down 3 --x 400 --y 300"
assert_exit_zero "scroll human exits 0"
assert_contains "shows scroll info" "Scrolled"
assert_contains "shows direction" "down"

section "scroll — auto-snapshot contract (post-action UI state)"

cu_json scroll down 1 --x 600 --y 400 --app Finder
assert_ok "scroll with default attaches snapshot"
PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('|'.join([
    'has_snapshot=' + str('snapshot' in d),
    'has_settle_ms=' + str('settle_ms' in d),
]))
" 2>/dev/null || echo "malformed")
[[ "$PARSED" == *"has_snapshot=True"* ]]   && _pass "scroll attaches snapshot"   || _fail "scroll attaches snapshot"   "$PARSED"
[[ "$PARSED" == *"has_settle_ms=True"* ]]  && _pass "scroll has settle_ms"       || _fail "scroll settle_ms"           "$PARSED"

cu_json scroll down 1 --x 600 --y 400 --app Finder --no-snapshot
assert_ok "scroll --no-snapshot ok"
NO_SNAP=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('absent' if 'snapshot' not in d else 'present')
" 2>/dev/null || echo "error")
if [[ "$NO_SNAP" == "absent" ]]; then
  _pass "--no-snapshot omits snapshot"
else
  _fail "scroll --no-snapshot" "snapshot was present"
fi

summary
