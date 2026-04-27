#!/bin/bash
# Test: top-level help layout (G1)
source "$(dirname "$0")/helpers.sh"

# `cu --help` exits 0 with help on stdout; `cu` (no args) exits 2 with help on stderr.
section "help — categorization always visible"

for ARGS in "" "-h" "--help"; do
  if [[ -z "$ARGS" ]]; then
    OUT_TXT=$("$CU" 2>&1 || true)
    LABEL="cu (no args)"
  else
    OUT_TXT=$("$CU" $ARGS 2>&1)
    LABEL="cu $ARGS"
  fi

  if echo "$OUT_TXT" | grep -q "COMMANDS BY CATEGORY"; then
    _pass "$LABEL shows COMMANDS BY CATEGORY block"
  else
    _fail "$LABEL category block" "missing COMMANDS BY CATEGORY"
  fi

  for CAT in "Discover" "Observe" "Act" "Script & System"; do
    if echo "$OUT_TXT" | grep -qF "$CAT"; then
      _pass "$LABEL has '$CAT' category"
    else
      _fail "$LABEL '$CAT' category" "missing"
    fi
  done

  # Sanity: each newly-added VLM command appears in the categories
  for CMD in "find" "nearest" "observe-region"; do
    if echo "$OUT_TXT" | grep -qF "$CMD"; then
      _pass "$LABEL lists '$CMD'"
    else
      _fail "$LABEL '$CMD'" "not in categorization"
    fi
  done
done

section "help — flat command list still present in --help"

OUT_TXT=$("$CU" --help 2>&1)
if echo "$OUT_TXT" | grep -qE '^Commands:$'; then
  _pass "flat 'Commands:' section present"
else
  _fail "flat Commands section" "missing"
fi

# All 24 subcommands listed
EXPECTED_CMDS="setup apps snapshot type perform set-value key wait find nearest observe-region ocr click scroll hover drag screenshot window launch menu defaults sdef tell examples"
ALL_FOUND=yes
for cmd in $EXPECTED_CMDS; do
  if ! echo "$OUT_TXT" | grep -qE "^  $cmd  "; then
    ALL_FOUND="no ($cmd missing)"
    break
  fi
done
if [[ "$ALL_FOUND" == "yes" ]]; then
  _pass "all 24 commands appear in flat listing"
else
  _fail "flat listing complete" "$ALL_FOUND"
fi

section "help — long_about narrative present in --help"

OUT_TXT=$("$CU" --help 2>&1)
if echo "$OUT_TXT" | grep -q "THREE-TIER CONTROL"; then
  _pass "--help still shows THREE-TIER CONTROL workflow"
else
  _fail "--help workflow narrative" "THREE-TIER missing"
fi

if echo "$OUT_TXT" | grep -q "WORKFLOW FOR VLM AGENTS"; then
  _pass "--help shows VLM-agent workflow"
else
  _fail "--help VLM workflow" "missing"
fi

section "help — subcommand help still accessible"

OUT_TXT=$("$CU" snapshot --help 2>&1)
if echo "$OUT_TXT" | grep -qE "^Usage: cu snapshot"; then
  _pass "cu <subcmd> --help still works"
else
  _fail "subcmd help" "${OUT_TXT:0:200}"
fi

section "help — PREFER blocks on overlapping subcommands (G4)"

# Subcommands whose use cases overlap with another subcommand should
# include a `PREFER:` block in --help that disambiguates the choice.
for CMD in set-value type perform tell find nearest observe-region; do
  OUT_TXT=$("$CU" $CMD --help 2>&1)
  if echo "$OUT_TXT" | grep -qE '^PREFER:'; then
    _pass "cu $CMD --help has PREFER block"
  else
    _fail "cu $CMD PREFER" "missing"
  fi
done

summary
