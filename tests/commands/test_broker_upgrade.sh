#!/bin/bash
# Test: Broker reuse across CLI versions (protocol-level compatibility)
#
# Behavior tests (Rule 1): the scenario this exists for is a global cu install
# and an Agent-bundled newer cu alternating on one machine. Same-protocol
# Brokers must be reused — never restarted per call — so alternating versions
# cannot churn the Broker or return target_busy. Cross-version cases run
# against the real official v0.7.2 release binary (protocol 2, older version
# string), downloaded once into tests/fixtures/ with a pinned SHA-256.
source "$(dirname "$0")/helpers.sh"

FIXTURE_VERSION="0.7.2"
FIXTURE="$ROOT_DIR/tests/fixtures/cu-v$FIXTURE_VERSION"
FIXTURE_SHA256="4b5bf85efcd5d2da22c238c2a0bdab3679bf0469566f454e5875722aecd9593d"
FIXTURE_URL="https://github.com/relixiaobo/computer-pilot/releases/download/v$FIXTURE_VERSION/cu-arm64"

SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/cu-broker-upgrade.XXXXXX")"

stop_sandbox_broker() {
  local home="$1" pid
  pid=$(_run_with_timeout 15 env COMPUTER_PILOT_HOME="$home" "$CU" status 2>/dev/null |
    python3 -c "import json,sys; print(json.load(sys.stdin).get('pid',''))" 2>/dev/null || true)
  [[ "$pid" =~ ^[0-9]+$ ]] && kill "$pid" 2>/dev/null || true
}

cleanup_sandbox() {
  stop_sandbox_broker "$SANDBOX/same"
  stop_sandbox_broker "$SANDBOX/cross"
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

section "broker reuse — same binary"

HOME_SAME="$SANDBOX/same"
mkdir -p "$HOME_SAME"

run_cu "$CU" "$HOME_SAME" status
assert_ok "first call starts a sandbox Broker"
PID_FIRST=$(json_get '.pid')
CURRENT_VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -1)"
assert_json_field "status reports the Broker version" ".version" "$CURRENT_VERSION"

run_cu "$CU" "$HOME_SAME" status
PID_SECOND=$(json_get '.pid')
if [[ -n "$PID_FIRST" && "$PID_FIRST" == "$PID_SECOND" ]]; then
  _pass "second call reuses the same Broker (pid $PID_FIRST)"
else
  _fail "second call reuses the same Broker" "pid $PID_FIRST -> $PID_SECOND"
fi

section "broker reuse — alternating official v$FIXTURE_VERSION and current"

if [[ ! -x "$FIXTURE" ]]; then
  if command -v curl >/dev/null 2>&1 &&
    curl -fsSL --retry 2 -o "$FIXTURE.download" "$FIXTURE_URL" 2>/dev/null; then
    ACTUAL_SHA=$(shasum -a 256 "$FIXTURE.download" | awk '{print $1}')
    if [[ "$ACTUAL_SHA" == "$FIXTURE_SHA256" ]]; then
      chmod +x "$FIXTURE.download"
      mv "$FIXTURE.download" "$FIXTURE"
    else
      rm -f "$FIXTURE.download"
      _fail "fixture download integrity" "SHA-256 mismatch for $FIXTURE_URL: got $ACTUAL_SHA"
    fi
  else
    rm -f "$FIXTURE.download" 2>/dev/null || true
  fi
fi

if [[ ! -x "$FIXTURE" ]]; then
  _skip "cross-version Broker reuse" \
    "official v$FIXTURE_VERSION binary unavailable (offline?) — expected at $FIXTURE"
else
  HOME_CROSS="$SANDBOX/cross"
  mkdir -p "$HOME_CROSS"

  # The OLD official CLI starts the Broker first — the deployed-global-install
  # scenario. Its Broker reports version 0.7.2 on protocol 2.
  run_cu "$FIXTURE" "$HOME_CROSS" status
  assert_ok "official v$FIXTURE_VERSION starts its Broker"
  PID_OLD=$(json_get '.pid')

  # The current CLI must reuse that Broker as-is: same protocol, different
  # version. A restart here is the churn bug this test exists to prevent.
  run_cu "$CU" "$HOME_CROSS" status
  assert_ok "current CLI accepts the v$FIXTURE_VERSION Broker"
  PID_NEW=$(json_get '.pid')
  if [[ -n "$PID_OLD" && "$PID_OLD" == "$PID_NEW" ]]; then
    _pass "current CLI reused the old Broker (pid $PID_OLD, no restart)"
  else
    _fail "current CLI reused the old Broker" "pid $PID_OLD -> $PID_NEW"
  fi
  # The old Broker predates the status version field — its absence here is
  # itself proof the v0.7.2 Broker is still the one serving requests.
  VERSION_FIELD=$(json_get '.version' || true)
  if [[ -z "$VERSION_FIELD" || "$VERSION_FIELD" == "__MISSING__" ]]; then
    _pass "responses still come from the v$FIXTURE_VERSION Broker"
  else
    _fail "responses still come from the v$FIXTURE_VERSION Broker" \
      "status.version='$VERSION_FIELD' implies a restarted Broker"
  fi

  # Alternate several rounds — the Tenon coexistence scenario. The Broker pid
  # must never move and no call may fail.
  STABLE=1
  for round in 1 2 3; do
    run_cu "$FIXTURE" "$HOME_CROSS" status
    [[ $EXIT -eq 0 && "$(json_get '.pid')" == "$PID_OLD" ]] || STABLE=0
    run_cu "$CU" "$HOME_CROSS" status
    [[ $EXIT -eq 0 && "$(json_get '.pid')" == "$PID_OLD" ]] || STABLE=0
  done
  if [[ $STABLE -eq 1 ]]; then
    _pass "3 alternating rounds: zero Broker churn, zero failures"
  else
    _fail "3 alternating rounds: zero Broker churn, zero failures" \
      "a call failed or the Broker pid moved (last: $(json_get '.pid') exit=$EXIT)"
  fi

  # Real work through the old Broker, not just pings: a snapshot command from
  # the current CLI coordinated by the v0.7.2 Broker must succeed.
  run_cu "$CU" "$HOME_CROSS" snapshot Finder --limit 3
  assert_ok "current CLI runs real commands through the v$FIXTURE_VERSION Broker"
fi

summary
