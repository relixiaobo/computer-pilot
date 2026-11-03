#!/bin/bash
# Exercise native activation after AX accepts focus without activating the app.
source "$(dirname "$0")/helpers.sh"
section "focus activation — fresh foreground evidence"

if ! interactive_tests_enabled; then
  _skip "exact-PID activation between owned windows" "requires a dedicated foreground test interval"
  summary
  exit 0
fi

FIXTURE_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/cu-focus-activation.XXXXXX")
trap 'rm -rf "$FIXTURE_ROOT"; cleanup_run' EXIT
BUNDLE="$FIXTURE_ROOT/FocusFixture.app/Contents"
mkdir -p "$BUNDLE/MacOS"
swiftc "$ROOT_DIR/tests/fixtures/focus-activation.swift" -o "$BUNDLE/MacOS/FocusFixture"

if python3 - "$CU" "$BUNDLE" <<'PY'
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import time

cu, bundle = sys.argv[1], Path(sys.argv[2])
exe = str(bundle / 'MacOS/FocusFixture')
(bundle / 'Info.plist').write_bytes(plistlib.dumps({
    'CFBundleExecutable': 'FocusFixture',
    'CFBundleIdentifier': 'test.computer-pilot.focus',
    'CFBundleName': 'FocusFixture',
    'CFBundlePackageType': 'APPL',
}))

def observe(*args):
    return json.loads(subprocess.check_output([exe, *args], text=True, timeout=5))

def run(*args):
    result = subprocess.run([cu, '--json', *args], capture_output=True, text=True, timeout=10)
    value = json.loads(result.stdout or result.stderr)
    assert result.returncode == 0 and value.get('ok') is True, value
    return value

before = observe('--frontmost')
fixtures = []
try:
    for label in ('A', 'B'):
        process = subprocess.Popen([exe], env=dict(os.environ, FOCUS_FIXTURE_TITLE='CP Focus ' + label),
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        fixtures.append(process)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            assert process.poll() is None, 'Fixture exited during setup'
            try:
                windows = run('window', 'list', '--app', f'pid:{process.pid}')['windows']
                if len(windows) == 1 and windows[0]['title'] == 'CP Focus ' + label:
                    break
            except AssertionError:
                pass
            time.sleep(.1)
        else:
            raise AssertionError('Fixture did not expose its owned window')

    # Both processes share a name and bundle identity; every focus must select
    # the requested PID. A fresh observer, not cu's own receipt, checks effect.
    for process in fixtures * 3:
        target = f'pid:{process.pid}'
        value = run('window', 'focus', '--app', target)
        actual = observe('--frontmost')
        assert value['frontmost_pid'] == actual['pid'] == process.pid, (value, actual)
        assert run('state', target, '--no-screenshot')['frontmost'] is True
        assert (actual['x'], actual['y']) == (before['x'], before['y']), 'Pointer moved'

    # An invalid window must fail without activating another process.
    failed = subprocess.run([cu, '--json', 'window', 'focus', '--app', f'pid:{fixtures[0].pid}',
                             '--window', '999'], capture_output=True, text=True, timeout=10)
    assert failed.returncode != 0
    error = json.loads(failed.stderr)
    assert error.get('ok') is False and error.get('code') == 'window_not_found', error
    assert observe('--frontmost')['pid'] == fixtures[-1].pid
finally:
    for process in fixtures:
        process.terminate()
        process.wait(timeout=5)
    if before['pid']:
        restored = observe('--restore', str(before['pid']))
        assert restored['pid'] == before['pid'], ('Foreground restoration failed', restored)
PY
then
  _pass "native activation switches exact PIDs, refreshes evidence and preserves pointer"
else
  _fail "native activation behavior" "see independent foreground assertions above"
fi
summary
