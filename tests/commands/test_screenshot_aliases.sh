#!/bin/bash
# Test: cu screenshot accepts --app and --output aliases (G2).
#
# Why this exists: agents repeatedly tried `cu screenshot --app Foo --out path`
# because every other cu command takes --app and --path/--output. The
# positional-only / --path-only layout was an inconsistency that cost real
# debugging time in a live agent session. This locks the new aliases in.
source "$(dirname "$0")/helpers.sh"

section "screenshot aliases — positional form"

cu_json screenshot Finder --path /tmp/cu-alias-pos.png
assert_ok "positional Finder + --path ok"
assert_file_png "PNG written via positional + --path" /tmp/cu-alias-pos.png
rm -f /tmp/cu-alias-pos.png

section "screenshot aliases — --app form (the consistency fix)"

cu_json screenshot --app Finder --path /tmp/cu-alias-app.png
assert_ok "--app Finder + --path ok"
assert_file_png "PNG written via --app + --path" /tmp/cu-alias-app.png
rm -f /tmp/cu-alias-app.png

section "screenshot aliases — --output as --path"

cu_json screenshot Finder --output /tmp/cu-alias-out.png
assert_ok "positional + --output ok"
assert_file_png "PNG written via --output" /tmp/cu-alias-out.png
rm -f /tmp/cu-alias-out.png

section "screenshot aliases — --app + --output combined"

cu_json screenshot --app Finder --output /tmp/cu-alias-both.png
assert_ok "--app + --output ok"
assert_file_png "PNG written via --app + --output" /tmp/cu-alias-both.png
rm -f /tmp/cu-alias-both.png

section "screenshot aliases — positional + --app conflicts"

cu_json screenshot Finder --app Finder --path /tmp/cu-alias-conflict.png
# clap's conflicts_with should reject this with non-zero exit
if [[ "$EXIT" -ne 0 ]]; then
  _pass "positional + --app rejected (clap conflict)"
else
  _fail "positional + --app rejected" "expected non-zero exit, got $EXIT"
fi
rm -f /tmp/cu-alias-conflict.png

summary
