#!/bin/bash
# Exercise queued AX notifications and mixed observation/action limits in an
# owned background app, without touching an existing TextEdit session.
source "$(dirname "$0")/helpers.sh"
section "action latency — synchronous notification, fresh state, and typing"

FIXTURE_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/cu-action-latency.XXXXXX")
trap 'rm -rf "$FIXTURE_ROOT"; cleanup_run' EXIT
BUNDLE="$FIXTURE_ROOT/LatencyFixture.app/Contents"
mkdir -p "$BUNDLE/MacOS"
swiftc "$ROOT_DIR/tests/fixtures/action-latency.swift" -o "$BUNDLE/MacOS/LatencyFixture"

if python3 - "$CU" "$BUNDLE" <<'PY'
import json
import pathlib
import plistlib
import subprocess
import sys
import time

cu, bundle = sys.argv[1], pathlib.Path(sys.argv[2])
(bundle / "Info.plist").write_bytes(plistlib.dumps({
    "CFBundleExecutable": "LatencyFixture",
    "CFBundleIdentifier": "test.computer-pilot.latency",
    "CFBundleName": "LatencyFixture",
    "CFBundlePackageType": "APPL",
    "LSUIElement": True,
}))

def run(*args, ok=True):
    p = subprocess.run([cu, "--json", "--client-key", "latency-fixture", *args],
                       capture_output=True, text=True, timeout=15)
    d = json.loads(p.stdout or p.stderr)
    if ok:
        assert p.returncode == 0 and d.get("ok"), d
    else:
        assert p.returncode != 0 and d.get("ok") is False, d
    return d

fixture = subprocess.Popen([str(bundle / "MacOS/LatencyFixture")],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    app = f"pid:{fixture.pid}"
    for attempt in range(40):
        try:
            snap = run("snapshot", app, "--limit", "1")
            if snap.get("elements"):
                break
        except AssertionError:
            pass
        time.sleep(0.1)
    else:
        raise AssertionError("background fixture did not expose its input")
    field = next(e for e in snap["elements"] if e["role"] == "textfield")
    ref = str(field["ref"])
    path = field["axPath"]

    # The fixture changes its value and posts AXValueChanged during the AX
    # setter. A subscription created after dispatch misses this notification.
    # The action also requests a larger tree than its original Observation.
    changed = run("set-value", ref, "queued-notification", "--app", app,
                  "--observation", snap["observation_id"], "--limit", "50")
    assert changed["snapshot"]["focused"]["value"] == "queued-notification", changed
    assert changed["settle_ms"] < 250, changed

    # A smaller action limit must not make freshness checks compare a short
    # prefix against the hash of the original, larger observation.
    snap = run("snapshot", app, "--limit", "50")
    changed = run("set-value", ref, "smaller-limit", "--app", app,
                  "--observation", snap["observation_id"], "--limit", "1")
    assert len(changed["snapshot"]["elements"]) == 1, changed
    assert changed["snapshot"]["focused"]["value"] == "smaller-limit", changed

    # Reusing the earlier Observation must still fail before changing the field,
    # including the opt-out path which skips only effect verification.
    stale = run("click", ref, "--app", app, "--observation", snap["observation_id"],
                "--no-snapshot", "--no-verify", ok=False)
    assert stale["code"] == "stale_observation", stale
    snap = run("snapshot", app)
    run("click", ref, "--app", app, "--observation", snap["observation_id"],
        "--no-snapshot", "--no-verify")

    # Exercise the single final post-type snapshot with real Unicode events.
    run("set-value", ref, "", "--ax-path", path, "--app", app)
    typed = run("type", "native中文😀", "--app", app, "--no-paste")
    assert typed["effect_verified"] is True, typed
    assert typed["snapshot"]["focused"]["value"] == "native中文😀", typed
    observed = run("snapshot", app)
    assert observed["focused"]["value"] == "native中文😀", observed
finally:
    fixture.terminate()
    fixture.wait(timeout=5)
PY
then
  _pass "queued change avoids timeout; limits, stale refs, and Unicode input remain correct"
else
  _fail "action latency behavior" "background fixture assertions failed"
fi
summary
