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
  python3 tests/agent/run.py --model claude-sonnet-4-6
"""

import argparse
import json
import os
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


def cu(args: list[str]) -> str:
    """Run a cu command, return stdout."""
    try:
        result = subprocess.run(
            [CU] + args, capture_output=True, text=True, timeout=30
        )
        return result.stdout.strip()
    except Exception as e:
        return json.dumps({"ok": False, "error": str(e)})


def call_llm(model: str, messages: list[dict]) -> tuple[str, int, int]:
    """Call LLM, return (text, input_tokens, output_tokens)."""
    if "claude" in model or "opus" in model or "sonnet" in model:
        import anthropic
        client = anthropic.Anthropic()
        response = client.messages.create(
            model=model, max_tokens=4096,
            system=SYSTEM_PROMPT, messages=messages,
        )
        return response.content[0].text, response.usage.input_tokens, response.usage.output_tokens
    else:
        import openai
        client = openai.OpenAI()
        response = client.chat.completions.create(
            model=model, max_completion_tokens=4096,
            messages=[{"role": "system", "content": SYSTEM_PROMPT}] + messages,
        )
        return response.choices[0].message.content, response.usage.prompt_tokens, response.usage.completion_tokens


def _extract_cu_commands(text: str) -> list[str]:
    """Extract cu commands from LLM response, handling multi-line scripts in quotes."""
    commands = []
    lines = text.split('\n')
    i = 0
    while i < len(lines):
        line = lines[i].strip().strip('`')
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


def run_setup(task: dict):
    """Run setup commands."""
    for cmd in task.get("setup", []):
        cu(cmd[1:])  # skip "cu" prefix
        time.sleep(0.5)


def run_cleanup(task: dict):
    """Run cleanup commands."""
    for cmd in task.get("cleanup", []):
        cu(cmd[1:])
        time.sleep(0.5)


def verify_task(task: dict) -> list[dict]:
    """Run verification checks. Returns list of {check, passed, detail}."""
    results = []
    for check in task.get("verify", []):
        desc = check["description"]

        if "command" in check:
            output = cu(check["command"][1:])
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
                source_out = cu(cc["source_command"][1:])
                target_out = cu(cc["target_command"][1:])
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
                found = any(t.lower() in target_val.lower() for t in tokens[:5]) if tokens else False

                results.append({
                    "check": f"{desc} (cross-check)", "passed": found,
                    "detail": f"source tokens: {tokens[:5]}, found in target: {found}"
                })
            elif "source_command" in cc:
                # source_command only — check against the main command output
                source_out = cu(cc["source_command"][1:])
                main_out = cu(check["command"][1:]) if "command" in check else ""
                try:
                    source_val = str(json.loads(source_out).get("result", ""))
                    main_val = str(json.loads(main_out).get("result", ""))
                except json.JSONDecodeError:
                    source_val = source_out
                    main_val = main_out

                source_clean = source_val.strip().strip('"')
                tokens = [t for t in source_clean.split() if len(t) > 3]
                found = any(t.lower() in main_val.lower() for t in tokens[:5]) if tokens else False
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
    run_setup(task)
    time.sleep(1)

    messages = []
    total_input = 0
    total_output = 0
    steps = 0
    status = "incomplete"

    for step in range(1, max_steps + 1):
        steps = step

        # Build prompt
        apps_out = cu(["apps"])
        user_msg = f"""Task: {goal}

Step {step}/{max_steps}.

Running apps:
{apps_out[:500]}

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
            results.append(f"$ {cmd_line[:200]}\n{r[:1000]}")
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

        time.sleep(0.3)

    # Verify
    time.sleep(1)
    verify_results = verify_task(task)
    all_passed = all(v["passed"] for v in verify_results)

    if verbose:
        print(f"\n  Agent status: {status}")
        print(f"  Verification:")
        for v in verify_results:
            mark = "PASS" if v["passed"] else "FAIL"
            print(f"    [{mark}] {v['check']}")
            if not v["passed"]:
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
    parser.add_argument("--model", default="claude-sonnet-4-6")
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
            cm = "✓" if c["passed"] else "✗"
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
