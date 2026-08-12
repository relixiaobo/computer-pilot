#!/bin/bash
# Test: L2 cleanup failures are loud, and the real task cleanup removes every
# exact fixture it owns rather than swallowing AppleScript errors.
source "$(dirname "$0")/helpers.sh"

section "agent cleanup — failures cannot read as a verified task"

LOGIC_OUT=$(cd "$ROOT_DIR" && python3 - <<'PY' 2>&1
import importlib.util
import json

spec = importlib.util.spec_from_file_location("harness", "tests/agent/run.py")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)
harness.time.sleep = lambda _: None

commands = [["first"], ["second"], ["third"]]
responses = iter([
    {"ok": False, "text": "first broke"},
    {"ok": True, "text": "second cleaned"},
    {"ok": False, "text": "third broke"},
    {"ok": False, "text": "fixture still visible"},
    {"ok": True, "text": "fixture gone"},
])
called = []
def fixture_cmd(cmd):
    called.append(cmd[0])
    return next(responses)
harness._run_fixture_cmd = fixture_cmd
failures = harness.run_cleanup(
    {"cleanup": commands, "cleanup_verify": [["verify"]]}, verbose=False)

until_responses = iter([
    {"ok": False, "text": "one fixture remains"},
    {"ok": True, "text": "one fixture removed"},
    {"ok": False, "text": "one fixture remains"},
    {"ok": True, "text": "last fixture removed"},
    {"ok": True, "text": "fixture gone"},
])
until_called = []
def until_cmd(cmd):
    until_called.append(cmd[0])
    return next(until_responses)
harness._run_fixture_cmd = until_cmd
until_failures = harness.run_cleanup({
    "cleanup_until": [{"command": ["remove-one"], "verify": ["verify-none"]}]
}, verbose=False)

# A successful pre-clean followed by a failed final cleanup must fail the gate
# without rewriting what the agent itself did.
cleanup_calls = 0
def final_cleanup_fails(task, verbose=True):
    global cleanup_calls
    cleanup_calls += 1
    return [] if cleanup_calls == 1 else ["cleanup failed: exact fixture remains"]
harness.run_cleanup = final_cleanup_fails
harness.cu = lambda args: '{"ok":true,"apps":[]}'
harness.call_llm = lambda model, messages: ("DONE", 1, 1)
post_failure = harness.run_agent_task(
    {"id": "post", "name": "post cleanup fails", "goal": "g", "verify": []},
    "stub-model", max_steps=1, verbose=False)

# A dirty starting world must stop before the model spends a step.
model_called = False
def should_not_run(model, messages):
    global model_called
    model_called = True
    raise AssertionError("model ran against a dirty fixture")
harness.run_cleanup = lambda task, verbose=True: ["cleanup failed: stale fixture remains"]
harness.call_llm = should_not_run
pre_failure = harness.run_agent_task(
    {"id": "pre", "name": "pre cleanup fails", "goal": "g", "verify": []},
    "stub-model", max_steps=1, verbose=False)

print(json.dumps({
    "all_commands_ran": called,
    "failure_count": len(failures),
    "failure_details": failures,
    "until_commands": until_called,
    "until_failures": until_failures,
    "post_status": post_failure["agent_status"],
    "post_verified": post_failure["verified"],
    "post_error": post_failure.get("cleanup_error", ""),
    "post_has_failed_check": any(not c["passed"] for c in post_failure["checks"]),
    "pre_status": pre_failure["agent_status"],
    "pre_verified": pre_failure["verified"],
    "pre_steps": pre_failure["steps"],
    "pre_error": pre_failure.get("cleanup_error", ""),
    "model_called": model_called,
}))
PY
)

LOGIC_JSON=$(echo "$LOGIC_OUT" | tail -1)
if ! echo "$LOGIC_JSON" | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null; then
  _fail "cleanup result is inspectable" "could not drive cleanup logic: ${LOGIC_OUT:0:300}"
  summary
fi

logic_field() { echo "$LOGIC_JSON" | python3 -c "import json,sys; v=json.load(sys.stdin)['$1']; print(json.dumps(v) if isinstance(v,(list,dict)) else v)"; }

if [[ "$(logic_field until_commands)" == '["verify-none", "remove-one", "verify-none", "remove-one", "verify-none"]' \
   && "$(logic_field until_failures)" == "[]" ]]; then
  _pass "cleanup-until uses fresh mutations until its postcondition passes"
else
  _fail "cleanup-until uses fresh mutations until its postcondition passes" \
    "called=$(logic_field until_commands) failures=$(logic_field until_failures)"
fi

if [[ "$(logic_field all_commands_ran)" == '["first", "second", "third", "verify", "verify"]' \
   && "$(logic_field failure_count)" == "2" ]]; then
  _pass "cleanup continues after failures and retries the postcondition"
else
  _fail "cleanup continues after failures and retries the postcondition" \
    "called=$(logic_field all_commands_ran) failures=$(logic_field failure_count)"
fi

if [[ "$(logic_field post_status)" == "done" \
   && "$(logic_field post_verified)" == "False" \
   && "$(logic_field post_has_failed_check)" == "True" \
   && "$(logic_field post_error)" == *"exact fixture remains"* ]]; then
  _pass "a final cleanup failure fails the gate without blaming the agent"
else
  _fail "a final cleanup failure fails the gate without blaming the agent" \
    "status=$(logic_field post_status) verified=$(logic_field post_verified) error=$(logic_field post_error)"
fi

if [[ "$(logic_field pre_status)" == "setup_failed" \
   && "$(logic_field pre_verified)" == "False" \
   && "$(logic_field pre_steps)" == "0" \
   && "$(logic_field model_called)" == "False" \
   && "$(logic_field pre_error)" == *"stale fixture remains"* ]]; then
  _pass "a failed pre-clean stops before the model touches a dirty fixture"
else
  _fail "a failed pre-clean stops before the model touches a dirty fixture" \
    "status=$(logic_field pre_status) steps=$(logic_field pre_steps) model_called=$(logic_field model_called)"
fi

section "agent cleanup — real task fixtures are removed"

BEHAVIOR_OUT=$(cd "$ROOT_DIR" && CU="$CU" python3 - <<'PY' 2>&1
import importlib.util
import json
from pathlib import Path

spec = importlib.util.spec_from_file_location("harness", "tests/agent/run.py")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)

task_dir = Path("tests/agent/tasks")
tasks = {
    name: json.loads((task_dir / name).read_text())
    for name in ("calendar_reminder.json", "finder_organize.json", "mail_to_notes.json")
}

def run(cmd):
    result = harness._run_fixture_cmd(cmd)
    if not result["ok"]:
        raise RuntimeError(f"fixture command failed: {cmd[:3]}: {result['text']}")
    return json.loads(result["text"]).get("result", "") if cmd[0] == "cu" else result["text"]

def calendar_count():
    return int(run(["cu", "tell", "Calendar", "set total to 0\nrepeat with c in calendars\n  set total to total + (count of (every event of c whose summary is \"Agent Test Task\"))\nend repeat\nreturn total"]))

def reminder_count():
    return int(run(["cu", "tell", "Reminders", "set total to 0\nrepeat with l in lists\n  set total to total + (count of (every reminder of l whose name is \"Agent Test Task\"))\nend repeat\nreturn total"]))

def note_count(name):
    return int(run(["cu", "tell", "Notes", f'count of (every note whose name is "{name}")']))

def desktop_count():
    desktop = Path.home() / "Desktop"
    return sum((desktop / name).exists() for name in (
        "cu-l2-fixture-report.txt", "cu-l2-fixture-notes.md", "cu-l2-fixture-archive"))

all_tasks = list(tasks.values())
for task in all_tasks:
    failures = harness.run_cleanup(task, verbose=False)
    if failures:
        raise RuntimeError("pre-test cleanup failed: " + " | ".join(failures))

try:
    run(["cu", "tell", "Reminders", "repeat 2 times\n  make new reminder with properties {name:\"Agent Test Task\", body:\"cleanup behavior fixture\"}\nend repeat\nreturn \"created\""])
    run(["cu", "tell", "Calendar", "set startDate to current date\nset endDate to startDate + 3600\nset targetCalendar to first calendar whose writable is true\nrepeat 2 times\n  make new event at end of events of targetCalendar with properties {summary:\"Agent Test Task\", start date:startDate, end date:endDate}\nend repeat\nreturn \"created\""])
    run(["cu", "tell", "Notes", "repeat 2 times\n  make new note with properties {name:\"Agent Test - Desktop Inventory\", body:\"cleanup behavior fixture\"}\n  make new note with properties {name:\"Agent Test - Mail Summary\", body:\"cleanup behavior fixture\"}\nend repeat\nreturn \"created\""])
    setup_failure = harness.run_setup(tasks["finder_organize.json"], verbose=False)
    if setup_failure:
        raise RuntimeError(setup_failure)

    before = {
        "calendar": calendar_count(),
        "reminders": reminder_count(),
        "inventory_notes": note_count("Agent Test - Desktop Inventory"),
        "mail_notes": note_count("Agent Test - Mail Summary"),
        "desktop": desktop_count(),
    }
    cleanup_failures = []
    for task in all_tasks:
        cleanup_failures.extend(harness.run_cleanup(task, verbose=False))
    after = {
        "calendar": calendar_count(),
        "reminders": reminder_count(),
        "inventory_notes": note_count("Agent Test - Desktop Inventory"),
        "mail_notes": note_count("Agent Test - Mail Summary"),
        "desktop": desktop_count(),
    }
    print(json.dumps({"before": before, "after": after, "failures": cleanup_failures}))
finally:
    for task in all_tasks:
        harness.run_cleanup(task, verbose=False)
PY
)

BEHAVIOR_JSON=$(echo "$BEHAVIOR_OUT" | tail -1)
if ! echo "$BEHAVIOR_JSON" | python3 -c 'import json,sys; json.load(sys.stdin)' 2>/dev/null; then
  _fail "real cleanup behavior is inspectable" "could not run fixture scenario: ${BEHAVIOR_OUT:0:500}"
  summary
fi

behavior_field() { echo "$BEHAVIOR_JSON" | python3 -c "import json,sys; v=json.load(sys.stdin)$1; print(json.dumps(v,sort_keys=True) if isinstance(v,(list,dict)) else v)"; }

BEFORE=$(behavior_field "['before']")
if echo "$BEFORE" | python3 -c 'import json,sys; d=json.load(sys.stdin); raise SystemExit(0 if all(v >= 2 for v in d.values()) else 1)'; then
  _pass "the behavior test constructs duplicate fixtures in every target"
else
  _fail "the behavior test constructs duplicate fixtures in every target" "got $BEFORE"
fi

AFTER=$(behavior_field "['after']")
if echo "$AFTER" | python3 -c 'import json,sys; d=json.load(sys.stdin); raise SystemExit(0 if all(v == 0 for v in d.values()) else 1)' \
   && [[ "$(behavior_field "['failures']")" == "[]" ]]; then
  _pass "task cleanup removes every exact fixture and reports no failure"
else
  _fail "task cleanup removes every exact fixture and reports no failure" \
    "after=$AFTER failures=$(behavior_field "['failures']")"
fi

summary
