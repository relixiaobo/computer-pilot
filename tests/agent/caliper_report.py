#!/usr/bin/env python3
"""Translate run.py agent results into CaliperRecord JSON for `caliper score`.

Usage:
  # 1. Run agent tests (unchanged)
  python tests/agent/run.py

  # 2. Convert latest results to CaliperRecord format
  python tests/agent/caliper_report.py

  # 3. Score with caliper
  cd ~/Documents/Coding/caliper && uv run caliper score \
      ~/Documents/Coding/computer-pilot/tests/agent/caliper_records.json
"""

import json
import sys
from glob import glob
from pathlib import Path

TASKS_DIR = Path(__file__).parent / "tasks"
OUTPUT = Path(__file__).parent / "caliper_records.json"


def load_task_goals() -> dict[str, str]:
    """Load goal text from task JSON files, keyed by task_id."""
    goals = {}
    for f in TASKS_DIR.glob("*.json"):
        task = json.loads(f.read_text())
        goals[task["id"]] = task["goal"]
    return goals


def find_latest_results() -> Path | None:
    """Find the most recent agent-*.json in test-results/."""
    results_dir = Path(__file__).resolve().parent.parent.parent / "test-results"
    files = sorted(results_dir.glob("agent-*.json"))
    return files[-1] if files else None


def convert(results_path: Path) -> list[dict]:
    """Convert run.py output to CaliperRecord dicts."""
    data = json.loads(results_path.read_text())
    goals = load_task_goals()
    model = data.get("model", "")
    records = []

    for r in data["results"]:
        task_id = r["task_id"]
        records.append({
            "sample_id": task_id,
            "bucket": "agent-e2e",
            "goal": goals.get(task_id, r.get("task_name", task_id)),
            "agent_answer": "DONE" if r.get("agent_status") == "done" else "",
            "observed": True,  # cu agent always snapshots before acting
            "verify_results": [
                {"passed": c["passed"], "description": c["check"]}
                for c in r.get("checks", [])
            ],
            "input_tokens": r.get("input_tokens", 0),
            "output_tokens": r.get("output_tokens", 0),
            "has_cache_info": True,
            "commands_run": r.get("steps", 0),
            "project": "computer-pilot",
            "model": model,
        })

    return records


def main():
    path = find_latest_results()
    if not path:
        print("No agent results found in test-results/agent-*.json", file=sys.stderr)
        sys.exit(1)

    print(f"Converting: {path.name}")
    records = convert(path)
    OUTPUT.write_text(json.dumps(records, indent=2, ensure_ascii=False) + "\n")
    print(f"Wrote {len(records)} records to {OUTPUT}")
    print(f"\nNext: cd ~/Documents/Coding/caliper && uv run caliper score {OUTPUT}")


if __name__ == "__main__":
    main()
