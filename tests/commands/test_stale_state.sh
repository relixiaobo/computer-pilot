#!/bin/bash
# Behavior test: refs are bound to client-scoped Observations and stale refs
# fail before action dispatch.
source "$(dirname "$0")/helpers.sh"

section "observation — snapshot returns bound identity"

cu_json --client-key stale.agent snapshot Finder --limit 30
if ! is_json; then
  _skip "Observation identity" "snapshot Finder did not return JSON: ${OUT:0:120}"
  summary
  exit 0
fi
assert_json_field_exists "observation_id present" ".observation_id"
assert_json_field "client key bound" ".client_key" "stale.agent"
assert_json_field_exists "pid present" ".pid"
assert_json_field_exists "bundle_id present" ".bundle_id"
assert_json_field_exists "window_id present" ".window_id"
assert_json_field_exists "AX generation present" ".ax_generation"

OBSERVATION_ID=$(json_get '.observation_id')
OLD_X=$(echo "$OUT" | python3 -c 'import json,sys; print(round(json.load(sys.stdin)["window_frame"]["x"]))')
OLD_Y=$(echo "$OUT" | python3 -c 'import json,sys; print(round(json.load(sys.stdin)["window_frame"]["y"]))')

section "observation — another client cannot use the ref"

EXIT=0
OUT=$("$CU" --json --client-key other.agent click 1 --app Finder --observation "$OBSERVATION_ID" --no-snapshot 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_nonzero "cross-client ref action rejected"
CROSS_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
if [[ "$CROSS_CODE" == "observation_not_found" ]]; then
  _pass "cross-client rejection has stable code"
else
  _fail "cross-client rejection code" "got: $CROSS_CODE"
fi

section "observation — real UI drift fails before dispatch"

NEW_X=$((OLD_X + 30))
NEW_Y=$((OLD_Y + 30))
cu_json --client-key stale.agent window move "$NEW_X" "$NEW_Y" --app Finder
assert_ok "fixture moved Finder window"

EXIT=0
OUT=$("$CU" --json --client-key stale.agent click 1 --app Finder --observation "$OBSERVATION_ID" --no-snapshot 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_nonzero "stale ref action rejected"
STALE_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
if [[ "$STALE_CODE" == "stale_observation" ]]; then
  _pass "stale action has stable stale_observation code"
else
  _fail "stale action code" "got: $STALE_CODE stderr=${ERR:0:160}"
fi

if echo "$ERR" | python3 -c 'import json,sys; assert json.load(sys.stdin).get("ok") is False' 2>/dev/null; then
  _pass "stale action returned failure rather than advisory success"
else
  _fail "stale action failure envelope" "stderr=${ERR:0:160}"
fi

# Restore the user's Finder window.
cu_json --client-key stale.agent window move "$OLD_X" "$OLD_Y" --app Finder
assert_ok "Finder window restored"

section "observation — fresh ref action succeeds"

cu_json --client-key stale.agent snapshot Finder --limit 30
assert_ok "fresh snapshot succeeds"
FRESH_OBSERVATION=$(json_get '.observation_id')

cu_json --client-key stale.agent click 1 --app Finder --observation "$FRESH_OBSERVATION" --no-snapshot
assert_ok "fresh Observation ref dispatched"
assert_json_field "fresh ref preserved" ".ref" "1"

summary
