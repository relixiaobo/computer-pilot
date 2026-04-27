#!/bin/bash
# Test: cu why — failure diagnostic for refs (B7)
source "$(dirname "$0")/helpers.sh"

# Use Finder — always running, has many refs.
section "why — found ref"

cu_json why 1 --app Finder
assert_ok "why on running app returns ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
el = d.get('element') or {}
ck = d.get('checks') or {}
print('|'.join([
    'ok=' + str(d.get('ok')),
    'found=' + str(d.get('found')),
    'has_element=' + str(bool(el)),
    'has_role=' + str('role' in el),
    'has_axpath=' + str('axPath' in el),
    'has_click_xy=' + str('click_x' in el and 'click_y' in el),
    'in_snapshot=' + str(ck.get('in_snapshot')),
    'has_advice=' + str(bool(d.get('advice'))),
    'snapshot_size_ok=' + str(isinstance(d.get('snapshot_size'), int) and d.get('snapshot_size', 0) > 0),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"ok=True"* ]]              && _pass "ok=true"            || _fail "ok=true"            "$PARSED"
[[ "$PARSED" == *"found=True"* ]]           && _pass "found=true"         || _fail "found=true"         "$PARSED"
[[ "$PARSED" == *"has_element=True"* ]]     && _pass "element returned"   || _fail "element returned"   "$PARSED"
[[ "$PARSED" == *"has_role=True"* ]]        && _pass "element.role"       || _fail "element.role"       "$PARSED"
[[ "$PARSED" == *"has_axpath=True"* ]]      && _pass "element.axPath"     || _fail "element.axPath"     "$PARSED"
[[ "$PARSED" == *"has_click_xy=True"* ]]    && _pass "click_x/click_y"    || _fail "click_x/click_y"    "$PARSED"
[[ "$PARSED" == *"in_snapshot=True"* ]]     && _pass "checks.in_snapshot" || _fail "checks.in_snapshot" "$PARSED"
[[ "$PARSED" == *"has_advice=True"* ]]      && _pass "advice non-empty"   || _fail "advice non-empty"   "$PARSED"
[[ "$PARSED" == *"snapshot_size_ok=True"* ]] && _pass "snapshot_size > 0" || _fail "snapshot_size"      "$PARSED"

section "why — missing ref"

cu_json why 9999 --app Finder
assert_ok "why with missing ref still returns ok"

MISSING=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ck = d.get('checks') or {}
print('|'.join([
    'found=' + str(d.get('found')),
    'in_snapshot=' + str(ck.get('in_snapshot')),
    'element_null=' + str(d.get('element') is None),
    'has_advice=' + str(bool(d.get('advice'))),
]))
" 2>/dev/null || echo "malformed")

[[ "$MISSING" == *"found=False"* ]]        && _pass "found=false"        || _fail "found=false"        "$MISSING"
[[ "$MISSING" == *"in_snapshot=False"* ]]  && _pass "in_snapshot=false"  || _fail "in_snapshot=false"  "$MISSING"
[[ "$MISSING" == *"element_null=True"* ]]  && _pass "element is null"    || _fail "element null"       "$MISSING"
[[ "$MISSING" == *"has_advice=True"* ]]    && _pass "advice non-empty"   || _fail "advice non-empty"   "$MISSING"

section "why — non-running app fails"

cu_json why 1 --app NonExistentApp987654
assert_fail "why on non-existent app fails"

section "why — human mode"

cu_human why 1 --app Finder
assert_exit_zero "why human exits 0"
if echo "$OUT" | grep -qE "advice:"; then
  _pass "human prints 'advice:' line"
else
  _fail "human format" "${OUT:0:200}"
fi

summary
