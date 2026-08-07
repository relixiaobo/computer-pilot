#!/bin/bash
# Run every check that gates a change, in one command.
#
# Steps 1-6 mirror the hosted `test` job in .github/workflows/ci.yml exactly.
# Step 7 is the TCC-enabled command suite, which cannot run on a hosted runner
# (no GUI session can hold Accessibility/Screen Recording grants) and is not a
# CI gate on a public repository — see .github/workflows/tcc-suite.yml. Run
# this before pushing or merging; CI alone does not cover step 7.
#
# Usage:
#   bash scripts/verify.sh                # everything
#   bash scripts/verify.sh --skip-desktop # steps 1-6 only (no TCC/desktop)

set -uo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

SKIP_DESKTOP=""
for arg in "$@"; do
  case "$arg" in
    --skip-desktop) SKIP_DESKTOP=1 ;;
    *) echo "Usage: bash scripts/verify.sh [--skip-desktop]" >&2; exit 1 ;;
  esac
done

if [[ -t 1 ]]; then
  GREEN=$'\033[0;32m'; RED=$'\033[0;31m'; BOLD=$'\033[1m'; RESET=$'\033[0m'
else
  GREEN=''; RED=''; BOLD=''; RESET=''
fi

FAILED=()

step() {
  local name="$1"
  shift
  echo ""
  echo "${BOLD}── $name ──${RESET}"
  if "$@"; then
    echo "${GREEN}ok${RESET}  $name"
  else
    echo "${RED}FAIL${RESET}  $name"
    FAILED+=("$name")
  fi
}

validate_skill_metadata() {
  head -1 plugin/skills/computer-pilot/SKILL.md | grep -qx -- '---' &&
    grep -qx 'name: computer-pilot' plugin/skills/computer-pilot/SKILL.md &&
    python3 -m json.tool plugin/skills/computer-pilot/compatibility.json >/dev/null
}

# 1-6: identical to the hosted CI job.
step "release versions"   bash scripts/check-version-sync.sh
step "skill metadata"     validate_skill_metadata
step "cargo fmt"          cargo fmt -- --check
step "cargo clippy"       cargo clippy --all-targets -- -D warnings
step "cargo test"         cargo test
step "cargo build"        cargo build --release

# 7: needs a real desktop with TCC grants.
if [[ -n "$SKIP_DESKTOP" ]]; then
  echo ""
  echo "${BOLD}── command suite ──${RESET}"
  echo "skipped (--skip-desktop) — CI does not run this either, so nothing has checked it"
else
  step "command suite" bash tests/commands/run_all.sh
fi

echo ""
echo "${BOLD}════════════════════════════════════════${RESET}"
if [[ ${#FAILED[@]} -eq 0 ]]; then
  if [[ -n "$SKIP_DESKTOP" ]]; then
    echo "${GREEN}CI-equivalent checks passed${RESET} — the command suite was NOT run"
  else
    echo "${GREEN}All checks passed${RESET}"
  fi
  exit 0
fi
echo "${RED}Failed: ${FAILED[*]}${RESET}"
exit 1
