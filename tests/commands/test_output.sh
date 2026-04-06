#!/bin/bash
# Test: output mode switching (JSON vs human)
source "$(dirname "$0")/helpers.sh"

section "output mode — piped = JSON"

# When piped, output should be valid JSON
cu_json "setup"
assert_json "piped setup is JSON"

cu_json "apps"
assert_json "piped apps is JSON"

cu_json "snapshot Finder --limit 3"
assert_json "piped snapshot is JSON"

section "output mode — --human flag overrides"

# --human should produce non-JSON even when piped
cu_human "setup"
# Should contain human text, not start with {
if [[ "$OUT" == "{"* ]]; then
  _fail "--human setup is not JSON" "starts with {"
else
  _pass "--human setup is human text"
fi

cu_human "apps"
if [[ "$OUT" == "{"* ]]; then
  _fail "--human apps is not JSON" "starts with {"
else
  _pass "--human apps is human text"
fi

cu_human "snapshot Finder --limit 3"
if [[ "$OUT" == "{"* ]]; then
  _fail "--human snapshot is not JSON" "starts with {"
else
  _pass "--human snapshot is human text"
fi

section "output mode — JSON structure consistency"

# Commands with "ok" field
for cmd in "setup" "hover 100 100" "scroll down 3 --x 100 --y 100"; do
  cu_json "$cmd"
  OK_FIELD=$(json_get '.ok' 2>/dev/null || echo "missing")
  if [[ "$OK_FIELD" == "true" || "$OK_FIELD" == "false" ]]; then
    _pass "\"$cmd\" has ok field ($OK_FIELD)"
  else
    _fail "\"$cmd\" has ok field" "missing or invalid: $OK_FIELD"
  fi
done

# apps returns {apps: [...]} without ok field — just check it's valid JSON
cu_json "apps"
assert_json "apps is valid JSON"

section "output mode — action commands echo parameters"

cu_json "hover 123 456"
assert_json_field "hover echoes x" ".x" "123.0"
assert_json_field "hover echoes y" ".y" "456.0"

cu_json "scroll up 7 --x 100 --y 200"
assert_json_field "scroll echoes direction" ".direction" "up"
assert_json_field "scroll echoes amount" ".amount" "7"

summary
