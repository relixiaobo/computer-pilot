#!/bin/bash
# Test: cu ocr-text surfaces aggregate confidence stats (R6).
#
# Vision returns plausible-looking hallucinations in the 0.2-0.4 confidence
# range. R6 attaches min_confidence / mean_confidence / low_confidence_count
# / confidence_hint so agents see the unreliability signal as a *string*
# advisory, not just a per-text float they have to aggregate themselves.
source "$(dirname "$0")/helpers.sh"

section "ocr — aggregate confidence fields surface"

# Finder usually has at least a few high-confidence text elements (folder
# names, sidebar labels). OCR may take a few seconds — within the per-call
# timeout.
cu_json ocr Finder

if ! is_json; then
  _fail "ocr ok" "non-JSON output: ${OUT:0:200}"
  summary
  exit 0
fi

OK=$(json_get '.ok' || echo "")
if [[ "$OK" != "true" ]]; then
  _skip "ocr aggregate confidence" "ocr-text returned ok=false on this machine (possibly no Vision permission)"
  summary
  exit 0
fi
_pass "ocr ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
texts = d.get('texts', [])
print('|'.join([
    'count=' + str(len(texts)),
    'has_min=' + str('min_confidence' in d),
    'has_mean=' + str('mean_confidence' in d),
    'has_low_count=' + str('low_confidence_count' in d),
    'min_is_float=' + str(isinstance(d.get('min_confidence'), (int, float))),
    'mean_is_float=' + str(isinstance(d.get('mean_confidence'), (int, float))),
]))
" 2>/dev/null || echo "malformed")

if [[ "$PARSED" == *"count=0"* ]]; then
  _skip "aggregate confidence" "OCR found no text on this Finder window"
else
  [[ "$PARSED" == *"has_min=True"* ]]      && _pass "min_confidence present"      || _fail "min_confidence present"      "$PARSED"
  [[ "$PARSED" == *"has_mean=True"* ]]     && _pass "mean_confidence present"     || _fail "mean_confidence present"     "$PARSED"
  [[ "$PARSED" == *"has_low_count=True"* ]] && _pass "low_confidence_count present" || _fail "low_confidence_count present" "$PARSED"
  [[ "$PARSED" == *"min_is_float=True"* ]]  && _pass "min_confidence is numeric"   || _fail "min_confidence is numeric"   "$PARSED"
  [[ "$PARSED" == *"mean_is_float=True"* ]] && _pass "mean_confidence is numeric"  || _fail "mean_confidence is numeric"  "$PARSED"
fi

summary
