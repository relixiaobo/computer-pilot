#!/bin/bash
# Test: cu state — unified snapshot + windows + screenshot + frontmost (#4)
source "$(dirname "$0")/helpers.sh"

# Use Finder — always running, has at least one window.
section "state — full call returns all expected fields"

cu_json state Finder
assert_ok "state Finder returns ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('|'.join([
    'ok=' + str(d.get('ok')),
    'app=' + str(d.get('app')),
    'pid_ok=' + str(isinstance(d.get('pid'), int) and d.get('pid', 0) > 0),
    'frontmost_bool=' + str(isinstance(d.get('frontmost'), bool)),
    'windows_list=' + str(isinstance(d.get('windows'), list)),
    'displays_list=' + str(isinstance(d.get('displays'), list)),
    'elements_list=' + str(isinstance(d.get('elements'), list)),
    'snapshot_size_int=' + str(isinstance(d.get('snapshot_size'), int)),
    'truncated_bool=' + str(isinstance(d.get('tree_truncated'), bool)),
    'has_screenshot=' + str('screenshot' in d),
    'has_scale=' + str('image_scale' in d),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"ok=True"* ]]                   && _pass "ok=true"                || _fail "ok=true"                "$PARSED"
[[ "$PARSED" == *"app=Finder"* ]]                && _pass "app=Finder"             || _fail "app=Finder"             "$PARSED"
[[ "$PARSED" == *"pid_ok=True"* ]]               && _pass "pid populated"          || _fail "pid populated"          "$PARSED"
[[ "$PARSED" == *"frontmost_bool=True"* ]]       && _pass "frontmost is bool"      || _fail "frontmost is bool"      "$PARSED"
[[ "$PARSED" == *"windows_list=True"* ]]         && _pass "windows is array"       || _fail "windows is array"       "$PARSED"
[[ "$PARSED" == *"displays_list=True"* ]]        && _pass "displays is array"      || _fail "displays is array"      "$PARSED"
[[ "$PARSED" == *"elements_list=True"* ]]        && _pass "elements is array"      || _fail "elements is array"      "$PARSED"
[[ "$PARSED" == *"snapshot_size_int=True"* ]]    && _pass "snapshot_size set"      || _fail "snapshot_size set"      "$PARSED"
[[ "$PARSED" == *"truncated_bool=True"* ]]       && _pass "tree_truncated is bool" || _fail "tree_truncated is bool" "$PARSED"
[[ "$PARSED" == *"has_screenshot=True"* ]]       && _pass "screenshot path attached" || _fail "screenshot path attached" "$PARSED"
[[ "$PARSED" == *"has_scale=True"* ]]            && _pass "image_scale attached"   || _fail "image_scale attached"   "$PARSED"

# Verify the screenshot file actually exists and looks like a PNG.
SHOT_PATH=$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin).get('screenshot',''))" 2>/dev/null)
if [[ -n "$SHOT_PATH" && -f "$SHOT_PATH" ]]; then
  MAGIC=$(head -c 4 "$SHOT_PATH" | xxd -p 2>/dev/null)
  if [[ "$MAGIC" == "89504e47" ]]; then
    _pass "screenshot file is a PNG"
  else
    _fail "screenshot file is a PNG" "magic=$MAGIC"
  fi
  rm -f "$SHOT_PATH"
else
  _fail "screenshot file exists" "missing: $SHOT_PATH"
fi

section "state — --no-screenshot omits image fields"

cu_json state Finder --no-screenshot
assert_ok "state --no-screenshot returns ok"

NO_SHOT=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('has_screenshot=' + str('screenshot' in d) + '|has_scale=' + str('image_scale' in d))
" 2>/dev/null || echo "malformed")

[[ "$NO_SHOT" == *"has_screenshot=False"* ]] && _pass "screenshot omitted"   || _fail "screenshot omitted"   "$NO_SHOT"
[[ "$NO_SHOT" == *"has_scale=False"* ]]      && _pass "image_scale omitted"  || _fail "image_scale omitted"  "$NO_SHOT"

section "state — --output sets screenshot path"

OUT_PATH="/tmp/cu-state-test-$$.png"
cu_json state Finder --output "$OUT_PATH"
assert_ok "state --output returns ok"

if [[ -f "$OUT_PATH" ]]; then
  _pass "screenshot written to --output path"
  rm -f "$OUT_PATH"
else
  _fail "--output path written" "missing: $OUT_PATH"
fi

section "state — --limit shrinks the tree"

cu_json state Finder --limit 5 --no-screenshot
assert_ok "state --limit 5 returns ok"

SIZE=$(echo "$OUT" | python3 -c "import sys,json;print(json.load(sys.stdin).get('snapshot_size',-1))" 2>/dev/null || echo "-1")
if [[ "$SIZE" -le 5 && "$SIZE" -gt 0 ]]; then
  _pass "snapshot_size respects --limit (got $SIZE)"
else
  _fail "snapshot_size respects --limit" "got $SIZE"
fi

section "state — non-running app fails"

cu_json state NonExistentApp987654
assert_fail "state on non-existent app fails"

section "state — human mode"

cu_human state Finder --no-screenshot
assert_exit_zero "state human exits 0"
if echo "$OUT" | grep -qE "Finder \(pid [0-9]+\).*windows=.*elements="; then
  _pass "human prints 'app (pid N) windows=… elements=…'"
else
  _fail "human format" "${OUT:0:200}"
fi

summary
