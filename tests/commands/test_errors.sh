#!/bin/bash
# Test: error handling across all commands
source "$(dirname "$0")/helpers.sh"

section "errors — non-existent app"

cu_json "snapshot NonExistentApp98765"
assert_fail "snapshot: bad app"

cu_json "screenshot NonExistentApp98765"
assert_fail "screenshot: bad app"

cu_json "ocr NonExistentApp98765"
assert_fail "ocr: bad app"

cu_json "click 1 --app NonExistentApp98765 --no-snapshot"
assert_fail "click ref: bad app"

section "errors — invalid click targets"

cu_json "click 0 --app Finder --no-snapshot"
assert_fail "click ref=0"

cu_json "click abc"
assert_fail "click non-numeric ref"

section "errors — invalid scroll"

cu_json "scroll diagonal 3 --x 100 --y 100"
assert_fail "scroll bad direction"

section "errors — wait missing condition"

EXIT=0
OUT=$($CU wait --app Finder --timeout 1 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_nonzero "wait: no condition"

section "errors — JSON error format"

# Errors piped should be JSON with ok=false and error field
cu_json "snapshot NonExistentApp98765"
if [[ $EXIT -ne 0 ]]; then
  # Error goes to stderr in JSON mode — check ERR
  ERR_OK=$(echo "$ERR" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    has_ok = 'ok' in d and d['ok'] == False
    has_err = 'error' in d and len(d['error']) > 0
    print('ok' if (has_ok and has_err) else 'bad')
except: print('not_json')
" 2>/dev/null || echo "parse_error")
  if [[ "$ERR_OK" == "ok" ]]; then
    _pass "JSON error has ok=false + error message"
  elif [[ "$ERR_OK" == "not_json" ]]; then
    _fail "JSON error format" "stderr is not JSON: ${ERR:0:200}"
  else
    _fail "JSON error format" "missing ok/error fields: ${ERR:0:200}"
  fi
else
  _fail "JSON error format" "expected non-zero exit"
fi

section "errors — human error format"

cu_human "snapshot NonExistentApp98765"
if [[ $EXIT -ne 0 ]]; then
  if [[ "$ERR" == *"Error"* || "$ERR" == *"error"* || "$ERR" == *"not found"* || "$ERR" == *"not running"* ]]; then
    _pass "human error message is readable"
  else
    _fail "human error message" "stderr: ${ERR:0:200}"
  fi
else
  _fail "human error" "expected non-zero exit"
fi

summary
