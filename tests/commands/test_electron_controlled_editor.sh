#!/bin/bash
# Behavior test: exact-PID delivery and controlled-editor failure semantics in
# two same-name Electron processes.
source "$(dirname "$0")/helpers.sh"

if ! interactive_tests_enabled; then
  section "electron controlled editor"
  _skip "dual Electron foreground fixture" "set COMPUTER_PILOT_TEST_INTERACTIVE=1 on a dedicated desktop"
  summary
  exit 0
fi

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FIXTURE="$ROOT/tests/fixtures/electron-controlled-editor"
ELECTRON_BIN="${COMPUTER_PILOT_TEST_ELECTRON_BIN:-}"

if [[ -z "$ELECTRON_BIN" ]]; then
  for candidate in \
    "$ROOT/../lin-outliner-codex/node_modules/electron/dist/Electron.app/Contents/MacOS/Electron" \
    "$ROOT/../lin-outliner-codex-2/node_modules/electron/dist/Electron.app/Contents/MacOS/Electron"; do
    if [[ -x "$candidate" ]]; then
      ELECTRON_BIN="$candidate"
      break
    fi
  done
fi

if [[ -z "$ELECTRON_BIN" || ! -x "$ELECTRON_BIN" ]]; then
  section "electron controlled editor"
  _skip "dual Electron fixture" "set COMPUTER_PILOT_TEST_ELECTRON_BIN to an Electron executable"
  summary
  exit 0
fi

TMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/cu-electron-fixture.XXXXXX")
PID_A=""
PID_B=""
cleanup() {
  [[ -n "$PID_A" ]] && kill "$PID_A" 2>/dev/null || true
  [[ -n "$PID_B" ]] && kill "$PID_B" 2>/dev/null || true
  [[ -n "$PID_A" ]] && wait "$PID_A" 2>/dev/null || true
  [[ -n "$PID_B" ]] && wait "$PID_B" 2>/dev/null || true
  rm -rf "$TMP_ROOT"
}
trap 'cleanup; cleanup_run' EXIT

FIXTURE_WINDOW_TITLE="CP Fixture A" FIXTURE_USER_DATA="$TMP_ROOT/a" \
  "$ELECTRON_BIN" "$FIXTURE" >"$TMP_ROOT/a.log" 2>&1 &
PID_A=$!
FIXTURE_WINDOW_TITLE="CP Fixture B" FIXTURE_USER_DATA="$TMP_ROOT/b" \
  "$ELECTRON_BIN" "$FIXTURE" >"$TMP_ROOT/b.log" 2>&1 &
PID_B=$!

wait_for_fixture() {
  local pid="$1"
  local attempts=0
  while [[ $attempts -lt 40 ]]; do
    if COMPUTER_PILOT_CLIENT_KEY=electron-fixture "$CU" state "pid:$pid" --no-screenshot >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
    attempts=$((attempts + 1))
  done
  return 1
}

section "electron controlled editor — two exact targets"

if ! wait_for_fixture "$PID_A" || ! wait_for_fixture "$PID_B"; then
  _fail "dual Electron fixture starts" "A=$(tail -5 "$TMP_ROOT/a.log") B=$(tail -5 "$TMP_ROOT/b.log")"
  summary
  exit 0
fi
_pass "dual Electron fixture starts"

export COMPUTER_PILOT_CLIENT_KEY="electron-controlled-editor-test"

cu_json state "pid:$PID_A" --no-screenshot
assert_json_field "fixture A window identity" ".windows[0].title" "CP Fixture A"
cu_json state "pid:$PID_B" --no-screenshot
assert_json_field "fixture B window identity" ".windows[0].title" "CP Fixture B"

cu_json state Electron --no-screenshot
assert_fail "same-name Electron selector is rejected"
AMBIGUOUS_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
[[ "$AMBIGUOUS_CODE" == "ambiguous_target" ]] \
  && _pass "same-name selector returns ambiguous_target" \
  || _fail "same-name selector code" "got '$AMBIGUOUS_CODE'"

section "electron controlled editor — verified foreground input"

cu_json window focus --app "pid:$PID_A"
assert_ok "focus fixture A"
assert_json_field "focus reports fixture A PID" ".frontmost_pid" "$PID_A"

cu_json snapshot "pid:$PID_A" --limit 100
OBS_A=$(json_get '.observation_id' || echo "")
REF_A=$(echo "$OUT" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(next(e["ref"] for e in d["elements"] if e["role"] in ("textfield","textarea")))' 2>/dev/null || echo "")
if [[ ! "$REF_A" =~ ^[0-9]+$ ]]; then
  _fail "fixture A exposes text input" "snapshot did not contain a text input"
  summary
  exit 0
fi
_pass "fixture A exposes text input"

cu_json click "$REF_A" --app "pid:$PID_A" --observation "$OBS_A"
assert_ok "click fixture A editor"
assert_json_field "editor focus verified" ".focus_verified" "true"

cu_json type "Probe" --app "pid:$PID_A" --no-snapshot
assert_ok "type prefix into fixture A"
assert_json_field "prefix effect verified" ".effect_verified" "true"
assert_json_field "WebArea input uses PID paste" ".method" "paste-pid"
PASTE_REASON=$(json_get '.paste_reason' || echo "")
[[ "$PASTE_REASON" == *"AXWebArea"* ]] \
  && _pass "WebArea paste reason is explicit" \
  || _fail "WebArea paste reason" "got '$PASTE_REASON'"

cu_json snapshot "pid:$PID_A" --limit 100
A_HAS_PROBE=$(echo "$OUT" | python3 -c 'import json,sys; d=json.load(sys.stdin); print("yes" if any("Probe" in str(e.get("value", "")) for e in d["elements"]) else "no")' 2>/dev/null || echo "no")
[[ "$A_HAS_PROBE" == "yes" ]] && _pass "fixture A draft changed" || _fail "fixture A draft changed" "Probe prefix absent"

cu_json snapshot "pid:$PID_B" --limit 100
B_HAS_PROBE=$(echo "$OUT" | python3 -c 'import json,sys; d=json.load(sys.stdin); print("yes" if any("Probe" in str(e.get("value", "")) for e in d["elements"]) else "no")' 2>/dev/null || echo "no")
[[ "$B_HAS_PROBE" == "no" ]] && _pass "fixture B was not modified" || _fail "fixture B isolation" "Probe prefix leaked"

section "electron controlled editor — AXValue risk is loud"

cu_json window focus --app "pid:$PID_B"
assert_ok "focus fixture B"
cu_json snapshot "pid:$PID_B" --limit 100
OBS_B=$(json_get '.observation_id' || echo "")
REF_B=$(echo "$OUT" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(next(e["ref"] for e in d["elements"] if e["role"] in ("textfield","textarea")))' 2>/dev/null || echo "")
cu_json set-value "$REF_B" "Shadow only" --app "pid:$PID_B" --observation "$OBS_B" --no-snapshot
assert_ok "AXValue write returns structured result"
assert_json_field "controlled editor risk marked" ".controlled_editor_risk" "true"
assert_json_field "controlled editor confidence low" ".confidence" "low"

cu_json ocr "pid:$PID_B"
OCR_TEXT=$(echo "$OUT" | python3 -c 'import json,sys; print(" ".join(t["text"] for t in json.load(sys.stdin).get("texts", [])))' 2>/dev/null || echo "")
if [[ "$OCR_TEXT" == *"Draft: empty"* ]]; then
  _pass "AXValue did not masquerade as controlled draft state"
else
  _fail "controlled draft remains empty" "OCR was: $OCR_TEXT"
fi

section "electron controlled editor — dropped background input fails loud"

FOCUS_B=false
for _ in 1 2; do
  cu_json snapshot "pid:$PID_B" --limit 100
  OBS_B=$(json_get '.observation_id' || echo "")
  REF_B=$(echo "$OUT" | python3 -c 'import json,sys; d=json.load(sys.stdin); print(next(e["ref"] for e in d["elements"] if e["role"] in ("textfield","textarea")))' 2>/dev/null || echo "")
  cu_json click "$REF_B" --app "pid:$PID_B" --observation "$OBS_B"
  if [[ "$EXIT" -eq 0 ]] && [[ "$(json_get '.focus_verified' 2>/dev/null || true)" == "true" ]]; then
    FOCUS_B=true
    break
  fi
done
if [[ "$FOCUS_B" == "true" ]]; then
  _pass "focus fixture B editor"
else
  _fail "focus fixture B editor" "two fresh-observation attempts failed: ${ERR:0:500}"
fi
cu_json window focus --app "pid:$PID_A"
assert_ok "move frontmost identity back to fixture A"

cu_json type "SHOULD_NOT_LAND" --app "pid:$PID_B" --no-snapshot
assert_fail "background Electron input is not reported as success"
TYPE_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
[[ "$TYPE_CODE" == "verification_failed" ]] \
  && _pass "dropped background input returns verification_failed" \
  || _fail "dropped background input code" "got '$TYPE_CODE': ${ERR:0:240}"

cu_json snapshot "pid:$PID_B" --limit 100
B_HAS_DROPPED=$(echo "$OUT" | python3 -c 'import json,sys; d=json.load(sys.stdin); print("yes" if any("SHOULD_NOT_LAND" in str(e.get("value", "")) for e in d["elements"]) else "no")' 2>/dev/null || echo "no")
[[ "$B_HAS_DROPPED" == "no" ]] && _pass "failed input did not alter fixture B" || _fail "failed input isolation" "text unexpectedly landed"

summary
