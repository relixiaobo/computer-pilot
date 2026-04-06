#!/bin/bash
# Test: cu setup
source "$(dirname "$0")/helpers.sh"

section "setup — JSON mode"

cu_json "setup"
assert_ok "setup returns ok"
assert_json_field "version present" ".version" "$(grep '^version' "$ROOT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
assert_json_field "platform is macos" ".platform" "macos"
assert_json_field_exists "accessibility field" ".accessibility"
assert_json_field_exists "screen_recording field" ".screen_recording"
assert_json_field_exists "ready field" ".ready"
assert_json_field_exists "automation field" ".automation"
assert_json_field_exists "scripting_ready field" ".scripting_ready"

section "setup — human mode"

cu_human "setup"
assert_exit_zero "setup human exits 0"
assert_contains "shows version" "cu v"
assert_contains "shows accessibility" "Accessibility"
assert_contains "shows screen recording" "Screen Recording"

section "setup — permissions granted"

cu_json "setup"
ACCESSIBILITY=$(json_get '.accessibility')
SCREEN_REC=$(json_get '.screen_recording')
READY=$(json_get '.ready')

if [[ "$ACCESSIBILITY" == "true" ]]; then
  _pass "accessibility is granted"
else
  _skip "accessibility not granted" "grant in System Settings to test fully"
fi

if [[ "$SCREEN_REC" == "true" ]]; then
  _pass "screen recording is granted"
else
  _skip "screen recording not granted" "grant in System Settings to test fully"
fi

if [[ "$READY" == "true" ]]; then
  _pass "ready = true (both permissions)"
else
  _skip "not ready" "need both permissions"
fi

summary
