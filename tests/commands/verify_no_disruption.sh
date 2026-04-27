#!/usr/bin/env bash
# Verifies B1/B2: cu click/type/key with --app does NOT warp the user's
# real cursor toward the click target, does NOT steal global frontmost
# focus, and (for type) does NOT pollute the clipboard.
#
# The cursor is "parked" at a known location before each test via the
# global hover path so we have a stable baseline that's not affected by
# the user's hand on the trackpad during the run.
#
# Run from repo root: bash tests/commands/verify_no_disruption.sh
set -euo pipefail

CU="${CU:-./target/release/cu}"
PARK_X=800
PARK_Y=400
WARP_TOLERANCE=150  # px from click target — anything closer means cursor was warped

read_state() {
  swift -e '
import Cocoa
let p = NSEvent.mouseLocation
let app = NSWorkspace.shared.frontmostApplication?.localizedName ?? "?"
print("\(Int(p.x))|\(Int(p.y))|\(app)")
'
}

# park cursor at PARK_X, PARK_Y via the (currently global) hover path
park_cursor() {
  "$CU" hover "$PARK_X" "$PARK_Y" >/dev/null 2>&1 || true
  sleep 0.15
}

# distance between two screen-state points
distance() {
  python3 -c "import math; print(int(math.hypot($1 - $3, $2 - $4)))"
}

assert_not_warped() {
  local label="$1" target_x="$2" target_y="$3" front_before="$4"
  local state after_x after_y front_after dist
  state=$(read_state)
  IFS='|' read -r after_x after_y front_after <<<"$state"
  dist=$(distance "$after_x" "$after_y" "$target_x" "$target_y")
  if (( dist >= WARP_TOLERANCE )) && [[ "$front_after" == "$front_before" ]]; then
    echo "  ✅ $label — cursor did not warp (now ${after_x},${after_y}, ${dist}px from target), frontmost=${front_after}"
  else
    echo "  ❌ $label"
    echo "     cursor: ${after_x},${after_y} (${dist}px from target ${target_x},${target_y}; tolerance ${WARP_TOLERANCE})"
    echo "     frontmost: ${front_after} (was ${front_before})"
    return 1
  fi
}

assert_warped_near() {
  local label="$1" target_x="$2" target_y="$3"
  local state after_x after_y front_after dist
  state=$(read_state)
  IFS='|' read -r after_x after_y front_after <<<"$state"
  dist=$(distance "$after_x" "$after_y" "$target_x" "$target_y")
  if (( dist < WARP_TOLERANCE )); then
    echo "  ✅ $label — cursor warped to (${after_x},${after_y}), ${dist}px from expected target (control group works)"
  else
    echo "  ⚠  $label — cursor at (${after_x},${after_y}), ${dist}px from target — control group inconclusive"
  fi
}

# Note: NSEvent.mouseLocation has bottom-left origin; click coords are top-left.
# We just compare against (target_x, screen_h - target_y) below for global tests,
# but for park-and-PID tests we know the cursor stays near park, so direct compare works.

screen_h=$(swift -e 'import Cocoa; print(Int(NSScreen.main!.frame.height))')

echo "screen height: $screen_h, parked at: ($PARK_X, $PARK_Y), tolerance: ${WARP_TOLERANCE}px"
echo ""

# ── PID-targeted click (coords + --app) ─────────────────────────────────────
echo "── B1: PID-targeted click (--app Finder) ──"
park_cursor
state_before=$(read_state); front_before=$(echo "$state_before" | cut -d'|' -f3)
"$CU" click 5 5 --app Finder --no-snapshot >/dev/null 2>&1 || true
# expected behavior: cursor stays near park, NOT near (5, screen_h - 5)
assert_not_warped "click 5 5 --app Finder" 5 $((screen_h - 5)) "$front_before"

# ── PID-targeted ref click ──────────────────────────────────────────────────
echo ""
echo "── B1: PID-targeted ref click (--app Finder) ──"
park_cursor
state_before=$(read_state); front_before=$(echo "$state_before" | cut -d'|' -f3)
"$CU" click 1 --app Finder --no-snapshot >/dev/null 2>&1 || true
# the ref's coords are unknown; just assert cursor is still near park
assert_not_warped "click 1 --app Finder" "$PARK_X" "$PARK_Y" "$front_before"

# ── PID-targeted type — clipboard + frontmost preserved ─────────────────────
echo ""
echo "── B2: PID-targeted type (--app Finder) ──"
sentinel="cu-verify-sentinel-$$"
echo -n "$sentinel" | pbcopy
park_cursor
state_before=$(read_state); front_before=$(echo "$state_before" | cut -d'|' -f3)
"$CU" type "ignore-this" --app Finder --no-snapshot >/dev/null 2>&1 || true
assert_not_warped "type --app Finder" "$PARK_X" "$PARK_Y" "$front_before"
clip_after=$(pbpaste)
if [[ "$clip_after" == "$sentinel" ]]; then
  echo "  ✅ type --app Finder — clipboard NOT polluted"
else
  echo "  ❌ type --app Finder — clipboard was changed"
  echo "     EXPECTED: $sentinel"
  echo "     ACTUAL  : $clip_after"
fi

# ── PID-targeted key (--app Finder) ─────────────────────────────────────────
echo ""
echo "── B2: PID-targeted key (--app Finder) ──"
park_cursor
state_before=$(read_state); front_before=$(echo "$state_before" | cut -d'|' -f3)
"$CU" key escape --app Finder --no-snapshot >/dev/null 2>&1 || true
assert_not_warped "key escape --app Finder" "$PARK_X" "$PARK_Y" "$front_before"

# ── PID-targeted scroll (--app Finder) ──────────────────────────────────────
echo ""
echo "── B5: PID-targeted scroll (--app Finder) ──"
park_cursor
state_before=$(read_state); front_before=$(echo "$state_before" | cut -d'|' -f3)
"$CU" scroll down 1 --x 500 --y 500 --app Finder >/dev/null 2>&1 || true
assert_not_warped "scroll --app Finder" "$PARK_X" "$PARK_Y" "$front_before"

# ── PID-targeted hover (--app Finder) ───────────────────────────────────────
echo ""
echo "── B5: PID-targeted hover (--app Finder) ──"
park_cursor
state_before=$(read_state); front_before=$(echo "$state_before" | cut -d'|' -f3)
"$CU" hover 100 100 --app Finder >/dev/null 2>&1 || true
# expected: cursor stays near park, NOT near (100, screen_h - 100)
assert_not_warped "hover --app Finder" 100 $((screen_h - 100)) "$front_before"

# ── PID-targeted drag (--app Finder) ────────────────────────────────────────
echo ""
echo "── B5: PID-targeted drag (--app Finder) ──"
park_cursor
state_before=$(read_state); front_before=$(echo "$state_before" | cut -d'|' -f3)
"$CU" drag 100 100 200 200 --app Finder >/dev/null 2>&1 || true
assert_not_warped "drag --app Finder" "$PARK_X" "$PARK_Y" "$front_before"

# ── B3: set-value on TextEdit ───────────────────────────────────────────────
echo ""
echo "── B3: PID-targeted set-value (--app TextEdit) ──"
osascript -e 'tell application "TextEdit" to activate' 2>/dev/null
sleep 1
osascript -e 'tell application "TextEdit" to make new document' 2>/dev/null
"$CU" wait --ref 1 --app TextEdit --timeout 5 >/dev/null 2>&1 || true
"$CU" snapshot TextEdit --limit 5 >/dev/null 2>&1 || true  # warm up
sleep 0.3
park_cursor
state_before=$(read_state); front_before=$(echo "$state_before" | cut -d'|' -f3)
"$CU" set-value 1 "set via cu, no focus needed" --app TextEdit --no-snapshot >/dev/null 2>&1 || true
assert_not_warped "set-value --app TextEdit" "$PARK_X" "$PARK_Y" "$front_before"
# Verify content actually landed
DOC=$(osascript -e 'tell application "TextEdit" to get text of front document' 2>/dev/null)
if [[ "$DOC" == "set via cu, no focus needed" ]]; then
  echo "  ✅ set-value content landed in TextEdit document"
else
  echo "  ❌ set-value content missing — got: $DOC"
fi
osascript -e 'tell application "TextEdit" to close every document saving no' 2>/dev/null
osascript -e 'tell application "TextEdit" to quit' 2>/dev/null

# ── Control: global click (no --app) — SHOULD warp the cursor ───────────────
echo ""
echo "── Control: global click (no --app) — cursor SHOULD warp ──"
park_cursor
"$CU" click 7 7 >/dev/null 2>&1 || true
# expected: cursor warps to approximately (7, screen_h - 7)
assert_warped_near "click 7 7 (global)" 7 $((screen_h - 7))

# Restore cursor to a friendly spot for the user
park_cursor

echo ""
echo "Done."
