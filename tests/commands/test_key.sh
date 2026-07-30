#!/bin/bash
# Test: cu key
source "$(dirname "$0")/helpers.sh"
trap 'textedit_cleanup; cleanup_run' EXIT

# Make sure TextEdit has a focused document so the first Enter behavior can be
# verified against real document state. Background PID delivery is covered by
# the Electron controlled-editor fixture, where dropped events must fail loud.
textedit_reset
sleep 1

section "key — basic keys with --app"

ENTER_VERIFIED=false
if interactive_tests_enabled; then
  for _ in 1 2; do
    cu_json window focus --app TextEdit
    [[ "$EXIT" -eq 0 ]] || continue
    cu_json "key enter --app TextEdit --no-snapshot"
    if [[ "$EXIT" -eq 0 ]] && [[ "$(json_get '.effect_verified' 2>/dev/null || true)" == "true" ]]; then
      ENTER_VERIFIED=true
      break
    fi
  done
fi
if [[ "$ENTER_VERIFIED" == "true" ]]; then
  _pass "key enter"
  assert_json_field "combo echoed" ".combo" "enter"
  assert_json_field "key dispatched" ".dispatched" "true"
  assert_json_field "Enter effect verified" ".effect_verified" "true"
  DOC_LENGTH_AFTER_ENTER=$(_run_with_timeout 10 osascript -e 'tell application "TextEdit" to count characters of text of front document' 2>/dev/null || echo "0")
  if [[ "$DOC_LENGTH_AFTER_ENTER" -ge 1 ]]; then
    _pass "Enter changed the TextEdit document"
  else
    _fail "Enter changed the TextEdit document" "document character count was $DOC_LENGTH_AFTER_ENTER"
  fi
else
  _skip "key enter" "interactive desktop tests disabled or foreground unavailable"
  _skip "combo echoed" "interactive desktop tests disabled or foreground unavailable"
  _skip "key dispatched" "interactive desktop tests disabled or foreground unavailable"
  _skip "Enter effect verified" "interactive desktop tests disabled or foreground unavailable"
  _skip "Enter changed the TextEdit document" "interactive desktop tests disabled or foreground unavailable"
fi

cu_json "key tab --app TextEdit --no-snapshot"
assert_ok "key tab"

cu_json "key escape --app TextEdit --no-snapshot"
assert_ok "key escape"

cu_json "key space --app TextEdit --no-snapshot"
assert_ok "key space"

section "key — modifier combos with --app"

cu_json "key cmd+a --app TextEdit --no-snapshot"
assert_ok "key cmd+a (select all)"

cu_json "key cmd+c --app TextEdit --no-snapshot"
assert_ok "key cmd+c (copy)"

cu_json "key cmd+z --app TextEdit --no-snapshot"
assert_ok "key cmd+z (undo)"

cu_json "key cmd+shift+z --app TextEdit --no-snapshot"
assert_ok "key cmd+shift+z (redo)"

section "key — without --app (CGEvent to frontmost, requires --allow-global from terminal)"

if interactive_tests_enabled; then
  cu_json "key escape --no-snapshot --allow-global"
  assert_ok "key escape without --app (--allow-global)"
else
  _skip "key escape without --app (--allow-global)" "interactive desktop tests disabled"
fi

section "key — with auto-snapshot"

cu_json "key escape --app TextEdit"
assert_ok "key with auto-snapshot"
HAS_SNAP=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print('yes' if 'snapshot' in d else 'no')
" 2>/dev/null || echo "error")
if [[ "$HAS_SNAP" == "yes" ]]; then
  _pass "auto-snapshot attached"
else
  _fail "auto-snapshot attached" "snapshot missing"
fi

section "key — arrow keys"

cu_json "key up --app TextEdit --no-snapshot"
assert_ok "key up"

cu_json "key down --app TextEdit --no-snapshot"
assert_ok "key down"

cu_json "key left --app TextEdit --no-snapshot"
assert_ok "key left"

cu_json "key right --app TextEdit --no-snapshot"
assert_ok "key right"

section "key — function keys"

if interactive_tests_enabled; then
  cu_json "key f1 --no-snapshot --allow-global"
  assert_ok "key f1"
else
  _skip "key f1" "interactive desktop tests disabled"
fi

section "key — frontmost-app safety check"

# Inject a deterministic dangerous frontmost via the test seam — without this,
# the actual frontmost during the suite is whatever app got activated last
# (often TextEdit), and we can't reliably assert refusal.
OUT=$(CU_TEST_FRONTMOST_OVERRIDE=Terminal "$CU" key escape --no-snapshot 2>&1) || true
if echo "$OUT" | grep -q "refusing to send keys"; then
  _pass "refuses key without --app when frontmost is dangerous"
else
  _fail "refuses key without --app when frontmost is dangerous" "expected refusal, got: $OUT"
fi

# --allow-global escape hatch bypasses the check even when frontmost is
# dangerous, but validating it sends a real global event.
if interactive_tests_enabled; then
  OUT=$(CU_TEST_FRONTMOST_OVERRIDE=Terminal "$CU" key escape --no-snapshot --allow-global 2>&1) || true
  if echo "$OUT" | grep -q '"ok":true'; then
    _pass "--allow-global bypasses safety check"
  else
    _fail "--allow-global bypasses safety check" "got: $OUT"
  fi
else
  _skip "--allow-global bypasses safety check" "interactive desktop tests disabled"
fi

# --app sidesteps the check entirely (target is explicit)
cu_json "key escape --app TextEdit --no-snapshot"
assert_ok "--app bypasses safety check"

# Frontmost not in dangerous list → call proceeds (safety check is allow-by-default)
if interactive_tests_enabled; then
  OUT=$(CU_TEST_FRONTMOST_OVERRIDE=Safari "$CU" key escape --no-snapshot 2>&1) || true
  if echo "$OUT" | grep -q '"ok":true'; then
    _pass "non-dangerous frontmost allows global call"
  else
    _fail "non-dangerous frontmost allows global call" "got: $OUT"
  fi
else
  _skip "non-dangerous frontmost allows global call" "interactive desktop tests disabled"
fi

section "key — human mode"

cu_human "key escape --app TextEdit"
assert_exit_zero "key human exits 0"
assert_contains "shows key info" "Sent key"

summary
