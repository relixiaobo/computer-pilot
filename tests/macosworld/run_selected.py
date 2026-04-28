#!/usr/bin/env python3
"""Run selected macOSWorld tasks with multiple models."""

import json, os, sys, time
from pathlib import Path

# Load .env
env_path = Path(__file__).resolve().parent.parent.parent / ".env"
if env_path.exists():
    for line in env_path.read_text().splitlines():
        line = line.strip()
        if line and not line.startswith("#") and "=" in line:
            k, _, v = line.partition("=")
            os.environ.setdefault(k.strip(), v.strip())

sys.path.insert(0, str(Path(__file__).parent))
from run_benchmark import run_task

TASK_LIST = os.environ.get("TASK_LIST", "/tmp/macosworld_final.txt")
# Comma-separated. Default is the v0.4.0 baseline pair; override per-run via:
#   MODELS=claude-sonnet-4-6 python tests/macosworld/run_selected.py
MODELS = [m.strip() for m in os.environ.get(
    "MODELS", "claude-sonnet-4-6,gpt-5.4"
).split(",") if m.strip()]

with open(TASK_LIST) as f:
    task_files = [l.strip() for l in f if l.strip()]

all_results = {}

for model in MODELS:
    print(f'\n{"="*60}')
    print(f'Model: {model}')
    print(f'{"="*60}')

    results = []
    for i, tf in enumerate(task_files):
        task = json.load(open(tf))
        task_text = task['task'].get('en', '')
        print(f'\n[{i+1}/{len(task_files)}] {task_text[:70]}')

        try:
            r = run_task(task, model, max_steps=12, verbose=False)
            mark = 'PASS' if r['grade'] else 'FAIL'
            print(f'  [{mark}] steps={r["steps"]}, status={r["status"]}')
            results.append(r)
        except Exception as e:
            print(f'  [ERROR] {e}')
            results.append({
                'task_id': task.get('id','?'), 'task': task_text[:60],
                'grade': False, 'status': 'error', 'steps': 0,
                'input_tokens': 0, 'output_tokens': 0
            })
        time.sleep(2)

    passed = sum(1 for r in results if r.get('grade'))
    total = len(results)
    tokens = sum(r.get('input_tokens',0)+r.get('output_tokens',0) for r in results)

    print(f'\n--- {model}: {passed}/{total} passed, {tokens:,} tokens ---')
    all_results[model] = {'passed': passed, 'total': total, 'tokens': tokens, 'results': results}

    os.makedirs('test-results', exist_ok=True)
    tag = model.replace('.','-')
    with open(f'test-results/macosworld-{tag}-{int(time.time())}.json', 'w') as f:
        json.dump({'model': model, 'passed': passed, 'total': total, 'results': results}, f, indent=2, ensure_ascii=False)

# Final comparison
print(f'\n{"="*60}')
print(f'COMPARISON')
print(f'{"="*60}')
for model, data in all_results.items():
    print(f'  {model}: {data["passed"]}/{data["total"]} passed, {data["tokens"]:,} tokens')
