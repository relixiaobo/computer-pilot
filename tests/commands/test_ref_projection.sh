#!/bin/bash
# Test: O1 canonical numeric-ref projection across snapshot consumers.
source "$(dirname "$0")/helpers.sh"
trap 'textedit_cleanup; cleanup_run' EXIT

section "ref projection — snapshot/find/nearest share identity"

cu_json snapshot Finder --limit 120
assert_ok "Finder snapshot available"
SNAPSHOT_JSON="$OUT"

cu_json find --app Finder --role row --first --limit 120
assert_ok "find returns a row"
ROW_REF=$(json_get '.match.ref' || echo "")
ROW_X=$(json_get '.match.x' || echo "")
ROW_Y=$(json_get '.match.y' || echo "")

if [[ -n "$ROW_REF" && "$ROW_REF" != "__MISSING__" ]]; then
  SNAP_MATCH=$(echo "$SNAPSHOT_JSON" | python3 -c '
import json, sys
d = json.load(sys.stdin)
ref = int(sys.argv[1])
for e in d.get("elements", []):
    if e.get("ref") == ref:
        print("|".join(str(e.get(k, "")) for k in ("role", "x", "y", "axPath")))
        break
' "$ROW_REF")
  IFS='|' read -r SNAP_ROLE SNAP_X SNAP_Y SNAP_PATH <<< "$SNAP_MATCH"
  [[ "$ROW_X" == "$SNAP_X" && "$ROW_Y" == "$SNAP_Y" ]] \
    && _pass "find ref [$ROW_REF] matches snapshot geometry" \
    || _fail "find ref matches snapshot" "find=($ROW_X,$ROW_Y) snapshot=($SNAP_X,$SNAP_Y)"

  CENTER_X=$(echo "$SNAPSHOT_JSON" | python3 -c '
import json, sys
d = json.load(sys.stdin)
ref = int(sys.argv[1])
e = next(x for x in d["elements"] if x["ref"] == ref)
print(e["x"] + e["width"] / 2)
' "$ROW_REF")
  CENTER_Y=$(echo "$SNAPSHOT_JSON" | python3 -c '
import json, sys
d = json.load(sys.stdin)
ref = int(sys.argv[1])
e = next(x for x in d["elements"] if x["ref"] == ref)
print(e["y"] + e["height"] / 2)
' "$ROW_REF")
  cu_json nearest "$CENTER_X" "$CENTER_Y" --app Finder --limit 120
  assert_ok "nearest resolves the same row"
  assert_json_field "nearest ref matches" ".match.ref" "$ROW_REF"
else
  _skip "find/nearest identity" "Finder exposed no row in the current desktop"
fi

section "ref projection — why uses the same ref order"

if [[ -n "$ROW_REF" && "$ROW_REF" != "__MISSING__" ]]; then
  cu_json why "$ROW_REF" --app Finder --limit 120
  assert_ok "why resolves the snapshot ref"
  assert_json_field "why found=true" ".found" "true"
  WHY_REF=$(json_get '.element.ref' || echo "")
  WHY_PATH=$(json_get '.element.axPath' || echo "")
  [[ "$WHY_REF" == "$ROW_REF" && "$WHY_PATH" == "$SNAP_PATH" ]] \
    && _pass "why identity matches snapshot (ref/path)" \
    || _fail "why identity matches snapshot" "why=($WHY_REF,$WHY_PATH) snapshot=($ROW_REF,$SNAP_PATH)"
else
  _skip "why identity" "no row ref"
fi

section "ref projection — TextEdit action paths use the same order"

if textedit_reset; then
  "$CU" snapshot TextEdit --limit 5 >/dev/null
  cu_json set-value 1 "canonical-ref-test" --app TextEdit --no-snapshot
  if [[ "$EXIT" -ne 0 ]]; then
    # The command's preflight may observe a just-created TextEdit window at a
    # different limit; refresh the observation once before treating it as a
    # projection failure.
    "$CU" snapshot TextEdit --limit 5 >/dev/null
    cu_json set-value 1 "canonical-ref-test" --app TextEdit --no-snapshot
  fi
  assert_ok "set-value ref 1 resolves"
  assert_json_field "set-value method" ".method" "ax-set-value"

  "$CU" snapshot TextEdit --limit 5 >/dev/null
  cu_json click 1 --app TextEdit --no-snapshot
  if [[ "$EXIT" -ne 0 ]]; then
    "$CU" snapshot TextEdit --limit 5 >/dev/null
    cu_json click 1 --app TextEdit --no-snapshot
  fi
  assert_ok "click ref 1 resolves"
  CLICK_REF=$(json_get '.ref' || echo "")
  [[ "$CLICK_REF" == "1" ]] && _pass "click consumed canonical ref 1" || _fail "click ref" "got $CLICK_REF"
else
  _skip "TextEdit action identity" "TextEdit fixture unavailable"
fi

section "ref projection — static/zero-size nodes do not create gaps"

REF_SEQUENCE=$(echo "$SNAPSHOT_JSON" | python3 -c '
import json, sys
d = json.load(sys.stdin)
refs = [e.get("ref") for e in d.get("elements", [])]
print("yes" if refs == list(range(1, len(refs) + 1)) else "no")
')
[[ "$REF_SEQUENCE" == "yes" ]] \
  && _pass "snapshot refs are contiguous after skipped nodes" \
  || _fail "snapshot refs contiguous" "sequence=$REF_SEQUENCE"

section "ref projection — unreadable subtree fallback preserves identity"

FAULT_ROLE=$(echo "$SNAPSHOT_JSON" | python3 -c '
import json, sys
d=json.load(sys.stdin)
roles={e.get("axPath", "").split("/")[-1].split("[")[0] for e in d.get("elements", [])}
print("AXRow" if "row" in {r.lower() for r in roles} else "")
')
if [[ -n "$FAULT_ROLE" ]]; then
  BASE_COUNT=$(echo "$SNAPSHOT_JSON" | python3 -c 'import json,sys; print(len(json.load(sys.stdin).get("elements", [])))')
  BASE_AFTER=$(echo "$SNAPSHOT_JSON" | python3 -c '
import json,sys
d=json.load(sys.stdin); es=d.get("elements", [])
print(next((e["ref"] for e in es if e.get("role")=="statictext"), ""))
')
  FAULT_JSON=$(CU_TEST_AX_BATCH_FAIL_ROLE="$FAULT_ROLE" COMPUTER_PILOT_BROKER_CHILD=1 "$CU" snapshot Finder --limit 120 2>/dev/null || true)
  FAULT_OK=$(echo "$FAULT_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("ok",False))' 2>/dev/null || echo false)
  if [[ "$FAULT_OK" == "True" ]]; then
    FAULT_COUNT=$(echo "$FAULT_JSON" | python3 -c 'import json,sys; print(len(json.load(sys.stdin).get("elements", [])))')
    FAULT_AFTER=$(echo "$FAULT_JSON" | python3 -c '
import json,sys
d=json.load(sys.stdin); es=d.get("elements", [])
print(next((e["ref"] for e in es if e.get("role")=="statictext"), ""))
')
    [[ "$FAULT_COUNT" == "$BASE_COUNT" ]] && _pass "batch failure keeps descendant count" || _fail "batch failure count" "baseline=$BASE_COUNT fault=$FAULT_COUNT"
    [[ "$FAULT_AFTER" == "$BASE_AFTER" ]] && _pass "batch failure keeps later refs" || _fail "batch failure later refs" "baseline=$BASE_AFTER fault=$FAULT_AFTER"
  else
    _fail "batch failure injection snapshot" "fault role=$FAULT_ROLE output=${FAULT_JSON:0:160}"
  fi

  CHILD_FAULT_JSON=$(CU_TEST_AX_BATCH_CHILDREN_FALLBACK_ROLE="$FAULT_ROLE" COMPUTER_PILOT_BROKER_CHILD=1 "$CU" snapshot Finder --limit 120 2>/dev/null || true)
  CHILD_FAULT_OK=$(echo "$CHILD_FAULT_JSON" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("ok",False))' 2>/dev/null || echo false)
  if [[ "$CHILD_FAULT_OK" == "True" ]]; then
    CHILD_FAULT_COUNT=$(echo "$CHILD_FAULT_JSON" | python3 -c 'import json,sys; print(len(json.load(sys.stdin).get("elements", [])))')
    CHILD_FAULT_AFTER=$(echo "$CHILD_FAULT_JSON" | python3 -c '
import json,sys
d=json.load(sys.stdin); es=d.get("elements", [])
print(next((e["ref"] for e in es if e.get("role")=="statictext"), ""))
')
    [[ "$CHILD_FAULT_COUNT" == "$BASE_COUNT" ]] && _pass "children-slot fallback keeps descendant count" || _fail "children-slot fallback count" "baseline=$BASE_COUNT fault=$CHILD_FAULT_COUNT"
    [[ "$CHILD_FAULT_AFTER" == "$BASE_AFTER" ]] && _pass "children-slot fallback keeps later refs" || _fail "children-slot fallback later refs" "baseline=$BASE_AFTER fault=$CHILD_FAULT_AFTER"
  else
    _fail "children-slot fallback injection snapshot" "fault role=$FAULT_ROLE output=${CHILD_FAULT_JSON:0:160}"
  fi
else
  _skip "unreadable subtree fallback" "no AXRow subtree in Finder snapshot"
fi

summary
