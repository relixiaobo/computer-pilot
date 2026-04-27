#!/bin/bash
# Test: cu examples (G2) — built-in recipe library
source "$(dirname "$0")/helpers.sh"

section "examples — list (no topic)"

cu_json examples
assert_ok "examples no-topic ok"
assert_json_field_exists "topics array" ".topics"

COUNT=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print(len(d.get('topics', [])))
" 2>/dev/null || echo "0")
if [[ "$COUNT" -ge 8 ]]; then
  _pass "topics list has $COUNT entries (≥ 8)"
else
  _fail "topics list size" "got: $COUNT"
fi

# Each topic has name + summary
SHAPE_OK=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ts = d.get('topics', [])
ok = all(isinstance(t, dict) and 'name' in t and 'summary' in t for t in ts)
print('yes' if ok else 'no')
" 2>/dev/null || echo "no")
if [[ "$SHAPE_OK" == "yes" ]]; then
  _pass "every topic has name + summary"
else
  _fail "topics shape" "$SHAPE_OK"
fi

section "examples — by topic"

# Each topic should produce a non-empty recipe
for TOPIC in launch-app fill-form dismiss-modal read-app-data wait-for-ui \
             vlm-click-by-image vlm-coord-to-ref vlm-region-candidates \
             diff-after-action menu-click region-screenshot system-pref; do
  cu_json examples "$TOPIC"
  assert_ok "examples $TOPIC ok"
  RECIPE=$(json_get '.recipe' || echo "")
  if [[ -n "$RECIPE" && "$RECIPE" != "__MISSING__" && "$(echo -n "$RECIPE" | wc -c)" -gt 10 ]]; then
    _pass "recipe for $TOPIC is non-trivial"
  else
    _fail "recipe for $TOPIC" "empty or trivial"
  fi
done

section "examples — recipes contain real cu commands"

cu_json examples launch-app
RECIPE=$(json_get '.recipe' || echo "")
if echo "$RECIPE" | grep -q "cu key cmd+space"; then
  _pass "launch-app recipe references cu key cmd+space"
else
  _fail "launch-app recipe content" "${RECIPE:0:100}"
fi

cu_json examples vlm-click-by-image
RECIPE=$(json_get '.recipe' || echo "")
if echo "$RECIPE" | grep -q "snapshot.*--annotated"; then
  _pass "vlm-click-by-image recipe references --annotated"
else
  _fail "vlm-click recipe content" "${RECIPE:0:100}"
fi

section "examples — unknown topic error"

cu_json examples not_a_real_topic_xyz
assert_fail "unknown topic rejected"
JSON_OUT="${OUT:-$ERR}"
PARSED=$(echo "$JSON_OUT" | python3 -c "
import sys, json
try: d = json.load(sys.stdin)
except: print('malformed'); sys.exit()
print('|'.join([
    'has_hint=' + str('hint' in d),
    'lists_topics=' + str('launch-app' in (d.get('hint') or '')),
    'has_next=' + str(isinstance(d.get('suggested_next'), list) and 'cu examples' in (d.get('suggested_next', [None])[0] or '')),
]))
" 2>/dev/null || echo "malformed")
[[ "$PARSED" == *"has_hint=True"* ]]      && _pass "unknown topic returns hint"          || _fail "hint"          "$PARSED"
[[ "$PARSED" == *"lists_topics=True"* ]]  && _pass "hint lists known topics"             || _fail "hint lists"    "$PARSED"
[[ "$PARSED" == *"has_next=True"* ]]      && _pass "suggested_next points to cu examples" || _fail "next"          "$PARSED"

section "examples — human mode"

cu_human examples
assert_exit_zero "examples human exits 0"
if echo "$OUT" | grep -q "Available recipe topics"; then
  _pass "human mode shows 'Available recipe topics'"
else
  _fail "human topics line" "${OUT:0:200}"
fi
if echo "$OUT" | grep -q "Run \`cu examples <topic>\`"; then
  _pass "human mode shows usage hint"
else
  _fail "human usage hint" "${OUT:0:200}"
fi

cu_human examples launch-app
assert_exit_zero "examples <topic> human exits 0"
if echo "$OUT" | grep -q "^# launch-app —"; then
  _pass "human mode shows '# topic — summary' header"
else
  _fail "human topic header" "${OUT:0:200}"
fi

summary
