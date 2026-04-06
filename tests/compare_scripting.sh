#!/bin/bash
# compare_scripting.sh — Side-by-side comparison: UI automation vs AppleScript scripting
# Measures: success rate, speed, output size (proxy for token cost)
#
# Usage: bash tests/compare_scripting.sh

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CU="$ROOT/target/release/cu"

if [[ ! -x "$CU" ]]; then
  echo "Build first: cargo build --release" >&2; exit 1
fi

# ── Colors ──────────────────────────────────────────────────────────────────
GREEN='\033[0;32m'; RED='\033[0;31m'; CYAN='\033[0;36m'
YELLOW='\033[0;33m'; BOLD='\033[1m'; DIM='\033[2m'; RESET='\033[0m'

# ── Helpers ─────────────────────────────────────────────────────────────────

RESULTS=()

# Run a task two ways: UI automation vs scripting
# Usage: compare "task name" "ui_commands" "script_command" "verify_command"
compare() {
  local task="$1"
  local ui_cmds="$2"
  local script_cmd="$3"
  local verify="$4"

  echo -e "\n${CYAN}${BOLD}━━━ $task ━━━${RESET}"

  # ── Method A: UI automation ──
  echo -e "${DIM}  [UI automation]${RESET}"
  local ui_start ui_end ui_ms ui_ok ui_output ui_bytes
  ui_start=$(python3 -c "import time; print(int(time.time()*1000))")
  ui_output=""
  ui_ok="true"

  while IFS= read -r cmd; do
    [[ -z "$cmd" ]] && continue
    local out
    out=$(eval "$CU $cmd" 2>/dev/null) || { ui_ok="false"; break; }
    ui_output+="$out"
  done <<< "$ui_cmds"

  ui_end=$(python3 -c "import time; print(int(time.time()*1000))")
  ui_ms=$((ui_end - ui_start))
  ui_bytes=${#ui_output}

  if [[ "$ui_ok" == "true" ]]; then
    echo -e "    ${GREEN}OK${RESET}  ${ui_ms}ms  ${ui_bytes} bytes output"
  else
    echo -e "    ${RED}FAIL${RESET}  ${ui_ms}ms"
  fi

  # ── Method B: AppleScript scripting ──
  echo -e "${DIM}  [AppleScript scripting]${RESET}"
  local sc_start sc_end sc_ms sc_ok sc_output sc_bytes
  sc_start=$(python3 -c "import time; print(int(time.time()*1000))")

  sc_output=$(eval "$CU $script_cmd" 2>/dev/null) && sc_ok="true" || sc_ok="false"

  sc_end=$(python3 -c "import time; print(int(time.time()*1000))")
  sc_ms=$((sc_end - sc_start))
  sc_bytes=${#sc_output}

  if [[ "$sc_ok" == "true" ]]; then
    echo -e "    ${GREEN}OK${RESET}  ${sc_ms}ms  ${sc_bytes} bytes output"
  else
    echo -e "    ${RED}FAIL${RESET}  ${sc_ms}ms"
  fi

  # ── Verify result ──
  local verified="skip"
  if [[ -n "$verify" && "$sc_ok" == "true" ]]; then
    if eval "$verify" >/dev/null 2>&1; then
      verified="pass"
    else
      verified="fail"
    fi
  fi

  # ── Speedup ──
  local speedup="N/A"
  if [[ "$ui_ms" -gt 0 && "$sc_ms" -gt 0 ]]; then
    speedup=$(python3 -c "print(f'{$ui_ms/$sc_ms:.1f}x')")
  fi

  echo -e "  ${BOLD}Speedup: ${speedup}  Bytes: ${ui_bytes} → ${sc_bytes} (${YELLOW}$(python3 -c "print(f'{$sc_bytes/$ui_bytes*100:.0f}%' if $ui_bytes > 0 else 'N/A')")${RESET})"

  RESULTS+=("$task|$ui_ok|$ui_ms|$ui_bytes|$sc_ok|$sc_ms|$sc_bytes|$speedup|$verified")
}

# ── Tests ───────────────────────────────────────────────────────────────────

echo -e "${BOLD}cu scripting vs UI automation comparison${RESET}"
echo "Binary: $CU"
echo ""

# Ensure Finder has a window
osascript -e 'tell application "Finder"
  if (count of Finder windows) is 0 then make new Finder window
end tell' 2>/dev/null
sleep 0.3

# ── Task 1: Get Finder window name ──
compare "Get Finder window name" \
  "snapshot Finder --limit 50" \
  'tell Finder "app.finderWindows[0].name()"' \
  ""

# ── Task 2: List files in Finder's current folder ──
compare "List files in current Finder folder" \
  "snapshot Finder --limit 200" \
  'tell Finder "app.finderWindows[0].target.items().slice(0,20).map(function(f){return {name:f.name(), kind:f.kind()}})"' \
  ""

# ── Task 3: Get dark mode status ──
compare "Check dark mode" \
  'snapshot "System Events" --limit 50' \
  'tell "System Events" "app.appearancePreferences.darkMode()"' \
  ""

# ── Task 4: Create and read a note ──
compare "Create and read a note" \
  "snapshot Notes --limit 30
snapshot Notes --limit 30" \
  'tell Notes "var n = app.Note({name:\"cu-test-note\", body:\"hello from cu\"}); app.notes.push(n); ({title: app.notes[0].name(), body: app.notes[0].plaintext().substring(0,50)})"' \
  ""
# Cleanup
$CU tell Notes 'var ns = app.notes.whose({name:"cu-test-note"}); for(var i=ns.length-1;i>=0;i--) app.delete(ns[i])' 2>/dev/null

# ── Task 5: List Calendar names ──
compare "List calendars" \
  "snapshot Calendar --limit 50" \
  'tell Calendar "app.calendars().map(function(c){return {name:c.name(), writable:c.writable()}})"' \
  ""

# ── Task 6: Get Safari tabs (if running) ──
if pgrep -x Safari >/dev/null 2>&1; then
  compare "Get Safari tabs" \
    "snapshot Safari --limit 100" \
    'tell Safari "app.windows[0].tabs().map(function(t){return {title:t.name(), url:t.url()}})"' \
    ""
fi

# ── Task 7: Get Chrome tabs (if running) ──
if pgrep -x "Google Chrome" >/dev/null 2>&1; then
  compare "Get Chrome tabs" \
    'snapshot "Google Chrome" --limit 100' \
    'tell "Google Chrome" "app.windows[0].tabs().map(function(t){return {title:t.title(), url:t.url()}})"' \
    ""
fi

# ── Task 8: Get Reminders lists ──
compare "List reminder lists" \
  "snapshot Reminders --limit 30" \
  'tell Reminders "app.lists().map(function(l){return {name:l.name(), count:l.reminders().length}})"' \
  ""

# ── Task 9: Read clipboard ──
compare "Read clipboard" \
  "snapshot Finder --limit 10" \
  'tell "System Events" "app.theClipboard()"' \
  ""

# ── Task 10: Get Finder selection ──
compare "Get Finder selection" \
  "snapshot Finder --limit 50" \
  'tell Finder "app.selection().map(function(f){return f.name()})"' \
  ""

# ── Summary ─────────────────────────────────────────────────────────────────

echo -e "\n${BOLD}════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}Summary${RESET}\n"
printf "%-35s %6s %6s %7s %7s %8s\n" "Task" "UI ms" "AS ms" "UI bytes" "AS bytes" "Speedup"
printf "%-35s %6s %6s %7s %7s %8s\n" "---" "---" "---" "---" "---" "---"

total_ui_ms=0; total_sc_ms=0; total_ui_bytes=0; total_sc_bytes=0; count=0
for r in "${RESULTS[@]}"; do
  IFS='|' read -r task ui_ok ui_ms ui_bytes sc_ok sc_ms sc_bytes speedup verified <<< "$r"
  printf "%-35s %6s %6s %7s %7s %8s\n" "${task:0:35}" "$ui_ms" "$sc_ms" "$ui_bytes" "$sc_bytes" "$speedup"
  total_ui_ms=$((total_ui_ms + ui_ms))
  total_sc_ms=$((total_sc_ms + sc_ms))
  total_ui_bytes=$((total_ui_bytes + ui_bytes))
  total_sc_bytes=$((total_sc_bytes + sc_bytes))
  ((count++))
done

echo ""
printf "%-35s %6s %6s %7s %7s %8s\n" "TOTAL" "$total_ui_ms" "$total_sc_ms" "$total_ui_bytes" "$total_sc_bytes" \
  "$(python3 -c "print(f'{$total_ui_ms/$total_sc_ms:.1f}x' if $total_sc_ms > 0 else 'N/A')")"

echo -e "\n${BOLD}Key insight:${RESET}"
echo "  UI automation returns raw AX tree (agent must parse → reason → act → verify)"
echo "  AppleScript scripting returns structured data directly (single step, done)"
echo ""
echo "  Token cost reduction: UI=$(python3 -c "print(f'{$total_ui_bytes:,}')") bytes → AppleScript=$(python3 -c "print(f'{$total_sc_bytes:,}')") bytes"
echo "  Speed improvement: ${total_ui_ms}ms → ${total_sc_ms}ms"
echo -e "${BOLD}════════════════════════════════════════════════════════════════════${RESET}"
