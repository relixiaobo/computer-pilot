#!/usr/bin/env python3
"""Fresh-task native UI comparison: triage, document editing, and async wizard.

Coordinator-only fixture lifecycle, independent verification, and rollout metrics.
Each trial gets one new Codex task and one new app/Broker/output directory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import signal
import subprocess
import tempfile
import uuid

from collect_codex_benchmark import collect_turns, summarize

ROOT = Path(__file__).resolve().parents[2]
CU = ROOT / 'target/release/cu'
MODES = ('triage', 'document', 'wizard')


def source(mode, seed):
    if mode == 'triage':
        items = [dict(ID=f'T{seed}{i+1:02}', Customer=f'客户 {seed}-{i+1}',
                      Status='Closed' if i in (1, 5) else 'Open',
                      Severity=['Critical', 'High', 'Normal', 'Low'][(i + seed) % 4],
                      Plan='Business' if (i + seed) % 3 else 'Personal',
                      Region='East' if (i + seed) % 2 else 'West',
                      **{'Initial urgent': str(i % 2 == 0).lower()}) for i in range(8)]
    elif mode == 'document':
        items = [dict(ID=f'S{seed}{i+1:02}', Title=f'章节 {seed}-{i+1} · {title}',
                      Text=f'草案 {seed}-{i+1}：{title}需要核对中文、标点和数量，保留引用“样本 {seed*10+i}”。\n第二段：本草案仅用于本地测试；Reference {seed}-{i+1:02}, keep line breaks.')
                 for i, title in enumerate(['目标', '范围', '交付', '质量', '风险', '结论'])]
    else:
        items = [dict(ID=f'P{seed}{i+1:02}', Title=f'服务 {seed}-{i+1}',
                      Environment='Production' if (i + seed) % 2 else 'Sandbox',
                      Route='Proxy' if (i + seed) % 2 else 'Direct',
                      Host=f'node-{seed}-{i+1}.example.test',
                      Proxy=f'proxy-{seed}-{i+1}.example.test:8080',
                      Limit=str(100*(i+1) + seed*17), Tag=f'release-{seed}-区{i+1}') for i in range(4)]
    return dict(mode=mode, items=items)


def expected(spec):
    mode, items = spec['mode'], spec['items']
    initial = {}
    for item in items:
        if mode == 'triage':
            if item['Status'] != 'Open':
                continue
            urgent = item['Severity'] == 'Critical' or (item['Severity'] == 'High' and item['Plan'] == 'Business')
            sla = '4' if item['Severity'] == 'Critical' else '8' if urgent else '24'
            queue = item['Region'] + ' desk'
            record = {'Queue': queue, 'SLA hours': sla, 'Urgent': str(urgent).lower(),
                      'Note': f'{item["ID"]} / {queue} / {sla}h'}
        elif mode == 'document':
            record = dict(Heading=f'{item["ID"]} · {item["Title"]}',
                          Body=item['Text'].replace('草案', '定稿') + f'\nReviewed: {item["ID"]}',
                          Reviewed='true')
        else:
            production = item['Environment'] == 'Production'
            record = {'Host': item['Host'], 'Route': item['Route'],
                      'Port': '443' if production else '80', 'TLS': str(production).lower(),
                      'Retention days': '30' if production else '7', 'Audit': str(production).lower(),
                      'Limit': item['Limit'], 'Tag': item['Tag']}
            if item['Route'] == 'Proxy':
                record['Proxy server'] = item['Proxy']
        initial[item['ID']] = record
    final = {key: dict(value) for key, value in initial.items()}
    order = [item['ID'] for item in items]
    if mode == 'triage':
        key = items[2]['ID']; final[key]['SLA hours'] = '2'
        final[key]['Note'] = f'{key} / {final[key]["Queue"]} / 2h'
    elif mode == 'document':
        final[items[2]['ID']]['Heading'] += '（复核）'
        order = [order[5], order[0], order[2], order[1], order[3], order[4]]
    else:
        final[items[1]['ID']]['Retention days'] = '14'
    return initial, final, order


def verify(spec, saved):
    actual = json.loads(saved.read_text()) if saved.exists() else {}
    initial, final, order = expected(spec)
    events = actual.get('events', [])
    first = {}
    for e in events:
        if e['action'] == ('policy-saved' if spec['mode'] == 'wizard' else 'save'):
            first.setdefault(e['ID'], {k: v for k, v in e.items() if k not in ('action', 'ID')})
    checks = dict(final_fields_exact=actual.get('records') == final,
                  initial_versions_exact=first == initial,
                  order_exact=actual.get('order') == order,
                  completed=actual.get('completed') is True)
    if spec['mode'] == 'triage':
        checks['filter_and_search_used'] = any(e['action'] == 'filter-open' for e in events) and any(
            e['action'] == 'search' and e.get('query') == spec['items'][2]['ID'] for e in events)
    elif spec['mode'] == 'wizard':
        checks['review_correction_used_back'] = any(e['action'] == 'back-policy' and e['ID'] == spec['items'][1]['ID'] for e in events)
        checks['all_applies_confirmed'] = {e['ID'] for e in events if e['action'] == 'apply-confirmed'} == set(final)
    return dict(passed=all(checks.values()), checks=checks, expected=final, actual=actual)


def goal(spec):
    mode, items = spec['mode'], spec['items']
    if mode == 'triage':
        return f'''Filter to Open tickets, then process all six Open tickets across the paginated list. Leave Closed tickets untouched.
For each ticket read its live source facts. Queue = East desk for East, otherwise West desk. Urgent on iff Severity is Critical OR (High AND Plan is Business). SLA hours = 4 for Critical, 8 for High Business, otherwise 24. Note must be exactly "ID / Queue / SLAh", substituting its values (for example T000 / East desk / 24h). Save each original assignment.
After all originals, use Search ID to find {items[2]['ID']}, change SLA hours to 2 and update Note to match; preserve Queue and Urgent. Save, choose Finish triage, confirm the local write, and observe "Workflow complete: triage".'''
    if mode == 'document':
        return f'''Edit all six sections. Read Source heading and Source text from each live section UI. Heading = "ID · Source heading". Body = the full Source text with every 草案 replaced by 定稿, preserving punctuation and line breaks, followed by a new line "Reviewed: ID". Turn Reviewed on, and save every original section.
After all originals: move {items[5]['ID']} to position 1, then move {items[1]['ID']} to position 4. Reopen {items[2]['ID']}, append （复核） to its Heading, preserving Body and Reviewed, and save. Finish document, confirm the local write, and observe "Workflow complete: document".'''
    return f'''Configure all four deployment profiles through Network, Policy, Review, and the confirmation sheet. Read all values from the live UI.
Network: Host exactly from Source Host; select Source Route in the Route dropdown; for Proxy, fill Proxy server from Source Proxy. TLS on only for Production; Port 443 for Production and 80 for Sandbox.
Policy: Retention days 30 for Production and 7 for Sandbox; Audit on only for Production; copy Source Limit and Source Tag exactly.
For {items[1]['ID']}, first reach Review with those standard values, then use Back: policy and change Retention days to 14 before returning to Review. Leave its other fields unchanged.
Apply and confirm each profile; saves complete asynchronously, so wait for the actual list before choosing another profile. Finish wizard, confirm, and observe "Workflow complete: wizard".'''


def prepare(root):
    root.mkdir(parents=True, exist_ok=False)
    executable = root / 'VariedWorkflow'
    subprocess.run(['swiftc', str(ROOT / 'tests/fixtures/varied-workflows.swift'), '-o', str(executable)], check=True)
    sequence = []
    for mode_index, mode in enumerate(MODES):
        for repeat in (1, 2):
            arms = ('native', 'cu') if (mode_index + repeat) % 2 else ('cu', 'native')
            sequence.extend(dict(label=f'{mode}-{repeat}-{arm}', mode=mode, repeat=repeat, arm=arm) for arm in arms)
    manifest = dict(sequence=sequence, cu_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                    cu_sha256=hashlib.sha256(CU.read_bytes()).hexdigest(),
                    fixture_sha256=hashlib.sha256((ROOT / 'tests/fixtures/varied-workflows.swift').read_bytes()).hexdigest())
    (root / 'manifest.json').write_text(json.dumps(manifest, indent=2))
    print(json.dumps(manifest))


def start(root, label):
    entry = next(e for e in json.loads((root / 'manifest.json').read_text())['sequence'] if e['label'] == label)
    trial = root / label; trial.mkdir()
    bundle = trial / 'VariedWorkflow.app/Contents'; (bundle / 'MacOS').mkdir(parents=True)
    executable = bundle / 'MacOS/VariedWorkflow'; shutil.copy2(root / 'VariedWorkflow', executable)
    (bundle / 'Info.plist').write_bytes(plistlib.dumps(dict(CFBundleExecutable='VariedWorkflow',
        CFBundleIdentifier='test.computer-pilot.varied.' + label + '.' + uuid.uuid4().hex,
        CFBundleName='VariedWorkflow', CFBundlePackageType='APPL', LSUIElement=True)))
    spec = source(entry['mode'], entry['repeat'])
    (trial / 'source.json').write_text(json.dumps(spec, ensure_ascii=False))
    saved = trial / 'saved.json'
    app = subprocess.Popen([str(executable), str(trial / 'source.json'), str(saved)],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    runtime = tempfile.mkdtemp(prefix='cu-varied-') if entry['arm'] == 'cu' else None
    meta = dict(entry, pid=app.pid, app_path=str(bundle.parent), runtime_home=runtime,
                source_sha256=hashlib.sha256((trial / 'source.json').read_bytes()).hexdigest(),
                client_key=label, output_dir=str(trial / 'output'))
    (trial / 'meta.json').write_text(json.dumps(meta, indent=2))
    common = f'''You are a participant in a user-authorized desktop benchmark. Use only the supplied disposable app. Do not inspect other tasks, source/expected/saved files, benchmark scripts or prior results. Do not invoke another model or change model/runtime settings. Do not alter repository files. Task-owned shell configuration may be created in your trial directory. No real user data or external messages are involved.

Workflow for the measured GO turn:
{goal(spec)}

Read source values from live UI, not files or memory from another trial. Normal efficient grouping and generated code using the allowed tools are permitted, but prewritten solvers are not. Do not add artificial sleeps. If actually blocked, report the blocker instead of claiming completion. Reply DONE only after observing workflow completion. The coordinator independently verifies every saved value and required original/correction states.

This is a fresh task, app process and state directory. Initialization is reported separately and combined with execution; the complete GO turn is timed. Do only the untimed setup now and wait for GO without UI mutations.
'''
    if entry['arm'] == 'cu':
        common += f'''Your arm: use only the existing shell and {CU} for app reads/actions. Read plugin/skills/computer-pilot/SKILL.md and initialize its multi-round task configuration. CU_APP=pid:{app.pid}; COMPUTER_PILOT_CLIENT_KEY={label}; COMPUTER_PILOT_HOME={runtime}; COMPUTER_PILOT_OUTPUT_DIR={trial / 'output'}. Computer Pilot and permissions are already verified. Take one observation of this PID. Do not use another automation backend. Reply READY.'''
    else:
        common += f'''Your arm: use only actual mcp__cua_repl native App APIs. This explicitly authorizes that API instead of the repository cu integration. Bind {bundle.parent} using cua.getApp, read its documentation and take the initial observation. Do not use cu or another backend. Reply READY.'''
    (trial / 'brief.txt').write_text(common)
    print(json.dumps(dict(meta, brief=str(trial / 'brief.txt'))))


def collect(root, label, rollout_dir):
    trial = root / label; meta = json.loads((trial / 'meta.json').read_text())
    tid = (trial / 'thread.txt').read_text().strip()
    paths = list(rollout_dir.rglob(f'*{tid}.jsonl'))
    if len(paths) != 1: raise RuntimeError(f'Expected one rollout for {tid}')
    result = dict(meta, thread_id=tid, setup=[], runs=[])
    for turn in collect_turns(paths[0], meta['arm'], tid):
        if 'elapsed_ms' not in turn: continue
        summarize(turn)
        if 'label' not in turn:
            result['setup'].append({k: v for k, v in turn.items() if k != 'tools'})
        else:
            (trial / 'trace.json').write_text(json.dumps(turn, ensure_ascii=False, indent=2))
            result['runs'].append({k: v for k, v in turn.items() if k != 'tools'})
    result['verification'] = verify(source(meta['mode'], meta['repeat']), trial / 'saved.json')
    result['setup_ms'] = sum(t['elapsed_ms'] for t in result['setup'])
    result['execution_ms'] = sum(t['elapsed_ms'] for t in result['runs'])
    result['combined_turn_ms'] = result['setup_ms'] + result['execution_ms']
    (trial / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2))
    print(json.dumps(dict(label=label, setup_ms=result['setup_ms'], execution_ms=result['execution_ms'],
                         combined_turn_ms=result['combined_turn_ms'], verified=result['verification']['passed'],
                         checks=result['verification']['checks'],
                         runs=[{k: r[k] for k in ('model', 'reasoning_effort', 'model_calls', 'tool_calls', 'tool_errors', 'tool_output_chars')} for r in result['runs']])))


def stop(root, label):
    trial = root / label; path = trial / 'meta.json'; meta = json.loads(path.read_text())
    if meta['pid']:
        try: os.kill(meta['pid'], signal.SIGTERM)
        except ProcessLookupError: pass
        meta['pid'] = None
    if meta['runtime_home']:
        env = dict(os.environ, COMPUTER_PILOT_HOME=meta['runtime_home'], COMPUTER_PILOT_CLIENT_KEY=meta['client_key'])
        p = subprocess.run([str(CU), '--json', 'status'], env=env, capture_output=True, text=True)
        if p.returncode == 0:
            status = json.loads(p.stdout)
            if status.get('running'): os.kill(status['pid'], signal.SIGTERM)
    path.write_text(json.dumps(meta, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('action', choices=['prepare', 'start', 'verify', 'collect', 'stop'])
    parser.add_argument('label', nargs='?')
    parser.add_argument('--rollouts', type=Path, default=Path.home() / '.codex/sessions')
    args = parser.parse_args(); root = args.root.resolve()
    if args.action == 'prepare': prepare(root)
    elif args.action == 'start': start(root, args.label)
    elif args.action == 'stop': stop(root, args.label)
    elif args.action == 'collect': collect(root, args.label, args.rollouts)
    else:
        meta = json.loads((root / args.label / 'meta.json').read_text())
        result = verify(source(meta['mode'], meta['repeat']), root / args.label / 'saved.json')
        print(json.dumps(result, ensure_ascii=False))
        raise SystemExit(0 if result['passed'] else 1)

if __name__ == '__main__':
    main()
