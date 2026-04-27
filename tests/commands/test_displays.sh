#!/bin/bash
# Test: snapshot includes displays list with global-coord bounds (D1)
source "$(dirname "$0")/helpers.sh"

section "snapshot — displays array present"

cu_json snapshot Finder --limit 5
assert_ok "snapshot ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ds = d.get('displays')
ok_shape = isinstance(ds, list) and len(ds) >= 1 and all(
    all(k in dd for k in ('id','main','x','y','width','height')) for dd in ds
)
mains = sum(1 for dd in (ds or []) if dd.get('main'))
print('|'.join([
    'is_list=' + str(isinstance(ds, list)),
    'count=' + str(len(ds) if isinstance(ds, list) else -1),
    'shape_ok=' + str(ok_shape),
    'exactly_one_main=' + str(mains == 1),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"is_list=True"* ]]          && _pass "displays is a JSON array"             || _fail "displays is array"             "$PARSED"
[[ "$PARSED" == *"shape_ok=True"* ]]         && _pass "every display has id/main/x/y/w/h"   || _fail "display shape"                 "$PARSED"
[[ "$PARSED" == *"exactly_one_main=True"* ]] && _pass "exactly one display has main=true"   || _fail "main display count"            "$PARSED"

section "snapshot --diff — displays still present"

cu_json snapshot Finder --diff --limit 5
cu_json snapshot Finder --diff --limit 5
assert_ok "warm diff ok"
HAS=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('yes' if isinstance(d.get('displays'), list) and len(d['displays']) >= 1 else 'no')
" 2>/dev/null || echo "error")
if [[ "$HAS" == "yes" ]]; then
  _pass "diff JSON includes displays"
else
  _fail "diff displays" "got $HAS"
fi

section "displays — bounds are realistic"

# At least the main display must have positive width/height. Many setups have
# the main at (0,0); secondaries can have negative origin coordinates.
SANE=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
ds = d.get('displays', [])
m = next((x for x in ds if x.get('main')), None)
if not m: print('no main'); sys.exit()
print('ok' if (m['width'] >= 800 and m['height'] >= 600 and m['width'] < 8000) else 'bad')
" 2>/dev/null || echo "error")
if [[ "$SANE" == "ok" ]]; then
  _pass "main display bounds realistic"
else
  _fail "main display bounds" "$SANE"
fi

summary
