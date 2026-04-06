#!/bin/bash
# Test: cu menu (list app menu bar items)
source "$(dirname "$0")/helpers.sh"

section "menu — Finder (always running)"

cu_json menu Finder
assert_ok "menu Finder ok"
assert_json_field "app is Finder" ".app" "Finder"
assert_json_field_exists "items array" ".items"

# Should have standard menu items
ITEM_COUNT=$(echo "$OUT" | python3 -c "
import sys, json; d = json.load(sys.stdin); print(len(d.get('items', [])))
" 2>/dev/null || echo "0")
if [[ "$ITEM_COUNT" -ge 10 ]]; then
  _pass "has menu items ($ITEM_COUNT)"
else
  _fail "has menu items" "got $ITEM_COUNT"
fi

# Should contain standard menus (File, Edit, View, etc.)
HAS_FILE=$(echo "$OUT" | python3 -c "
import sys, json
d = json.load(sys.stdin)
has = any(i['menu'] == 'File' for i in d.get('items', []))
print('yes' if has else 'no')
" 2>/dev/null || echo "no")
if [[ "$HAS_FILE" == "yes" ]]; then
  _pass "has File menu"
else
  _fail "File menu" "not found"
fi

section "menu — human mode"

cu_human menu Finder
assert_exit_zero "menu human exits 0"
assert_contains "shows menu name" "File"

section "menu — non-running app error"

cu_json menu NonExistentApp99999
assert_fail "non-existent app fails"

summary
