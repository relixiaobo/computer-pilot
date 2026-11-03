#!/bin/bash
# Launch an owned bundle whose filename and display name deliberately differ.
source "$(dirname "$0")/helpers.sh"
section "launch — localized identity, cold/warm readiness, and background delivery"
# Install the disposable bundle under the user app root so Launch Services
# can resolve its filename alias (not reliable for bundles under TMPDIR).
mkdir -p "$HOME/Applications"
FIXTURE_ROOT=$(mktemp -d "$HOME/Applications/cu-launch-identity.XXXXXX")
trap 'rm -rf "$FIXTURE_ROOT"; cleanup_run' EXIT
BUNDLE="$FIXTURE_ROOT/PilotLaunchFixture$$.app"
mkdir -p "$BUNDLE/Contents/MacOS"
swiftc "$ROOT_DIR/tests/fixtures/launch-identity.swift" -o "$BUNDLE/Contents/MacOS/LaunchFixture"

if python3 - "$CU" "$BUNDLE" <<'PY'
import json
import os
from pathlib import Path
import plistlib
import signal
import shutil
import subprocess
import sys
import time

cu, bundle = sys.argv[1], Path(sys.argv[2])
exe = str(bundle / 'Contents/MacOS/LaunchFixture')
bundle_id = 'test.computer-pilot.launch.' + str(os.getpid())
other = bundle.with_name('OtherLaunchFixture.app')
display_name = '启动测试 ' + str(os.getpid())
(bundle / 'Contents/Info.plist').write_bytes(plistlib.dumps({
    'CFBundleExecutable': 'LaunchFixture', 'CFBundleIdentifier': bundle_id,
    'CFBundleName': bundle.stem, 'CFBundleDisplayName': bundle.stem,
    'CFBundleDevelopmentRegion': 'en', 'CFBundleLocalizations': ['en', 'zh-Hans'],
    'CFBundlePackageType': 'APPL',
}))
# A localized InfoPlist is needed for NSRunningApplication.localizedName to
# differ from the filename on every host, including English-language macOS.
for language in ('en', 'zh-Hans', 'Base'):
    folder = bundle / 'Contents/Resources' / (language + '.lproj')
    folder.mkdir(parents=True)
    (folder / 'InfoPlist.strings').write_text(f'"CFBundleDisplayName" = "{display_name}";\n"CFBundleName" = "{display_name}";\n')

register = '/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister'
subprocess.run([register, '-f', str(bundle)], check=True, capture_output=True, timeout=10)

def invoke(*args):
    return subprocess.run([cu, *args], capture_output=True, text=True, timeout=20)

def run(*args):
    result = invoke('--json', *args)
    data = json.loads(result.stdout or result.stderr)
    assert result.returncode == 0 and data.get('ok', True), (args, data)
    return data

def apps():
    return [a for a in run('apps')['apps'] if a.get('bundle_id') == bundle_id]

def observe():
    return json.loads(subprocess.check_output([exe, '--observe'], text=True, timeout=5))

def stop_owned():
    for app in apps():
        os.kill(app['pid'], signal.SIGTERM)
    deadline = time.monotonic() + 5
    while apps() and time.monotonic() < deadline:
        time.sleep(.05)
    assert not apps(), 'Owned app did not terminate'

def ready(selector):
    before = observe()
    data = run('launch', selector, '--timeout', '8')
    matches = apps()
    assert len(matches) == 1 and data['pid'] == matches[0]['pid'], (data, matches)
    assert data['app'] == matches[0]['name'] == display_name, (data, matches)
    assert data['window']['width'] > 0 and data['window']['height'] > 0, data
    assert data['waited'] is True and data['warmup_ms'] >= 0 and data['ready_in_ms'] >= 0, data
    assert run('state', f"pid:{data['pid']}", '--no-screenshot')['window'] == 'Computer Pilot launch identity fixture'
    assert observe() == before, 'Launch changed frontmost PID or pointer'
    return data

try:
    # First open registers the unique bundle with Launch Services.
    first = ready(str(bundle))
    assert ready(bundle.stem)['pid'] == first['pid'], 'Warm launch created a different process'
    stop_owned()
    ready(bundle.stem)  # cold launch by the installed filename, despite different localizedName
    stop_owned()
    ready(bundle_id)
    stop_owned()
    result = run('launch', str(bundle), '--no-wait')
    assert result['waited'] is False and result['ready_in_ms'] == 0, result
    ready(bundle_id)
    human = invoke('--human', 'launch', bundle.stem, '--timeout', '8')
    assert human.returncode == 0 and f'Launched {display_name}' in human.stdout, human
    # A second installation can share the bundle ID. Its delayed startup must
    # not let launch readiness report the first installation's existing window.
    shutil.copytree(bundle, other)
    info = other / 'Contents/Info.plist'
    metadata = plistlib.loads(info.read_bytes())
    metadata['CPLaunchDelay'] = 3.0
    info.write_bytes(plistlib.dumps(metadata))
    mismatch = invoke('--json', 'launch', str(other), '--timeout', '1')
    assert mismatch.returncode != 0, ('launch accepted the wrong installation', mismatch)
    error = json.loads(mismatch.stderr)
    assert error['code'] in ('ambiguous_target', 'command_failed', 'app_not_found'), error
    assert error['code'] == 'ambiguous_target' or 'timed out' in error['error'], error
    missing = invoke('--json', 'launch', 'NonExistentApp987654', '--timeout', '1')
    assert missing.returncode != 0 and json.loads(missing.stderr)['ok'] is False, missing
finally:
    # Also clean a delayed process when an assertion fails immediately after
    # open. A failure must not leave another owned app or LS registration behind.
    if other.exists():
        time.sleep(3.5)
    try:
        stop_owned()
    finally:
        for path in (other, bundle):
            if path.exists():
                subprocess.run([register, '-u', str(path)], check=True, capture_output=True, timeout=10)
PY
then
  _pass "name/path/bundle launches return the exact localized process and owned window without foreground changes"
else
  _fail "launch identity behavior" "see exact process/window assertions above"
fi
summary
