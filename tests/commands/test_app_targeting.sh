#!/bin/bash
# Behavior test: duplicate GUI instances are never selected by active/first
# heuristics; callers must choose one exact PID selector.
source "$(dirname "$0")/helpers.sh"

BUNDLE_ID="com.apple.TextEdit"
CREATED_PIDS=""

app_pids() {
  "$CU" --json apps 2>/dev/null | python3 -c '
import json, sys
for app in json.load(sys.stdin).get("apps", []):
    if app.get("bundle_id") == "com.apple.TextEdit":
        print(app["pid"])
' 2>/dev/null || true
}

active_app_pid() {
  "$CU" --json apps 2>/dev/null | python3 -c '
import json, sys
for app in json.load(sys.stdin).get("apps", []):
    if app.get("bundle_id") == "com.apple.TextEdit" and app.get("active"):
        print(app["pid"])
        break
' 2>/dev/null || true
}

BEFORE_PIDS=$(app_pids)

record_created_pids() {
  local pid
  for pid in $(app_pids); do
    if ! printf '%s\n' "$BEFORE_PIDS" | grep -qxF "$pid" \
      && ! printf '%s\n' "$CREATED_PIDS" | grep -qxF "$pid"; then
      CREATED_PIDS="${CREATED_PIDS}${CREATED_PIDS:+ }$pid"
    fi
  done
}

cleanup_targeting_test() {
  local pid
  for pid in $CREATED_PIDS; do
    kill "$pid" 2>/dev/null || true
  done
}
trap cleanup_targeting_test EXIT

section "app targeting — construct duplicate running instances"

for _ in 1 2; do
  CURRENT_PIDS=$(app_pids)
  if [[ $(printf '%s\n' "$CURRENT_PIDS" | awk 'NF { count++ } END { print count+0 }') -ge 2 ]]; then
    break
  fi
  /usr/bin/open -n -b "$BUNDLE_ID" >/dev/null 2>&1 || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    sleep 0.2
    record_created_pids
    CURRENT_PIDS=$(app_pids)
    if [[ $(printf '%s\n' "$CURRENT_PIDS" | awk 'NF { count++ } END { print count+0 }') -ge 2 ]]; then
      break
    fi
  done
done

CURRENT_PIDS=$(app_pids)
INSTANCE_COUNT=$(printf '%s\n' "$CURRENT_PIDS" | awk 'NF { count++ } END { print count+0 }')
if [[ "$INSTANCE_COUNT" -lt 2 ]]; then
  _skip "duplicate instance behavior" "TextEdit did not allow two running instances"
  summary
  exit 0
fi
_pass "two TextEdit GUI processes are running"

section "app targeting — ambiguous bundle is rejected"

cu_json snapshot "$BUNDLE_ID" --limit 5
assert_fail "duplicate bundle selector fails before observation"
CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
CANDIDATE_COUNT=$(echo "$ERR" | python3 -c 'import json,sys; print(len(json.load(sys.stdin).get("diagnostics", {}).get("candidates", [])))' 2>/dev/null || echo 0)
if [[ "$CODE" == "ambiguous_target" && "$CANDIDATE_COUNT" -ge 2 ]]; then
  _pass "ambiguous_target returns candidate processes"
else
  _fail "ambiguous target diagnostics" "code=$CODE candidates=$CANDIDATE_COUNT stderr=${ERR:0:240}"
fi

section "app targeting — PID selector binds observation"

TARGET_PID=$(active_app_pid)
if [[ -z "$TARGET_PID" ]]; then
  TARGET_PID=$(printf '%s\n' "$CURRENT_PIDS" | awk 'NF { print; exit }')
fi
for _ in 1 2 3 4 5 6 7 8 9 10; do
  cu_json snapshot "pid:$TARGET_PID" --limit 5
  [[ "$EXIT" -eq 0 ]] && break
  sleep 0.2
done
assert_ok "PID selector snapshots one exact instance"
assert_json_field "observation is bound to requested PID" ".pid" "$TARGET_PID"

summary
