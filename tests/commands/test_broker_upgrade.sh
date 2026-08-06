#!/bin/bash
# Test: Broker reuse and retirement across CLI versions
#
# Behavior tests (Rule 1). The policy under test is `broker_is_current`:
# same protocol AND Broker version >= CLI version -> reuse as-is (a second CLI
# never restarts a Broker that already speaks its wire format); older or
# unidentifiable -> retire through StopIfIdle so a Broker-side fix actually
# takes effect after an upgrade; older but busy -> keep serving through it
# rather than failing the user's command.
#
# The matrix runs against scripted Brokers so both orderings are covered
# offline and deterministically. An optional section additionally exercises a
# real published release binary when one is available.
source "$(dirname "$0")/helpers.sh"

CURRENT_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -1)"
SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/cu-broker-upgrade.XXXXXX")"
FAKE_PIDS=()

cleanup_sandbox() {
  local pid home
  for pid in "${FAKE_PIDS[@]:-}"; do
    [[ -n "$pid" ]] && kill "$pid" 2>/dev/null || true
  done
  # Stop any real Broker this test started in its sandboxes.
  for home in "$SANDBOX"/*; do
    [[ -S "$home/broker.sock" ]] || continue
    pid=$(_run_with_timeout 15 env COMPUTER_PILOT_HOME="$home" "$CU" status 2>/dev/null |
      python3 -c "import json,sys; print(json.load(sys.stdin).get('pid',''))" 2>/dev/null || true)
    [[ "$pid" =~ ^[0-9]+$ ]] && kill "$pid" 2>/dev/null || true
  done
  rm -rf "$SANDBOX"
}
trap 'cleanup_sandbox; cleanup_run' EXIT

# Run a cu binary against a sandboxed Broker home. Sets OUT / ERR / EXIT.
run_cu() {
  local binary="$1" home="$2"
  shift 2
  EXIT=0
  OUT=$(_run_with_timeout 30 env COMPUTER_PILOT_HOME="$home" "$binary" "$@" \
    2>/tmp/cu-test-stderr) || EXIT=$?
  ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
}

# Start a scripted Broker: <home> <protocol> <version> <busy:true|false>.
# It answers ping/status, and on stop_if_idle either refuses with target_busy
# (busy=true) or replies stopping, removes its socket and exits — exactly what
# a real Broker does in each case. Echoes its pid.
start_fake_broker() {
  local home="$1" protocol="$2" version="$3" busy="$4"
  mkdir -p "$home"
  # stdout/stderr must be detached: callers use command substitution to read
  # the pid, and a background child holding that pipe open would hang it.
  python3 - "$home" "$protocol" "$version" "$busy" >/dev/null 2>&1 <<'PY' &
import json, os, signal, socket, sys
home, protocol, version, busy = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4] == "true"
signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
os.makedirs(home, mode=0o700, exist_ok=True)
with open(home + "/broker.token", "w", encoding="utf-8") as handle:
    handle.write("fake-broker-token")
os.chmod(home + "/broker.token", 0o600)
path = home + "/broker.sock"
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(path)
server.listen()
while True:
    connection, _ = server.accept()
    request = json.loads(connection.makefile("rb").readline())
    kind = request.get("type")
    if kind == "ping":
        response = {"type": "pong", "protocol": protocol, "version": version, "pid": os.getpid()}
    elif kind == "status":
        response = {"type": "status", "status": {
            "running": True, "pid": os.getpid(), "protocol": protocol, "version": version,
            "command_count": 0, "active_count": 0, "uncertain_count": 0,
        }}
    elif kind == "stop_if_idle" and busy:
        response = {"type": "error", "code": "target_busy",
                    "error": "1 persisted command(s) are still active", "retryable": True}
    elif kind == "stop_if_idle":
        response = {"type": "stopping"}
    else:
        response = {"type": "error", "code": "invalid_argument",
                    "error": "unexpected", "retryable": False}
    connection.sendall((json.dumps(response) + "\n").encode())
    connection.close()
    if kind == "stop_if_idle" and not busy:
        break
server.close()
try:
    os.unlink(path)
except FileNotFoundError:
    pass
PY
  local pid=$!
  FAKE_PIDS+=("$pid")
  local _attempt
  for _attempt in {1..200}; do
    [[ -S "$home/broker.sock" ]] && break
    sleep 0.01
  done
  echo "$pid"
}

served_pid() {
  python3 -c "import json,sys; print(json.load(sys.stdin).get('pid',''))" <<<"$OUT" 2>/dev/null || true
}

section "broker reuse — same binary"

HOME_SAME="$SANDBOX/same"
mkdir -p "$HOME_SAME"

run_cu "$CU" "$HOME_SAME" status
assert_ok "first call starts a sandbox Broker"
PID_FIRST=$(served_pid)
assert_json_field "status names the serving Broker version" ".version" "$CURRENT_VERSION"

run_cu "$CU" "$HOME_SAME" status
if [[ -n "$PID_FIRST" && "$PID_FIRST" == "$(served_pid)" ]]; then
  _pass "second call reuses the same Broker (pid $PID_FIRST)"
else
  _fail "second call reuses the same Broker" "pid $PID_FIRST -> $(served_pid)"
fi

section "broker reuse — a NEWER Broker serves an older CLI unchanged"

# The coexistence direction: an Agent-bundled newer cu started the Broker, then
# an older global install runs. Restarting here is the churn bug.
HOME_NEWER="$SANDBOX/newer"
NEWER_PID=$(start_fake_broker "$HOME_NEWER" 2 "99.0.0" false)
run_cu "$CU" "$HOME_NEWER" status
if [[ $EXIT -eq 0 && "$(served_pid)" == "$NEWER_PID" ]] && kill -0 "$NEWER_PID" 2>/dev/null; then
  _pass "newer same-protocol Broker is reused in place, not restarted"
else
  _fail "newer same-protocol Broker is reused in place" \
    "exit=$EXIT served=$(served_pid) fake=$NEWER_PID"
fi
kill "$NEWER_PID" 2>/dev/null || true

section "broker retirement — an OLDER Broker is replaced so upgrades take effect"

HOME_OLDER="$SANDBOX/older"
OLDER_PID=$(start_fake_broker "$HOME_OLDER" 2 "0.0.1" false)
run_cu "$CU" "$HOME_OLDER" status
assert_ok "command succeeds after retiring the older Broker"
if [[ "$(served_pid)" != "$OLDER_PID" ]]; then
  _pass "an older same-protocol Broker no longer serves the upgraded CLI"
else
  _fail "an older same-protocol Broker no longer serves the upgraded CLI" \
    "still served by fake pid $OLDER_PID — a Broker-side fix would never take effect"
fi
assert_json_field "the replacement reports this CLI's version" ".version" "$CURRENT_VERSION"

section "broker retirement — a busy older Broker keeps serving instead of failing"

HOME_BUSY="$SANDBOX/busy"
BUSY_PID=$(start_fake_broker "$HOME_BUSY" 2 "0.0.1" true)
run_cu "$CU" "$HOME_BUSY" status
if [[ $EXIT -eq 0 && "$(served_pid)" == "$BUSY_PID" ]]; then
  _pass "a busy older Broker serves the command rather than returning target_busy"
else
  _fail "a busy older Broker serves the command rather than returning target_busy" \
    "exit=$EXIT served=$(served_pid) fake=$BUSY_PID err=${ERR:0:160}"
fi
kill "$BUSY_PID" 2>/dev/null || true

section "broker retirement — an incompatible protocol is always replaced"

HOME_PROTO="$SANDBOX/protocol"
PROTO_PID=$(start_fake_broker "$HOME_PROTO" 1 "99.0.0" false)
run_cu "$CU" "$HOME_PROTO" status
assert_ok "command succeeds after replacing a protocol-1 Broker"
if [[ "$(served_pid)" != "$PROTO_PID" ]]; then
  _pass "a newer version does not excuse an incompatible protocol"
else
  _fail "a newer version does not excuse an incompatible protocol" "still served by fake pid $PROTO_PID"
fi

section "broker upgrade — real published release binary"

FIXTURE_VERSION="0.7.2"
FIXTURE="$ROOT_DIR/tests/fixtures/cu-v$FIXTURE_VERSION"
FIXTURE_SHA256="4b5bf85efcd5d2da22c238c2a0bdab3679bf0469566f454e5875722aecd9593d"
FIXTURE_URL="https://github.com/relixiaobo/computer-pilot/releases/download/v$FIXTURE_VERSION/cu-arm64"
FIXTURE_NOTE=""

if [[ ! -x "$FIXTURE" ]] && command -v curl >/dev/null 2>&1; then
  if curl -fsSL --retry 2 -o "$FIXTURE.download" "$FIXTURE_URL" 2>/dev/null; then
    ACTUAL_SHA=$(shasum -a 256 "$FIXTURE.download" | awk '{print $1}')
    if [[ "$ACTUAL_SHA" == "$FIXTURE_SHA256" ]]; then
      chmod +x "$FIXTURE.download"
      mv "$FIXTURE.download" "$FIXTURE"
    else
      rm -f "$FIXTURE.download"
      # Never execute an unpinned download. The subject of this suite is the
      # product, not the fixture, so an unusable fixture skips loudly instead
      # of failing an unrelated change.
      FIXTURE_NOTE="v$FIXTURE_VERSION cu-arm64 now hashes to $ACTUAL_SHA, not the pinned $FIXTURE_SHA256 — re-pin FIXTURE_SHA256 if the asset was deliberately re-signed"
    fi
  else
    rm -f "$FIXTURE.download" 2>/dev/null || true
    FIXTURE_NOTE="could not download the official v$FIXTURE_VERSION binary (offline?)"
  fi
fi

if [[ ! -x "$FIXTURE" ]]; then
  _skip "real-binary upgrade" \
    "${FIXTURE_NOTE:-official v$FIXTURE_VERSION binary unavailable} — the scripted-Broker matrix above still covers the policy"
else
  HOME_REAL="$SANDBOX/real"
  mkdir -p "$HOME_REAL"

  run_cu "$FIXTURE" "$HOME_REAL" status
  assert_ok "official v$FIXTURE_VERSION starts its own Broker"
  REAL_OLD_PID=$(served_pid)

  run_cu "$CU" "$HOME_REAL" status
  assert_ok "current CLI takes over from the v$FIXTURE_VERSION Broker"
  if [[ "$(served_pid)" != "$REAL_OLD_PID" ]]; then
    _pass "the real older Broker was retired, not kept"
  else
    _fail "the real older Broker was retired, not kept" "still served by pid $REAL_OLD_PID"
  fi
  assert_json_field "the replacement is this build" ".version" "$CURRENT_VERSION"

  # Both installs must keep working while they coexist. Released CLIs still
  # carry the exact-version check, so each may reclaim the Broker — what must
  # never happen is a failed command.
  ALL_OK=1
  for round in 1 2 3; do
    run_cu "$FIXTURE" "$HOME_REAL" status
    [[ $EXIT -eq 0 ]] || ALL_OK=0
    run_cu "$CU" "$HOME_REAL" status
    [[ $EXIT -eq 0 ]] || ALL_OK=0
  done
  if [[ $ALL_OK -eq 1 ]]; then
    _pass "3 alternating rounds: every command succeeded on both installs"
  else
    _fail "3 alternating rounds: every command succeeded on both installs" \
      "a call failed (last exit=$EXIT err=${ERR:0:160})"
  fi

  run_cu "$CU" "$HOME_REAL" snapshot Finder --limit 3
  assert_ok "current CLI runs real commands after the takeover"
fi

summary
