#!/bin/bash
# Quick local test: run a few macOSWorld tasks on this machine.
# No AWS needed. Uses cu directly.
set -euo pipefail

CU="${CU:-./target/release/cu}"
RESULTS_DIR="${RESULTS_DIR:-./test-results/macosworld-local}"
PASS=0
FAIL=0
SKIP=0

if [ ! -x "$CU" ]; then
  echo "cu binary not found. Run: cargo build --release" >&2
  exit 1
fi

mkdir -p "$RESULTS_DIR"

# ── Test helpers ─────────────────────────────────────────────────────────────

run_test() {
  local name="$1"
  local description="$2"
  local grade_cmd="$3"

  echo -n "  $name: $description ... "

  # Run grading command
  if eval "$grade_cmd" > /dev/null 2>&1; then
    echo "PASS"
    PASS=$((PASS + 1))
  else
    echo "FAIL"
    FAIL=$((FAIL + 1))
  fi
}

skip_test() {
  local name="$1"
  local reason="$2"
  echo "  $name: SKIP ($reason)"
  SKIP=$((SKIP + 1))
}

# ── Core capability tests ────────────────────────────────────────────────────

echo "=== cu capability tests ==="

echo ""
echo "--- Perception ---"
run_test "snapshot" "AX tree returns elements" \
  "$CU snapshot Finder --limit 5 | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"ok\"] and len(d[\"elements\"])>0'"

run_test "screenshot" "Window capture without activation" \
  "$CU screenshot Finder --path /tmp/cu-test-cap.png | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"ok\"]' && test -f /tmp/cu-test-cap.png && rm /tmp/cu-test-cap.png"

run_test "ocr" "Vision OCR returns text" \
  "$CU ocr Finder | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"ok\"] and len(d[\"texts\"])>0'"

run_test "apps" "Lists running apps" \
  "$CU apps | python3 -c 'import sys,json; d=json.load(sys.stdin); assert len(d[\"apps\"])>0'"

run_test "wait" "Wait for known text" \
  "$CU wait --text Favorites --app Finder --timeout 3 | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"ok\"]'"

echo ""
echo "--- Actions ---"
run_test "click-coord" "Click at coordinates" \
  "$CU click 0 0 --no-snapshot | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"ok\"]'"

run_test "click-ref" "Click element by ref" \
  "$CU click 1 --app Finder --no-snapshot | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"ok\"]'"

run_test "key" "Send keyboard shortcut" \
  "$CU key escape --no-snapshot | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"ok\"]'"

run_test "scroll" "Scroll at coordinates" \
  "$CU scroll down 3 --x 400 --y 400 | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"ok\"]'"

run_test "hover" "Hover at coordinates" \
  "$CU hover 400 400 | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"ok\"]'"

run_test "clipboard" "Copy and paste via key+pbpaste" \
  "echo -n 'test-cu-clip' | pbcopy && pbpaste | grep -q 'test-cu-clip'"

echo ""
echo "--- Error handling ---"
run_test "bad-ref" "Invalid ref returns error" \
  "! $CU click 9999 --app Finder --no-snapshot 2>/dev/null"

run_test "bad-app" "Missing app returns error" \
  "! $CU snapshot NonExistent12345 2>/dev/null"

run_test "setup" "Permission check works" \
  "$CU setup | python3 -c 'import sys,json; d=json.load(sys.stdin); assert d[\"ok\"]'"

echo ""
echo "--- Performance ---"
run_test "snapshot-speed" "Snapshot 200 elements completes" \
  "$CU snapshot Finder --limit 200 > /dev/null"

run_test "screenshot-speed" "Screenshot completes" \
  "$CU screenshot Finder --path /tmp/cu-perf.png > /dev/null && rm -f /tmp/cu-perf.png"

# ── Auto-snapshot contract ───────────────────────────────────────────────────

echo ""
echo "--- Auto-snapshot ---"
run_test "auto-snap-key" "key returns snapshot in JSON" \
  "$CU key escape | python3 -c 'import sys,json; d=json.load(sys.stdin); assert \"snapshot\" in d'"

run_test "no-snap-flag" "--no-snapshot suppresses snapshot" \
  "$CU key escape --no-snapshot | python3 -c 'import sys,json; d=json.load(sys.stdin); assert \"snapshot\" not in d'"

# ── Summary ──────────────────────────────────────────────────────────────────

echo ""
echo "=== Results: $PASS passed, $FAIL failed, $SKIP skipped ==="

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
