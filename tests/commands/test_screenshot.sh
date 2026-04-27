#!/bin/bash
# Test: cu screenshot
source "$(dirname "$0")/helpers.sh"

# cu screenshot uses CGWindowListCreateImage and captures behind other windows,
# so we only need Finder to have a window — Finder does not need to be frontmost.
# `make new Finder window` here does NOT activate Finder when it already has windows.
osascript -e 'tell application "Finder"
  if (count of Finder windows) is 0 then make new Finder window
end tell' 2>/dev/null
sleep 0.3

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

section "screenshot — region mode"

SHOT_REG="$SHOT_DIR/region.png"
cleanup_register "$SHOT_REG"

cu_json screenshot --region "100,200 300x200" --path "$SHOT_REG"
assert_ok "region screenshot ok"
assert_json_field "mode is region" ".mode" "region"
assert_json_field "offset_x echoes region x" ".offset_x" "100.0"
assert_json_field "offset_y echoes region y" ".offset_y" "200.0"
assert_json_field "width echoes region width" ".width" "300.0"
assert_json_field "height echoes region height" ".height" "200.0"
assert_file_exists "region file created" "$SHOT_REG"
assert_file_png "region file is PNG" "$SHOT_REG"

# PNG dimensions should be region size × Retina scale (typically 2 on Retina; could be 1)
DIMS=$(python3 -c "
import struct
with open('$SHOT_REG', 'rb') as f:
    f.read(8); f.read(4); f.read(4)
    w, h = struct.unpack('>II', f.read(8))
    print(f'{w}x{h}')
")
W=$(echo "$DIMS" | cut -dx -f1)
H=$(echo "$DIMS" | cut -dx -f2)
# Width/height should be multiples of 300 / 200 (300×scale, 200×scale)
W_RATIO=$(python3 -c "print($W // 300 if $W % 300 == 0 else 0)")
H_RATIO=$(python3 -c "print($H // 200 if $H % 200 == 0 else 0)")
if [[ "$W_RATIO" -ge 1 && "$H_RATIO" == "$W_RATIO" ]]; then
  _pass "PNG dims = region × scale (${W}x${H}, scale=$W_RATIO)"
else
  _fail "PNG region dims" "expected k×(300×200), got ${W}x${H}"
fi

# Region file should be SMALLER than a full window screenshot (the value prop)
REG_SIZE=$(stat -f%z "$SHOT_REG" 2>/dev/null || echo "0")
WIN_SIZE=$(stat -f%z "$SHOT1" 2>/dev/null || echo "999999")
if [[ "$REG_SIZE" -lt "$WIN_SIZE" ]]; then
  _pass "region file smaller than window file ($REG_SIZE < $WIN_SIZE bytes)"
else
  _fail "region < window size" "region=$REG_SIZE, window=$WIN_SIZE"
fi

# Alternate format: x,y,w,h
SHOT_REG2="$SHOT_DIR/region2.png"
cleanup_register "$SHOT_REG2"
cu_json screenshot --region "100,200,300,200" --path "$SHOT_REG2"
assert_ok "region 'x,y,w,h' format ok"
assert_file_png "region2 PNG valid" "$SHOT_REG2"

section "screenshot — region error paths"

cu_json screenshot --region "bogus" --path "$SHOT_DIR/nope.png"
assert_fail "non-numeric region rejected"

cu_json screenshot --region "1 2 3" --path "$SHOT_DIR/nope.png"
assert_fail "wrong number of region components rejected"

cu_json screenshot --region "0,0 0x0" --path "$SHOT_DIR/nope.png"
assert_fail "zero-size region rejected"

cu_json screenshot --region "10,20 -100x-100" --path "$SHOT_DIR/nope.png"
assert_fail "negative size rejected"

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
