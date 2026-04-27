#!/bin/bash
# Test: cu snapshot --annotated (A3) — annotated screenshot for VLM agents
source "$(dirname "$0")/helpers.sh"

OUT_PATH="/tmp/cu-test-annotated-$$.png"
trap 'rm -f "$OUT_PATH"' EXIT

section "snapshot --annotated — basic JSON output"

cu_json snapshot Finder --limit 30 --annotated --output "$OUT_PATH"
assert_ok "snapshot --annotated ok"
assert_json_field "annotated_screenshot path returned" ".annotated_screenshot" "$OUT_PATH"
assert_json_field_exists "image_scale field present" ".image_scale"
assert_json_field_exists "elements still present" ".elements"

# image_scale should be > 0 (typically 1 or 2 on Retina)
SCALE=$(json_get '.image_scale' || echo "0")
SCALE_OK=$(python3 -c "print('yes' if $SCALE > 0 else 'no')")
if [[ "$SCALE_OK" == "yes" ]]; then
  _pass "image_scale > 0 ($SCALE)"
else
  _fail "image_scale > 0" "got: $SCALE"
fi

section "snapshot --annotated — file output"

assert_file_exists "PNG file written" "$OUT_PATH"
assert_file_png "output is a valid PNG" "$OUT_PATH"

# File should be reasonably sized (a real screenshot, not a tiny placeholder)
SIZE=$(stat -f%z "$OUT_PATH" 2>/dev/null || echo "0")
if [[ "$SIZE" -gt 10000 ]]; then
  _pass "PNG size > 10KB ($SIZE bytes)"
else
  _fail "PNG size > 10KB" "got: $SIZE bytes"
fi

# Image dimensions should match window dimensions × scale (Retina test)
DIMS=$(python3 -c "
import struct
with open('$OUT_PATH', 'rb') as f:
    f.read(8)            # PNG signature
    f.read(4)            # IHDR length
    f.read(4)            # 'IHDR'
    w, h = struct.unpack('>II', f.read(8))
    print(f'{w}x{h}')
")
echo "  PNG dimensions: $DIMS"
WIDTH=$(echo "$DIMS" | cut -dx -f1)
HEIGHT=$(echo "$DIMS" | cut -dx -f2)
if [[ "$WIDTH" -ge 100 && "$HEIGHT" -ge 100 ]]; then
  _pass "PNG is reasonably sized (${WIDTH}x${HEIGHT})"
else
  _fail "PNG dimensions" "${WIDTH}x${HEIGHT}"
fi

section "snapshot --annotated — default output path"

# Without --output, should default to /tmp/cu-annotated-<ts>.png
cu_json snapshot Finder --limit 5 --annotated
assert_ok "default-path --annotated ok"
DEFAULT_PATH=$(json_get '.annotated_screenshot' || echo "")
if [[ "$DEFAULT_PATH" == /tmp/cu-annotated-*.png ]]; then
  _pass "default path matches /tmp/cu-annotated-*.png ($DEFAULT_PATH)"
else
  _fail "default path pattern" "got: $DEFAULT_PATH"
fi
if [[ -f "$DEFAULT_PATH" ]]; then
  _pass "default path file exists"
  rm -f "$DEFAULT_PATH"
else
  _fail "default path file exists" "missing $DEFAULT_PATH"
fi

section "snapshot --annotated — coexists with normal snapshot"

# Plain snapshot still works (no annotated_screenshot field)
cu_json snapshot Finder --limit 10
assert_ok "plain snapshot still ok"
NO_ANNO=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('absent' if 'annotated_screenshot' not in d else 'present')
" 2>/dev/null || echo "error")
if [[ "$NO_ANNO" == "absent" ]]; then
  _pass "plain snapshot has no annotated_screenshot field"
else
  _fail "plain snapshot field absence" "$NO_ANNO"
fi

section "snapshot --annotated — human mode"

cu_human snapshot Finder --limit 5 --annotated --output "$OUT_PATH"
assert_exit_zero "human --annotated exits 0"
if echo "$OUT" | grep -q "Annotated screenshot:"; then
  _pass "human mode prints 'Annotated screenshot:' line"
else
  _fail "human annotated line" "${OUT:0:200}"
fi

section "snapshot --annotated — error path: non-existent app"

cu_json snapshot NonExistentApp99887 --limit 5 --annotated
assert_fail "non-existent app fails (no window to annotate)"

summary
