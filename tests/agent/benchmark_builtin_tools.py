#!/usr/bin/env python3
"""Owned fixture lifecycle and cu arm of the built-in tool comparison.

The native arm runs builtin_order_workflow.js in Codex's actual cua_repl.
Use a fresh session path: SESSION init; SESSION start LABEL; SESSION cu/verify;
SESSION stop. No model calls are made by this deterministic benchmark.
"""
import argparse
import importlib.util
import json
import os
import signal
import subprocess
import tempfile
import time
import shutil
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('bench', ROOT / 'tests/agent/benchmark_long_workflow.py')
b = importlib.util.module_from_spec(spec)
spec.loader.exec_module(b)
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('session', type=Path)
parser.add_argument('action', choices=['init', 'start', 'cu', 'verify', 'stop'])
parser.add_argument('label', nargs='?')
args = parser.parse_args()
mp = args.session.resolve()
if args.action == 'init':
    root = mp.parent
    root.mkdir(parents=True, exist_ok=True)
    exe, source = b.prepare_fixture(root)
    meta = dict(root=str(root), app_path=str(exe.parents[2]), executable=str(exe), source=str(source), pid=None)
    mp.write_text(json.dumps(meta, indent=2))
    print(json.dumps(meta))
    raise SystemExit()
meta = json.loads(mp.read_text())
root = Path(meta['root'])
if args.action == 'start':
    if meta.get('pid'):
        try:
            os.kill(meta['pid'], signal.SIGTERM)
        except ProcessLookupError:
            pass
    label = args.label
    if not label:
        parser.error('start requires a label')
    saved = root / (label + '-saved.json')
    p = subprocess.Popen([meta['executable'], meta['source'], str(saved)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    meta.update(pid=p.pid, label=label, saved=str(saved))
    mp.write_text(json.dumps(meta))
    print(json.dumps(meta))
    raise SystemExit()
if args.action == 'verify':
    result = b.verify_saved(Path(meta['saved']))
    (root / (meta['label'] + '-verification.json')).write_text(json.dumps(result, ensure_ascii=False, indent=2))
    print(json.dumps({'label': meta['label'], 'verified': result['passed'], 'checks': result['checks']}))
    raise SystemExit(0 if result['passed'] else 1)
if args.action == 'stop':
    os.kill(meta['pid'], signal.SIGTERM)
    meta['pid'] = None
    mp.write_text(json.dumps(meta))
    raise SystemExit()
if args.action != 'cu':
    raise ValueError(args.action)
env = dict(os.environ, COMPUTER_PILOT_HOME=tempfile.mkdtemp(prefix='cu-builtin-ab-'), COMPUTER_PILOT_CLIENT_KEY='builtin-ab', COMPUTER_PILOT_VISUAL_CURSOR='0')
cu = str(ROOT / 'target/release/cu')
app = f"pid:{meta['pid']}"
trace = []

def call(*args):
    t = time.perf_counter()
    p = subprocess.run([cu, '--json', *args], env=env, capture_output=True, text=True, timeout=20)
    elapsed = (time.perf_counter() - t) * 1000
    d = json.loads(p.stdout or p.stderr)
    trace.append(dict(command=args[0], args=args, ms=elapsed, ok=p.returncode == 0, output=d))
    if p.returncode:
        raise RuntimeError(d)
    return d

def snapshot():
    return call('snapshot', app, '--limit', '200')

def element(s, title, role=None):
    return next((e for e in s['elements'] if e.get('title') == title and (not role or e['role'] == role)))

def write(s, name, value):
    call('set-value', str(element(s, name, 'textfield')['ref']), value, '--app', app, '--observation', s['observation_id'], '--no-snapshot')

def click(s, title):
    call('click', str(element(s, title)['ref']), '--app', app, '--observation', s['observation_id'], '--no-snapshot', '--no-verify')
try:
    # Fixture readiness and the matching native binding/read are outside timing.
    for attempt in range(40):
        try:
            snapshot()
            break
        except RuntimeError as error:
            if error.args[0].get('code') != 'app_not_found' or attempt == 39:
                raise
            time.sleep(0.05)
    trace.clear()
    start = time.perf_counter()
    s = snapshot()
    for i in range(6):
        values = {}
        for e in s['elements']:
            text = e.get('value') or e.get('title') or ''
            if text.startswith('Source '):
                key, value = text.removeprefix('Source ').split(': ', 1)
                values[key] = value
        for key in ['Customer', 'SKU', 'Quantity', 'City', 'Memo']:
            write(s, key, values[key])
        if int(values['Quantity']) >= 10:
            click(s, 'Priority')
        s = snapshot()
        click(s, 'Save and next')
        s = snapshot()
    click(s, 'Edit order 2')
    s = snapshot()
    write(s, 'City', '杭州')
    s = snapshot()
    click(s, 'Save correction')
    s = snapshot()
    click(s, 'Edit order 5')
    s = snapshot()
    write(s, 'Quantity', '14')
    click(s, 'Priority')
    s = snapshot()
    click(s, 'Save correction')
    s = snapshot()
    click(s, 'Finish archive')
    s = snapshot()
    elapsed = (time.perf_counter() - start) * 1000
    groups = {}
    for t in trace:
        g = groups.setdefault(t['command'], {'count': 0, 'ms': 0})
        g['count'] += 1
        g['ms'] += t['ms']
    result = {'label': meta['label'], 'elapsed_ms': elapsed, 'api_calls': len(trace), 'groups': groups, 'verification': b.verify_saved(Path(meta['saved'])), 'trace': trace}
    (root / (meta['label'] + '-result.json')).write_text(json.dumps(result, ensure_ascii=False, indent=2))
    print(json.dumps({k: v for k, v in result.items() if k not in ('trace', 'verification')}))
    print(json.dumps(result['verification']['checks']))
    if not result['verification']['passed']:
        raise SystemExit(1)
finally:
    p = subprocess.run([cu, '--json', 'status'], env=env, capture_output=True, text=True)
    if p.returncode == 0:
        pid = json.loads(p.stdout).get('pid')
        if pid:
            os.kill(pid, signal.SIGTERM)
    shutil.rmtree(env['COMPUTER_PILOT_HOME'], ignore_errors=True)
