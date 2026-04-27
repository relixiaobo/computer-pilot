#!/bin/bash
# Test: axPath stable selector (A2) — every snapshot element has axPath, and
# action commands accept --ax-path to resolve elements without ref drift.
source "$(dirname "$0")/helpers.sh"

# Need at least one Finder window so the AX tree is non-trivial.
osascript -e 'tell application "Finder"
  if (count of Finder windows) is 0 then make new Finder window
end tell' 2>/dev/null
sleep 0.3

section "snapshot — every element has an axPath"

cu_json snapshot Finder --limit 30
assert_ok "snapshot ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
els = d.get('elements', [])
have = sum(1 for e in els if isinstance(e.get('axPath'), str) and e['axPath'])
print('|'.join([
    'count=' + str(len(els)),
    'with_path=' + str(have),
    'all_present=' + str(have == len(els) and len(els) > 0),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"all_present=True"* ]] && _pass "every element has non-empty axPath" \
  || _fail "every element has axPath" "$PARSED"

section "axPath — sibling disambiguation (:N)"

# In Finder's sidebar, multiple rows are siblings of the same outline. The
# walker must produce distinct paths via the :N suffix.
DISTINCT=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
paths = [e.get('axPath') for e in d.get('elements', [])]
print('|'.join([
    'unique=' + str(len(set(paths))),
    'total=' + str(len(paths)),
    'has_indexed=' + str(any(':' in p.rsplit('/', 1)[-1] for p in paths if p)),
]))
" 2>/dev/null || echo "malformed")

[[ "$DISTINCT" == *"has_indexed=True"* ]] && _pass "at least one segment uses :N suffix" \
  || _fail "axPath :N disambiguation" "$DISTINCT"

# All paths in the snapshot must be unique
echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
paths = [e.get('axPath') for e in d.get('elements', [])]
sys.exit(0 if len(set(paths)) == len(paths) else 1)
" 2>/dev/null
if [[ "$?" -eq 0 ]]; then
  _pass "all axPaths in snapshot are unique"
else
  _fail "axPath uniqueness" "duplicates found"
fi

section "axPath — round-trip: snapshot's axPath resolves back to same element"

# Pick an element with a stable identity (a row), grab its (x,y), then click via
# axPath and check the response's (x,y) center matches snapshot's center.
PICK=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
for e in d.get('elements', []):
    if e['role'] == 'row' and e.get('axPath') and e['width'] > 0 and e['height'] > 0:
        cx = e['x'] + e['width'] / 2
        cy = e['y'] + e['height'] / 2
        print(f\"{e['axPath']}|{cx}|{cy}\")
        break
")

if [[ -n "$PICK" ]]; then
  AX_PATH=$(echo "$PICK" | cut -d'|' -f1)
  EXPECTED_X=$(echo "$PICK" | cut -d'|' -f2)
  EXPECTED_Y=$(echo "$PICK" | cut -d'|' -f3)

  cu_json click --ax-path "$AX_PATH" --app Finder --no-snapshot
  assert_ok "click via axPath returns ok"

  GOT_X=$(json_get '.x' || echo "0")
  GOT_Y=$(json_get '.y' || echo "0")
  METHOD=$(json_get '.method' || echo "")

  # Coordinates should match within 1px (rounding allowed)
  CLOSE=$(python3 -c "print('yes' if abs($GOT_X - $EXPECTED_X) <= 1 and abs($GOT_Y - $EXPECTED_Y) <= 1 else 'no')")
  if [[ "$CLOSE" == "yes" ]]; then
    _pass "axPath resolved to same coordinates as snapshot ($GOT_X,$GOT_Y ≈ $EXPECTED_X,$EXPECTED_Y)"
  else
    _fail "axPath coord round-trip" "got ($GOT_X,$GOT_Y) want ($EXPECTED_X,$EXPECTED_Y)"
  fi

  # Method should be ax-action (path resolved + AX action chain ran)
  if [[ "$METHOD" == "ax-action" || "$METHOD" == "cgevent-pid" ]]; then
    _pass "method = $METHOD (PID-targeted, non-disruptive)"
  else
    _fail "method check" "got '$METHOD'"
  fi
else
  _skip "axPath round-trip" "no row element to test"
fi

section "axPath — error: missing element"

cu_json click --ax-path "window/this/does/not/exist:99" --app Finder --no-snapshot
assert_fail "non-existent axPath fails"

section "axPath — error: empty path"

cu_json click --ax-path "" --app Finder --no-snapshot
assert_fail "empty axPath fails"

section "axPath — set-value rejects axPath without textfield-like element"

# set-value needs a value-bearing role. A row should reject AXValue write.
if [[ -n "$PICK" ]]; then
  AX_PATH=$(echo "$PICK" | cut -d'|' -f1)
  cu_json set-value --ax-path "$AX_PATH" "test" --app Finder --no-snapshot
  assert_fail "set-value on a row rejects (read-only role)"
else
  _skip "set-value rejection test" "no element"
fi

section "axPath — perform requires --ax-path or ref"

cu_json perform AXPress --app Finder --no-snapshot
assert_fail "perform without ref or --ax-path errors"

summary
