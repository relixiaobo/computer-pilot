#!/bin/bash
# Test: cu apps
source "$(dirname "$0")/helpers.sh"

section "apps — JSON mode"

cu_json "apps"
assert_json "output is valid JSON"
assert_json_field_exists "apps array present" ".apps"
APPS_JSON="$OUT"

# Finder is always running on macOS
FINDER=$(echo "$APPS_JSON" | python3 -c "
import sys, json
d = json.load(sys.stdin)
matches = [a for a in d.get('apps', []) if a['name'] == 'Finder']
print('found' if matches else 'missing')
" 2>/dev/null || echo "error")
if [[ "$FINDER" == "found" ]]; then
  _pass "Finder listed in apps"
else
  _fail "Finder listed in apps" "Finder not found in app list"
fi

FINDER_PID=$(echo "$APPS_JSON" | python3 -c '
import sys, json
apps = json.load(sys.stdin).get("apps", [])
print(next((a["pid"] for a in apps if a["name"] == "Finder"), ""))
' 2>/dev/null || true)
FINDER_SELECTOR=$(echo "$APPS_JSON" | python3 -c '
import sys, json
apps = json.load(sys.stdin).get("apps", [])
print(next((a.get("selector", "") for a in apps if a["name"] == "Finder"), ""))
' 2>/dev/null || true)

# Check app structure
APP_FIELDS=$(echo "$APPS_JSON" | python3 -c "
import sys, json
d = json.load(sys.stdin)
apps = d.get('apps', [])
if not apps: print('empty'); sys.exit()
a = apps[0]
fields = sorted(a.keys())
print(' '.join(fields))
" 2>/dev/null || echo "error")
if [[ "$APP_FIELDS" == *"name"* && "$APP_FIELDS" == *"pid"* && "$APP_FIELDS" == *"selector"* && "$APP_FIELDS" == *"bundle_path"* ]]; then
  _pass "apps have identity and reusable selector fields"
else
  _fail "apps identity fields" "fields: $APP_FIELDS"
fi

section "apps — PID selector resolves the exact process"

if [[ -n "$FINDER_PID" && "$FINDER_SELECTOR" == "pid:$FINDER_PID" ]]; then
  cu_json state "$FINDER_SELECTOR" --no-screenshot --limit 5
  assert_ok "state accepts selector returned by apps"
  assert_json_field "state resolves exact Finder PID" ".pid" "$FINDER_PID"
else
  _fail "Finder reusable selector" "pid=$FINDER_PID selector=$FINDER_SELECTOR"
fi

# Check there's at least a few apps
APP_COUNT=$(echo "$APPS_JSON" | python3 -c "
import sys, json; d = json.load(sys.stdin); print(len(d.get('apps', [])))
" 2>/dev/null || echo "0")
if [[ "$APP_COUNT" -ge 3 ]]; then
  _pass "at least 3 apps running ($APP_COUNT)"
else
  _fail "at least 3 apps running" "only $APP_COUNT apps"
fi

section "apps — sdef_classes for scriptable apps"

CLASSES_OK=$(echo "$APPS_JSON" | python3 -c "
import sys, json
d = json.load(sys.stdin)
scriptable = [a for a in d.get('apps', []) if a.get('scriptable')]
with_classes = [a for a in scriptable if 'sdef_classes' in a and isinstance(a['sdef_classes'], int)]
print('ok' if len(with_classes) > 0 else 'none')
" 2>/dev/null || echo "error")
if [[ "$CLASSES_OK" == "ok" ]]; then
  _pass "scriptable apps have sdef_classes (int)"
else
  _fail "sdef_classes field" "$CLASSES_OK"
fi

section "apps — human mode"

cu_human "apps"
assert_exit_zero "apps human exits 0"
assert_contains "Finder in human output" "Finder"
# Human format: "* Finder (pid NNN)" or " S Finder (pid NNN)"
if echo "$OUT" | grep -qE 'Finder \(pid [0-9]+\)'; then
  _pass "human format: name (pid N)"
else
  _fail "human format: name (pid N)" "${OUT:0:200}"
fi

summary
