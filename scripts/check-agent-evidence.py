#!/usr/bin/env python3
"""Check reviewed external L2 evidence against the pre-version-bump tree.

This checks completeness and freshness, not authenticity of a model execution.
Maintainers review the recorded run and explicitly select it at release time.
"""
import hashlib
import json
from pathlib import Path
import sys


def source_paths(root):
    paths = {root / 'Cargo.toml', root / 'Cargo.lock', root / 'tests/agent/run.py'}
    for directory in ('src', 'plugin/skills/computer-pilot', 'tests/agent/tasks/e2e'):
        paths.update(p for p in (root / directory).rglob('*') if p.is_file())
    if (root / 'build.rs').exists():
        paths.add(root / 'build.rs')
    return sorted(p.relative_to(root).as_posix() for p in paths)


def validate(report, root):
    root = root.resolve()
    report = report.resolve()
    report.relative_to(root)  # A release cites evidence inside its repository.
    data = json.loads(report.read_text())
    tasks = [json.loads(p.read_text()) for p in sorted((root / 'tests/agent/tasks/e2e').glob('*.json'))]
    expected = {task['id']: task for task in tasks}
    results = data['results']
    if not tasks or len(expected) != len(tasks) or len(results) != len(tasks):
        raise ValueError('evidence must cover every release task exactly once')
    if data['total'] != len(tasks) or data['passed'] != len(tasks):
        raise ValueError('not all release tasks passed')
    seen = set()
    models = set()
    for result in results:
        task_id = result['task_id']
        if task_id not in expected or task_id in seen:
            raise ValueError('unknown or duplicate task: ' + str(task_id))
        seen.add(task_id)
        if result['agent_status'] != 'done' or result['verified'] is not True:
            raise ValueError('task did not complete and verify: ' + task_id)
        if result['cleanup_errors'] != []:
            raise ValueError('cleanup did not pass: ' + task_id)
        checks = result['checks']
        definitions = expected[task_id]['verify']
        if not definitions or len(checks) != len(definitions):
            raise ValueError('missing verification checks: ' + task_id)
        for check, definition in zip(checks, definitions):
            if (check['passed'] is not True or check.get('skipped', False) is not False
                    or not check['check'].startswith(definition['description'])):
                raise ValueError('failed, skipped or mismatched check: ' + task_id)
        model = result['model']
        if not isinstance(model, str) or not model.strip():
            raise ValueError('missing model: ' + task_id)
        models.add(model)
    execution = data['execution']
    if not isinstance(execution, str) or not execution.strip():
        raise ValueError('missing execution route')
    recorded = data['source_sha256']
    required = source_paths(root)
    if set(recorded) != set(required):
        raise ValueError('source inventory differs; refresh evidence after source/task additions or removals')
    for path in required:
        if hashlib.sha256((root / path).read_bytes()).hexdigest() != recorded[path]:
            raise ValueError('evidence is stale: ' + path)
    return f"{len(tasks)}/{len(tasks)} tasks; models: {', '.join(sorted(models))}; route: {execution}"


def main():
    try:
        if len(sys.argv) != 2:
            raise ValueError('usage: check-agent-evidence.py <repository-report.json>')
        print('Accepted reviewed external L2 evidence: ' + validate(Path(sys.argv[1]), Path(__file__).resolve().parent.parent))
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as exc:
        print('Invalid L2 evidence: ' + str(exc), file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
