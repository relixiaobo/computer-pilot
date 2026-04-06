#!/bin/bash
# Test: cu tell (AppleScript execution)
source "$(dirname "$0")/helpers.sh"

section "tell — basic AppleScript"

cu_json tell Finder 'get name of front Finder window'
assert_ok "tell Finder get window name"
assert_json_field "app field" ".app" "Finder"
assert_json_field_exists "result present" ".result"

cu_json tell Finder 'get version'
assert_ok "tell Finder get version"

section "tell — list results"

cu_json tell Finder 'get name of every Finder window'
assert_ok "tell Finder every window name"
# -ss output wraps lists in {}
assert_contains "list format" "{"

cu_json tell Reminders 'get name of every list'
assert_ok "tell Reminders every list"

section "tell — System Events"

cu_json tell "System Events" 'get dark mode of appearance preferences'
assert_ok "tell System Events dark mode"
DARK=$(json_get '.result' || echo "")
if [[ "$DARK" == "true" || "$DARK" == "false" ]]; then
  _pass "dark mode is boolean ($DARK)"
else
  _fail "dark mode boolean" "got: $DARK"
fi

section "tell — empty result"

cu_json tell Finder 'get selection'
assert_ok "tell Finder selection (may be empty)"

section "tell — write and cleanup"

cu_json tell Notes 'make new note with properties {name:"cu-test-tell-sh", body:"test"}'
assert_ok "create note"
cu_json tell Notes 'delete note "cu-test-tell-sh"'
assert_ok "delete note"

section "tell — full tell block passthrough"

cu_json tell Finder 'tell application "Finder" to get name of home'
assert_ok "full tell block passthrough"

section "tell — error handling"

cu_json tell Finder 'get name of window 99999'
assert_fail "invalid window index"

section "tell — timeout"

cu_json tell Finder 'delay 20' --timeout 2
assert_fail "timeout kills long script"

section "tell — human mode"

cu_human tell Finder 'get version'
assert_exit_zero "tell human exits 0"
# Should show the version string
if [[ -n "$OUT" ]]; then
  _pass "human output non-empty"
else
  _fail "human output" "empty output"
fi

summary
