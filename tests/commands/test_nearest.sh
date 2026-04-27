#!/bin/bash
# Test: cu nearest — pixel → ref reverse lookup (A8)
source "$(dirname "$0")/helpers.sh"

# Get a known element from Finder so we can probe its center + corners.
cu_json find --app Finder --role row --first --limit 200
FIRST=$(json_get '.match.ref' || echo "")
if [[ -z "$FIRST" || "$FIRST" == "__MISSING__" ]]; then
  _skip "nearest suite" "no Finder rows to probe against"
  summary
  exit 0
fi
EX=$(json_get '.match.x'); EY=$(json_get '.match.y')
EW=$(json_get '.match.width'); EH=$(json_get '.match.height')
CX=$(python3 -c "print($EX + $EW / 2)")
CY=$(python3 -c "print($EY + $EH / 2)")

section "nearest — point inside an element"

cu_json nearest "$CX" "$CY" --app Finder --limit 200
assert_ok "nearest at element center ok"
M_REF=$(json_get '.match.ref' || echo "")
M_DIST=$(json_get '.match.distance' || echo "1")
M_INSIDE=$(json_get '.match.inside' || echo "false")
if [[ "$M_REF" == "$FIRST" ]]; then
  _pass "ref matches the element we probed against ($FIRST)"
else
  _fail "ref matches" "expected $FIRST, got $M_REF"
fi
if [[ "$M_DIST" == "0" || "$M_DIST" == "0.0" ]]; then
  _pass "distance is 0 inside element"
else
  _fail "distance=0 inside" "got: $M_DIST"
fi
if [[ "$M_INSIDE" == "true" ]]; then
  _pass "inside=true reported"
else
  _fail "inside=true" "got: $M_INSIDE"
fi
assert_json_field_exists "query echoed" ".query.x"
assert_json_field_exists "scanned field" ".scanned"

section "nearest — point outside, returns positive distance"

# A point far away in negative space should match SOMETHING with distance > 0.
cu_json nearest 99999 99999 --app Finder --limit 200
assert_ok "nearest at far-away point ok"
FAR_DIST=$(json_get '.match.distance' || echo "0")
FAR_INSIDE=$(json_get '.match.inside' || echo "true")
DIST_OK=$(python3 -c "print('yes' if $FAR_DIST > 100 else 'no')")
if [[ "$DIST_OK" == "yes" ]]; then
  _pass "distance > 100 from far point ($FAR_DIST)"
else
  _fail "far distance" "got: $FAR_DIST"
fi
if [[ "$FAR_INSIDE" == "false" ]]; then
  _pass "inside=false from far point"
else
  _fail "inside=false from far" "got: $FAR_INSIDE"
fi

section "nearest — --max-distance filters"

# Point inside element 1 → with max-distance 5, still hits (distance is 0)
cu_json nearest "$CX" "$CY" --app Finder --max-distance 5 --limit 200
M=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('hit' if d.get('match') else 'null')
")
if [[ "$M" == "hit" ]]; then
  _pass "max-distance=5 still matches inside-element point"
else
  _fail "max-distance inside" "got: $M"
fi

# Far-away point with max-distance 10 → null
cu_json nearest 99999 99999 --app Finder --max-distance 10 --limit 200
NULL_M=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
m = d.get('match')
print('null' if m is None else 'hit')
")
if [[ "$NULL_M" == "null" ]]; then
  _pass "max-distance=10 filters far-away to null"
else
  _fail "max-distance filters far" "got: $NULL_M"
fi
# max_distance echoed in response
MD=$(json_get '.max_distance' || echo "")
if [[ "$MD" == "10" || "$MD" == "10.0" ]]; then
  _pass "max_distance echoed in response"
else
  _fail "max_distance echoed" "got: $MD"
fi

section "nearest — refs match snapshot refs"

# ref returned by nearest should be a real ref in snapshot at same coordinates
cu_json snapshot Finder --limit 200
SNAP_X=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ref = $FIRST
for e in d.get('elements', []):
    if e.get('ref') == ref:
        print(e.get('x', ''))
        break
")
if [[ "$SNAP_X" == "$EX" ]]; then
  _pass "nearest's ref [$FIRST] matches snapshot's ref [$FIRST] at x=$EX"
else
  _fail "ref consistency" "snap.x=$SNAP_X find.x=$EX"
fi

section "nearest — error: invalid coords"

cu_json nearest nan 100 --app Finder
assert_fail "non-numeric coord rejected"

section "nearest — error: non-existent app"

cu_json nearest 100 100 --app NonExistentApp99887
assert_fail "non-existent app fails"

section "nearest — human mode"

cu_human nearest "$CX" "$CY" --app Finder --limit 200
assert_exit_zero "human nearest exits 0"
if echo "$OUT" | grep -qE '^\[[0-9]+\]'; then
  _pass "human format: [ref] role"
else
  _fail "human format" "${OUT:0:200}"
fi
if echo "$OUT" | grep -q "(inside)"; then
  _pass "human marks (inside) for inside-element point"
else
  _fail "human (inside)" "${OUT:0:200}"
fi

summary
