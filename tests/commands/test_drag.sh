#!/bin/bash
# Test: cu drag
source "$(dirname "$0")/helpers.sh"

section "drag — basic"

cu_json "drag 100 200 400 200"
assert_ok "drag horizontal"
assert_json_field_exists "from coordinates" ".from"
assert_json_field_exists "to coordinates" ".to"

# Check from/to values
FROM_X=$(echo "$OUT" | python3 -c "import sys,json; print(json.load(sys.stdin)['from']['x'])" 2>/dev/null || echo "")
TO_X=$(echo "$OUT" | python3 -c "import sys,json; print(json.load(sys.stdin)['to']['x'])" 2>/dev/null || echo "")
if [[ "$FROM_X" == "100.0" && "$TO_X" == "400.0" ]]; then
  _pass "from/to coordinates correct"
else
  _fail "from/to coordinates" "from.x=$FROM_X to.x=$TO_X"
fi

cu_json "drag 300 100 300 500"
assert_ok "drag vertical"

section "drag — with modifiers"

cu_json "drag 100 200 400 200 --shift"
assert_ok "drag with shift"

cu_json "drag 100 200 400 200 --alt"
assert_ok "drag with alt (option)"

cu_json "drag 100 200 400 200 --cmd"
assert_ok "drag with cmd"

section "drag — human mode"

cu_human "drag 100 200 400 200"
assert_exit_zero "drag human exits 0"
assert_contains "shows drag info" "Dragged"

summary
