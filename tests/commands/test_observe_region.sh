#!/bin/bash
# Test: cu observe-region (A11) — region → candidate refs query
source "$(dirname "$0")/helpers.sh"

# Use the real Finder window's frame so the region tests aren't off-screen.
cu_json snapshot Finder --limit 5
WX=$(json_get '.window_frame.x' || echo "0")
WY=$(json_get '.window_frame.y' || echo "0")
WW=$(json_get '.window_frame.width' || echo "0")
WH=$(json_get '.window_frame.height' || echo "0")
if [[ "$WW" == "0" || "$WW" == "__MISSING__" ]]; then
  _skip "observe-region suite" "no Finder window_frame to anchor against"
  summary
  exit 0
fi

# Use a generous slice of the window's interior
RX=$(python3 -c "print($WX + 10)")
RY=$(python3 -c "print($WY + 60)")
RW=$(python3 -c "print($WW - 20)")
RH=$(python3 -c "print($WH - 100)")

section "observe-region — intersect (default mode)"

cu_json observe-region "$RX" "$RY" "$RW" "$RH" --app Finder --limit 200
assert_ok "observe-region default ok"
assert_json_field "mode is intersect" ".mode" "intersect"
assert_json_field_exists "matches array" ".matches"
assert_json_field_exists "count field" ".count"
assert_json_field_exists "region echoed" ".region.x"
INT_COUNT=$(json_get '.count' || echo "0")
if [[ "$INT_COUNT" -ge 1 ]]; then
  _pass "intersect mode finds elements ($INT_COUNT)"
else
  _fail "intersect mode finds elements" "got 0"
fi

section "observe-region — modes narrow the result set"

cu_json observe-region "$RX" "$RY" "$RW" "$RH" --app Finder --limit 200 --mode center
assert_ok "center mode ok"
CEN_COUNT=$(json_get '.count' || echo "0")

cu_json observe-region "$RX" "$RY" "$RW" "$RH" --app Finder --limit 200 --mode inside
assert_ok "inside mode ok"
INS_COUNT=$(json_get '.count' || echo "0")

# inside ⊆ center ⊆ intersect (in element count)
if [[ "$INS_COUNT" -le "$CEN_COUNT" && "$CEN_COUNT" -le "$INT_COUNT" ]]; then
  _pass "inside ($INS_COUNT) ≤ center ($CEN_COUNT) ≤ intersect ($INT_COUNT)"
else
  _fail "mode narrowing order" "inside=$INS_COUNT center=$CEN_COUNT intersect=$INT_COUNT"
fi

section "observe-region — center mode: each match's center IS inside the rect"

cu_json observe-region "$RX" "$RY" "$RW" "$RH" --app Finder --limit 200 --mode center
ALL_CENTERS_IN=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
r = d.get('region', {})
rx, ry, rw, rh = r['x'], r['y'], r['width'], r['height']
ok = True
for m in d.get('matches', []):
    cx = m['x'] + m['width']/2
    cy = m['y'] + m['height']/2
    if not (rx <= cx < rx + rw and ry <= cy < ry + rh):
        ok = False; break
print('yes' if ok else 'no')
")
if [[ "$ALL_CENTERS_IN" == "yes" ]]; then
  _pass "every center-mode match has its center inside the rect"
else
  _fail "center-mode invariant" "found a match whose center is outside"
fi

section "observe-region — inside mode: each match's bbox IS fully inside"

cu_json observe-region "$RX" "$RY" "$RW" "$RH" --app Finder --limit 200 --mode inside
ALL_INSIDE=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
r = d.get('region', {})
rx, ry, rw, rh = r['x'], r['y'], r['width'], r['height']
ok = True
for m in d.get('matches', []):
    if not (m['x'] >= rx and m['y'] >= ry
            and m['x'] + m['width']  <= rx + rw
            and m['y'] + m['height'] <= ry + rh):
        ok = False; break
print('yes' if ok else 'no')
")
if [[ "$ALL_INSIDE" == "yes" ]]; then
  _pass "every inside-mode match's bbox is fully inside the rect"
else
  _fail "inside-mode invariant" "found a match not fully inside"
fi

section "observe-region — empty region (off-screen)"

cu_json observe-region 99999 99999 10 10 --app Finder
assert_ok "off-screen region ok=true (empty result)"
ZERO=$(json_get '.count' || echo "1")
if [[ "$ZERO" == "0" ]]; then
  _pass "off-screen region returns count=0"
else
  _fail "off-screen count=0" "got: $ZERO"
fi

section "observe-region — refs match snapshot refs"

cu_json observe-region "$RX" "$RY" "$RW" "$RH" --app Finder --limit 200 --mode inside
FIRST_REF=$(json_get '.matches[0].ref' 2>/dev/null || echo "")
FIRST_X=$(json_get '.matches[0].x' 2>/dev/null || echo "")

if [[ -n "$FIRST_REF" && "$FIRST_REF" != "__MISSING__" ]]; then
  cu_json snapshot Finder --limit 200
  SNAP_X=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ref = $FIRST_REF
for e in d.get('elements', []):
    if e.get('ref') == ref:
        print(e.get('x', '')); break
" 2>/dev/null || echo "")
  if [[ "$SNAP_X" == "$FIRST_X" ]]; then
    _pass "observe-region's ref [$FIRST_REF] matches snapshot at x=$FIRST_X"
  else
    _fail "ref consistency" "snap.x=$SNAP_X observe.x=$FIRST_X"
  fi
else
  _skip "ref consistency" "no inside-mode matches"
fi

section "observe-region — error paths"

cu_json observe-region "$RX" "$RY" 0 0 --app Finder
assert_fail "zero size rejected"

cu_json observe-region "$RX" "$RY" 100 100 --app Finder --mode bogus
assert_fail "unknown mode rejected"
PARSED=$(echo "${OUT:-$ERR}" | python3 -c "
import sys, json
try: d = json.load(sys.stdin)
except: print('malformed'); sys.exit()
print('|'.join([
    'has_hint=' + str('hint' in d),
    'has_next=' + str(isinstance(d.get('suggested_next'), list) and len(d['suggested_next']) > 0),
]))
" 2>/dev/null || echo "malformed")
[[ "$PARSED" == *"has_hint=True"* ]] && _pass "unknown mode error has hint"        || _fail "mode hint"        "$PARSED"
[[ "$PARSED" == *"has_next=True"* ]] && _pass "unknown mode error has next"        || _fail "mode next"        "$PARSED"

cu_json observe-region "$RX" "$RY" "$RW" "$RH" --app NonExistentApp99887
assert_fail "non-existent app fails"

section "observe-region — human mode"

cu_human observe-region "$RX" "$RY" "$RW" "$RH" --app Finder --limit 30
assert_exit_zero "human observe-region exits 0"
if echo "$OUT" | grep -qE '^\[[0-9]+\]'; then
  _pass "human format: [ref] role"
else
  _fail "human format" "${OUT:0:200}"
fi

# Empty region produces "No elements" line in human mode
cu_human observe-region 99999 99999 10 10 --app Finder
if echo "$OUT" | grep -q "^No elements in region"; then
  _pass "empty region prints 'No elements in region'"
else
  _fail "empty human line" "${OUT:0:200}"
fi

summary
