#!/bin/bash
# Test: the packaged task launcher preserves -- text separators when adding --app
source "$(dirname "$0")/helpers.sh"

section "session launcher — --app before -- text separator"

TMP_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/cu-session-launcher.XXXXXX")
trap 'rm -rf "$TMP_ROOT"' EXIT
ARGS_FILE="$TMP_ROOT/args"
FAKE_BIN="$TMP_ROOT/fake-cu"
cat >"$FAKE_BIN" <<'EOF'
#!/bin/sh
printf '%s\n' "$@" >"$CU_SESSION_ARGS_FILE"
EOF
chmod 700 "$FAKE_BIN"

CU_BIN="$FAKE_BIN" \
CU_APP="pid:1234" \
CU_SESSION_ARGS_FILE="$ARGS_FILE" \
COMPUTER_PILOT_CLIENT_KEY=session-launcher-test \
  sh plugin/skills/computer-pilot/scripts/session.sh type --no-snapshot --no-verify -- '-leading value'

EXPECTED=$(cat <<'EOF'
--compact
type
--app
pid:1234
--no-snapshot
--no-verify
--
-leading value
EOF
)
ACTUAL=$(cat "$ARGS_FILE")
if [[ "$ACTUAL" == "$EXPECTED" ]]; then
  _pass "--app is inserted before the text separator"
else
  _fail "--app is inserted before the text separator" "got: $ACTUAL"
fi

CU_BIN="$FAKE_BIN" \
CU_APP="pid:1234" \
CU_SESSION_ARGS_FILE="$ARGS_FILE" \
COMPUTER_PILOT_CLIENT_KEY=session-launcher-test \
  sh plugin/skills/computer-pilot/scripts/session.sh find --role button --app pid:1234
APP_COUNT=$(grep -c '^--app$' "$ARGS_FILE")
if [[ "$APP_COUNT" == "1" ]]; then
  _pass "an explicit --app is not duplicated"
else
  _fail "an explicit --app is not duplicated" "count: $APP_COUNT"
fi

summary
