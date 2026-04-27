#!/bin/bash
# Test: cu wait — advanced conditions (C3)
source "$(dirname "$0")/helpers.sh"

section "wait — error: missing condition flag"

cu_json wait --app Finder --timeout 1
assert_fail "wait without any condition fails"
HINT=$(echo "${OUT:-$ERR}" | python3 -c "
import sys, json
try: print(json.load(sys.stdin).get('error', ''))
except Exception: pass
" 2>/dev/null || echo "")
if [[ "$HINT" == *"--new-window"* ]] || [[ "$HINT" == *"--modal"* ]]; then
  _pass "error message lists new conditions"
else
  _fail "error mentions new conditions" "got: $HINT"
fi

section "wait --new-window — times out when no new window opens"

# Make sure there's at least one Finder window so baseline > 0. We do NOT
# activate Finder — wait works over AX without requiring it frontmost.
osascript -e 'tell application "Finder"
  if (count of Finder windows) = 0 then
    make new Finder window
  end if
end tell' 2>/dev/null
sleep 0.4

START=$(python3 -c "import time; print(int(time.time()*1000))")
cu_json wait --new-window --app Finder --timeout 2
END=$(python3 -c "import time; print(int(time.time()*1000))")
ELAPSED=$((END - START))
assert_fail "no new window → timeout (exit 1)"
if [[ "$ELAPSED" -ge 1500 && "$ELAPSED" -le 4000 ]]; then
  _pass "new-window timeout duration ~2s ($ELAPSED ms)"
else
  _fail "new-window timing" "elapsed=$ELAPSED"
fi

section "wait --new-window — succeeds when one is opened during the wait"

# Open a new Finder window 800ms in
(sleep 0.8 && osascript -e 'tell application "Finder" to make new Finder window' >/dev/null 2>&1) &
HELPER=$!

cu_json wait --new-window --app Finder --timeout 5
assert_ok "new-window detected → ok"
ELAPSED_MS=$(json_get '.elapsed_ms' || echo "9999")
if [[ "$ELAPSED_MS" -ge 500 && "$ELAPSED_MS" -le 4500 ]]; then
  _pass "new-window detected within window (${ELAPSED_MS}ms)"
else
  _fail "new-window timing" "elapsed_ms=$ELAPSED_MS"
fi
wait $HELPER 2>/dev/null || true

section "wait --modal — times out when no modal appears"

# Best-effort: TextEdit Cmd+W rarely shows save sheet on this Mac (iCloud auto-save).
# So we only assert the timeout path here.
cu_json wait --modal --app Finder --timeout 1
assert_fail "no modal → timeout"

section "wait --focused-changed — error path verifiable; timing depends on env"

# Sanity-check that the flag is wired (timeout fires when nothing changes).
cu_json wait --focused-changed --app Finder --timeout 1
assert_fail "no focus change → timeout"

# Cleanup: close any extra Finder windows the test opened
osascript -e 'tell application "Finder"
  set wlist to every Finder window
  if (count of wlist) > 1 then
    repeat with i from (count of wlist) to 2 by -1
      try
        close item i of wlist
      end try
    end repeat
  end if
end tell' >/dev/null 2>&1

summary
