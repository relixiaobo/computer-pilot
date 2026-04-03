#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
CU="$ROOT_DIR/target/release/cu"

if [ ! -x "$CU" ]; then
  echo "Binary not found. Run: cargo build --release" >&2
  exit 1
fi

echo "=== cu status ==="
$CU --human status

echo "=== cu apps ==="
$CU --human apps | head -5

echo "=== cu snapshot ==="
$CU --human snapshot Finder --limit 3

echo "=== cu setup ==="
$CU --human setup

echo "=== cu screenshot ==="
$CU --human screenshot Finder --path /tmp/cu-test-screenshot.png
rm -f /tmp/cu-test-screenshot.png

echo "=== JSON output ==="
$CU status | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['ok']; print('JSON: ok')"

echo "=== error handling ==="
if $CU snapshot NonExistentApp12345 2>/dev/null; then
  echo "Expected failure" >&2; exit 1
fi
echo "Error handling: ok"

echo ""
echo "All tests passed."
