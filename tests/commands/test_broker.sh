#!/bin/bash
# Test: private Broker identity, recovery, and public CLI boundary
source "$(dirname "$0")/helpers.sh"

BROKER_TEST_HOME="$(mktemp -d /tmp/cu-broker-test.XXXXXX)"
export COMPUTER_PILOT_HOME="$BROKER_TEST_HOME"
export CU_TEST_BROKER_CHILD_DELAY_MS=600

cleanup_broker() {
  if [[ -S "$BROKER_TEST_HOME/broker.sock" ]]; then
    local state pid
    state=$("$CU" --json --client-key test.cleanup status 2>/dev/null || true)
    pid=$(echo "$state" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("pid", ""))' 2>/dev/null || true)
    if [[ "$pid" =~ ^[0-9]+$ ]]; then
      kill "$pid" 2>/dev/null || true
    fi
  fi
  rm -rf "$BROKER_TEST_HOME"
}
trap 'cleanup_broker; cleanup_run' EXIT

section "broker — private public boundary"

OUT_TXT=$("$CU" --help 2>&1)
if echo "$OUT_TXT" | grep -q "__broker"; then
  _fail "internal Broker command hidden" "__broker appeared in help"
else
  _pass "internal Broker command hidden"
fi

EXIT=0
OUT=$("$CU" --json bridge --stdio 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_nonzero "removed public bridge is rejected"

section "broker — same-protocol version mismatch is reused, not restarted"

# The compatibility contract is INTERNAL_PROTOCOL, not the version string:
# a same-protocol Broker from another installed version must be reused as-is
# (alternating old/new CLIs must not churn each other's Broker).
REUSE_HOME="$BROKER_TEST_HOME/reuse-home"
python3 - "$REUSE_HOME" <<'PY' &
import json, os, signal, socket, sys
home = sys.argv[1]
signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
os.makedirs(home, mode=0o700, exist_ok=True)
token = "fake-reuse-token"
with open(home + "/broker.token", "w", encoding="utf-8") as handle:
    handle.write(token)
os.chmod(home + "/broker.token", 0o600)
path = home + "/broker.sock"
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(path)
server.listen()
while True:
    connection, _ = server.accept()
    request = json.loads(connection.makefile("rb").readline())
    if request.get("type") == "ping":
        response = {"type": "pong", "protocol": 2, "version": "0.0.0", "pid": os.getpid()}
    elif request.get("type") == "status":
        response = {"type": "status", "status": {
            "running": True, "pid": os.getpid(), "protocol": 2,
            "command_count": 0, "active_count": 0, "uncertain_count": 0,
        }}
    else:
        response = {"type": "error", "code": "invalid_argument", "error": "unexpected", "retryable": False}
    connection.sendall((json.dumps(response) + "\n").encode())
    connection.close()
PY
REUSE_BROKER_PID=$!
for _ in {1..100}; do
  [[ -S "$REUSE_HOME/broker.sock" ]] && break
  sleep 0.01
done
EXIT=0
OUT=$(COMPUTER_PILOT_HOME="$REUSE_HOME" "$CU" --json --client-key agent.upgrade status 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
REUSED_PID=$(echo "$OUT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("pid", ""))' 2>/dev/null || true)
if [[ "$EXIT" -eq 0 && "$REUSED_PID" == "$REUSE_BROKER_PID" ]] && kill -0 "$REUSE_BROKER_PID" 2>/dev/null; then
  _pass "same-protocol Broker with another version is reused in place"
else
  _fail "same-protocol Broker reuse" "exit=$EXIT served_pid=$REUSED_PID fake_pid=$REUSE_BROKER_PID stderr=${ERR:0:200}"
fi
kill "$REUSE_BROKER_PID" 2>/dev/null || true
wait "$REUSE_BROKER_PID" 2>/dev/null || true
rm -f "$REUSE_HOME/broker.sock"

section "broker — protocol mismatch performs a safe upgrade"

UPGRADE_HOME="$BROKER_TEST_HOME/upgrade-home"
python3 - "$UPGRADE_HOME" <<'PY' &
import json, os, socket, sys
home = sys.argv[1]
os.makedirs(home, mode=0o700, exist_ok=True)
token = "fake-upgrade-token"
with open(home + "/broker.token", "w", encoding="utf-8") as handle:
    handle.write(token)
os.chmod(home + "/broker.token", 0o600)
path = home + "/broker.sock"
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(path)
server.listen()
while True:
    connection, _ = server.accept()
    request = json.loads(connection.makefile("rb").readline())
    if request.get("type") == "ping":
        response = {"type": "pong", "protocol": 1, "version": "0.0.0", "pid": os.getpid()}
    elif request.get("type") == "stop_if_idle":
        response = {"type": "stopping"}
    else:
        response = {"type": "error", "code": "invalid_argument", "error": "unexpected", "retryable": False}
    connection.sendall((json.dumps(response) + "\n").encode())
    connection.close()
    if request.get("type") == "stop_if_idle":
        break
server.close()
try:
    os.unlink(path)
except FileNotFoundError:
    pass
PY
FAKE_BROKER_PID=$!
for _ in {1..100}; do
  [[ -S "$UPGRADE_HOME/broker.sock" ]] && break
  sleep 0.01
done
EXIT=0
OUT=$(COMPUTER_PILOT_HOME="$UPGRADE_HOME" "$CU" --json --client-key agent.upgrade status 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
if [[ "$EXIT" -eq 0 ]] && echo "$OUT" | python3 -c 'import json,sys; assert json.load(sys.stdin)["running"] is True' 2>/dev/null; then
  _pass "incompatible-protocol Broker is replaced through StopIfIdle"
else
  _fail "protocol-aware Broker upgrade" "exit=$EXIT stderr=${ERR:0:200}"
fi
UPGRADED_PID=$(echo "$OUT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("pid", ""))' 2>/dev/null || true)
if [[ "$UPGRADED_PID" =~ ^[0-9]+$ ]]; then
  kill "$UPGRADED_PID" 2>/dev/null || true
fi
wait "$FAKE_BROKER_PID" || true

section "broker — legacy upgrade preserves active work"

LEGACY_HOME="$BROKER_TEST_HOME/legacy-upgrade-home"
python3 - "$LEGACY_HOME" <<'PY' &
import json, os, signal, socket, sys
home = sys.argv[1]
signal.signal(signal.SIGTERM, lambda *_: sys.exit(0))
os.makedirs(home + "/commands", mode=0o700, exist_ok=True)
token = "fake-legacy-token"
with open(home + "/broker.token", "w", encoding="utf-8") as handle:
    handle.write(token)
os.chmod(home + "/broker.token", 0o600)
path = home + "/broker.sock"
server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(path)
server.listen()
while True:
    connection, _ = server.accept()
    request = json.loads(connection.makefile("rb").readline())
    if request.get("type") == "ping":
        response = {"type": "pong", "protocol": 1, "version": "0.5.9", "pid": os.getpid()}
    else:
        response = {
            "type": "error",
            "code": "invalid_argument",
            "error": "invalid private request: unknown variant `stop_if_idle`",
            "retryable": False,
        }
    connection.sendall((json.dumps(response) + "\n").encode())
    connection.close()
PY
LEGACY_BROKER_PID=$!
for _ in {1..100}; do
  [[ -S "$LEGACY_HOME/broker.sock" ]] && break
  sleep 0.01
done
cat >"$LEGACY_HOME/commands/active.json" <<'JSON'
{"descriptor":{"status":"dispatched"}}
JSON
EXIT=0
OUT=$(COMPUTER_PILOT_HOME="$LEGACY_HOME" "$CU" --json --client-key agent.upgrade status 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
LEGACY_BUSY_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
if [[ "$EXIT" -ne 0 && "$LEGACY_BUSY_CODE" == "target_busy" ]] && kill -0 "$LEGACY_BROKER_PID" 2>/dev/null && [[ -S "$LEGACY_HOME/broker.sock" ]]; then
  _pass "legacy Broker stays available while a command is active"
else
  _fail "legacy active upgrade guard" "exit=$EXIT code=$LEGACY_BUSY_CODE alive=$(kill -0 "$LEGACY_BROKER_PID" 2>/dev/null; echo $?) stderr=${ERR:0:200}"
fi
rm "$LEGACY_HOME/commands/active.json"
EXIT=0
OUT=$(COMPUTER_PILOT_HOME="$LEGACY_HOME" "$CU" --json --client-key agent.upgrade status 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
if [[ "$EXIT" -eq 0 ]] && echo "$OUT" | python3 -c 'import json,sys; assert json.load(sys.stdin)["running"] is True' 2>/dev/null; then
  _pass "idle legacy Broker is replaced without manual cleanup"
else
  _fail "legacy idle upgrade" "exit=$EXIT stderr=${ERR:0:200}"
fi
LEGACY_UPGRADED_PID=$(echo "$OUT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("pid", ""))' 2>/dev/null || true)
if [[ "$LEGACY_UPGRADED_PID" =~ ^[0-9]+$ ]]; then
  kill "$LEGACY_UPGRADED_PID" 2>/dev/null || true
fi
kill "$LEGACY_BROKER_PID" 2>/dev/null || true
wait "$LEGACY_BROKER_PID" || true

section "broker — command identity and replay"

EXIT=0
OUT=$("$CU" --json --client-key agent.alpha --request-id request-1 apps 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_zero "first request succeeds"
assert_json "first request is JSON"
FIRST_ID=$(echo "$OUT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("command_id", ""))')
if [[ "$FIRST_ID" == command:* ]]; then
  _pass "first request returns command_id"
else
  _fail "first request command_id" "got '$FIRST_ID'"
fi

EXIT=0
OUT=$("$CU" --json --client-key agent.alpha --request-id request-1 apps 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_zero "duplicate request succeeds"
assert_json_field "duplicate request is replayed" ".replayed" "true"
SECOND_ID=$(echo "$OUT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("command_id", ""))')
if [[ "$SECOND_ID" == "$FIRST_ID" ]]; then
  _pass "duplicate request preserves command_id"
else
  _fail "duplicate request command_id" "first=$FIRST_ID second=$SECOND_ID"
fi

section "broker — request identity survives restart"

EXIT=0
OUT=$("$CU" --json --client-key agent.restart --request-id restart-1 apps 2>/tmp/cu-test-stderr) || EXIT=$?
RESTART_FIRST_ID=$(echo "$OUT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("command_id", ""))' 2>/dev/null || true)
BROKER_STATE=$("$CU" --json --client-key agent.restart status 2>/dev/null || true)
BROKER_PID=$(echo "$BROKER_STATE" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("pid", ""))' 2>/dev/null || true)
if [[ "$BROKER_PID" =~ ^[0-9]+$ ]]; then
  kill "$BROKER_PID" 2>/dev/null || true
  for _ in {1..50}; do
    kill -0 "$BROKER_PID" 2>/dev/null || break
    sleep 0.02
  done
fi

EXIT=0
OUT=$("$CU" --json --client-key agent.restart --request-id restart-1 apps 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
RESTART_SECOND_ID=$(echo "$OUT" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("command_id", ""))' 2>/dev/null || true)
if [[ "$EXIT" -eq 0 && -n "$RESTART_FIRST_ID" && "$RESTART_SECOND_ID" == "$RESTART_FIRST_ID" ]]; then
  _pass "Broker restart preserves request command_id"
else
  _fail "Broker restart request identity" "exit=$EXIT first=$RESTART_FIRST_ID second=$RESTART_SECOND_ID"
fi
assert_json_field "Broker restart replays persisted result" ".replayed" "true"

section "broker — client isolation and recovery"

EXIT=0
OUT=$("$CU" --json --client-key agent.alpha status 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_zero "status succeeds"
assert_json_field "Broker reports running" ".running" "true"
assert_json_field "alpha has one command" ".command_count" "1"

EXIT=0
OUT=$("$CU" --json --client-key agent.alpha commands --limit 5 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_zero "commands succeeds"
assert_json_field "alpha command is visible" ".commands[0].id" "$FIRST_ID"

EXIT=0
OUT=$("$CU" --json --client-key agent.beta commands --limit 5 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_zero "isolated client commands succeeds"
COUNT=$(echo "$OUT" | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["commands"]))')
if [[ "$COUNT" == "0" ]]; then
  _pass "beta cannot see alpha commands"
else
  _fail "client command isolation" "beta saw $COUNT command(s)"
fi

EXIT=0
OUT=$("$CU" --json --client-key agent.alpha command "$FIRST_ID" 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_zero "command lookup succeeds"
assert_json_field "command lookup status" ".status" "completed"

section "broker — validation"

EXIT=0
OUT=$("$CU" --json --client-key x apps 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_nonzero "invalid client key rejected"
if echo "$ERR" | python3 -c 'import json,sys; assert json.load(sys.stdin)["code"] == "invalid_argument"' 2>/dev/null; then
  _pass "invalid client key has stable code"
else
  _fail "invalid client key code" "stderr=\${ERR:0:200}"
fi

section "broker — request ID conflict"

EXIT=0
OUT=$("$CU" --json --client-key agent.alpha --request-id request-1 setup 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
assert_exit_nonzero "request ID reuse with different command is rejected"
CONFLICT_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
if [[ "$CONFLICT_CODE" == "request_id_conflict" ]]; then
  _pass "request ID conflict has stable code"
else
  _fail "request ID conflict code" "got '$CONFLICT_CODE': ${ERR:0:160}"
fi

section "broker — independent reads run concurrently"

"$CU" --json --client-key agent.parallel --request-id read-1 apps >"$BROKER_TEST_HOME/read-1.out" 2>"$BROKER_TEST_HOME/read-1.err" &
READ_PID_1=$!
"$CU" --json --client-key agent.parallel --request-id read-2 apps >"$BROKER_TEST_HOME/read-2.out" 2>"$BROKER_TEST_HOME/read-2.err" &
READ_PID_2=$!
sleep 0.15
READS=$("$CU" --json --client-key agent.parallel commands --limit 10 2>/dev/null || true)
DISPATCHED_READS=$(echo "$READS" | python3 -c '
import json,sys
commands=json.load(sys.stdin).get("commands", [])
print(sum(c.get("request_id") in {"read-1","read-2"} and c.get("status")=="dispatched" for c in commands))
' 2>/dev/null || echo 0)
if [[ "$DISPATCHED_READS" == "2" ]]; then
  _pass "two independent reads are dispatched together"
else
  _fail "parallel read dispatch" "dispatched=$DISPATCHED_READS commands=${READS:0:240}"
fi
wait "$READ_PID_1" || true
wait "$READ_PID_2" || true

section "broker — upgrade stop checks global activity"

"$CU" --json --client-key agent.active --request-id active-read apps >"$BROKER_TEST_HOME/active.out" 2>"$BROKER_TEST_HOME/active.err" &
ACTIVE_PID=$!
sleep 0.15
STOP_RESPONSE=$(python3 - "$BROKER_TEST_HOME" <<'PY'
import json, socket, sys
home = sys.argv[1]
with open(home + "/broker.token", encoding="utf-8") as handle:
    token = handle.read().strip()
client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
client.connect(home + "/broker.sock")
client.sendall((json.dumps({"type": "stop_if_idle", "token": token}) + "\n").encode())
data = b""
while not data.endswith(b"\n"):
    chunk = client.recv(4096)
    if not chunk:
        break
    data += chunk
print(data.decode().strip())
PY
)
STOP_CODE=$(echo "$STOP_RESPONSE" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
if [[ "$STOP_CODE" == "target_busy" ]]; then
  _pass "safe upgrade observes commands from every client"
else
  _fail "safe upgrade global activity check" "response=${STOP_RESPONSE:0:240}"
fi
wait "$ACTIVE_PID" || true

section "broker — canonical app identity serializes aliases"

"$CU" --json --client-key agent.serial --request-id mutation-1 window move 250 150 --app Finder >"$BROKER_TEST_HOME/mutation-1.out" 2>"$BROKER_TEST_HOME/mutation-1.err" &
MUTATION_PID_1=$!
"$CU" --json --client-key agent.serial --request-id mutation-2 window move 250 150 --app com.apple.finder >"$BROKER_TEST_HOME/mutation-2.out" 2>"$BROKER_TEST_HOME/mutation-2.err" &
MUTATION_PID_2=$!
MUTATIONS=""
MUTATION_STATES=""
for _ in {1..40}; do
  MUTATIONS=$("$CU" --json --client-key agent.serial commands --limit 10 2>/dev/null || true)
  MUTATION_STATES=$(echo "$MUTATIONS" | python3 -c '
import json,sys
commands=json.load(sys.stdin).get("commands", [])
states=sorted(c.get("status") for c in commands if c.get("request_id") in {"mutation-1","mutation-2"})
print(",".join(states))
' 2>/dev/null || true)
  [[ "$MUTATION_STATES" == "accepted,dispatched" ]] && break
  sleep 0.05
done
if [[ "$MUTATION_STATES" == "accepted,dispatched" ]]; then
  _pass "app name and bundle ID share one mutation lock"
else
  _fail "canonical app mutation serialization" "states=$MUTATION_STATES commands=${MUTATIONS:0:240}"
fi
wait "$MUTATION_PID_1" || true
wait "$MUTATION_PID_2" || true

section "broker — launch holds the desktop lock"

"$CU" --json --client-key agent.launch-lock --request-id launch-lock-1 launch NonExistentLaunchTarget99999 --no-wait >"$BROKER_TEST_HOME/launch-lock-1.out" 2>"$BROKER_TEST_HOME/launch-lock-1.err" &
LAUNCH_PID=$!
"$CU" --json --client-key agent.launch-lock --request-id launch-lock-2 window move 250 150 --app Finder >"$BROKER_TEST_HOME/launch-lock-2.out" 2>"$BROKER_TEST_HOME/launch-lock-2.err" &
LAUNCH_MUTATION_PID=$!
sleep 0.15
LAUNCH_COMMANDS=$("$CU" --json --client-key agent.launch-lock commands --limit 10 2>/dev/null || true)
LAUNCH_STATES=$(echo "$LAUNCH_COMMANDS" | python3 -c '
import json,sys
commands=json.load(sys.stdin).get("commands", [])
states=sorted(c.get("status") for c in commands if c.get("request_id") in {"launch-lock-1","launch-lock-2"})
print(",".join(states))
' 2>/dev/null || true)
if [[ "$LAUNCH_STATES" == "accepted,dispatched" ]]; then
  _pass "launch serializes other desktop mutations"
else
  _fail "launch desktop lock" "states=$LAUNCH_STATES commands=${LAUNCH_COMMANDS:0:240}"
fi
wait "$LAUNCH_PID" || true
wait "$LAUNCH_MUTATION_PID" || true

section "broker — cancellation and deadlines"

"$CU" --json --client-key agent.cancel --request-id cancel-read apps >"$BROKER_TEST_HOME/cancel.out" 2>"$BROKER_TEST_HOME/cancel.err" &
CANCEL_PID=$!
sleep 0.15
CANCEL_COMMANDS=$("$CU" --json --client-key agent.cancel commands --limit 5 2>/dev/null || true)
CANCEL_ID=$(echo "$CANCEL_COMMANDS" | python3 -c 'import json,sys; print(json.load(sys.stdin)["commands"][0]["id"])' 2>/dev/null || true)
if [[ -n "$CANCEL_ID" ]]; then
  "$CU" --json --client-key agent.cancel cancel "$CANCEL_ID" >/dev/null 2>/dev/null || true
fi
wait "$CANCEL_PID" || true
CANCEL_CODE=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("code", ""))' "$BROKER_TEST_HOME/cancel.err" 2>/dev/null || true)
if [[ "$CANCEL_CODE" == "command_cancelled" ]]; then
  _pass "dispatched read cancellation has stable code"
else
  _fail "read cancellation" "id=$CANCEL_ID code=$CANCEL_CODE"
fi

EXIT=0
OUT=$("$CU" --json --client-key agent.deadline --request-id expire-read --timeout 100 apps 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
EXPIRED_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
if [[ "$EXIT" -ne 0 && "$EXPIRED_CODE" == "command_expired" ]]; then
  _pass "read deadline expires with command_expired"
else
  _fail "read deadline" "exit=$EXIT code=$EXPIRED_CODE stderr=${ERR:0:160}"
fi

EXIT=0
OUT=$("$CU" --json --client-key agent.deadline --request-id expire-mutation --timeout 100 window focus --app Finder 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
UNKNOWN_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
if [[ "$EXIT" -ne 0 && "$UNKNOWN_CODE" == "unknown_outcome" ]]; then
  _pass "mutation deadline reports unknown_outcome"
else
  _fail "mutation deadline" "exit=$EXIT code=$UNKNOWN_CODE stderr=${ERR:0:160}"
fi

section "broker — timeout terminates the complete worker process group"

ps -axo pid=,comm= | awk '$2 ~ /(^|\/)osascript$/ {print $1}' | sort -n >"$BROKER_TEST_HOME/osascript.before"
EXIT=0
OUT=$(CU_TEST_BROKER_CHILD_DELAY_MS=0 "$CU" --json --client-key agent.process-tree --request-id process-tree-1 --timeout 1000 tell Finder 'delay 20' 2>/tmp/cu-test-stderr) || EXIT=$?
ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
sleep 0.3
ps -axo pid=,comm= | awk '$2 ~ /(^|\/)osascript$/ {print $1}' | sort -n >"$BROKER_TEST_HOME/osascript.after"
NEW_OSASCRIPT=$(comm -13 "$BROKER_TEST_HOME/osascript.before" "$BROKER_TEST_HOME/osascript.after" | tr '\n' ' ')
TREE_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
if [[ "$EXIT" -ne 0 && "$TREE_CODE" == "unknown_outcome" && -z "$NEW_OSASCRIPT" ]]; then
  _pass "deadline leaves no descendant osascript process"
else
  _fail "worker process-group termination" "exit=$EXIT code=$TREE_CODE new_osascript=$NEW_OSASCRIPT"
fi

summary
