#!/bin/bash
# Test: the promotion gate tells a network failure apart from a missing release.
#
# `gh ... || fail "release does not exist"` treats every non-zero exit the
# same, so a TLS timeout — which this repo hit for real while promoting
# v0.9.2 — reads as "REFUSED: release v0.9.2 does not exist or is not
# visible". The operator then goes looking for a broken release that is fine.
# Same defect class as grading a check against a failed command's output,
# this time in the last step of the release chain.
source "$(dirname "$0")/helpers.sh"

section "promote — a network failure is not a verdict on the release"

PROMOTE="$ROOT_DIR/scripts/promote-skill-stable.sh"
SANDBOX=$(mktemp -d "${TMPDIR:-/tmp}/cu-promote-transport.XXXXXX")
cleanup_sandbox() { rm -rf "$SANDBOX"; }
trap cleanup_sandbox EXIT

# A gh shim on PATH decides what "GitHub" says. Everything else about the
# script is untouched, so this exercises the real control flow.
make_gh_shim() {
  local mode=$1
  mkdir -p "$SANDBOX/bin"
  case "$mode" in
    transport)
      cat >"$SANDBOX/bin/gh" <<'EOF'
#!/bin/sh
echo 'Get "https://api.github.com/repos/x/y/releases/tags/v9.9.9": net/http: TLS handshake timeout' >&2
exit 1
EOF
      ;;
    missing)
      cat >"$SANDBOX/bin/gh" <<'EOF'
#!/bin/sh
echo 'release not found' >&2
exit 1
EOF
      ;;
  esac
  chmod +x "$SANDBOX/bin/gh"
}

run_promote() {
  (cd "$ROOT_DIR" && PATH="$SANDBOX/bin:$PATH" bash "$PROMOTE" 9.9.9 2>&1) || true
}

# --- transport failure ------------------------------------------------------
make_gh_shim transport
START=$(python3 -c "import time; print(int(time.time()))")
OUT=$(run_promote)
ELAPSED=$(( $(python3 -c "import time; print(int(time.time()))") - START ))

if [[ "$OUT" == *"network failure"* ]]; then
  _pass "a transport failure is reported as a network failure"
else
  _fail "a transport failure is reported as a network failure" "got: ${OUT:0:220}"
fi

if [[ "$OUT" != *"does not exist or is not visible"* ]]; then
  _pass "a transport failure never claims the release is missing"
else
  _fail "a transport failure never claims the release is missing" "got: ${OUT:0:220}"
fi

# 3 attempts with 5s and 10s backoff — proof it retried rather than giving up
# on the first blip, which is what made promotion unusable on a degraded link.
if [[ "$ELAPSED" -ge 10 ]]; then
  _pass "a transport failure is retried before giving up (${ELAPSED}s)"
else
  _fail "a transport failure is retried before giving up" "returned after ${ELAPSED}s — no backoff happened"
fi

# --- a definitive answer ----------------------------------------------------
make_gh_shim missing
START=$(python3 -c "import time; print(int(time.time()))")
OUT=$(run_promote)
ELAPSED=$(( $(python3 -c "import time; print(int(time.time()))") - START ))

if [[ "$OUT" == *"does not exist or is not visible"* ]]; then
  _pass "GitHub saying no still refuses the promotion"
else
  _fail "GitHub saying no still refuses the promotion" "got: ${OUT:0:220}"
fi

# The gate must not soften: a real "not found" is answered immediately, never
# retried into a maybe.
if [[ "$ELAPSED" -lt 5 ]]; then
  _pass "a definitive refusal is not retried (${ELAPSED}s)"
else
  _fail "a definitive refusal is not retried" "took ${ELAPSED}s — it retried a verdict"
fi

summary
