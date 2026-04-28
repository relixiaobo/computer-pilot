#!/bin/bash
# Test: capture-protected windows return structured error (sharing_state=0)
# Skips when no protected app is running. WeChat is the canonical example.
source "$(dirname "$0")/helpers.sh"

# Find any running app whose layer-0 window has kCGWindowSharingState=0
PROTECTED_APP=$(swift - <<'SWIFT' 2>/dev/null
import Foundation
import CoreGraphics
let opts: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
guard let wins = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) as? [[String: Any]] else { exit(1) }
for w in wins {
    guard let layer = w["kCGWindowLayer"] as? Int, layer == 0,
          let share = w["kCGWindowSharingState"] as? Int, share == 0,
          let name = w["kCGWindowOwnerName"] as? String else { continue }
    print(name)
    exit(0)
}
SWIFT
)

if [[ -z "$PROTECTED_APP" ]]; then
  section "capture-protection — no protected app running, skipping"
  _skip "cu screenshot returns capture-protected error" "no app with sharing_state=0 is running (WeChat / Microsoft Office Mac App Store builds typically trigger this)"
  _skip "cu state surfaces screenshot_error" "no protected app available"
  summary
  exit 0
fi

section "capture-protection — cu screenshot refuses with structured error"

OUT=$("$CU" screenshot "$PROTECTED_APP" --path /tmp/cu-protected-test.png 2>&1) || true
if echo "$OUT" | grep -q "capture-protected"; then
  _pass "screenshot of '$PROTECTED_APP' refused with capture-protected error"
else
  _fail "screenshot of '$PROTECTED_APP' refused" "got: $OUT"
fi

if echo "$OUT" | grep -q '"ok":false'; then
  _pass "ok=false on protected window"
else
  _fail "ok=false on protected window" "got: $OUT"
fi

if echo "$OUT" | grep -q "kCGWindowSharingState"; then
  _pass "error message names the sharing-state field for diagnosability"
else
  _fail "error names sharing-state field" "got: $OUT"
fi

# Make sure we didn't write a blank PNG
if [[ -f /tmp/cu-protected-test.png ]]; then
  _fail "no blank PNG left behind" "/tmp/cu-protected-test.png exists"
  rm -f /tmp/cu-protected-test.png
else
  _pass "no blank PNG left behind on refusal"
fi

section "capture-protection — cu state surfaces screenshot_error, snapshot still works"

cu_json state "$PROTECTED_APP"
assert_ok "cu state on protected app still ok=true (snapshot is the load-bearing field)"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('|'.join([
    'has_elements=' + str('elements' in d),
    'has_screenshot=' + str('screenshot' in d),
    'has_screenshot_error=' + str('screenshot_error' in d),
    'mentions_protected=' + str('capture-protected' in str(d.get('screenshot_error','')))
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"has_elements=True"* ]]            && _pass "elements still present"          || _fail "elements still present"          "$PARSED"
[[ "$PARSED" == *"has_screenshot=False"* ]]         && _pass "screenshot path absent"           || _fail "screenshot path absent"           "$PARSED"
[[ "$PARSED" == *"has_screenshot_error=True"* ]]    && _pass "screenshot_error attached"        || _fail "screenshot_error attached"        "$PARSED"
[[ "$PARSED" == *"mentions_protected=True"* ]]      && _pass "screenshot_error names the cause" || _fail "screenshot_error names the cause" "$PARSED"

section "capture-protection — sharing=ReadOnly/ReadWrite apps still capture normally"

# Finder is a baseline: layer-0, sharing=1
cu_json screenshot Finder --path /tmp/cu-shareable-test.png
assert_ok "screenshot of normal app (Finder) still ok"
if [[ -f /tmp/cu-shareable-test.png ]]; then
  MAGIC=$(head -c 4 /tmp/cu-shareable-test.png | xxd -p)
  [[ "$MAGIC" == "89504e47" ]] && _pass "PNG file produced for shareable app" || _fail "PNG produced" "magic=$MAGIC"
  rm -f /tmp/cu-shareable-test.png
else
  _fail "PNG written" "/tmp/cu-shareable-test.png missing"
fi

summary
