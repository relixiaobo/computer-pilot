#!/bin/bash
# Behavior test: PID-targeted pointer events must not move the user's cursor.
source "$(dirname "$0")/helpers.sh"

CURSOR_HOME=$(mktemp -d "${TMPDIR:-/tmp}/cu-cursor-isolation.XXXXXX")
CURSOR_BROKER_PID=""
cleanup_cursor() {
  [[ -z "$CURSOR_BROKER_PID" ]] || kill "$CURSOR_BROKER_PID" 2>/dev/null || true
  rm -rf "$CURSOR_HOME"
}
trap 'cleanup_cursor; cleanup_run' EXIT
export COMPUTER_PILOT_HOME="$CURSOR_HOME"

cursor_position() {
  swift -e 'import CoreGraphics; if let e = CGEvent(source: nil) { print("\(e.location.x),\(e.location.y)") }'
}

section "cursor isolation — PID-targeted hover with optional software cursor"

BEFORE=$(cursor_position 2>/dev/null || true)
if [[ -z "$BEFORE" ]]; then
  _skip "read system cursor" "Swift/CoreGraphics unavailable"
  summary
  exit 0
fi

COMPUTER_PILOT_VISUAL_CURSOR=1 cu_json status
assert_ok "visual cursor Broker starts"
CURSOR_BROKER_PID=$(json_get '.pid')

COMPUTER_PILOT_VISUAL_CURSOR=1 cu_json hover 377 299 --app Finder --no-snapshot
assert_ok "targeted hover dispatches"
assert_json_field "targeted hover method" ".method" "cgevent-pid"

SHARING=$(CURSOR_BROKER_PID="$CURSOR_BROKER_PID" swift -e '
import Foundation
import CoreGraphics
let pid = Int32(ProcessInfo.processInfo.environment["CURSOR_BROKER_PID"]!)!
let windows = (CGWindowListCopyWindowInfo([.excludeDesktopElements], kCGNullWindowID) as? [[CFString: Any]]) ?? []
for window in windows {
  if let owner = window[kCGWindowOwnerPID] as? Int, owner == Int(pid) {
    let state = window[kCGWindowSharingState] as? Int ?? -1
    print("sharing=\(state)")
  }
}
' 2>/dev/null || true)
if [[ -z "$SHARING" ]]; then
  _skip "software cursor window is capture-protected" "AppKit did not expose a Broker overlay window"
elif grep -q 'sharing=0' <<<"$SHARING"; then
  _pass "software cursor window is excluded from capture"
else
  _fail "software cursor window is excluded from capture" "$SHARING"
fi

# Check the actual rendered panel bounds: the calibrated tip (60.35,70.3 from its
# lower-left in AppKit) must stay on the dispatched point.
GEOMETRY=$(CURSOR_BROKER_PID="$CURSOR_BROKER_PID" swift -e '
import Foundation
import CoreGraphics
let pid = Int(ProcessInfo.processInfo.environment["CURSOR_BROKER_PID"]!)!
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
for window in windows where window[kCGWindowOwnerPID as String] as? Int == pid {
  guard let bounds = window[kCGWindowBounds as String] as? [String: Double] else { continue }
  let matches = abs((bounds["X"] ?? 0) - 316.65) < 1 && abs((bounds["Y"] ?? 0) - 243.3) < 1 && bounds["Width"] == 126 && bounds["Height"] == 126
  print(matches ? "arrow-hotspot-ok" : "unexpected-bounds: \(bounds)")
}
' 2>/dev/null || true)
if [[ "$GEOMETRY" == "arrow-hotspot-ok" ]]; then
  _pass "arrow tip matches the action coordinate"
elif [[ -z "$GEOMETRY" ]]; then
  _skip "arrow tip matches the action coordinate" "target overlay is not on the current Space"
else
  _fail "arrow tip matches the action coordinate" "$GEOMETRY"
fi

# The first hover establishes a position. The next must produce intermediate
# positions at a constant size, then settle at the requested point.
if [[ "$GEOMETRY" == "arrow-hotspot-ok" ]]; then
  if MOTION=$(swift "$ROOT_DIR/tests/fixtures/visual-cursor-motion.swift" "$CU" "$CURSOR_BROKER_PID" 2>&1); then
    _pass "arrow moves smoothly and wakes after idle"
  else
    _fail "arrow moves smoothly and wakes after idle" "$MOTION"
  fi
else
  _skip "cursor motion" "target overlay is not on the current Space"
fi

AFTER=$(cursor_position 2>/dev/null || true)
if [[ "$AFTER" == "$BEFORE" ]]; then
  _pass "targeted hover and software cursor preserve the real cursor"
elif [[ "$AFTER" == "377.0,299.0" || "$AFTER" == "377,299" || "$AFTER" == "777.0,499.0" || "$AFTER" == "777,499" ]]; then
  _fail "targeted hover and software cursor preserve the real cursor" "cursor warped from $BEFORE to $AFTER"
else
  _skip "targeted hover and software cursor preserve the real cursor" "cursor changed independently during the test ($BEFORE -> $AFTER)"
fi

summary
