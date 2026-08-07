#!/bin/bash
# Test: the L2 agent harness (tests/agent/run.py) treats a failed verification
# command as a failed check.
#
# Why this lives in L1: run.py is the only gate that can catch SKILL.md
# regressions, but nothing automated watches run.py itself. It shipped with a
# verification path that matched `expect_contains` against the *error* text of
# a failed command — and AppleScript quotes the object it could not find
# (`Can't get note "Agent Test - Desktop Inventory"`), so the check that was
# supposed to prove the note exists passed precisely when it did not.
source "$(dirname "$0")/helpers.sh"

section "agent harness — a failed verification command cannot pass"

RUNNER="$ROOT_DIR/tests/agent/run.py"

if [[ ! -f "$RUNNER" ]]; then
  _fail "agent harness present" "tests/agent/run.py is missing"
  summary
fi

# `cu snapshot NoSuchApp-ZZZ` exits non-zero and echoes the requested name in
# its error — the same shape as the AppleScript failure that caused the bug.
HARNESS_OUT=$(cd "$ROOT_DIR" && CU="$CU" python3 - <<'PY' 2>&1
import json, sys
from pathlib import Path

sys.path.insert(0, str(Path("tests/agent").resolve()))
import run as harness

ECHOING_FAILURE = ["cu", "snapshot", "NoSuchApp-ZZZ"]

cases = {
    "expect_contains_on_failure": {
        "verify": [{
            "description": "object exists",
            "command": ECHOING_FAILURE,
            "expect_contains": "NoSuchApp-ZZZ",
        }],
    },
    "expect_min_length_on_failure": {
        "verify": [{
            "description": "output is substantial",
            "command": ECHOING_FAILURE,
            "expect_min_length": 10,
        }],
    },
    "cross_check_target_failure": {
        "verify": [{
            "description": "content matches source",
            "command": ECHOING_FAILURE,
            "cross_check": {"source_command": ["cu", "apps"]},
        }],
    },
    "passing_command": {
        "verify": [{
            "description": "cu responds",
            "command": ["cu", "apps"],
            "expect_contains": "ok",
        }],
    },
}

print(json.dumps({k: harness.verify_task(v) for k, v in cases.items()}))
PY
)

if [[ -z "$HARNESS_OUT" ]] || ! echo "$HARNESS_OUT" | tail -1 | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null; then
  _fail "harness verification is inspectable" "could not run verify_task: ${HARNESS_OUT:0:300}"
  summary
fi

HARNESS_JSON=$(echo "$HARNESS_OUT" | tail -1)

check_case() {
  local case_name=$1 expected_passed=$2 label=$3
  local actual
  actual=$(echo "$HARNESS_JSON" | python3 -c "
import json, sys
results = json.load(sys.stdin)['$case_name']
print(all(r['passed'] for r in results) if results else 'no-results')
")
  if [[ "$actual" == "$expected_passed" ]]; then
    _pass "$label"
  else
    _fail "$label" "expected passed=$expected_passed, got $actual"
  fi
}

check_case expect_contains_on_failure False \
  "expect_contains does not match the error text of a failed command"
check_case expect_min_length_on_failure False \
  "expect_min_length does not count the error text of a failed command"
check_case cross_check_target_failure False \
  "cross_check fails when its target command fails"
check_case passing_command True \
  "a successful command still passes its check"

# The failure detail must name the command failure, so a run that goes red
# points at the environment rather than looking like an agent mistake.
DETAIL=$(echo "$HARNESS_JSON" | python3 -c "
import json, sys
print(json.load(sys.stdin)['expect_contains_on_failure'][0]['detail'])
")
if [[ "$DETAIL" == *"command failed"* && "$DETAIL" == *"app_not_found"* ]]; then
  _pass "failure detail reports the underlying command error"
else
  _fail "failure detail reports the underlying command error" "got: ${DETAIL:0:200}"
fi

section "agent harness — model selection matches the credentials present"

# release.sh gates L2 on *a* key being present and then runs run.py with no
# flags. When the only key is OpenAI-compatible, the model default has to come
# from .env too, or the run dies mid-loop as a generic "LLM error" after the
# release has already built.
CRED_OUT=$(cd "$ROOT_DIR" && python3 - <<'PY' 2>&1
import importlib.util, json, os, sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("harness", "tests/agent/run.py")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)

# run.py loads .env at import, so clear the keys here to make the checks
# independent of whatever this machine happens to have configured.
for var in ("ANTHROPIC_API_KEY", "OPENAI_API_KEY"):
    os.environ.pop(var, None)

out = {
    "routes_claude": harness.is_claude_model("claude-sonnet-5"),
    "routes_openai": harness.is_claude_model("gpt-5.6-sol"),
    "claude_without_key": harness.credential_error("claude-sonnet-5"),
    "openai_without_key": harness.credential_error("gpt-5.6-sol"),
}
os.environ["ANTHROPIC_API_KEY"] = "test"
out["claude_with_key"] = harness.credential_error("claude-sonnet-5")
del os.environ["ANTHROPIC_API_KEY"]
os.environ["OPENAI_API_KEY"] = "test"
out["openai_with_key"] = harness.credential_error("gpt-5.6-sol")
print(json.dumps(out))
PY
)

CRED_JSON=$(echo "$CRED_OUT" | tail -1)
if ! echo "$CRED_JSON" | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null; then
  _fail "credential preflight is inspectable" "could not import run.py: ${CRED_OUT:0:300}"
  summary
fi

cred_field() { echo "$CRED_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin)['$1'])"; }

if [[ "$(cred_field routes_claude)" == "True" && "$(cred_field routes_openai)" == "False" ]]; then
  _pass "one predicate decides which provider a model id routes to"
else
  _fail "one predicate decides which provider a model id routes to" \
    "is_claude_model misclassified a known model id"
fi

for case in claude_without_key openai_without_key; do
  value=$(cred_field "$case")
  if [[ "$value" != "None" && -n "$value" ]]; then
    _pass "$case reports the missing credential"
  else
    _fail "$case reports the missing credential" "credential_error returned $value"
  fi
done

for case in claude_with_key openai_with_key; do
  if [[ "$(cred_field "$case")" == "None" ]]; then
    _pass "$case accepts the matching credential"
  else
    _fail "$case accepts the matching credential" "credential_error rejected a usable key"
  fi
done

# The default has to read AGENT_MODEL; a hardcoded Claude id would ignore a
# release machine that only has an OpenAI-compatible key.
if grep -Fq 'os.environ.get("AGENT_MODEL"' "$RUNNER"; then
  _pass "model default reads AGENT_MODEL"
else
  _fail "model default reads AGENT_MODEL" "--model default is hardcoded again"
fi

# The preflight must run before the agent loop, so a misconfigured release
# fails immediately instead of after touching the desktop.
if grep -Fq 'problem = credential_error(args.model)' "$RUNNER" \
  && [[ $(grep -n 'problem = credential_error' "$RUNNER" | cut -d: -f1) -lt \
        $(grep -n 'result = run_agent_task' "$RUNNER" | cut -d: -f1) ]]; then
  _pass "credential preflight runs before the first task"
else
  _fail "credential preflight runs before the first task" \
    "run.py starts tasks before checking credentials"
fi

section "agent harness — command results carry success separately from text"

# cu_result keeps ok and text apart; collapsing them back into a bare string is
# what let error text be graded as output.
if grep -Fq 'res = cu_result(check["command"][1:])' "$RUNNER"; then
  _pass "verification reads command success, not just its text"
else
  _fail "verification reads command success, not just its text" \
    "verify_task no longer routes the check command through cu_result"
fi

summary
