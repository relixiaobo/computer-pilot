#!/bin/bash
# Behavior test: PID-targeted pointer events must not move the user's cursor.
source "$(dirname "$0")/helpers.sh"

cursor_position() {
  swift -e 'import CoreGraphics; if let e = CGEvent(source: nil) { print("\(e.location.x),\(e.location.y)") }'
}

section "cursor isolation — PID-targeted hover"

BEFORE=$(cursor_position 2>/dev/null || true)
if [[ -z "$BEFORE" ]]; then
  _skip "read system cursor" "Swift/CoreGraphics unavailable"
  summary
  exit 0
fi

cu_json hover 377 299 --app Finder --no-snapshot
assert_ok "targeted hover dispatches"
assert_json_field "targeted hover method" ".method" "cgevent-pid"

AFTER=$(cursor_position 2>/dev/null || true)
if [[ "$AFTER" == "$BEFORE" ]]; then
  _pass "targeted hover preserves the real cursor"
elif [[ "$AFTER" == "377.0,299.0" || "$AFTER" == "377,299" ]]; then
  _fail "targeted hover preserves the real cursor" "cursor warped from $BEFORE to $AFTER"
else
  _skip "targeted hover preserves the real cursor" "cursor changed independently during the test ($BEFORE -> $AFTER)"
fi

summary
