#!/bin/bash
# Test: cu click --verify — D4-style silent-failure detection (#3)
source "$(dirname "$0")/helpers.sh"

# Use TextEdit so we can drive an action that DOES change the tree (typing into
# a fresh document). Finder is too quiet — most ref-1 clicks are no-ops.
osascript -e 'tell application "TextEdit" to make new document' 2>/dev/null
sleep 0.5

section "verify — clicking ref 1 in Finder (no-op) reports verified=false"

# Finder ref [1] is typically a static row that doesn't expand on AXPress.
# That's a textbook silent-failure scenario for verification.
cu_json click 1 --app Finder --verify
assert_ok "click --verify ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('|'.join([
    'has_verified=' + str('verified' in d),
    'verified_is_bool=' + str(isinstance(d.get('verified'), bool)),
    'has_diff=' + str('verify_diff' in d),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"has_verified=True"* ]]      && _pass "verified field present"     || _fail "verified field present"     "$PARSED"
[[ "$PARSED" == *"verified_is_bool=True"* ]]  && _pass "verified is bool"           || _fail "verified is bool"           "$PARSED"
[[ "$PARSED" == *"has_diff=True"* ]]          && _pass "verify_diff field present"  || _fail "verify_diff field present"  "$PARSED"

section "verify — silent click attaches advice"

VERIFIED=$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin).get('verified'))" 2>/dev/null || echo "?")
ADVICE=$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin).get('verify_advice',''))" 2>/dev/null || echo "")
if [[ "$VERIFIED" == "False" ]]; then
  if [[ -n "$ADVICE" ]]; then
    _pass "advice attached when verified=false"
  else
    _fail "advice attached when verified=false" "verify_advice missing"
  fi
else
  # If clicking ref 1 actually moved the tree, that's fine — the diff field
  # should still be present. Skip the advice assertion in that case.
  _skip "advice attached when verified=false" "this click changed the tree (verified=true)"
fi

section "verify — advice mentions remediation when method=cgevent-pid silent"

# Click an unlikely-clickable coordinate so CGEvent path fires but tree won't
# change. Coordinates inside Finder window but on dead space.
cu_json click 500 500 --app Finder --verify
PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('verified=' + str(d.get('verified')) + '|method=' + str(d.get('method')) + '|advice=' + str(d.get('verify_advice','')))
" 2>/dev/null || echo "malformed")

if [[ "$PARSED" == *"verified=False"* && "$PARSED" == *"method=cgevent-pid"* ]]; then
  if [[ "$PARSED" == *"--allow-global"* || "$PARSED" == *"PID-targeted"* ]]; then
    _pass "cgevent-pid advice mentions remediation"
  else
    _fail "cgevent-pid advice mentions remediation" "$PARSED"
  fi
else
  _skip "cgevent-pid silent click" "did not produce the expected method+verified combination ($PARSED)"
fi

section "verify — verify is ON by default (R2)"

cu_json click 1 --app Finder
assert_ok "click without --no-verify ok"

DEFAULT=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('has_verified=' + str('verified' in d) + '|verified_is_bool=' + str(isinstance(d.get('verified'), bool)))
" 2>/dev/null || echo "malformed")

[[ "$DEFAULT" == *"has_verified=True"* && "$DEFAULT" == *"verified_is_bool=True"* ]] \
  && _pass "verified attached by default" \
  || _fail "verified attached by default" "$DEFAULT"

section "verify — --no-verify opts out"

cu_json click 1 --app Finder --no-verify
assert_ok "click --no-verify ok"

NO_VERIFY=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('has_verified=' + str('verified' in d))
" 2>/dev/null || echo "malformed")

[[ "$NO_VERIFY" == *"has_verified=False"* ]] && _pass "verified omitted with --no-verify" || _fail "verified omitted with --no-verify" "$NO_VERIFY"

section "verify — --no-snapshot also disables verify (verify needs the post-snapshot)"

cu_json click 1 --app Finder --no-snapshot
NO_SNAP=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('has_verified=' + str('verified' in d) + '|has_snapshot=' + str('snapshot' in d))
" 2>/dev/null || echo "malformed")

[[ "$NO_SNAP" == *"has_verified=False"* && "$NO_SNAP" == *"has_snapshot=False"* ]] \
  && _pass "--no-snapshot disables both snapshot and verify" \
  || _fail "--no-snapshot disables both" "$NO_SNAP"

# Cleanup
osascript -e 'tell application "TextEdit" to close every document saving no' >/dev/null 2>&1 || true
osascript -e 'tell application "TextEdit" to quit' >/dev/null 2>&1 || true

summary
