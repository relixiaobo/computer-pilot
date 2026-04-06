#!/bin/bash
# Test helpers — sourced by each test_*.sh file
# Provides: assertions, pass/fail tracking, JSON helpers

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CU="$ROOT_DIR/target/release/cu"

# ── Counters ────────────────────────────────────────────────────────────────

PASS=0
FAIL=0
SKIP=0
ERRORS=()

# ── Colors ──────────────────────────────────────────────────────────────────

if [[ -t 1 ]]; then
  GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'
  CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'
else
  GREEN=''; RED=''; YELLOW=''; CYAN=''; BOLD=''; RESET=''
fi

# ── Core helpers ────────────────────────────────────────────────────────────

_pass() {
  ((PASS++))
  echo -e "  ${GREEN}PASS${RESET} $1"
}

_fail() {
  ((FAIL++))
  ERRORS+=("$1: $2")
  echo -e "  ${RED}FAIL${RESET} $1 — $2"
}

_skip() {
  ((SKIP++))
  echo -e "  ${YELLOW}SKIP${RESET} $1 — $2"
}

section() {
  echo -e "\n${CYAN}${BOLD}── $1 ──${RESET}"
}

# Print summary; return 1 if any failures
summary() {
  local total=$((PASS + FAIL + SKIP))
  echo ""
  echo -e "${BOLD}Results: ${GREEN}${PASS} passed${RESET}, ${RED}${FAIL} failed${RESET}, ${YELLOW}${SKIP} skipped${RESET} / ${total} total"
  if [[ ${#ERRORS[@]} -gt 0 ]]; then
    echo -e "\n${RED}Failures:${RESET}"
    for e in "${ERRORS[@]}"; do
      echo "  - $e"
    done
  fi
  [[ $FAIL -eq 0 ]]
}

# ── cu runners ──────────────────────────────────────────────────────────────

# Run cu in JSON mode (piped), capture stdout+stderr+exit code
# Usage: cu_json "snapshot Finder --limit 5"
#   or:  cu_json snapshot Finder --limit 5
# Sets: OUT (stdout), ERR (stderr), EXIT (exit code)
# When called with one arg:  cu_json "snapshot Finder --limit 5"  → word-split
# When called with many args: cu_json type "hello world" --app X  → proper quoting
cu_json() {
  EXIT=0
  if [[ $# -eq 1 ]]; then
    # Single string arg — word-split it (legacy callers)
    # shellcheck disable=SC2086
    OUT=$($CU $1 2>/tmp/cu-test-stderr) || EXIT=$?
  else
    OUT=$("$CU" "$@" 2>/tmp/cu-test-stderr) || EXIT=$?
  fi
  ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
}

# Run cu in human mode (--human flag)
cu_human() {
  EXIT=0
  if [[ $# -eq 1 ]]; then
    # shellcheck disable=SC2086
    OUT=$($CU --human $1 2>/tmp/cu-test-stderr) || EXIT=$?
  else
    OUT=$("$CU" --human "$@" 2>/tmp/cu-test-stderr) || EXIT=$?
  fi
  ERR=$(cat /tmp/cu-test-stderr 2>/dev/null || true)
}

# ── JSON helpers (require python3) ──────────────────────────────────────────

# Extract a JSON field: json_get '.ok'  or  json_get '.elements | length'
json_get() {
  echo "$OUT" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
except: sys.exit(1)
import functools
keys = '''$1'''.strip('.').split('.')
val = d
for k in keys:
    if k.endswith(']'):
        # handle array index like 'elements[0]'
        name, idx = k[:-1].split('[')
        val = val[name][int(idx)]
    elif '|' in k:
        # pipe: only support 'length'
        parts = k.split('|')
        field = parts[0].strip()
        op = parts[1].strip()
        if field: val = val[field]
        if op == 'length': val = len(val)
    else:
        val = val[k]
print(val if not isinstance(val, bool) else str(val).lower())
" 2>/dev/null
}

# Check if OUT is valid JSON
is_json() {
  echo "$OUT" | python3 -c "import sys,json; json.load(sys.stdin)" 2>/dev/null
}

# ── Assertions ──────────────────────────────────────────────────────────────

# assert_ok "test name" — OUT must be valid JSON with .ok == true
assert_ok() {
  local name="$1"
  if ! is_json; then
    _fail "$name" "output is not valid JSON"
    return
  fi
  local ok
  ok=$(json_get '.ok' || echo "")
  if [[ "$ok" == "true" ]]; then
    _pass "$name"
  else
    _fail "$name" "expected ok=true, got: ${OUT:0:200}"
  fi
}

# assert_fail "test name" — command should exit non-zero or return ok=false
assert_fail() {
  local name="$1"
  if [[ $EXIT -ne 0 ]]; then
    _pass "$name"
    return
  fi
  if is_json; then
    local ok
    ok=$(json_get '.ok' || echo "")
    if [[ "$ok" == "false" ]]; then
      _pass "$name"
      return
    fi
  fi
  _fail "$name" "expected failure, but got exit=$EXIT out=${OUT:0:200}"
}

# assert_exit_zero "test name"
assert_exit_zero() {
  local name="$1"
  if [[ $EXIT -eq 0 ]]; then
    _pass "$name"
  else
    _fail "$name" "expected exit 0, got $EXIT: ${ERR:0:200}"
  fi
}

# assert_exit_nonzero "test name"
assert_exit_nonzero() {
  local name="$1"
  if [[ $EXIT -ne 0 ]]; then
    _pass "$name"
  else
    _fail "$name" "expected non-zero exit"
  fi
}

# assert_json "test name" — OUT is valid JSON
assert_json() {
  local name="$1"
  if is_json; then
    _pass "$name"
  else
    _fail "$name" "not valid JSON: ${OUT:0:200}"
  fi
}

# assert_json_field "test name" ".field.path" "expected_value"
assert_json_field() {
  local name="$1" path="$2" expected="$3"
  local actual
  actual=$(json_get "$path" || echo "__MISSING__")
  if [[ "$actual" == "$expected" ]]; then
    _pass "$name"
  else
    _fail "$name" ".$path: expected '$expected', got '$actual'"
  fi
}

# assert_json_field_exists "test name" ".field.path"
assert_json_field_exists() {
  local name="$1" path="$2"
  local actual
  actual=$(json_get "$path" 2>/dev/null || echo "__MISSING__")
  if [[ "$actual" != "__MISSING__" && -n "$actual" ]]; then
    _pass "$name"
  else
    _fail "$name" "field $path does not exist or is empty"
  fi
}

# assert_json_field_gte "test name" ".field" min_value
assert_json_field_gte() {
  local name="$1" path="$2" min="$3"
  local actual
  actual=$(json_get "$path" || echo "0")
  if [[ "$actual" -ge "$min" ]] 2>/dev/null; then
    _pass "$name"
  else
    _fail "$name" "$path: expected >= $min, got '$actual'"
  fi
}

# assert_contains "test name" "substring" — OUT contains substring
assert_contains() {
  local name="$1" substr="$2"
  if [[ "$OUT" == *"$substr"* ]]; then
    _pass "$name"
  else
    _fail "$name" "output does not contain '$substr': ${OUT:0:200}"
  fi
}

# assert_not_contains "test name" "substring"
assert_not_contains() {
  local name="$1" substr="$2"
  if [[ "$OUT" != *"$substr"* ]]; then
    _pass "$name"
  else
    _fail "$name" "output should not contain '$substr'"
  fi
}

# assert_file_exists "test name" "/path/to/file"
assert_file_exists() {
  local name="$1" path="$2"
  if [[ -f "$path" ]]; then
    _pass "$name"
  else
    _fail "$name" "file not found: $path"
  fi
}

# assert_file_png "test name" "/path/to/file" — file exists and starts with PNG magic
assert_file_png() {
  local name="$1" path="$2"
  if [[ ! -f "$path" ]]; then
    _fail "$name" "file not found: $path"
    return
  fi
  local magic
  magic=$(xxd -l 4 -p "$path" 2>/dev/null || echo "")
  if [[ "$magic" == "89504e47" ]]; then
    _pass "$name"
  else
    _fail "$name" "not a PNG file (magic: $magic)"
  fi
}

# ── Cleanup ─────────────────────────────────────────────────────────────────

CLEANUP_FILES=("")

# Register a file for cleanup at exit
cleanup_register() {
  CLEANUP_FILES+=("$1")
}

cleanup_run() {
  for f in "${CLEANUP_FILES[@]}"; do
    [[ -n "$f" ]] && rm -f "$f" 2>/dev/null || true
  done
}

trap cleanup_run EXIT
