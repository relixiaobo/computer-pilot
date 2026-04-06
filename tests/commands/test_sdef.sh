#!/bin/bash
# Test: cu sdef (scripting dictionary introspection)
source "$(dirname "$0")/helpers.sh"

section "sdef — Finder (always scriptable)"

cu_json sdef Finder
assert_ok "sdef Finder ok"
assert_json_field "app is Finder" ".app" "Finder"
assert_json_field_exists "suites present" ".suites"

SUITE_COUNT=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print(len(d.get('suites', [])))
" 2>/dev/null || echo "0")
if [[ "$SUITE_COUNT" -ge 1 ]]; then
  _pass "has suites ($SUITE_COUNT)"
else
  _fail "has suites" "got $SUITE_COUNT"
fi

section "sdef — class structure"

# Check that classes have name and properties
HAS_CLASSES=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
for suite in d.get('suites', []):
    for cls in suite.get('classes', []):
        if 'name' in cls and 'properties' in cls:
            print('ok')
            sys.exit()
print('no')
" 2>/dev/null || echo "error")
if [[ "$HAS_CLASSES" == "ok" ]]; then
  _pass "classes have name and properties"
else
  _fail "class structure" "$HAS_CLASSES"
fi

section "sdef — Notes (always installed)"

cu_json sdef Notes
if [[ $EXIT -eq 0 ]] && is_json; then
  assert_ok "sdef Notes ok"
  # Notes should have note class
  HAS_NOTE=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
for suite in d.get('suites', []):
    for cls in suite.get('classes', []):
        if cls['name'] == 'note':
            print('yes')
            sys.exit()
print('no')
" 2>/dev/null || echo "error")
  if [[ "$HAS_NOTE" == "yes" ]]; then
    _pass "Notes has note class"
  else
    _fail "Notes note class" "$HAS_NOTE"
  fi
else
  _skip "sdef Notes" "Notes not running or not found"
fi

section "sdef — Safari"

cu_json sdef Safari
assert_ok "sdef Safari ok"

# Safari should have tab class
HAS_TAB=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
for suite in d.get('suites', []):
    for cls in suite.get('classes', []):
        if cls['name'] == 'tab':
            print('yes')
            sys.exit()
print('no')
" 2>/dev/null || echo "error")
if [[ "$HAS_TAB" == "yes" ]]; then
  _pass "Safari has tab class"
else
  _fail "Safari tab class" "$HAS_TAB"
fi

section "sdef — error: non-scriptable app"

cu_json sdef "Activity Monitor"
assert_fail "non-scriptable app fails"
# Error should mention "not scriptable" or "not found"
if [[ "$ERR" == *"not scriptable"* || "$ERR" == *"not found"* ]]; then
  _pass "error mentions not scriptable or not found"
else
  _fail "error message" "stderr: ${ERR:0:200}"
fi

section "sdef — error: non-existent app"

cu_json sdef FakeApp99999
assert_fail "non-existent app fails"

section "sdef — human mode"

cu_human sdef Finder
assert_exit_zero "sdef human exits 0"
assert_contains "shows suite" "suite"
assert_contains "shows props" "props:"

summary
