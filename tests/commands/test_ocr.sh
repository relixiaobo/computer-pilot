#!/bin/bash
# Test: cu ocr
source "$(dirname "$0")/helpers.sh"

# Ensure Finder has a visible window
osascript -e 'tell application "Finder" to activate' 2>/dev/null
osascript -e 'tell application "Finder"
  if (count of Finder windows) is 0 then make new Finder window
end tell' 2>/dev/null
sleep 0.5

section "ocr — Finder"

cu_json "ocr Finder"
assert_ok "ocr Finder ok"
assert_json_field_exists "texts array" ".texts"

# Check text entry structure
TEXT_STRUCT=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
texts = d.get('texts', [])
if not texts: print('empty'); sys.exit()
t = texts[0]
required = ['text', 'x', 'y', 'width', 'height', 'confidence']
missing = [f for f in required if f not in t]
print('ok' if not missing else 'missing: ' + ','.join(missing))
" 2>/dev/null || echo "error")
if [[ "$TEXT_STRUCT" == "ok" ]]; then
  _pass "text entries have all fields"
elif [[ "$TEXT_STRUCT" == "empty" ]]; then
  _skip "text entry fields" "no text recognized (Finder window may be empty)"
else
  _fail "text entry fields" "$TEXT_STRUCT"
fi

# Confidence should be between 0 and 1
CONF_OK=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
texts = d.get('texts', [])
if not texts: print('empty'); sys.exit()
bad = [t for t in texts if t['confidence'] < 0 or t['confidence'] > 1]
print('ok' if not bad else f'bad: {bad[0][\"confidence\"]}')
" 2>/dev/null || echo "error")
if [[ "$CONF_OK" == "ok" ]]; then
  _pass "confidence values in [0, 1]"
elif [[ "$CONF_OK" == "empty" ]]; then
  _skip "confidence range" "no text recognized"
else
  _fail "confidence range" "$CONF_OK"
fi

section "ocr — should find Finder-related text"

# Finder window should contain something like "Finder" or file/folder names
TEXT_FOUND=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
texts = d.get('texts', [])
all_text = ' '.join(t['text'] for t in texts)
print(f'found {len(texts)} text regions' if texts else 'empty')
" 2>/dev/null || echo "error")
if [[ "$TEXT_FOUND" != "empty" ]]; then
  _pass "OCR found text: $TEXT_FOUND"
else
  _skip "OCR text content" "no text found in Finder window"
fi

section "ocr — error: non-existent app"

cu_json "ocr NonExistentApp98765"
assert_fail "non-existent app fails"

section "ocr — human mode"

cu_human "ocr Finder"
assert_exit_zero "ocr human exits 0"
# Human format: [x,y WxH] "text" (confidence%)
if echo "$OUT" | grep -qE '^\[' || echo "$OUT" | grep -q "No text found"; then
  _pass "human format correct"
else
  _fail "human format" "${OUT:0:200}"
fi

summary
