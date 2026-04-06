#!/usr/bin/env python3
"""
Local macOSWorld benchmark runner for computer-pilot.
Runs tasks on THIS machine (no AWS needed).

Usage:
  # Run a single task
  python tests/macosworld/run_benchmark.py --task /tmp/macosworld/tasks/sys_apps/48cf0af3-*.json

  # Run all sys_apps tasks
  python tests/macosworld/run_benchmark.py --tasks-dir /tmp/macosworld/tasks/sys_apps

  # Run with specific model
  python tests/macosworld/run_benchmark.py --tasks-dir /tmp/macosworld/tasks/sys_apps --model claude-sonnet-4-6

  # Dry run (show tasks without executing)
  python tests/macosworld/run_benchmark.py --tasks-dir /tmp/macosworld/tasks/sys_apps --dry-run
"""

import argparse
import json
import os
import shlex
import subprocess
import sys
import time
from pathlib import Path
from glob import glob

# ── cu CLI interface ─────────────────────────────────────────────────────────

CU = os.environ.get("CU", "./target/release/cu")

def cu(command: str) -> dict:
    """Run a cu command locally, return parsed JSON."""
    try:
        result = subprocess.run(
            [CU] + command.split(),
            capture_output=True, text=True, timeout=30
        )
        stdout = result.stdout.strip()
        if stdout:
            return json.loads(stdout)
        return {"ok": False, "error": result.stderr.strip() or "empty output"}
    except json.JSONDecodeError:
        return {"ok": True, "raw": result.stdout.strip()}
    except subprocess.TimeoutExpired:
        return {"ok": False, "error": "timeout"}
    except Exception as e:
        return {"ok": False, "error": str(e)}

def cu_raw(args: list[str]) -> str:
    """Run cu with raw args, return stdout."""
    try:
        result = subprocess.run(
            [CU] + args, capture_output=True, text=True, timeout=30
        )
        return result.stdout.strip()
    except Exception as e:
        return json.dumps({"ok": False, "error": str(e)})

def cu_line(line: str) -> str:
    """Run a cu command from a string line, handling quoted args properly."""
    try:
        parts = shlex.split(line)
        return cu_raw(parts)
    except Exception as e:
        return json.dumps({"ok": False, "error": str(e)})

# ── LLM interface ────────────────────────────────────────────────────────────

_skill_path = os.path.join(os.path.dirname(__file__), '..', '..', 'plugin', 'skills', 'computer-pilot', 'SKILL.md')
_skill_content = ""
if os.path.exists(_skill_path):
    _raw = open(_skill_path).read()
    if _raw.startswith("---"):
        _raw = _raw.split("---", 2)[-1]
    _skill_content = _raw.strip()

SYSTEM_PROMPT = f"""You are a macOS desktop automation agent. You control a Mac through the `cu` CLI tool.

{_skill_content}

## Rules
- Use --app to target specific apps
- Refs change after every action — always re-snapshot
- Output ONLY `cu` commands, one per line
- When done, output: DONE
- If stuck, output: FAIL
"""

def call_llm(model: str, messages: list[dict]) -> tuple[str, int, int]:
    """Call LLM, return (response_text, input_tokens, output_tokens)."""
    if "claude" in model or "opus" in model or "sonnet" in model:
        return _call_anthropic(model, messages)
    else:
        return _call_openai(model, messages)

def _call_anthropic(model: str, messages: list[dict]) -> tuple[str, int, int]:
    import anthropic
    client = anthropic.Anthropic()
    response = client.messages.create(
        model=model,
        max_tokens=4096,
        system=SYSTEM_PROMPT,
        messages=messages,
    )
    text = response.content[0].text
    return text, response.usage.input_tokens, response.usage.output_tokens

def _call_openai(model: str, messages: list[dict]) -> tuple[str, int, int]:
    import openai
    client = openai.OpenAI()
    response = client.chat.completions.create(
        model=model,
        max_completion_tokens=4096,
        messages=[{"role": "system", "content": SYSTEM_PROMPT}] + messages,
    )
    text = response.choices[0].message.content
    return text, response.usage.prompt_tokens, response.usage.completion_tokens

# ── Task execution ───────────────────────────────────────────────────────────

def run_task(task_dict: dict, model: str, max_steps: int = 15, verbose: bool = True) -> dict:
    """Run a single macOSWorld task. Returns result dict."""
    task_id = task_dict["id"]
    task_text = task_dict["task"].get("en", "")
    grading = task_dict.get("grading_command", [])

    if verbose:
        print(f"\n{'='*60}")
        print(f"Task: {task_text}")
        print(f"ID: {task_id}")
        print(f"{'='*60}")

    messages = []
    total_input = 0
    total_output = 0
    steps_taken = 0
    status = "incomplete"

    for step in range(1, max_steps + 1):
        steps_taken = step

        # Observe
        apps = cu_raw(["apps"])
        snapshot = cu_raw(["snapshot", "--limit", "50"])

        user_msg = f"""Task: {task_text}

Step {step}/{max_steps}.

Running apps:
{apps[:500]}

UI Snapshot:
{snapshot[:3000]}

What cu commands should I run next?
- Output cu commands one per line
- After commands execute, you'll see the results, then decide the next step
- Say DONE (with NO commands) only AFTER verifying the task is complete via snapshot
- Say FAIL (with NO commands) only if truly stuck after multiple attempts"""

        messages.append({"role": "user", "content": user_msg})

        # Ask LLM
        try:
            response, inp, out = call_llm(model, messages)
        except Exception as e:
            if verbose:
                print(f"  Step {step}: LLM error: {e}")
            status = "error"
            break

        total_input += inp
        total_output += out
        messages.append({"role": "assistant", "content": response})

        if verbose:
            print(f"  Step {step}: {response[:150]}...")

        # Execute ALL cu commands first, THEN check DONE/FAIL
        results = []
        for line in response.split('\n'):
            line = line.strip().strip('`').strip()
            if line.startswith('cu '):
                cmd_part = line[3:]  # Remove 'cu ' prefix
                if verbose:
                    print(f"    $ cu {cmd_part}")
                r = cu_line(cmd_part)
                results.append(f"$ cu {cmd_part}\n{r[:500]}")
                if verbose:
                    print(f"      → {r[:100]}")
                time.sleep(0.3)

        if results:
            messages.append({"role": "user", "content": "Results:\n" + "\n".join(results)})

        # Only check DONE/FAIL when there were NO commands (pure status response)
        if not results:
            if "DONE" in response:
                status = "done"
                break
            if "FAIL" in response:
                status = "fail"
                break

        time.sleep(0.3)

    # Grade — wait briefly for UI to settle, then evaluate
    grade_pass = False
    if grading:
        time.sleep(1)
        local_user = os.environ.get("USER", "ec2-user")
        for cmd_pair in grading:
            cmd = cmd_pair[0].replace("ec2-user", local_user)
            try:
                result = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=15)
                output = result.stdout.strip()
                if output.lower() == "true":
                    grade_pass = True
                    break
            except Exception:
                pass

    if verbose:
        grade_str = "PASS" if grade_pass else "FAIL"
        print(f"\n  Result: agent={status}, grade={grade_str}, steps={steps_taken}, tokens={total_input+total_output}")

    return {
        "task_id": task_id,
        "task": task_text,
        "status": status,
        "grade": grade_pass,
        "steps": steps_taken,
        "input_tokens": total_input,
        "output_tokens": total_output,
    }

# ── Main ─────────────────────────────────────────────────────────────────────

def main():
    parser = argparse.ArgumentParser(description="Local macOSWorld benchmark runner")
    parser.add_argument("--task", help="Path to a single task JSON")
    parser.add_argument("--tasks-dir", help="Directory of task JSONs")
    parser.add_argument("--model", default="claude-sonnet-4-6", help="LLM model name")
    parser.add_argument("--max-steps", type=int, default=15)
    parser.add_argument("--dry-run", action="store_true", help="List tasks without executing")
    parser.add_argument("--limit", type=int, default=0, help="Max tasks to run (0=all)")
    args = parser.parse_args()

    # Collect task files
    task_files = []
    if args.task:
        task_files = glob(args.task)
    elif args.tasks_dir:
        task_files = sorted(glob(os.path.join(args.tasks_dir, "*.json")))
    else:
        print("Specify --task or --tasks-dir", file=sys.stderr)
        sys.exit(1)

    if args.limit > 0:
        task_files = task_files[:args.limit]

    if args.dry_run:
        for f in task_files:
            d = json.load(open(f))
            print(f"{d['id'][:8]}  {d['task'].get('en', '')[:70]}")
        print(f"\n{len(task_files)} tasks")
        return

    # Run tasks
    results = []
    for f in task_files:
        task_dict = json.load(open(f))
        result = run_task(task_dict, args.model, args.max_steps)
        results.append(result)

    # Summary
    passed = sum(1 for r in results if r["grade"])
    total = len(results)
    total_tokens = sum(r["input_tokens"] + r["output_tokens"] for r in results)

    print(f"\n{'='*60}")
    print(f"BENCHMARK RESULTS: {passed}/{total} passed ({100*passed/total:.1f}%)")
    print(f"Total tokens: {total_tokens:,}")
    print(f"Model: {args.model}")
    print(f"{'='*60}")

    for r in results:
        mark = "PASS" if r["grade"] else "FAIL"
        print(f"  [{mark}] {r['task'][:60]}  (steps={r['steps']}, tokens={r['input_tokens']+r['output_tokens']})")

    # Save results
    out_path = f"test-results/macosworld-{int(time.time())}.json"
    os.makedirs("test-results", exist_ok=True)
    with open(out_path, 'w') as f:
        json.dump({"model": args.model, "passed": passed, "total": total, "results": results}, f, indent=2)
    print(f"\nResults saved: {out_path}")

if __name__ == "__main__":
    main()
