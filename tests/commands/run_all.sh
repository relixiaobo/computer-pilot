#!/bin/bash
# Run all cu command tests
# Usage: bash tests/commands/run_all.sh [test_name ...]
# Examples:
#   bash tests/commands/run_all.sh              # run all
#   bash tests/commands/run_all.sh snapshot key  # run specific tests

set -uo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$DIR/../.." && pwd)"
CU="$ROOT/target/release/cu"

# Colors
if [[ -t 1 ]]; then
  GREEN='\033[0;32m'; RED='\033[0;31m'; CYAN='\033[0;36m'
  BOLD='\033[1m'; RESET='\033[0m'
else
  GREEN=''; RED=''; CYAN=''; BOLD=''; RESET=''
fi

# Check binary
if [[ ! -x "$CU" ]]; then
  echo "Binary not found: $CU"
  echo "Run: cargo build --release"
  exit 1
fi

echo -e "${BOLD}cu command test suite${RESET}"
echo "Binary: $CU"
echo "Version: $($CU --human setup 2>/dev/null | head -1 || echo 'unknown')"
echo ""

# Collect test files
TESTS=()
if [[ $# -gt 0 ]]; then
  for name in "$@"; do
    f="$DIR/test_${name}.sh"
    if [[ -f "$f" ]]; then
      TESTS+=("$f")
    else
      echo "Test not found: $f"
      exit 1
    fi
  done
else
  for f in "$DIR"/test_*.sh; do
    [[ -f "$f" ]] && TESTS+=("$f")
  done
fi

if [[ ${#TESTS[@]} -eq 0 ]]; then
  echo "No test files found."
  exit 1
fi

echo "Running ${#TESTS[@]} test files..."

# Run each test file, collect results
TOTAL_PASS=0
TOTAL_FAIL=0
TOTAL_SKIP=0
FAILED_SUITES=()

for test_file in "${TESTS[@]}"; do
  name=$(basename "$test_file" .sh | sed 's/^test_//')
  echo -e "\n${CYAN}${BOLD}━━━ $name ━━━${RESET}"

  # Run in subshell to isolate failures
  output=$(bash "$test_file" 2>&1) || true
  echo "$output"

  # Extract counts from the Results line
  pass=$(echo "$output" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' || echo "0")
  fail=$(echo "$output" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' || echo "0")
  skip=$(echo "$output" | grep -oE '[0-9]+ skipped' | grep -oE '[0-9]+' || echo "0")

  TOTAL_PASS=$((TOTAL_PASS + pass))
  TOTAL_FAIL=$((TOTAL_FAIL + fail))
  TOTAL_SKIP=$((TOTAL_SKIP + skip))

  if [[ "$fail" -gt 0 ]]; then
    FAILED_SUITES+=("$name")
  fi
done

# Final summary
TOTAL=$((TOTAL_PASS + TOTAL_FAIL + TOTAL_SKIP))
echo ""
echo -e "${BOLD}════════════════════════════════════════${RESET}"
echo -e "${BOLD}TOTAL: ${GREEN}${TOTAL_PASS} passed${RESET}, ${RED}${TOTAL_FAIL} failed${RESET}, ${TOTAL_SKIP} skipped / ${TOTAL}${RESET}"

if [[ ${#FAILED_SUITES[@]} -gt 0 ]]; then
  echo -e "${RED}Failed suites: ${FAILED_SUITES[*]}${RESET}"
fi

echo -e "${BOLD}════════════════════════════════════════${RESET}"

[[ $TOTAL_FAIL -eq 0 ]]
