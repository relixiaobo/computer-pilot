#!/bin/bash
# Test: cu snapshot --with-screenshot (A10) — single-call tree+image fusion
# NOTE: helpers.sh uses a global variable named OUT for cu_json output.
# We use PNG_PATH for the file path to avoid the name clash.
source "$(dirname "$0")/helpers.sh"

PNG_PATH="/tmp/cu-test-fused-$$.png"
trap 'rm -f /tmp/cu-test-fused-*.png' EXIT

section "snapshot --with-screenshot — basic JSON"

cu_json snapshot Finder --limit 30 --with-screenshot --output "$PNG_PATH"
assert_ok "snapshot --with-screenshot ok"
assert_json_field "screenshot path returned" ".screenshot" "$PNG_PATH"
assert_json_field_exists "image_scale field" ".image_scale"
assert_json_field_exists "elements still present" ".elements"
assert_json_field_exists "window_frame still present" ".window_frame"

NO_ANNO=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin)
print('absent' if 'annotated_screenshot' not in d else 'present')
" 2>/dev/null || echo "error")
if [[ "$NO_ANNO" == "absent" ]]; then
  _pass "no annotated_screenshot field on plain --with-screenshot"
else
  _fail "no annotated field" "$NO_ANNO"
fi

section "snapshot --with-screenshot — file written"

assert_file_exists "PNG file written" "$PNG_PATH"
assert_file_png "output is valid PNG" "$PNG_PATH"

DIMS=$(python3 -c "
import struct
with open('$PNG_PATH', 'rb') as f:
    f.read(8); f.read(4); f.read(4)
    w, h = struct.unpack('>II', f.read(8))
    print(f'{w}x{h}')
")
W=$(echo "$DIMS" | cut -dx -f1)
H=$(echo "$DIMS" | cut -dx -f2)
if [[ "$W" -ge 100 && "$H" -ge 100 ]]; then
  _pass "PNG reasonably sized (${W}x${H})"
else
  _fail "PNG dimensions" "${W}x${H}"
fi

section "snapshot --with-screenshot — default output path"

cu_json snapshot Finder --limit 5 --with-screenshot
assert_ok "default-path --with-screenshot ok"
DEFAULT_PATH=$(json_get '.screenshot' || echo "")
if [[ "$DEFAULT_PATH" == /tmp/cu-snapshot-*.png ]]; then
  _pass "default path matches /tmp/cu-snapshot-*.png ($DEFAULT_PATH)"
else
  _fail "default path pattern" "got: $DEFAULT_PATH"
fi
if [[ -f "$DEFAULT_PATH" ]]; then
  _pass "default path file exists"
  rm -f "$DEFAULT_PATH"
else
  _fail "default path file" "missing"
fi

section "snapshot — plain (no --with-screenshot) has no screenshot field"

cu_json snapshot Finder --limit 5
NO_SHOT=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin)
print('absent' if 'screenshot' not in d and 'image_scale' not in d else 'present')
" 2>/dev/null || echo "error")
if [[ "$NO_SHOT" == "absent" ]]; then
  _pass "plain snapshot has neither screenshot nor image_scale"
else
  _fail "plain snapshot fields absent" "$NO_SHOT"
fi

section "snapshot --annotated + --with-screenshot — annotated wins"

cu_json snapshot Finder --limit 5 --annotated --with-screenshot --output /tmp/cu-test-fused-both.png
assert_ok "both flags ok"
HAS_ANNO=$(json_get '.annotated_screenshot' || echo "")
NO_PLAIN=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin)
print('absent' if 'screenshot' not in d else 'present')
" 2>/dev/null || echo "error")
if [[ -n "$HAS_ANNO" && "$HAS_ANNO" != "__MISSING__" ]]; then
  _pass "annotated_screenshot is set when both flags given"
else
  _fail "annotated wins" "no annotated path"
fi
if [[ "$NO_PLAIN" == "absent" ]]; then
  _pass "plain screenshot field omitted (annotated includes one)"
else
  _fail "plain field omitted" "$NO_PLAIN"
fi

section "snapshot --with-screenshot + --diff — image attaches to diff result"

PID=$("$CU" apps 2>/dev/null | python3 -c "
import sys, json
d = json.load(sys.stdin)
for a in d.get('apps', []):
    if a.get('name') == 'Finder':
        print(a.get('pid', '')); break
" 2>/dev/null || true)
if [[ -n "$PID" ]]; then
  rm -f "/tmp/cu-snapshot-cache/$PID.json"
fi

cu_json snapshot Finder --limit 10 --diff --with-screenshot --output /tmp/cu-test-fused-diff1.png
assert_ok "first --diff --with-screenshot ok"
SHOT1=$(json_get '.screenshot' || echo "")
if [[ "$SHOT1" == "/tmp/cu-test-fused-diff1.png" ]]; then
  _pass "screenshot attached on first --diff call"
else
  _fail "first diff screenshot" "got: $SHOT1"
fi

cu_json snapshot Finder --limit 10 --diff --with-screenshot --output /tmp/cu-test-fused-diff2.png
assert_ok "second --diff --with-screenshot ok"
SHOT2=$(json_get '.screenshot' || echo "")
if [[ "$SHOT2" == "/tmp/cu-test-fused-diff2.png" ]]; then
  _pass "screenshot attached on warm --diff call"
else
  _fail "warm diff screenshot" "got: $SHOT2"
fi
assert_json_field_exists "diff field present" ".diff"

section "snapshot --with-screenshot — human mode"

cu_human snapshot Finder --limit 5 --with-screenshot --output /tmp/cu-test-fused-h.png
assert_exit_zero "human --with-screenshot exits 0"
if echo "$OUT" | grep -q "^Screenshot:"; then
  _pass "human prints 'Screenshot:' line"
else
  _fail "human Screenshot: line" "${OUT:0:200}"
fi

section "snapshot --with-screenshot — error: non-existent app"

cu_json snapshot NonExistentApp99887 --with-screenshot
assert_fail "non-existent app fails (no window)"

summary
