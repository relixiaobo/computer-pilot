#!/bin/bash
# Test: cu key
source "$(dirname "$0")/helpers.sh"

# Make sure TextEdit has a document. We do NOT activate — `cu key --app TextEdit`
# is PID-targeted, so TextEdit doesn't need to be frontmost.
osascript -e 'tell application "TextEdit" to make new document' 2>/dev/null
sleep 0.5

section "key — basic keys with --app"

cu_json "key enter --app TextEdit --no-snapshot"
assert_ok "key enter"
assert_json_field "combo echoed" ".combo" "enter"

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

cu_json "key escape --no-snapshot --allow-global"
assert_ok "key escape without --app (--allow-global)"

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

cu_json "key f1 --no-snapshot --allow-global"
assert_ok "key f1"

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

# --allow-global escape hatch bypasses the check even when frontmost is dangerous
OUT=$(CU_TEST_FRONTMOST_OVERRIDE=Terminal "$CU" key escape --no-snapshot --allow-global 2>&1) || true
if echo "$OUT" | grep -q '"ok":true'; then
  _pass "--allow-global bypasses safety check"
else
  _fail "--allow-global bypasses safety check" "got: $OUT"
fi

# --app sidesteps the check entirely (target is explicit)
cu_json "key escape --app TextEdit --no-snapshot"
assert_ok "--app bypasses safety check"

# Frontmost not in dangerous list → call proceeds (safety check is allow-by-default)
OUT=$(CU_TEST_FRONTMOST_OVERRIDE=Safari "$CU" key escape --no-snapshot 2>&1) || true
if echo "$OUT" | grep -q '"ok":true'; then
  _pass "non-dangerous frontmost allows global call"
else
  _fail "non-dangerous frontmost allows global call" "got: $OUT"
fi

section "key — human mode"

cu_human "key escape --app TextEdit"
assert_exit_zero "key human exits 0"
assert_contains "shows key info" "Sent key"

# Cleanup — `|| true` because quit may surface a save dialog (-128 user canceled),
# and we don't want set -e to kill the script before summary().
osascript -e 'tell application "TextEdit" to close every document saving no' >/dev/null 2>&1 || true
osascript -e 'tell application "TextEdit" to quit' >/dev/null 2>&1 || true

summary
