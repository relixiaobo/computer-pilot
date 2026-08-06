#!/bin/bash
# Test: cu setup
source "$(dirname "$0")/helpers.sh"

section "setup — JSON mode"

cu_json "setup"
assert_ok "setup returns ok"
assert_json_field "version present" ".version" "$(grep '^version' "$ROOT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')"
assert_json_field "platform is macos" ".platform" "macos"
assert_json_field_exists "accessibility field" ".accessibility"
assert_json_field_exists "screen_recording field" ".screen_recording"
assert_json_field_exists "ready field" ".ready"
assert_json_field "automation is per target app" ".automation.scope" "per_target_app"
assert_json_field "automation is not probed by setup" ".automation.status" "not_checked"
assert_json_field "scripting readiness is target-dependent" ".scripting_ready" "None"
assert_json_field_exists "scripting_ready field" ".scripting_ready"

section "setup — structured permissions and TCC subject"

assert_json_field_exists "accessibility settings_url" ".permissions.accessibility.settings_url"
assert_json_field_exists "screen_recording settings_url" ".permissions.screen_recording.settings_url"
assert_json_field_exists "automation settings_url" ".permissions.automation.settings_url"
assert_json_field "structured automation scope" ".permissions.automation.scope" "per_target_app"
assert_json_field_exists "tcc_subject executable" ".tcc_subject.executable"

# Structured granted must mirror the legacy booleans.
assert_json_field "structured accessibility mirrors legacy" \
  ".permissions.accessibility.granted" "$(json_get '.accessibility')"
assert_json_field "structured screen_recording mirrors legacy" \
  ".permissions.screen_recording.granted" "$(json_get '.screen_recording')"

# Principle 3: remediation is loud only on the degraded path — present exactly
# when the permission is missing, absent when granted.
for perm in accessibility screen_recording; do
  GRANTED=$(json_get ".permissions.$perm.granted")
  REMEDIATION=$(json_get ".permissions.$perm.remediation" || true)
  if [[ "$GRANTED" == "true" && -z "$REMEDIATION" ]]; then
    _pass "$perm granted -> no remediation"
  elif [[ "$GRANTED" == "false" && -n "$REMEDIATION" ]]; then
    _pass "$perm missing -> remediation present"
  else
    _fail "$perm remediation gating" "granted=$GRANTED remediation='$REMEDIATION'"
  fi
done

# The remediation subject must name the responsible process when one exists,
# otherwise the executable path.
RESPONSIBLE=$(json_get '.tcc_subject.responsible_process' || true)
EXECUTABLE=$(json_get '.tcc_subject.executable')
if [[ -n "$RESPONSIBLE" && "$RESPONSIBLE" != "None" ]]; then
  _pass "tcc_subject resolves the responsible process ($RESPONSIBLE)"
elif [[ -n "$EXECUTABLE" ]]; then
  _pass "tcc_subject falls back to the executable path"
else
  _fail "tcc_subject resolution" "neither responsible_process nor executable set"
fi

section "setup — human mode"

cu_human "setup"
assert_exit_zero "setup human exits 0"
assert_contains "shows version" "cu v"
assert_contains "shows accessibility" "Accessibility"
assert_contains "shows screen recording" "Screen Recording"

section "setup — permissions granted"

cu_json "setup"
ACCESSIBILITY=$(json_get '.accessibility')
SCREEN_REC=$(json_get '.screen_recording')
READY=$(json_get '.ready')

if [[ "$ACCESSIBILITY" == "true" ]]; then
  _pass "accessibility is granted"
else
  _skip "accessibility not granted" "grant in System Settings to test fully"
fi

if [[ "$SCREEN_REC" == "true" ]]; then
  _pass "screen recording is granted"
else
  _skip "screen recording not granted" "grant in System Settings to test fully"
fi

if [[ "$READY" == "true" ]]; then
  _pass "ready = true (both permissions)"
else
  _skip "not ready" "need both permissions"
fi

section "setup — capture-protected app enumeration"

cu_json "setup"
assert_json_field_exists "capture_protected_apps field present" ".capture_protected_apps"

# Cross-check setup against the actual capture behavior. WeChat versions differ:
# some expose sharing_state=0, while current builds may expose shareable windows.
if pgrep -x WeChat >/dev/null; then
  PROTECTED=$(echo "$OUT" | python3 -c "
import json, sys
d = json.load(sys.stdin)
print(','.join(d.get('capture_protected_apps', [])))
")

  PROBE_DIR="/private/tmp/cu-setup-protection-$$"
  mkdir -m 700 "$PROBE_DIR"
  PROBE_PATH="$PROBE_DIR/wechat.png"
  cu_json screenshot WeChat --path "$PROBE_PATH"
  PROBE_EXIT=$EXIT
  PROBE_ERROR=$ERR
  rm -f "$PROBE_PATH"
  rmdir "$PROBE_DIR" 2>/dev/null || true

  if [[ "$PROBE_EXIT" -ne 0 && "$PROBE_ERROR" == *"capture-protected"* ]]; then
    if [[ "$PROTECTED" == *"WeChat"* ]]; then
      _pass "capture-protected WeChat is enumerated"
    else
      _fail "capture-protected WeChat is enumerated" "screenshot refused capture but setup omitted WeChat"
    fi
  elif [[ "$PROBE_EXIT" -eq 0 ]]; then
    _skip "WeChat protection cross-check" "current WeChat windows allow capture"
  else
    _skip "WeChat protection cross-check" "screenshot probe unavailable: ${PROBE_ERROR:0:120}"
  fi
else
  _skip "WeChat enumeration" "WeChat not running — start it to verify capture-protected detection"
fi

summary
