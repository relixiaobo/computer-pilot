#!/usr/bin/env python3
"""
Agent end-to-end test runner.

Gives a real LLM agent a task, lets it call `cu` commands, then verifies
the actual app state matches expectations. This catches problems like:
- Agent fabricating content instead of reading real data
- Agent skipping steps
- Agent misusing commands

Usage:
  python3 tests/agent/run.py
  python3 tests/agent/run.py --task tests/agent/tasks/mail_to_notes.json
  python3 tests/agent/run.py --dry-run
  python3 tests/agent/run.py --model claude-sonnet-5
  python3 tests/agent/run.py --model gpt-5.6-sol   # any non-Claude id uses the OpenAI path

Model selection: --model, else AGENT_MODEL, else claude-sonnet-5. Put
AGENT_MODEL in .env next to the key it belongs to — release.sh runs this with
no flags, so .env is the only place a release can express which model to use.
An OpenAI-compatible relay needs OPENAI_BASE_URL set alongside OPENAI_API_KEY.
"""

import argparse
import json
import os
import re
import subprocess
import sys
import time
from glob import glob
from pathlib import Path

# Load .env file from project root (if exists)
_env_path = Path(__file__).resolve().parent.parent.parent / ".env"
if _env_path.exists():
    for line in _env_path.read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            key, _, value = line.partition("=")
            os.environ.setdefault(key.strip(), value.strip())

CU = os.environ.get("CU", "./target/release/cu")

# Load SKILL.md as the system prompt — same instructions the agent gets in production
_skill_path = Path(__file__).resolve().parent.parent.parent / "plugin" / "skills" / "computer-pilot" / "SKILL.md"
_skill_content = ""
if _skill_path.exists():
    _raw = _skill_path.read_text()
    # Strip YAML frontmatter
    if _raw.startswith("---"):
        _raw = _raw.split("---", 2)[-1]
    _skill_content = _raw.strip()

SYSTEM_PROMPT = f"""You are a macOS automation agent. You control a Mac through the `cu` CLI tool.

{_skill_content}

## Agent Test Rules
- Output `cu` commands one per line. They will be executed and you'll see the results.
- After completing the task, output: DONE
- If stuck after multiple attempts, output: FAIL
- ALWAYS read actual data before writing summaries. Never fabricate content.
"""


# `cu apps` is allowed up to 60s on machines with many running apps, so a
# shorter deadline here would kill legitimate commands. Matches the default in
# tests/commands/helpers.sh.
CU_TIMEOUT = int(os.environ.get("CU_TIMEOUT_SECS", "75"))

# How much of a command's output the agent gets to see. Truncation is
# announced in the text itself so a cut can never look like a short answer.
RESULT_CHAR_LIMIT = int(os.environ.get("AGENT_RESULT_CHARS", "6000"))

# SKILL.md's Initialize step requires both of these. Without them the agent
# under test runs with the default client key and no task-owned output
# directory, which is not the contract production agents follow.
AGENT_CLIENT_KEY = os.environ.get("COMPUTER_PILOT_CLIENT_KEY", "agent-e2e")
AGENT_OUTPUT_DIR = os.environ.get(
    "COMPUTER_PILOT_OUTPUT_DIR",
    str(Path(__file__).resolve().parent.parent.parent / "test-results" / "agent-output"),
)


def cu_result(args: list[str]) -> dict:
    """Run a cu command and return {"ok", "text"} as a real shell would show it.

    cu writes success JSON to stdout, error JSON to stderr, and signals failure
    through the exit status. Returning stdout alone made every failed command
    look like an empty success: the agent under test could not see the error
    code, the hint, or that anything had gone wrong — so it kept going blind,
    and verification could not tell a failed check from an absent result.
    """
    env = dict(os.environ)
    env["COMPUTER_PILOT_CLIENT_KEY"] = AGENT_CLIENT_KEY
    env["COMPUTER_PILOT_OUTPUT_DIR"] = AGENT_OUTPUT_DIR
    Path(AGENT_OUTPUT_DIR).mkdir(parents=True, exist_ok=True)
    try:
        result = subprocess.run(
            [CU] + args, capture_output=True, text=True, timeout=CU_TIMEOUT, env=env
        )
    except subprocess.TimeoutExpired:
        return {"ok": False, "text": json.dumps(
            {"ok": False, "code": "harness_timeout",
             "error": f"cu did not finish within {CU_TIMEOUT}s"})}
    except Exception as e:
        return {"ok": False, "text": json.dumps({"ok": False, "error": str(e)})}

    out = result.stdout.strip()
    err = result.stderr.strip()
    if result.returncode != 0:
        return {"ok": False, "text": err or out or json.dumps(
            {"ok": False, "code": "unknown_failure",
             "error": f"cu exited {result.returncode} with no output"})}
    return {"ok": True, "text": out or err}


def cu(args: list[str]) -> str:
    """Agent-facing view of a cu command: stdout on success, error JSON on failure."""
    return cu_result(args)["text"]


# Current Claude models run adaptive thinking by default, and max_tokens caps
# thinking PLUS response text together — a budget sized for reply text alone
# truncates the agent mid-command, which is indistinguishable from the agent
# failures this suite exists to detect.
MAX_TOKENS = int(os.environ.get("AGENT_MAX_TOKENS", "16000"))


def is_claude_model(model: str) -> bool:
    """Which provider a model id routes to. The credential preflight and the
    call site must agree, so both read this one predicate."""
    return "claude" in model or "opus" in model or "sonnet" in model


def credential_error(model: str) -> str | None:
    """Actionable message when the model can't be reached, else None.

    release.sh gates L2 on *a* key being present, which an OpenAI-only .env
    satisfies — but the model default used to be a Claude id regardless, so
    the run died inside the agent loop as a generic 'LLM error' after the
    release had already started building.
    """
    if is_claude_model(model):
        if not os.environ.get("ANTHROPIC_API_KEY"):
            return (f"model '{model}' needs ANTHROPIC_API_KEY (not found in the "
                    f"environment or .env). Set it, or select a model that "
                    f"matches the key you have via AGENT_MODEL / --model.")
    elif not os.environ.get("OPENAI_API_KEY"):
        return (f"model '{model}' needs OPENAI_API_KEY (not found in the "
                f"environment or .env), plus OPENAI_BASE_URL when it is served "
                f"by a relay. Set it, or select a Claude model via "
                f"AGENT_MODEL / --model.")
    return None


def call_llm(model: str, messages: list[dict]) -> tuple[str, int, int]:
    """Call LLM, return (text, input_tokens, output_tokens)."""
    if is_claude_model(model):
        import anthropic
        client = anthropic.Anthropic()
        response = client.messages.create(
            model=model, max_tokens=MAX_TOKENS,
            system=SYSTEM_PROMPT, messages=messages,
        )
        text = next((b.text for b in response.content if b.type == "text"), "")
        return text, response.usage.input_tokens, response.usage.output_tokens
    else:
        import openai
        client = openai.OpenAI()
        response = client.chat.completions.create(
            model=model, max_completion_tokens=MAX_TOKENS,
            messages=[{"role": "system", "content": SYSTEM_PROMPT}] + messages,
        )
        return response.choices[0].message.content, response.usage.prompt_tokens, response.usage.completion_tokens


# SKILL.md's Initialize step tells the agent to export PATH before calling cu,
# so a reply that joins them with `;` on one line is following the skill, not
# defying the harness. Requiring the line to *start* with `cu ` silently
# dropped every such line: one release run extracted 0 commands from 3 tasks
# and scored the agent as having failed them.
_SHELL_PREFIX = re.compile(
    r'''^\s*(?:(?:export\s+[A-Za-z_][A-Za-z0-9_]*=(?:"[^"]*"|'[^']*'|[^;\s]*)|cd\s+(?:"[^"]*"|'[^']*'|[^;\s]+))\s*;\s*)+'''
)


def _strip_shell_prefix(line: str) -> str:
    """Drop leading `export VAR=...;` / `cd ...;` segments. Everything from the
    command itself onward is left untouched, so quoting inside a cu tell script
    is never disturbed."""
    return _SHELL_PREFIX.sub('', line, count=1)


def _extract_cu_commands(text: str) -> list[str]:
    """Extract cu commands from LLM response, handling multi-line scripts in quotes."""
    commands = []
    lines = text.split('\n')
    i = 0
    while i < len(lines):
        line = _strip_shell_prefix(lines[i].strip().strip('`'))
        if line.startswith('cu '):
            # Check if this line has an unclosed single quote (multi-line script)
            if line.count("'") % 2 == 1:
                # Collect lines until closing quote
                multi = [line]
                i += 1
                while i < len(lines):
                    next_line = lines[i].rstrip()
                    # Strip leading ``` or backticks
                    if next_line.strip().startswith('`'):
                        next_line = next_line.strip().strip('`')
                    multi.append(next_line)
                    if "'" in next_line:
                        break
                    i += 1
                commands.append('\n'.join(multi))
            else:
                commands.append(line)
        i += 1
    return commands


# A cold Reminders needs more than the CLI's 10s default to accept its first
# write; the same is true of Mail and Calendar. That is an environment cost of
# the harness building its own fixtures, not product behaviour under test, so
# harness-owned `cu tell` calls get a longer budget. The agent's own commands
# are deliberately left on the product default — the test has to reflect what
# an agent actually experiences.
FIXTURE_TELL_TIMEOUT = os.environ.get("AGENT_FIXTURE_TELL_TIMEOUT", "30")


def with_fixture_timeout(cmd: list[str]) -> list[str]:
    """Give a harness-owned `cu tell` a longer timeout, unless it set its own."""
    if cmd[:2] != ["cu", "tell"] or "--timeout" in cmd:
        return cmd
    return cmd + ["--timeout", FIXTURE_TELL_TIMEOUT]


def fixture_cu(cmd: list[str]) -> dict:
    """Run a harness-owned `cu` command — verification reads, mostly."""
    return cu_result(with_fixture_timeout(cmd)[1:])


def _run_fixture_cmd(cmd: list[str]) -> dict:
    """Run one setup/cleanup entry. `cu ...` goes through the CLI; anything
    else runs as a plain command, so a task can build the world it needs
    (files on the Desktop, for instance) and not only drive apps."""
    cmd = with_fixture_timeout(cmd)
    if cmd and cmd[0] == "cu":
        return cu_result(cmd[1:])
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=CU_TIMEOUT)
        return {"ok": r.returncode == 0, "text": (r.stderr or r.stdout).strip()}
    except Exception as e:
        return {"ok": False, "text": str(e)}


def run_setup(task: dict, verbose: bool = True) -> str | None:
    """Run setup commands. Returns None on success, else why the task is void.

    A failed setup means the world the task describes was never built, so
    anything the agent does next is measured against the wrong state. Steps
    that are genuinely optional already say so in AppleScript (`try ... end
    try` returns ok), so a failure here is real.
    """
    for cmd in task.get("setup", []):
        res = _run_fixture_cmd(cmd)
        if not res["ok"]:
            reason = f"setup failed: {' '.join(cmd)[:80]} → {res['text'][:200]}"
            if verbose:
                print(f"  [setup] {reason}")
            return reason
        time.sleep(0.5)
    return None


def run_cleanup(task: dict, verbose: bool = True):
    """Run cleanup commands. Leftovers poison the next run, so report failures."""
    for cmd in task.get("cleanup", []):
        res = _run_fixture_cmd(cmd)
        if not res["ok"] and verbose:
            print(f"  [cleanup] non-zero: {' '.join(cmd)[:80]} → {res['text'][:120]}")
        time.sleep(0.5)


def verify_task(task: dict) -> list[dict]:
    """Run verification checks. Returns list of {check, passed, detail}."""
    results = []
    for check in task.get("verify", []):
        desc = check["description"]

        if "command" in check:
            res = fixture_cu(check["command"])
            # A failed command's error text is still matched by the checks
            # below, and AppleScript quotes the name it could not find
            # (Can't get note "Agent Test - X") — so a missing note satisfied
            # the expect_contains that was supposed to prove it exists. Same
            # for expect_min_length: an error message is long enough.
            if not res["ok"]:
                results.append({
                    "check": desc, "passed": False,
                    "detail": f"verification command failed: {res['text'][:200]}"
                })
            else:
                output = res["text"]
                try:
                    parsed = json.loads(output)
                    value = str(parsed.get("result", output))
                except json.JSONDecodeError:
                    value = output

                # expect_contains check
                if "expect_contains" in check:
                    expected = check["expect_contains"]
                    passed = expected.lower() in value.lower()
                    results.append({
                        "check": desc, "passed": passed,
                        "detail": f"expected '{expected}' in output, got: {value[:200]}"
                    })

                # expect_min_length check
                if "expect_min_length" in check:
                    try:
                        length = int(value.strip().strip('"'))
                    except ValueError:
                        length = len(value)
                    min_len = check["expect_min_length"]
                    passed = length >= min_len
                    results.append({
                        "check": desc, "passed": passed,
                        "detail": f"length={length}, min={min_len}"
                    })

        # cross_check: verify output contains data from another source
        if "cross_check" in check:
            cc = check["cross_check"]
            if "source_command" in cc and "target_command" in cc:
                source_res = fixture_cu(cc["source_command"])
                target_res = fixture_cu(cc["target_command"])
                # A failed source command produces no tokens too. Without this
                # guard it was reported as "env gap, not an agent failure" —
                # a real breakage silently marked passed.
                if not source_res["ok"] or not target_res["ok"]:
                    failed = "source" if not source_res["ok"] else "target"
                    detail = (source_res if not source_res["ok"] else target_res)["text"]
                    results.append({
                        "check": f"{desc} (cross-check)", "passed": False,
                        "detail": f"{failed} command failed: {detail[:200]}"
                    })
                    continue
                source_out = source_res["text"]
                target_out = target_res["text"]
                try:
                    source_val = str(json.loads(source_out).get("result", ""))
                    target_val = str(json.loads(target_out).get("result", ""))
                except json.JSONDecodeError:
                    source_val = source_out
                    target_val = target_out

                # Extract a meaningful substring from source to look for in target
                # Remove quotes, take first meaningful word/phrase
                source_clean = source_val.strip().strip('"').strip()
                # Find a specific token (>3 chars) from source in target
                tokens = [t for t in source_clean.split() if len(t) > 3]

                if not tokens:
                    # Source command returned nothing usable — typically an
                    # environmental gap (empty Desktop, no inbox messages,
                    # etc.) rather than an agent failure. Skip with passed=True
                    # so the task isn't blocked; the [SKIP] marker in output
                    # makes it obvious the check was bypassed, not satisfied.
                    results.append({
                        "check": f"{desc} (cross-check)",
                        "passed": True, "skipped": True,
                        "detail": f"source command returned no usable tokens (source={source_clean[:80]!r}) — env gap, not an agent failure"
                    })
                else:
                    found = any(t.lower() in target_val.lower() for t in tokens[:5])
                    results.append({
                        "check": f"{desc} (cross-check)", "passed": found,
                        "detail": f"source tokens: {tokens[:5]}, found in target: {found}"
                    })
            elif "source_command" in cc:
                # source_command only — check against the main command output
                source_res = fixture_cu(cc["source_command"])
                if not source_res["ok"]:
                    results.append({
                        "check": f"{desc} (cross-check)", "passed": False,
                        "detail": f"source command failed: {source_res['text'][:200]}"
                    })
                    continue
                source_out = source_res["text"]
                main_res = fixture_cu(check["command"]) if "command" in check else None
                if main_res is not None and not main_res["ok"]:
                    results.append({
                        "check": f"{desc} (cross-check)", "passed": False,
                        "detail": f"target command failed: {main_res['text'][:200]}"
                    })
                    continue
                main_out = main_res["text"] if main_res else ""
                try:
                    source_val = str(json.loads(source_out).get("result", ""))
                    main_val = str(json.loads(main_out).get("result", ""))
                except json.JSONDecodeError:
                    source_val = source_out
                    main_val = main_out

                source_clean = source_val.strip().strip('"')
                tokens = [t for t in source_clean.split() if len(t) > 3]

                if not tokens:
                    results.append({
                        "check": f"{desc} (cross-check)",
                        "passed": True, "skipped": True,
                        "detail": f"source command returned no usable tokens (source={source_clean[:80]!r}) — env gap, not an agent failure"
                    })
                else:
                    found = any(t.lower() in main_val.lower() for t in tokens[:5])
                    results.append({
                        "check": f"{desc} (cross-check)", "passed": found,
                        "detail": f"source: {source_clean[:100]}"
                    })

    return results


def run_agent_task(task: dict, model: str, max_steps: int = 15, verbose: bool = True) -> dict:
    """Run one agent task end-to-end."""
    task_id = task["id"]
    goal = task["goal"]

    if verbose:
        print(f"\n{'='*60}")
        print(f"Task: {task['name']}")
        print(f"Goal: {goal}")
        print(f"{'='*60}")

    # Setup
    setup_failure = run_setup(task)
    if setup_failure:
        # Running the agent now would spend a full step budget against state
        # the task never established, and report the result as an agent
        # failure. Fail here, where the reason is still legible.
        run_cleanup(task)
        return {
            "task_id": task_id,
            "task_name": task["name"],
            "agent_status": "setup_failed",
            "verified": False,
            "checks": [{"check": "task fixture established",
                        "passed": False, "detail": setup_failure}],
            "steps": 0,
            "input_tokens": 0,
            "output_tokens": 0,
        }
    time.sleep(1)

    messages = []
    total_input = 0
    total_output = 0
    steps = 0
    status = "incomplete"
    empty_replies = 0

    for step in range(1, max_steps + 1):
        steps = step

        # Build prompt
        apps_out = cu(["apps"])
        user_msg = f"""Task: {goal}

Step {step}/{max_steps}.

Running apps:
{apps_out[:1500]}

What cu commands should I run next? Output one command per line.
When done, say DONE. If stuck, say FAIL."""

        messages.append({"role": "user", "content": user_msg})

        try:
            response, inp, out = call_llm(model, messages)
        except Exception as e:
            if verbose: print(f"  Step {step}: LLM error: {e}")
            status = "error"
            break

        total_input += inp
        total_output += out
        messages.append({"role": "assistant", "content": response})

        if verbose:
            print(f"  Step {step}: {response[:150]}...")

        # Execute cu commands — handle multi-line cu tell scripts
        results = []
        cu_commands = _extract_cu_commands(response)
        for cmd_line in cu_commands:
            import shlex
            try:
                args = shlex.split(cmd_line[3:])  # skip "cu "
            except ValueError:
                args = cmd_line[3:].split()
            if verbose:
                display = cmd_line[:120] + ("..." if len(cmd_line) > 120 else "")
                print(f"    $ cu {' '.join(args[:5])}{'...' if len(args) > 5 else ''}")
            r = cu(args)
            # A snapshot easily exceeds 1000 chars; silently cutting it handed
            # the agent invalid JSON with no indication anything was missing.
            if len(r) > RESULT_CHAR_LIMIT:
                shown = (r[:RESULT_CHAR_LIMIT]
                         + f"\n[truncated by test harness at {RESULT_CHAR_LIMIT} "
                           f"of {len(r)} chars — re-run with a narrower --limit]")
            else:
                shown = r
            results.append(f"$ {cmd_line[:200]}\n{shown}")
            if verbose:
                print(f"      → {r[:150]}")
            time.sleep(0.5)

        if results:
            messages.append({"role": "user", "content": "Results:\n" + "\n---\n".join(results)})

        if not results:
            if "DONE" in response:
                status = "done"
                break
            if "FAIL" in response:
                status = "fail"
                break
            # The reply ran nothing and claimed nothing. Saying so is the whole
            # point: silence here reads to the agent as "that worked", and a
            # few quiet turns later it declares DONE against a desktop it never
            # touched — which the run then scores as an agent failure.
            empty_replies += 1
            messages.append({"role": "user", "content": (
                "Nothing ran: no cu command was found in that reply. Put each "
                "command on its own line beginning with `cu `. Environment "
                "setup may precede it on the same line, separated by `;`.")})
            if empty_replies >= 3:
                status = "no_commands"
                break

        time.sleep(0.3)

    # Verify
    time.sleep(1)
    verify_results = verify_task(task)
    all_passed = all(v["passed"] for v in verify_results)

    if verbose:
        print(f"\n  Agent status: {status}")
        print(f"  Verification:")
        for v in verify_results:
            if v.get("skipped"):
                mark = "SKIP"
            elif v["passed"]:
                mark = "PASS"
            else:
                mark = "FAIL"
            print(f"    [{mark}] {v['check']}")
            if not v["passed"] or v.get("skipped"):
                print(f"           {v['detail']}")

    # Cleanup
    run_cleanup(task)

    return {
        "task_id": task_id,
        "task_name": task["name"],
        "agent_status": status,
        "verified": all_passed,
        "checks": verify_results,
        "steps": steps,
        "input_tokens": total_input,
        "output_tokens": total_output,
    }


def main():
    parser = argparse.ArgumentParser(description="Agent E2E test runner")
    parser.add_argument("--task", help="Path to single task JSON")
    parser.add_argument("--tasks-dir", default="tests/agent/tasks", help="Directory of task JSONs")
    parser.add_argument("--model", default=os.environ.get("AGENT_MODEL", "claude-sonnet-5"))
    parser.add_argument("--max-steps", type=int, default=15)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    # Collect tasks
    task_files = []
    if args.task:
        task_files = [args.task]
    else:
        task_files = sorted(glob(os.path.join(args.tasks_dir, "*.json")))

    if not task_files:
        print("No task files found.", file=sys.stderr)
        sys.exit(1)

    tasks = []
    for f in task_files:
        with open(f) as fh:
            tasks.append(json.load(fh))

    if args.dry_run:
        for t in tasks:
            print(f"  {t['id']}: {t['name']}")
            print(f"    Goal: {t['goal'][:80]}...")
            print(f"    Verify: {len(t.get('verify', []))} checks")
        print(f"\n{len(tasks)} tasks")
        return

    # Fail before touching the desktop or the release, not 15 steps in.
    problem = credential_error(args.model)
    if problem:
        print(f"Cannot run agent tests: {problem}", file=sys.stderr)
        sys.exit(1)

    # Run
    results = []
    for t in tasks:
        result = run_agent_task(t, args.model, args.max_steps)
        results.append(result)

    # Summary
    passed = sum(1 for r in results if r["verified"])
    total = len(results)
    tokens = sum(r["input_tokens"] + r["output_tokens"] for r in results)

    print(f"\n{'='*60}")
    print(f"AGENT TEST RESULTS: {passed}/{total} verified")
    print(f"Total tokens: {tokens:,}")
    print(f"Model: {args.model}")
    print(f"{'='*60}")

    for r in results:
        mark = "PASS" if r["verified"] else "FAIL"
        print(f"  [{mark}] {r['task_name']} (steps={r['steps']}, status={r['agent_status']})")
        for c in r["checks"]:
            if c.get("skipped"):
                cm = "○"
            elif c["passed"]:
                cm = "✓"
            else:
                cm = "✗"
            print(f"         {cm} {c['check']}")

    # Save results
    out_dir = "test-results"
    os.makedirs(out_dir, exist_ok=True)
    out_path = f"{out_dir}/agent-{int(time.time())}.json"
    with open(out_path, 'w') as f:
        json.dump({"model": args.model, "passed": passed, "total": total, "results": results}, f, indent=2)
    print(f"\nResults saved: {out_path}")

    sys.exit(0 if passed == total else 1)


if __name__ == "__main__":
    main()
