#!/bin/bash
# Test: cu click --text --region disambiguates same label across panes.
#
# This is a behavior test (Rule 1): the --region flag was added to solve
# "same label appears in sidebar AND main pane" — so the test must construct
# that situation and assert the click landed in the intended pane, not the
# other one. Without this test, the existing structural test (offscreen rect
# returns failure) does not prove --region actually disambiguates a real
# multi-occurrence layout.
#
# Setup: Finder home folder shows "Applications" / "Documents" / etc. in
# both the sidebar (left) and the main view (right). OCR sees both. The
# test asks --region to pick one or the other and asserts the click coord
# falls inside the requested pane.
source "$(dirname "$0")/helpers.sh"

section "click --text --region — sidebar vs main pane disambiguation"

# Open Finder home folder so duplicate folder labels appear in both panes.
osascript -e 'tell application "Finder" to open home' >/dev/null 2>&1
sleep 1

# Find a label that genuinely appears 2+ times in the OCR output, capture
# the leftmost and rightmost occurrence. The leftmost is the sidebar; the
# rightmost is the main pane. If no duplicate exists in this Finder state
# (e.g. user customized sidebar), skip rather than fail.
cu_json ocr Finder
LABEL_AND_COORDS=$(echo "$OUT" | python3 -c "
import sys, json
from collections import defaultdict
d = json.load(sys.stdin)
texts = d.get('texts', [])
# Group occurrences by stripped text. We want a duplicate whose two hits
# are HORIZONTALLY separated by 100+ pts — that's the sidebar/main split.
# Same-pane multi-occurrence (e.g. 'Folder' label in two date-column rows)
# has near-identical x and would defeat the test premise.
groups = defaultdict(list)
for t in texts:
    s = t['text'].strip()
    if len(s) >= 5 and s.replace(' ','').isalpha():
        groups[s].append(t)
candidates = []
for label, hits in groups.items():
    if len(hits) < 2:
        continue
    xs = [h['x'] for h in hits]
    spread = max(xs) - min(xs)
    if spread >= 100:
        candidates.append((spread, label, hits))
if not candidates:
    print('NONE')
    sys.exit()
# Pick the candidate with the largest x-spread — most clearly sidebar-vs-main.
candidates.sort(reverse=True)
_, label, hits = candidates[0]
hits.sort(key=lambda t: t['x'])
left, right = hits[0], hits[-1]
print(f\"{label}|{left['x']:.0f}|{left['y']:.0f}|{right['x']:.0f}|{right['y']:.0f}\")
")

if [[ "$LABEL_AND_COORDS" == "NONE" || -z "$LABEL_AND_COORDS" ]]; then
  _skip "sidebar vs main disambiguation" "no duplicate folder label found in Finder home OCR — sidebar may be customized or window not in expected state"
  summary
  exit 0
fi

LABEL=$(echo "$LABEL_AND_COORDS" | cut -d'|' -f1)
SIDEBAR_X=$(echo "$LABEL_AND_COORDS" | cut -d'|' -f2)
SIDEBAR_Y=$(echo "$LABEL_AND_COORDS" | cut -d'|' -f3)
MAIN_X=$(echo "$LABEL_AND_COORDS" | cut -d'|' -f4)
MAIN_Y=$(echo "$LABEL_AND_COORDS" | cut -d'|' -f5)

echo "  duplicate label: '$LABEL' at sidebar=($SIDEBAR_X,$SIDEBAR_Y) main=($MAIN_X,$MAIN_Y)"

# Sanity: the two occurrences must be horizontally separated. If they're
# the same column (e.g. same pane wrapped), the test premise doesn't hold.
SEPARATION=$(python3 -c "print(int(abs($MAIN_X - $SIDEBAR_X)))")
if [[ "$SEPARATION" -lt 100 ]]; then
  _skip "sidebar vs main disambiguation" "two occurrences are only $SEPARATION pts apart — not a sidebar/main split"
  summary
  exit 0
fi
_pass "two occurrences of '$LABEL' are $SEPARATION pts apart (real multi-pane setup)"

# Construct two regions: a tight box around each occurrence (±30 pts).
# Each region encloses exactly one occurrence center.
SIDEBAR_REGION=$(python3 -c "print(f'{int($SIDEBAR_X - 30)},{int($SIDEBAR_Y - 30)} 60x60')")
MAIN_REGION=$(python3 -c "print(f'{int($MAIN_X - 30)},{int($MAIN_Y - 30)} 60x60')")

# 1. Click via sidebar-region. The click x should be the sidebar x, not the main x.
cu_json click --text "$LABEL" --app Finder --region "$SIDEBAR_REGION" --no-snapshot
assert_ok "click '$LABEL' --region <sidebar>"
GOT_X=$(json_get '.x' || echo "-1")
GOT_X_ROUND=$(python3 -c "print(int(round(float('$GOT_X'))))" 2>/dev/null || echo "-1")
SIDEBAR_X_ROUND=$(python3 -c "print(int(round(float('$SIDEBAR_X'))))")
MAIN_X_ROUND=$(python3 -c "print(int(round(float('$MAIN_X'))))")

# Expect: returned x near sidebar x, far from main x.
DIFF_FROM_SIDEBAR=$(python3 -c "print(abs($GOT_X_ROUND - $SIDEBAR_X_ROUND))")
DIFF_FROM_MAIN=$(python3 -c "print(abs($GOT_X_ROUND - $MAIN_X_ROUND))")
if [[ "$DIFF_FROM_SIDEBAR" -lt "$DIFF_FROM_MAIN" ]]; then
  _pass "sidebar-region click landed near sidebar (got x=$GOT_X_ROUND, sidebar=$SIDEBAR_X_ROUND, main=$MAIN_X_ROUND)"
else
  _fail "sidebar-region click should land near sidebar" "got x=$GOT_X_ROUND, distance to sidebar=$DIFF_FROM_SIDEBAR, to main=$DIFF_FROM_MAIN"
fi
SIDEBAR_CLICK_X=$GOT_X_ROUND

# 2. Click via main-region. The click x should be the main x.
cu_json click --text "$LABEL" --app Finder --region "$MAIN_REGION" --no-snapshot
assert_ok "click '$LABEL' --region <main>"
GOT_X=$(json_get '.x' || echo "-1")
GOT_X_ROUND=$(python3 -c "print(int(round(float('$GOT_X'))))" 2>/dev/null || echo "-1")
DIFF_FROM_SIDEBAR=$(python3 -c "print(abs($GOT_X_ROUND - $SIDEBAR_X_ROUND))")
DIFF_FROM_MAIN=$(python3 -c "print(abs($GOT_X_ROUND - $MAIN_X_ROUND))")
if [[ "$DIFF_FROM_MAIN" -lt "$DIFF_FROM_SIDEBAR" ]]; then
  _pass "main-region click landed near main (got x=$GOT_X_ROUND, sidebar=$SIDEBAR_X_ROUND, main=$MAIN_X_ROUND)"
else
  _fail "main-region click should land near main" "got x=$GOT_X_ROUND, distance to sidebar=$DIFF_FROM_SIDEBAR, to main=$DIFF_FROM_MAIN"
fi
MAIN_CLICK_X=$GOT_X_ROUND

# 3. The two clicks must produce different coordinates — that's the load-bearing
# assertion. Without --region working, they would return the same first OCR hit.
if [[ "$SIDEBAR_CLICK_X" -ne "$MAIN_CLICK_X" ]]; then
  _pass "sidebar and main clicks produced different coords ($SIDEBAR_CLICK_X vs $MAIN_CLICK_X) — disambiguation works"
else
  _fail "disambiguation" "both regions produced x=$SIDEBAR_CLICK_X — --region is not actually filtering"
fi

summary
