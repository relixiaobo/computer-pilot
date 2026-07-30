#!/bin/bash
# Test: cu hover
source "$(dirname "$0")/helpers.sh"

section "hover — basic"

cu_json "hover 500 300 --app Finder"
assert_ok "hover at (500, 300)"
assert_json_field "x coord" ".x" "500.0"
assert_json_field "y coord" ".y" "300.0"

cu_json "hover 0 0 --app Finder"
assert_ok "hover at origin (0, 0)"

cu_json "hover 1920 1080 --app Finder"
assert_ok "hover at large coords"

section "hover — human mode"

cu_human "hover 250 250 --app Finder"
assert_exit_zero "hover human exits 0"
assert_contains "shows hover info" "Hover"

section "hover — auto-snapshot contract"

cu_json hover 500 400 --app Finder
assert_ok "hover with default attaches snapshot"
HAS=$(echo "$OUT" | python3 -c "
import sys, json
print('yes' if 'snapshot' in json.load(sys.stdin) else 'no')
" 2>/dev/null || echo "error")
if [[ "$HAS" == "yes" ]]; then
  _pass "hover attaches snapshot"
else
  _fail "hover attaches snapshot" "got $HAS"
fi

cu_json hover 500 400 --app Finder --no-snapshot
assert_ok "hover --no-snapshot ok"
NO_SNAP=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('absent' if 'snapshot' not in d else 'present')
" 2>/dev/null || echo "error")
if [[ "$NO_SNAP" == "absent" ]]; then
  _pass "hover --no-snapshot omits snapshot"
else
  _fail "hover --no-snapshot" "snapshot was present"
fi

section "hover — frontmost-app safety check"

# Without --app the move goes through the global HID tap (warps the user's
# real cursor). Inject a dangerous frontmost via the test seam: cu must refuse.
OUT=$(CU_TEST_FRONTMOST_OVERRIDE=Terminal "$CU" hover 100 100 --no-snapshot 2>&1) || true
if echo "$OUT" | grep -q "refusing to hover"; then
  _pass "refuses hover without --app when frontmost is dangerous"
else
  _fail "refuses hover without --app" "expected refusal, got: ${OUT:0:120}"
fi

OUT=$(CU_TEST_FRONTMOST_OVERRIDE=Terminal "$CU" hover 500 400 --app Finder --no-snapshot 2>&1) || true
if echo "$OUT" | grep -q "refusing to hover"; then
  _fail "--app hover bypasses safety check" "was refused: ${OUT:0:120}"
else
  _pass "--app hover bypasses safety check"
fi

summary
