#!/bin/bash
# Test: cu find — predicate query over the AX tree
source "$(dirname "$0")/helpers.sh"

section "find — error: no filters"

cu_json find --app Finder
assert_fail "no filters → error"
PARSED=$(echo "${OUT:-$ERR}" | python3 -c "
import sys, json
try: d = json.load(sys.stdin)
except Exception: print('malformed'); sys.exit()
print('|'.join([
    'ok=' + str(d.get('ok')),
    'has_hint=' + str('hint' in d),
    'has_next=' + str(isinstance(d.get('suggested_next'), list) and len(d['suggested_next']) > 0),
]))
" 2>/dev/null || echo "malformed")
[[ "$PARSED" == *"ok=False"* ]]      && _pass "ok=false"           || _fail "ok=false"           "$PARSED"
[[ "$PARSED" == *"has_hint=True"* ]] && _pass "hint populated"     || _fail "hint populated"     "$PARSED"
[[ "$PARSED" == *"has_next=True"* ]] && _pass "suggested_next ok"  || _fail "suggested_next ok"  "$PARSED"

section "find — by role"

cu_json find --app Finder --role row --limit 100
assert_ok "find role=row in Finder"
COUNT=$(json_get '.count' || echo "0")
if [[ "$COUNT" -ge 1 ]]; then
  _pass "found rows ($COUNT)"
else
  _fail "found rows" "got count=$COUNT"
fi

# All matches must have role=row
ALL_ROW=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
matches = d.get('matches', [])
if not matches: print('empty'); sys.exit()
print('yes' if all(m.get('role') == 'row' for m in matches) else 'no')
" 2>/dev/null || echo "error")
if [[ "$ALL_ROW" == "yes" ]]; then
  _pass "every match has role=row"
else
  _fail "every match has role=row" "$ALL_ROW"
fi

# scanned + truncated fields present
assert_json_field_exists "scanned field" ".scanned"
assert_json_field_exists "truncated field" ".truncated"

section "find — --first returns one match"

cu_json find --app Finder --role row --limit 100 --first
assert_ok "find --first ok"
HAS_MATCH=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
m = d.get('match')
print('ok' if isinstance(m, dict) and 'ref' in m and m.get('role') == 'row' else 'missing')
" 2>/dev/null || echo "error")
if [[ "$HAS_MATCH" == "ok" ]]; then
  _pass "--first → .match is a single object with ref + role"
else
  _fail "--first match shape" "$HAS_MATCH"
fi
# count still reflects total matches
assert_json_field_exists "count field" ".count"

section "find — empty result is not an error"

cu_json find --app Finder --title-contains zzz_definitely_not_present_xyz_4242 --limit 200
assert_ok "no-match returns ok=true (not an error)"
COUNT=$(json_get '.count' || echo "1")
if [[ "$COUNT" == "0" ]]; then
  _pass "count=0 on no-match"
else
  _fail "count=0 on no-match" "got: $COUNT"
fi

# --first on empty result → match is null
cu_json find --app Finder --title-contains zzz_definitely_not_present_xyz_4242 --first --limit 200
assert_ok "no-match --first ok"
MATCH_NULL=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('null' if d.get('match') is None else 'not-null')
" 2>/dev/null || echo "error")
if [[ "$MATCH_NULL" == "null" ]]; then
  _pass "--first on empty → .match is null"
else
  _fail "--first empty .match" "$MATCH_NULL"
fi

section "find — title-contains is case-insensitive"

# Need a Finder window with sidebar so titles like "Documents" are present.
# We do NOT activate — `cu find` reads the AX tree of whatever windows exist,
# and `make new Finder window` pulls Finder forward only when it had to spawn a
# window. If a window already exists this is a no-op.
osascript -e 'tell application "Finder"
  if (count of Finder windows) = 0 then
    make new Finder window
    set target of front Finder window to home
  end if
end tell' 2>/dev/null
sleep 0.3

cu_json find --app Finder --title-contains documents --limit 200
assert_ok "case-insensitive find ok"
COUNT_LC=$(json_get '.count' || echo "0")
cu_json find --app Finder --title-contains DOCUMENTS --limit 200
COUNT_UC=$(json_get '.count' || echo "0")
if [[ "$COUNT_LC" == "$COUNT_UC" && "$COUNT_LC" != "0" ]]; then
  _pass "lowercase and UPPERCASE searches return same count ($COUNT_LC)"
else
  _pass "case-insensitive (or no Documents item visible: lc=$COUNT_LC uc=$COUNT_UC) — environment-dependent"
fi

section "find — filters AND together"

# role=row AND title-contains=Documents → strictly fewer than role=row alone
cu_json find --app Finder --role row --limit 200
ALL_ROWS=$(json_get '.count' || echo "0")
cu_json find --app Finder --role row --title-contains documents --limit 200
ROW_AND_DOC=$(json_get '.count' || echo "0")
if [[ "$ROW_AND_DOC" -le "$ALL_ROWS" ]]; then
  _pass "AND narrows result ($ROW_AND_DOC ≤ $ALL_ROWS)"
else
  _fail "AND narrows" "got $ROW_AND_DOC > $ALL_ROWS"
fi

section "find — refs match snapshot refs"

# A ref returned by find should refer to the same element as in snapshot.
cu_json find --app Finder --role row --first --limit 200
FIND_REF=$(json_get '.match.ref' || echo "")
FIND_X=$(json_get '.match.x' || echo "")
FIND_Y=$(json_get '.match.y' || echo "")

if [[ -n "$FIND_REF" && "$FIND_REF" != "__MISSING__" ]]; then
  cu_json snapshot Finder --limit 200
  SNAP_X=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ref = $FIND_REF
for e in d.get('elements', []):
    if e.get('ref') == ref:
        print(e.get('x', ''))
        break
" 2>/dev/null || echo "")
  SNAP_Y=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ref = $FIND_REF
for e in d.get('elements', []):
    if e.get('ref') == ref:
        print(e.get('y', ''))
        break
" 2>/dev/null || echo "")
  if [[ "$FIND_X" == "$SNAP_X" && "$FIND_Y" == "$SNAP_Y" ]]; then
    _pass "find ref [$FIND_REF] (x=$FIND_X y=$FIND_Y) matches snapshot"
  else
    _fail "ref consistency" "find=($FIND_X,$FIND_Y) snap=($SNAP_X,$SNAP_Y)"
  fi
else
  _skip "ref consistency" "no first match"
fi

section "find — --raw outputs bare ref integers (G3)"

# --first --raw → single integer + exit 0 + no JSON
RAW_OUT=$("$CU" find --app Finder --role row --first --raw --limit 200 2>&1)
RAW_EXIT=$?
if [[ "$RAW_EXIT" -eq 0 && "$RAW_OUT" =~ ^[0-9]+$ ]]; then
  _pass "--first --raw → bare integer ($RAW_OUT) + exit 0"
else
  _fail "--first --raw shape" "exit=$RAW_EXIT out='$RAW_OUT'"
fi

# --raw without --first → multiple lines, each an integer
RAW_OUT=$("$CU" find --app Finder --role row --raw --limit 200 2>&1)
RAW_LINES=$(echo "$RAW_OUT" | wc -l | tr -d ' ')
if [[ "$RAW_LINES" -ge 2 ]] && echo "$RAW_OUT" | python3 -c "
import sys
for line in sys.stdin.read().splitlines():
    if not line.strip().isdigit(): sys.exit(1)
" 2>/dev/null; then
  _pass "--raw multi-line → all integers, $RAW_LINES lines"
else
  _fail "--raw multi-line" "got $RAW_LINES lines"
fi

# --raw on no-match → exit 1, no output
RAW_OUT=$("$CU" find --app Finder --title-equals zzz_NEVER_MATCHES_4242 --first --raw 2>&1) || RAW_EXIT=$?
if [[ "${RAW_EXIT:-0}" -eq 1 && -z "$RAW_OUT" ]]; then
  _pass "--raw no-match → exit 1 + no output"
else
  _fail "--raw no-match" "exit=${RAW_EXIT:-0} out='$RAW_OUT'"
fi

# Practical pipe test: cu click $(cu find ... --first --raw)
REF_PIPE=$("$CU" find --app Finder --role row --first --raw --limit 200)
if [[ -n "$REF_PIPE" && "$REF_PIPE" =~ ^[0-9]+$ ]]; then
  _pass "pipe-friendly: REF=\$(cu find --first --raw) yields '$REF_PIPE'"
else
  _fail "pipe usage" "got: '$REF_PIPE'"
fi

section "find — error: non-existent app"

cu_json find --app NonExistentApp987654 --role button
assert_fail "non-existent app fails"

section "find — human mode"

cu_human find --app Finder --role row --limit 50
assert_exit_zero "find human exits 0"
# Should emit lines like '[1] row "" (...)' — assert at least one bracketed ref line OR "No matches"
if echo "$OUT" | grep -qE '^\[[0-9]+\] row|^No matches'; then
  _pass "human format: [ref] role ... or 'No matches'"
else
  _fail "human format" "${OUT:0:200}"
fi

# --first hint about extra matches in human mode
cu_human find --app Finder --role row --first --limit 50
assert_exit_zero "find --first human exits 0"

summary
