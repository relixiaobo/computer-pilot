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

# 30s: Reminders can take >10s to cold-launch and answer its first Apple event.
cu_json tell Reminders 'get name of every list' --timeout 30
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

NOTE_NAME="cu-test-tell-sh-$$"
cu_json tell Notes "make new note with properties {name:\"$NOTE_NAME\", body:\"test\"}" --timeout 30
assert_ok "create note"
cu_json tell Notes "delete note \"$NOTE_NAME\"" --timeout 30
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

section "tell — disruptive AppleScript refused"

# `activate` steals the user's frontmost app. Must refuse before execution
# (nothing is activated — safe to assert in the non-interactive suite).
cu_json tell Finder 'activate'
assert_fail "activate is refused"
if echo "$ERR$OUT" | grep -q 'refusing to run AppleScript containing `activate`'; then
  _pass "activate refusal names the construct"
else
  _fail "activate refusal message" "got: ${ERR:0:120}${OUT:0:120}"
fi

# System Events keystroke/key code send global keyboard input to whatever the
# user has focused — refused even when nested in an inner tell block.
cu_json tell "System Events" 'keystroke "a"'
assert_fail "keystroke is refused"

cu_json tell Finder 'tell application "System Events" to key code 36'
assert_fail "nested key code is refused"

# AppleScript tolerates flexible whitespace — the lint must too.
cu_json tell "System Events" 'key    code 36'
assert_fail "multi-space key code is refused"

cu_json tell "System Events" 'key
code 36'
assert_fail "newline-split key code is refused"

# key down / key up hold a key globally (worst case: stuck modifier).
cu_json tell "System Events" 'key down command'
assert_fail "key down is refused"

cu_json tell "System Events" 'key up command'
assert_fail "key up is refused"

# Word-boundary matching: 'activate' inside a longer word / string literal
# must not trigger the lint.
cu_json tell Finder 'get "xactivatey"'
assert_ok "word-boundary: embedded 'activate' substring is not refused"

# --allow-disruptive bypasses the lint (benign expression — nothing is
# actually activated in the non-interactive suite).
cu_json tell Finder 'get version' --allow-disruptive
assert_ok "--allow-disruptive flag is accepted"

summary
