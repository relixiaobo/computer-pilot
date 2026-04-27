#!/bin/bash
# Test: cu hover
source "$(dirname "$0")/helpers.sh"

section "hover — basic"

cu_json "hover 500 300"
assert_ok "hover at (500, 300)"
assert_json_field "x coord" ".x" "500.0"
assert_json_field "y coord" ".y" "300.0"

cu_json "hover 0 0"
assert_ok "hover at origin (0, 0)"

cu_json "hover 1920 1080"
assert_ok "hover at large coords"

section "hover — human mode"

cu_human "hover 250 250"
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

summary
