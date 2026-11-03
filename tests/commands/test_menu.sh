#!/bin/bash
# Test: cu menu (list app menu bar items)
source "$(dirname "$0")/helpers.sh"

section "menu — Finder (always running)"

cu_json menu com.apple.finder
assert_ok "menu Finder ok"
assert_json_field "app is Finder" ".app" "com.apple.finder"
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

# Menu names are localized. Verify semantic menu/item structure, then check
# the human rendering against the actual label returned by this app.
MENU_NAME=$(printf '%s' "$OUT" | python3 -c '
import json,sys
items=json.load(sys.stdin)["items"]
assert all(isinstance(i["menu"], str) and i["menu"] and isinstance(i["item"], str) for i in items)
menus=list(dict.fromkeys(i["menu"] for i in items))
assert len(menus) >= 3, menus
print(menus[1])
')
[[ -n "$MENU_NAME" ]] && _pass "localized menus have named items"

section "menu — human mode"

cu_human menu com.apple.finder
assert_exit_zero "menu human exits 0"
assert_contains "shows menu name" "$MENU_NAME"

section "menu — non-running app error"

cu_json menu NonExistentApp99999
assert_fail "non-existent app fails"
APP_ERROR_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
if [[ "$APP_ERROR_CODE" == "app_not_found" ]]; then
  _pass "non-existent app has stable code"
else
  _fail "non-existent app stable code" "got '$APP_ERROR_CODE'"
fi

summary
