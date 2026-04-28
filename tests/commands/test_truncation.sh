#!/bin/bash
# Test: cu snapshot truncation surfaces a loud, actionable hint (R3).
#
# Why this exists: prior to R3, `truncated: true` was the only signal that
# the snapshot was clipped. Agents skim for string advisories more reliably
# than for boolean flags, so we attach a `truncation_hint` whenever the
# limit is hit — and this test guards that behavior.
source "$(dirname "$0")/helpers.sh"

section "truncation — hint attached when --limit is reached"

# Finder reliably has well over 50 elements in its sidebar/columns.
cu_json snapshot Finder --limit 5
assert_ok "snapshot ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('|'.join([
    'truncated=' + str(d.get('truncated')),
    'has_hint=' + str('truncation_hint' in d),
    'hint_mentions_limit=' + str('--limit' in str(d.get('truncation_hint',''))),
    'hint_mentions_more=' + str('MORE' in str(d.get('truncation_hint','')) or 'more' in str(d.get('truncation_hint','')).lower()),
]))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"truncated=True"* ]]            && _pass "truncated flag set"           || _fail "truncated flag set"           "$PARSED"
[[ "$PARSED" == *"has_hint=True"* ]]             && _pass "truncation_hint attached"      || _fail "truncation_hint attached"      "$PARSED"
[[ "$PARSED" == *"hint_mentions_limit=True"* ]]  && _pass "hint names --limit retry path" || _fail "hint names --limit retry path" "$PARSED"
[[ "$PARSED" == *"hint_mentions_more=True"* ]]   && _pass "hint says MORE elements exist" || _fail "hint says MORE elements exist" "$PARSED"

section "truncation — hint absent when no truncation"

cu_json snapshot Finder --limit 500
assert_ok "snapshot --limit 500 ok"

PARSED=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('truncated=' + str(d.get('truncated')) + '|has_hint=' + str('truncation_hint' in d))
" 2>/dev/null || echo "malformed")

[[ "$PARSED" == *"truncated=False"* ]] && _pass "truncated=false on adequate limit"     || _fail "truncated=false on adequate limit" "$PARSED"
[[ "$PARSED" == *"has_hint=False"* ]]  && _pass "truncation_hint absent when not truncated" || _fail "truncation_hint absent when not truncated" "$PARSED"

summary
