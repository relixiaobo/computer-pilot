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

section "agent harness — mutable verification sources are frozen and matched completely"

FROZEN_OUT=$(cd "$ROOT_DIR" && python3 - <<'PY' 2>&1
import importlib.util, json

spec = importlib.util.spec_from_file_location("harness", "tests/agent/run.py")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)

source = ["cu", "tell", "Mail", "source"]
target = ["cu", "tell", "Notes", "target"]
task = {"id": "frozen", "name": "frozen source", "goal": "use live data", "verify": [{
    "description": "all three messages are represented",
    "cross_check": {
        "source_command": source,
        "target_command": target,
        "freeze_source": True,
        "source_timeout": 45,
        "match": "all_lines",
    },
}]}

calls = []
source_value = "Alpha subject\nBeta subject\nGamma subject"
def fixture(command, timeout=None):
    calls.append(command[-1])
    if command == source:
        # A later source read would return a new message. Verification must use
        # the task-start value instead of drifting while the agent works.
        source_reads = sum(1 for call in calls if call == "source")
        value = source_value if source_reads == 1 else "New arrival"
    else:
        # Deliberately use a different order: account enumeration and summary
        # layout must not change which complete source items are required.
        value = "Gamma subject details; Alpha subject details; Beta subject details"
    return {"ok": True, "text": json.dumps({"ok": True, "result": value})}

harness.time.sleep = lambda _: None
harness.run_cleanup = lambda task, verbose=True: []
harness.run_setup = lambda task, verbose=True: None
harness.cu = lambda args: '{"ok":true,"apps":[]}'
harness.call_llm = lambda model, messages: ("DONE", 1, 1)
harness.fixture_cu = fixture
passing_result = harness.run_agent_task(task, "stub-model", max_steps=1, verbose=False)

def missing_fixture(command, timeout=None):
    if command == target:
        value = "Gamma subject details; Alpha subject details"
        return {"ok": True, "text": json.dumps({"ok": True, "result": value})}
    raise AssertionError("frozen source was read again")

harness.fixture_cu = missing_fixture
captured = {0: {"ok": True, "text": json.dumps({"ok": True, "result": source_value})}}
missing = harness.verify_task(task, captured)

model_called = False
def should_not_run(model, messages):
    global model_called
    model_called = True
    raise AssertionError("model ran without a verification baseline")
harness.fixture_cu = lambda command, timeout=None: {"ok": False, "text": "Mail unavailable"}
harness.call_llm = should_not_run
source_failure = harness.run_agent_task(task, "stub-model", max_steps=1, verbose=False)

print(json.dumps({
    "source_reads": sum(1 for call in calls if call == "source"),
    "source_timeout": harness.with_fixture_timeout(source, 45)[-1],
    "default_timeout": harness.with_fixture_timeout(target)[-1],
    "expected_default_timeout": harness.FIXTURE_TELL_TIMEOUT,
    "passing_verified": passing_result["verified"],
    "passing": passing_result["checks"][0],
    "missing": missing[0],
    "failure_status": source_failure["agent_status"],
    "failure_steps": source_failure["steps"],
    "model_called": model_called,
}))
PY
)

FROZEN_JSON=$(echo "$FROZEN_OUT" | tail -1)
if ! echo "$FROZEN_JSON" | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null; then
  _fail "frozen cross-check is inspectable" "could not drive frozen source logic: ${FROZEN_OUT:0:300}"
  summary
fi

frozen_field() { echo "$FROZEN_JSON" | python3 -c "import json,sys; v=json.load(sys.stdin)$1; print(v)"; }

if [[ "$(frozen_field "['passing_verified']")" == "True" \
   && "$(frozen_field "['source_reads']")" == "1" ]]; then
  _pass "a mutable source is captured once before the agent runs"
else
  _fail "a mutable source is captured once before the agent runs" \
    "verified=$(frozen_field "['passing_verified']") reads=$(frozen_field "['source_reads']")"
fi

if [[ "$(frozen_field "['source_timeout']")" == "45" \
   && "$(frozen_field "['default_timeout']")" == "$(frozen_field "['expected_default_timeout']")" ]]; then
  _pass "source timeout is task-scoped and does not change ordinary fixture commands"
else
  _fail "source timeout is task-scoped and does not change ordinary fixture commands" \
    "source=$(frozen_field "['source_timeout']") default=$(frozen_field "['default_timeout']")"
fi

if [[ "$(frozen_field "['passing']['passed']")" == "True" \
   && "$(frozen_field "['passing']['detail']")" == *"3/3"* ]]; then
  _pass "all complete source lines match regardless of target order"
else
  _fail "all complete source lines match regardless of target order" \
    "got $(frozen_field "['passing']")"
fi

if [[ "$(frozen_field "['missing']['passed']")" == "False" \
   && "$(frozen_field "['missing']['detail']")" == *"2/3"* ]]; then
  _pass "omitting one of three source messages fails the cross-check"
else
  _fail "omitting one of three source messages fails the cross-check" \
    "got $(frozen_field "['missing']")"
fi

if [[ "$(frozen_field "['failure_status']")" == "setup_failed" \
   && "$(frozen_field "['failure_steps']")" == "0" \
   && "$(frozen_field "['model_called']")" == "False" ]]; then
  _pass "a missing verification baseline stops before the model runs"
else
  _fail "a missing verification baseline stops before the model runs" \
    "status=$(frozen_field "['failure_status']") steps=$(frozen_field "['failure_steps']") model_called=$(frozen_field "['model_called']")"
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

section "agent harness — a task whose fixture failed is void, not failed by the agent"

# A cold Reminders took longer than the CLI's 10s default to accept its first
# write, so the reminder the task describes was never created. The agent then
# spent its whole step budget looking for it and the run was reported as an
# agent failure — the one reading that is exactly backwards.
FIXTURE_OUT=$(cd "$ROOT_DIR" && python3 - <<'PY' 2>&1
import importlib.util, json, sys

spec = importlib.util.spec_from_file_location("harness", "tests/agent/run.py")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)

broken = {
    "id": "broken-fixture",
    "name": "task whose setup cannot succeed",
    "goal": "unreachable",
    "setup": [["cu", "snapshot", "NoSuchApp-ZZZ"]],
    "verify": [],
}
# A model that would need a key we deliberately do not pass: if the fixture
# check does not stop first, this raises instead of returning a result.
result = harness.run_agent_task(broken, "no-such-model", max_steps=1, verbose=False)

out = {
    "status": result["agent_status"],
    "verified": result["verified"],
    "steps": result["steps"],
    "output_tokens": result["output_tokens"],
    "detail": result["checks"][0]["detail"] if result["checks"] else "",
    # Harness-owned tell commands must carry a longer budget than the product
    # default; the agent's own commands must not be rewritten.
    "tell_gets_timeout": harness.with_fixture_timeout(["cu", "tell", "Notes", "x"]),
    "tell_keeps_own": harness.with_fixture_timeout(["cu", "tell", "Notes", "x", "--timeout", "5"]),
    "non_tell_untouched": harness.with_fixture_timeout(["cu", "snapshot", "Finder"]),
}
print(json.dumps(out))
PY
)

FIXTURE_JSON=$(echo "$FIXTURE_OUT" | tail -1)
if ! echo "$FIXTURE_JSON" | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null; then
  _fail "fixture failure is inspectable" "run_agent_task did not return: ${FIXTURE_OUT:0:300}"
  summary
fi

fixture_field() { echo "$FIXTURE_JSON" | python3 -c "import json,sys; v=json.load(sys.stdin)['$1']; print(json.dumps(v) if isinstance(v,(list,dict)) else v)"; }

if [[ "$(fixture_field status)" == "setup_failed" ]]; then
  _pass "a failed fixture is reported as setup_failed, not as an agent result"
else
  _fail "a failed fixture is reported as setup_failed, not as an agent result" \
    "got status=$(fixture_field status)"
fi

if [[ "$(fixture_field verified)" == "False" ]]; then
  _pass "a task with no fixture cannot report verified"
else
  _fail "a task with no fixture cannot report verified" "verified=$(fixture_field verified)"
fi

if [[ "$(fixture_field steps)" == "0" && "$(fixture_field output_tokens)" == "0" ]]; then
  _pass "no agent steps or tokens are spent against a fixture that failed"
else
  _fail "no agent steps or tokens are spent against a fixture that failed" \
    "steps=$(fixture_field steps) output_tokens=$(fixture_field output_tokens)"
fi

DETAIL=$(fixture_field detail)
if [[ "$DETAIL" == *"setup failed"* && "$DETAIL" == *"app_not_found"* ]]; then
  _pass "the reason names the setup command that failed"
else
  _fail "the reason names the setup command that failed" "got: ${DETAIL:0:200}"
fi

if [[ "$(fixture_field tell_gets_timeout)" == *"--timeout"* ]]; then
  _pass "harness-owned cu tell gets a longer timeout than the product default"
else
  _fail "harness-owned cu tell gets a longer timeout than the product default" \
    "got $(fixture_field tell_gets_timeout)"
fi

if [[ "$(fixture_field tell_keeps_own)" == *'"5"'* ]]; then
  _pass "a command that set its own timeout keeps it"
else
  _fail "a command that set its own timeout keeps it" "got $(fixture_field tell_keeps_own)"
fi

if [[ "$(fixture_field non_tell_untouched)" != *"--timeout"* ]]; then
  _pass "non-tell commands are left alone"
else
  _fail "non-tell commands are left alone" "got $(fixture_field non_tell_untouched)"
fi

section "agent harness — a reply that runs nothing must not read as success"

# SKILL.md's Initialize step tells the agent to export PATH before calling cu.
# When a reply joined them with `;` on one line, the extractor — which required
# the line to *start* with `cu ` — dropped it. One release run extracted 0
# commands across 3 tasks, and each was scored as the agent having failed.
LOOP_OUT=$(cd "$ROOT_DIR" && python3 - <<'PY' 2>&1
import importlib.util, json, sys

spec = importlib.util.spec_from_file_location("harness", "tests/agent/run.py")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)

extraction = {
    "export_prefix": harness._extract_cu_commands(
        'export PATH="/x:$PATH"; export COMPUTER_PILOT_CLIENT_KEY="k"; cu apps'),
    "cd_prefix": harness._extract_cu_commands('cd /tmp; cu apps'),
    "plain": harness._extract_cu_commands('cu snapshot Finder'),
    # A semicolon inside the AppleScript must survive untouched.
    "quoted_semicolon": harness._extract_cu_commands(
        "cu tell Notes 'set x to 1; set y to 2'"),
    "not_a_cu_line": harness._extract_cu_commands('echo hello; ls -la'),
}

# Drive the loop with a model that never emits a command: the run must say so
# and stop, not drift until the agent declares DONE against an untouched desktop.
seen = []
def stub_llm(model, messages):
    # The feedback is appended after the call returns, so record the whole
    # history rather than only the message that prompted this turn.
    seen.extend(m["content"] for m in messages if m["role"] == "user")
    return ("I will now do the task.", 1, 1)
harness.call_llm = stub_llm

result = harness.run_agent_task(
    {"id": "silent", "name": "reply with no command", "goal": "nothing", "verify": []},
    "stub-model", max_steps=10, verbose=False)

print(json.dumps({
    "extraction": extraction,
    "status": result["agent_status"],
    "steps": result["steps"],
    "told_agent": any("no cu command was found" in m for m in seen),
}))
PY
)

LOOP_JSON=$(echo "$LOOP_OUT" | tail -1)
if ! echo "$LOOP_JSON" | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null; then
  _fail "agent loop is inspectable" "could not drive run_agent_task: ${LOOP_OUT:0:300}"
  summary
fi

loop_field() { echo "$LOOP_JSON" | python3 -c "import json,sys; v=json.load(sys.stdin)$1; print(json.dumps(v) if isinstance(v,(list,dict)) else v)"; }

for case in export_prefix cd_prefix plain; do
  if [[ "$(loop_field "['extraction']['$case']")" == *"cu "* ]]; then
    _pass "extractor finds the command in a $case line"
  else
    _fail "extractor finds the command in a $case line" "got $(loop_field "['extraction']['$case']")"
  fi
done

if [[ "$(loop_field "['extraction']['quoted_semicolon']")" == *"set x to 1; set y to 2"* ]]; then
  _pass "a semicolon inside the script is not treated as a shell separator"
else
  _fail "a semicolon inside the script is not treated as a shell separator" \
    "got $(loop_field "['extraction']['quoted_semicolon']")"
fi

if [[ "$(loop_field "['extraction']['not_a_cu_line']")" == "[]" ]]; then
  _pass "a line with no cu command yields nothing"
else
  _fail "a line with no cu command yields nothing" "got $(loop_field "['extraction']['not_a_cu_line']")"
fi

if [[ "$(loop_field "['told_agent']")" == "True" ]]; then
  _pass "the agent is told when its reply ran nothing"
else
  _fail "the agent is told when its reply ran nothing" "no feedback message was sent"
fi

if [[ "$(loop_field "['status']")" == "no_commands" ]]; then
  _pass "a run that never executes anything ends as no_commands"
else
  _fail "a run that never executes anything ends as no_commands" \
    "got status=$(loop_field "['status']")"
fi

if [[ "$(loop_field "['steps']")" -lt 10 ]]; then
  _pass "it stops early instead of spending the whole step budget"
else
  _fail "it stops early instead of spending the whole step budget" \
    "steps=$(loop_field "['steps']")"
fi

section "agent harness — running out of budget is not the agent being wrong"

# calendar_reminder has finished in 4, 5, 11 and 15 steps on this machine, and
# the 15 was against a 15-step ceiling. When the budget runs out first, a
# failed check says nothing about the agent — but in the summary it reads
# exactly like one, which is how a harness limit becomes "L2 blocked the
# release".
BUDGET_OUT=$(cd "$ROOT_DIR" && python3 - <<'PY' 2>&1
import importlib.util, json, sys

spec = importlib.util.spec_from_file_location("harness", "tests/agent/run.py")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)

# An agent that keeps working and never declares DONE.
harness.call_llm = lambda model, messages: ("cu apps", 1, 1)

failing_check = {"description": "artifact exists",
                 "command": ["cu", "snapshot", "NoSuchApp-ZZZ"],
                 "expect_contains": "NoSuchApp-ZZZ"}

ran_out = harness.run_agent_task(
    {"id": "b", "name": "never finishes", "goal": "g", "verify": [failing_check]},
    "stub-model", max_steps=2, verbose=False)
ran_out_clean = harness.run_agent_task(
    {"id": "c", "name": "never finishes, nothing to check", "goal": "g", "verify": []},
    "stub-model", max_steps=2, verbose=False)

print(json.dumps({
    "status": ran_out["agent_status"],
    "steps": ran_out["steps"],
    "advice": ran_out.get("budget_advice", ""),
    "advice_when_verified": ran_out_clean.get("budget_advice", ""),
    "default_budget": harness.run_agent_task.__defaults__[0],
}))
PY
)

BUDGET_JSON=$(echo "$BUDGET_OUT" | tail -1)
if ! echo "$BUDGET_JSON" | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null; then
  _fail "budget outcome is inspectable" "could not drive run_agent_task: ${BUDGET_OUT:0:300}"
  summary
fi

budget_field() { echo "$BUDGET_JSON" | python3 -c "import json,sys; print(json.load(sys.stdin)['$1'])"; }

if [[ "$(budget_field status)" == "budget_exhausted" ]]; then
  _pass "a run cut off by the step budget says so"
else
  _fail "a run cut off by the step budget says so" "got status=$(budget_field status)"
fi

if [[ "$(budget_field steps)" == "2" ]]; then
  _pass "it spends the whole budget before reporting that"
else
  _fail "it spends the whole budget before reporting that" "steps=$(budget_field steps)"
fi

ADVICE=$(budget_field advice)
if [[ "$ADVICE" == *"budget was too small"* && "$ADVICE" == *"--max-steps"* ]]; then
  _pass "a failed check carries advice naming the budget as a possible cause"
else
  _fail "a failed check carries advice naming the budget as a possible cause" "got: ${ADVICE:0:160}"
fi

# The advisory belongs only on the degraded path — attaching it to a run whose
# checks passed would train the reader to skip it.
if [[ -z "$(budget_field advice_when_verified)" ]]; then
  _pass "no advice when the checks passed anyway"
else
  _fail "no advice when the checks passed anyway" "got: $(budget_field advice_when_verified)"
fi

if [[ "$(budget_field default_budget)" -ge 25 ]]; then
  _pass "the default budget leaves margin above the observed 15-step run"
else
  _fail "the default budget leaves margin above the observed 15-step run" \
    "default=$(budget_field default_budget)"
fi

section "agent harness — command results carry success separately from text"

# cu_result keeps ok and text apart; collapsing them back into a bare string is
# what let error text be graded as output.
if grep -Fq 'res = fixture_cu(check["command"])' "$RUNNER" \
  && grep -Fq 'if not res["ok"]:' "$RUNNER"; then
  _pass "verification reads command success, not just its text"
else
  _fail "verification reads command success, not just its text" \
    "verify_task no longer consults the ok/text pair of its check command"
fi

summary
