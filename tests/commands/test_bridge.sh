#!/bin/bash
# Test: Agent-neutral stdio bridge and black-box conformance runner
source "$(dirname "$0")/helpers.sh"

section "bridge — black-box stdio conformance"

EXIT=0
OUT=$(python3 "$ROOT_DIR/scripts/run-stdio-conformance.py" --timeout-ms 30000 -- "$CU") || EXIT=$?
ERR=""
assert_exit_zero "stdio conformance exits zero"
assert_json "stdio conformance report is JSON"
assert_json_field "stdio conformance passed" ".passed" "true"
assert_json_field "stdio protocol major" ".metadata.protocol.major" "1"
assert_json_field "stdio protocol minor" ".metadata.protocol.minor" "0"

section "bridge — explicit machine mode"

cu_json --json apps
assert_ok "--json apps has a normalized ok field"
assert_json_field "--json apps schema version" ".schema_version" "1.0"

section "bridge — explicit machine error"

cu_json --json snapshot NonExistentApp98765
assert_fail "--json invalid app exits non-zero"
ERR_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
ERR_SCHEMA=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("schema_version", ""))' 2>/dev/null || true)
if [[ "$ERR_CODE" == "command_failed" && "$ERR_SCHEMA" == "1.0" ]]; then
  _pass "--json error has stable code and schema"
else
  _fail "--json error has stable code and schema" "code=$ERR_CODE schema=$ERR_SCHEMA"
fi

cu_json --json apps --not-a-real-option
assert_fail "--json invalid option exits non-zero"
PARSE_CODE=$(echo "$ERR" | python3 -c 'import json,sys; print(json.load(sys.stdin).get("code", ""))' 2>/dev/null || true)
if [[ "$PARSE_CODE" == "invalid_argument" ]]; then
  _pass "--json parse error has stable invalid_argument code"
else
  _fail "--json parse error has stable invalid_argument code" "code=$PARSE_CODE"
fi

summary
