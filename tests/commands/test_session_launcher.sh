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

CU_BIN="$FAKE_BIN" \
CU_APP="pid:1234" \
CU_SESSION_ARGS_FILE="$ARGS_FILE" \
COMPUTER_PILOT_CLIENT_KEY=session-launcher-test \
  sh plugin/skills/computer-pilot/scripts/session.sh type --no-snapshot --no-verify -- --app

PAYLOAD_ARGS=$(cat "$ARGS_FILE")
if [[ "$PAYLOAD_ARGS" == *$'--app\npid:1234\n--no-snapshot\n--no-verify\n--\n--app' ]]; then
  _pass "payload --app does not suppress the bound target"
else
  _fail "payload --app does not suppress the bound target" "got: $PAYLOAD_ARGS"
fi

for command in state ocr menu; do
  CU_BIN="$FAKE_BIN" \
  CU_APP="pid:1234" \
  CU_SESSION_ARGS_FILE="$ARGS_FILE" \
  COMPUTER_PILOT_CLIENT_KEY=session-launcher-test \
    sh plugin/skills/computer-pilot/scripts/session.sh "$command"
  EXPECTED_PREFIX=$(printf '%s\n%s\n%s\n' '--compact' "$command" 'pid:1234')
  ACTUAL_PREFIX=$(head -n 3 "$ARGS_FILE")
  if [[ "$ACTUAL_PREFIX" == "$EXPECTED_PREFIX" ]]; then
    _pass "$command uses its positional app form"
  else
    _fail "$command uses its positional app form" "got: $ACTUAL_PREFIX"
  fi
done

section "documented launcher — preserve each host execution environment"
python3 - "$ROOT_DIR" "$TMP_ROOT" <<'PYTEST'
import os, pathlib, subprocess, sys
root, tmp = map(pathlib.Path, sys.argv[1:])
text = (root / "plugin/skills/computer-pilot/SKILL.md").read_text()
launcher = text.split("cat > ./pilot <<'EOF'\n", 1)[1].split("\nEOF", 1)[0]
fake = tmp / "host-cu"
fake.write_text('#!/bin/sh\nprintf "%s\\n" "$COMPUTER_PILOT_CLIENT_KEY" "$COMPUTER_PILOT_OUTPUT_DIR" "$COMPUTER_PILOT_INSTALL_ROOT" "$COMPUTER_PILOT_BIN_DIR" "$COMPUTER_PILOT_HOME"\n')
fake.chmod(0o700)
launcher = launcher.replace('/verified/path/to/cu', str(fake)).replace('/absolute/skill-directory', str(root / 'plugin/skills/computer-pilot'))
script = tmp / 'pilot'
script.write_text(launcher)
for call in ['one', 'two']:
    fields = dict(COMPUTER_PILOT_CLIENT_KEY='host-thread', COMPUTER_PILOT_OUTPUT_DIR=str(tmp/call), COMPUTER_PILOT_INSTALL_ROOT=str(tmp/'runtime'), COMPUTER_PILOT_BIN_DIR=str(tmp/'commands'), COMPUTER_PILOT_HOME=str(tmp/'broker'))
    actual = subprocess.check_output(['sh', str(script), 'screenshot'], env={**os.environ, **fields}, text=True).splitlines()
    assert actual == list(fields.values()), (actual, fields)
PYTEST
_pass "reused documented launcher inherits current host output, identity and install paths"

summary
