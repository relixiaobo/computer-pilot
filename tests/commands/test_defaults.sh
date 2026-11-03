#!/bin/bash
# Test cu defaults in an owned domain; user preferences may be absent by default.
source "$(dirname "$0")/helpers.sh"
DOMAIN="com.computer-pilot.tests.defaults.$$"
trap '/usr/bin/defaults delete "$DOMAIN" >/dev/null 2>&1 || true' EXIT

section "defaults write + exact read back"
cu_json defaults write "$DOMAIN" enabled -bool true
assert_ok "write boolean"
cu_json defaults read "$DOMAIN" enabled
assert_ok "read boolean"
assert_json_field "boolean persisted" ".value" "1"
cu_json defaults write "$DOMAIN" count -int 42
assert_ok "write integer"
cu_json defaults read "$DOMAIN" count
assert_ok "read integer"
assert_json_field "integer persisted" ".value" "42"
cu_json defaults write "$DOMAIN" label -string "Pilot fixture value"
assert_ok "write string"
cu_json defaults read "$DOMAIN" label
assert_ok "read string"
assert_json_field "string persisted" ".value" "Pilot fixture value"

section "defaults read — domain only"
cu_json defaults read "$DOMAIN"
assert_ok "read full domain"
assert_contains "domain contains owned key" "enabled"

section "defaults read — non-existent key"
cu_json defaults read "$DOMAIN" ZZZZNONEXISTENT99
assert_fail "non-existent key fails"

section "defaults — human mode"
cu_human defaults read "$DOMAIN" count
assert_exit_zero "defaults human exits 0"
assert_contains "human read returns persisted value" "42"

section "defaults — bad action"
cu_json defaults delete "$DOMAIN" enabled
assert_fail "unknown action fails"
summary
