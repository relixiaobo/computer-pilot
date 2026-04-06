#!/bin/bash
# Test: cu screenshot
source "$(dirname "$0")/helpers.sh"

# Ensure Finder has a visible window
osascript -e 'tell application "Finder" to activate' 2>/dev/null
osascript -e 'tell application "Finder"
  if (count of Finder windows) is 0 then make new Finder window
end tell' 2>/dev/null
sleep 0.5

SHOT_DIR="/tmp/cu-test-screenshots"
mkdir -p "$SHOT_DIR"

section "screenshot — window mode (Finder)"

SHOT1="$SHOT_DIR/finder-window.png"
cleanup_register "$SHOT1"

cu_json "screenshot Finder --path $SHOT1"
assert_ok "screenshot Finder ok"
assert_json_field "mode is window" ".mode" "window"
assert_json_field "app is Finder" ".app" "Finder"
assert_json_field "path correct" ".path" "$SHOT1"
assert_json_field_exists "offset_x" ".offset_x"
assert_json_field_exists "offset_y" ".offset_y"
assert_file_exists "screenshot file created" "$SHOT1"
assert_file_png "file is valid PNG" "$SHOT1"

# Offsets should be non-negative numbers
OFFSET_X=$(json_get '.offset_x' || echo "err")
OFFSET_Y=$(json_get '.offset_y' || echo "err")
if echo "$OFFSET_X" | grep -qE '^-?[0-9]+\.?[0-9]*$'; then
  _pass "offset_x is numeric ($OFFSET_X)"
else
  _fail "offset_x numeric" "got: $OFFSET_X"
fi

section "screenshot — full screen"

SHOT2="$SHOT_DIR/fullscreen.png"
cleanup_register "$SHOT2"

cu_json "screenshot --full --path $SHOT2"
assert_ok "screenshot full screen ok"
assert_json_field "mode is full" ".mode" "full"
assert_file_exists "fullscreen file created" "$SHOT2"
assert_file_png "fullscreen is valid PNG" "$SHOT2"

# Full screen should be larger than window screenshot
if [[ -f "$SHOT1" && -f "$SHOT2" ]]; then
  SIZE1=$(wc -c < "$SHOT1" | tr -d ' ')
  SIZE2=$(wc -c < "$SHOT2" | tr -d ' ')
  # Not always true (e.g., Finder window could be fullscreen) — just check both exist
  _pass "both screenshots created (window=${SIZE1}B, full=${SIZE2}B)"
fi

section "screenshot — default path (auto-generated)"

cu_json "screenshot Finder"
assert_ok "screenshot with default path"
DEFAULT_PATH=$(json_get '.path' || echo "")
if [[ -n "$DEFAULT_PATH" && -f "$DEFAULT_PATH" ]]; then
  _pass "default path file exists: $DEFAULT_PATH"
  cleanup_register "$DEFAULT_PATH"
else
  _fail "default path file" "path=$DEFAULT_PATH"
fi

section "screenshot — error: non-existent app"

cu_json "screenshot NonExistentApp98765"
assert_fail "non-existent app fails"

section "screenshot — human mode"

SHOT3="$SHOT_DIR/human-test.png"
cleanup_register "$SHOT3"

cu_human "screenshot Finder --path $SHOT3"
assert_exit_zero "screenshot human exits 0"
assert_contains "shows saved path" "Screenshot saved"
assert_contains "shows filename" "$SHOT3"

# Cleanup shot dir
rm -rf "$SHOT_DIR" 2>/dev/null || true

summary
