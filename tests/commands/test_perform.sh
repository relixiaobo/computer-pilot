#!/bin/bash
# Test: cu perform — generic AX action with structured failure hints
source "$(dirname "$0")/helpers.sh"

# Finder is always running; ref 1 is typically AXApplication / AXWindow.
section "perform — successful action"

"$CU" snapshot Finder --limit 50 >/dev/null
cu_json perform 1 AXShowDefaultUI --app Finder --no-snapshot
assert_ok "perform AXShowDefaultUI on Finder ref 1"
assert_json_field "method is ax-perform" ".method" "ax-perform"
assert_json_field "action echoed" ".action" "AXShowDefaultUI"
HAS_AVAIL=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('yes' if isinstance(d.get('available_actions'), list) else 'no')
" 2>/dev/null || echo "error")
if [[ "$HAS_AVAIL" == "yes" ]]; then
  _pass "available_actions returned on success"
else
  _fail "available_actions returned on success" "missing"
fi

section "perform — failure carries structured hint + suggested_next"

# Use an action the element does not support — should fail with diagnostics.
#
# Which element ref 1 lands on depends on what Finder happens to be showing:
# a row carries AXShowDefaultUI, a statictext carries no actions at all. The
# no-action path legitimately has nothing to suggest, so pinning this section
# to ref 1 made it assert on Finder's window contents rather than on cu. Probe
# for an element that actually has actions, and cover the other path below.
"$CU" snapshot Finder --limit 50 >/dev/null
ACTIONABLE_REF=""
for candidate in 1 2 3 4 5 6 7 8; do
  # Piping into grep would report cu's own non-zero exit under `set -o
  # pipefail`, so every probe "failed" no matter what it printed.
  PROBE=$("$CU" perform "$candidate" AXBogusActionName --app Finder --no-snapshot 2>&1 || true)
  if [[ "$PROBE" == *'"available_actions"'* ]]; then
    ACTIONABLE_REF="$candidate"
    break
  fi
done
if [[ -z "$ACTIONABLE_REF" ]]; then
  _skip "perform failure diagnostics" "no Finder element in refs 1-8 exposes any AX action"
  ACTIONABLE_REF=1
fi
cu_json perform "$ACTIONABLE_REF" AXBogusActionName --app Finder --no-snapshot
JSON_OUT="${OUT:-$ERR}"
PARSED=$(echo "$JSON_OUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
except Exception:
    print('malformed'); sys.exit(0)
print('|'.join([
    'ok=' + str(d.get('ok')),
    'has_error=' + str('error' in d),
    'has_hint=' + str('hint' in d),
    'has_next=' + str(isinstance(d.get('suggested_next'), list) and len(d['suggested_next']) > 0),
    'has_diag=' + str(isinstance(d.get('diagnostics'), dict) and 'available_actions' in d.get('diagnostics', {})),
]))
" 2>/dev/null || echo "malformed")
echo "  parsed: $PARSED"
[[ "$PARSED" == *"ok=False"* ]]    && _pass "ok=false reported" || _fail "ok=false reported" "$PARSED"
[[ "$PARSED" == *"has_error=True"* ]] && _pass "error field populated" || _fail "error field populated" "$PARSED"
[[ "$PARSED" == *"has_hint=True"* ]]  && _pass "hint field populated" || _fail "hint field populated" "$PARSED"
[[ "$PARSED" == *"has_next=True"* ]]  && _pass "suggested_next populated" || _fail "suggested_next populated" "$PARSED"
[[ "$PARSED" == *"has_diag=True"* ]]  && _pass "diagnostics.available_actions populated" || _fail "diagnostics.available_actions populated" "$PARSED"

# The other real path: an element with no actions has nothing to suggest, so
# the guidance has to arrive as a hint. Principle 3 — the degraded path stays
# loud even when there is no structured alternative to offer.
NO_ACTION_REF=""
for candidate in 1 2 3 4 5 6 7 8; do
  PROBE=$("$CU" perform "$candidate" AXBogusActionName --app Finder --no-snapshot 2>&1 || true)
  if [[ "$PROBE" == *"exposes no AX actions"* ]]; then
    NO_ACTION_REF="$candidate"
    break
  fi
done
if [[ -z "$NO_ACTION_REF" ]]; then
  _skip "action-less element explains itself" "every Finder element in refs 1-8 exposes actions"
else
  cu_json perform "$NO_ACTION_REF" AXBogusActionName --app Finder --no-snapshot
  NO_ACTION_HINT=$(echo "${OUT:-$ERR}" | python3 -c "
import sys, json
try: print(json.load(sys.stdin).get('hint', ''))
except Exception: print('')
" 2>/dev/null || echo "")
  if [[ "$NO_ACTION_HINT" == *"no AX actions"* && "$NO_ACTION_HINT" == *"child"* ]]; then
    _pass "action-less element explains itself and points at a child"
  else
    _fail "action-less element explains itself and points at a child" "hint: ${NO_ACTION_HINT:0:160}"
  fi
fi

section "perform — auto-snapshot"

"$CU" snapshot Finder --limit 50 >/dev/null
cu_json perform 1 AXShowDefaultUI --app Finder
assert_ok "perform with snapshot"
HAS_SNAP=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('yes' if 'snapshot' in d else 'no')
" 2>/dev/null || echo "error")
if [[ "$HAS_SNAP" == "yes" ]]; then
  _pass "auto-snapshot attached"
else
  _fail "auto-snapshot attached" "snapshot missing"
fi

section "perform — error: ref 0"

cu_json perform 0 AXPress --app Finder --no-snapshot
assert_fail "ref 0 rejected"

section "perform — error: non-existent ref"

cu_json perform 9999 AXPress --app Finder --no-snapshot
JSON_OUT="${OUT:-$ERR}"
NOT_FOUND=$(echo "$JSON_OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('yes' if d.get('code') == 'stale_observation' else 'no')
" 2>/dev/null || echo "no")
if [[ "$NOT_FOUND" == "yes" ]]; then
  _pass "ref 9999 rejected as stale Observation"
else
  _fail "ref 9999 rejected as stale Observation" "got: $JSON_OUT"
fi

section "perform — human mode"

"$CU" snapshot Finder --limit 50 >/dev/null
cu_human perform 1 AXShowDefaultUI --app Finder
assert_exit_zero "perform human exits 0"
assert_contains "shows performed action" "Performed"

summary
