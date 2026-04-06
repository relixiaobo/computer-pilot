#!/bin/bash
# Test: cu hover
source "$(dirname "$0")/helpers.sh"

section "hover — basic"

cu_json "hover 500 300"
assert_ok "hover at (500, 300)"
assert_json_field "x coord" ".x" "500.0"
assert_json_field "y coord" ".y" "300.0"

cu_json "hover 0 0"
assert_ok "hover at origin (0, 0)"

cu_json "hover 1920 1080"
assert_ok "hover at large coords"

section "hover — human mode"

cu_human "hover 250 250"
assert_exit_zero "hover human exits 0"
assert_contains "shows hover info" "Hover"

summary
