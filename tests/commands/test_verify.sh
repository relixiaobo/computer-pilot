#!/bin/bash
# Test: cu click --verify — D4-style silent-failure detection (#3)
source "$(dirname "$0")/helpers.sh"
trap 'textedit_cleanup; cleanup_run' EXIT

# Use TextEdit so we can drive an action that DOES change the tree (typing into
# a fresh document). Finder is too quiet — most ref-1 clicks are no-ops.
textedit_reset
sleep 0.5

click_fresh_finder_ref() {
  local code observation
  for _ in 1 2; do
    cu_json snapshot Finder --limit 50
    [[ "$EXIT" -eq 0 ]] || return
    observation=$(json_get '.observation_id' || echo "")
    cu_json click 1 --app Finder --observation "$observation" "$@"
    [[ "$EXIT" -eq 0 ]] && return
    code=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
    [[ "$code" == "stale_observation" ]] || return
  done
}

section "verify — clicking ref 1 in Finder (no-op) reports verified=false"

# Finder ref [1] is typically a static row that doesn't expand on AXPress.
# That's a textbook silent-failure scenario for verification.
click_fresh_finder_ref --verify
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

click_fresh_finder_ref
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

click_fresh_finder_ref --no-verify
assert_ok "click --no-verify ok"

NO_VERIFY=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('has_verified=' + str('verified' in d))
" 2>/dev/null || echo "malformed")

[[ "$NO_VERIFY" == *"has_verified=False"* ]] && _pass "verified omitted with --no-verify" || _fail "verified omitted with --no-verify" "$NO_VERIFY"

section "verify — --no-snapshot keeps private verification"

click_fresh_finder_ref --no-snapshot
NO_SNAP=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('has_verified=' + str('verified' in d) + '|has_snapshot=' + str('snapshot' in d))
" 2>/dev/null || echo "malformed")

[[ "$NO_SNAP" == *"has_verified=True"* && "$NO_SNAP" == *"has_snapshot=False"* ]] \
  && _pass "--no-snapshot omits output but keeps verification" \
  || _fail "--no-snapshot verification contract" "$NO_SNAP"

summary
