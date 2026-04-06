#!/bin/bash
# Test: cu defaults (read/write macOS preferences)
source "$(dirname "$0")/helpers.sh"

section "defaults read"

cu_json defaults read com.apple.dock autohide
assert_ok "read dock autohide"
assert_json_field_exists "value present" ".value"

cu_json defaults read com.apple.finder ShowPathbar
assert_ok "read finder ShowPathbar"

section "defaults read — domain only"

cu_json defaults read com.apple.dock
assert_ok "read full domain"

section "defaults read — non-existent key"

cu_json defaults read com.apple.dock ZZZZNONEXISTENT99
assert_fail "non-existent key fails"

section "defaults write + read back"

# Save original, write, read, restore
ORIG=$(./target/release/cu --human defaults read com.apple.dock show-recents 2>/dev/null || echo "1")
cu_json defaults write com.apple.dock show-recents -bool true
assert_ok "write dock show-recents"

cu_json defaults read com.apple.dock show-recents
assert_ok "read back after write"

section "defaults — human mode"

cu_human defaults read com.apple.dock autohide
assert_exit_zero "defaults human exits 0"

section "defaults — bad action"

cu_json defaults delete com.apple.dock autohide
assert_fail "unknown action fails"

summary
