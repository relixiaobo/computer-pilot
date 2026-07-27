#!/bin/bash
# Test: Agent-neutral Python reference host
source "$(dirname "$0")/helpers.sh"

section "stdio host — initialize, discover, call, shutdown"

EXIT=0
OUT=$(python3 "$ROOT_DIR/examples/stdio-host/host.py" --cu "$CU" 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_zero "reference host exits zero"
assert_json "reference host output is JSON"
assert_json_field "reference host ok" ".ok" "true"
assert_json_field "reference host protocol major" ".protocol.major" "1"
assert_json_field "reference host protocol minor" ".protocol.minor" "0"
assert_json_field "discover-only manifest has five tools" ".toolCount" "5"
assert_json_field "default tool is examples" ".tool" "computer.examples"
assert_json_field "tool result schema" ".outcome.result.schema_version" "1.0"
assert_json_field "tool result ok" ".outcome.result.ok" "true"

section "stdio host — unavailable capability is not routed around"

EXIT=0
OUT=$(python3 "$ROOT_DIR/examples/stdio-host/host.py" \
  --cu "$CU" \
  --tool computer.snapshot \
  --arguments '{"app":"Finder"}' \
  2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_nonzero "unavailable tool exits non-zero"
if echo "$ERR" | python3 -c 'import json,sys; assert json.load(sys.stdin)["code"] == "tool_not_available"' 2>/dev/null; then
  _pass "unavailable tool has stable host error"
else
  _fail "unavailable tool has stable host error" "stderr=${ERR:0:200}"
fi

summary
