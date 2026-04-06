#!/bin/bash
# Test: cu wait
source "$(dirname "$0")/helpers.sh"

section "wait --text — existing text in Finder"

# Get some text that's actually in Finder's UI
cu_json "snapshot Finder --limit 5"
SOME_TEXT=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
for e in d.get('elements', []):
    t = e.get('title') or e.get('value') or ''
    if len(t) > 2:
        print(t[:20])
        sys.exit()
print('')
" 2>/dev/null || echo "")

if [[ -n "$SOME_TEXT" ]]; then
  cu_json wait --text "$SOME_TEXT" --app Finder --timeout 3
  assert_ok "wait --text finds existing text"
  assert_json_field_exists "elapsed_ms" ".elapsed_ms"

  ELAPSED=$(json_get '.elapsed_ms' || echo "0")
  if [[ "$ELAPSED" -lt 3000 ]] 2>/dev/null; then
    _pass "found quickly (${ELAPSED}ms < 3000ms)"
  else
    _fail "found quickly" "took ${ELAPSED}ms"
  fi
else
  _skip "wait --text existing" "no text found in Finder snapshot"
fi

section "wait --text — non-existent text (timeout)"

cu_json "wait --text ZZZZNONEXISTENT99999 --app Finder --timeout 2"
assert_fail "timeout on non-existent text"

# Should timeout around 2 seconds
if [[ $EXIT -ne 0 ]]; then
  _pass "exits non-zero on timeout"
fi

section "wait --ref — existing ref"

cu_json "wait --ref-id 1 --app Finder --timeout 3"
if [[ $EXIT -eq 0 ]]; then
  assert_ok "wait --ref 1 found"
else
  _skip "wait --ref 1" "ref 1 not found in Finder"
fi

section "wait --gone — non-existent ref (immediate success)"

cu_json "wait --gone 9999 --app Finder --timeout 3"
assert_ok "wait --gone 9999 (already absent)"
ELAPSED=$(json_get '.elapsed_ms' || echo "9999")
if [[ "$ELAPSED" -lt 2000 ]] 2>/dev/null; then
  _pass "gone condition met quickly (${ELAPSED}ms)"
else
  _fail "gone immediate" "took ${ELAPSED}ms"
fi

section "wait — error: no condition specified"

EXIT=0
OUT=$($CU wait --app Finder --timeout 1 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_nonzero "missing condition fails"

section "wait — error: non-existent app"

cu_json "wait --text hello --app NonExistentApp98765 --timeout 1"
assert_fail "non-existent app fails"

summary
