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
    assert "effect_advice" not in changed and "advice" not in changed, changed

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
    assert stale["diagnostics"]["dispatched"] is False, stale
    recovered = stale["diagnostics"]["snapshot"]
    assert recovered["elements"][0]["value"] == "smaller-limit", recovered
    assert "axPath" in recovered["elements"][0], recovered
    # Reuse the already-read recovery state, without another snapshot command.
    run("click", str(recovered["elements"][0]["ref"]), "--app", app,
        "--observation", stale["observation_id"],
        "--no-snapshot", "--no-verify")

    # Unrelated content changes must not invalidate the selected input.
    snap = run("snapshot", app, "--limit", "200")
    other = next(e for e in snap["elements"] if e.get("title") == "other-input")
    assert other["ref"] > 100, other
    run("set-value", "unrelated-change", "--ax-path", other["axPath"],
        "--app", app, "--no-snapshot")
    clicked = run("click", ref, "--app", app, "--observation", snap["observation_id"])
    assert clicked["focus_verified"] is True, clicked
    assert clicked["settle_ms"] < 250, clicked
    complete = clicked["snapshot"]
    assert "base_observation_id" not in complete and "removed_refs" not in complete, complete
    state = {e["ref"]: e for e in complete["elements"]}
    assert state[other["ref"]]["value"] == "unrelated-change", complete
    # Unchanged controls are included and usable without merging a baseline.
    current = run("snapshot", app, "--limit", "200")
    assert complete["elements"] == current["elements"], complete
    assert int(ref) in state and len(state) > 100, complete
    run("set-value", ref, "", "--app", app, "--limit", "200",
        "--observation", clicked["observation_id"])

    # Long Unicode text crosses event boundaries and surrogate pairs. Require
    # exact final content, not merely a changed value or an output field.
    text = "x" * 19 + "😀" + "native中文😀" * 100
    started = time.perf_counter()
    typed = run("type", text, "--app", app, "--no-paste")
    elapsed = (time.perf_counter() - started) * 1000
    assert typed["effect_verified"] is True, typed
    assert typed["snapshot"]["focused"]["value"] == text, typed
    observed = run("snapshot", app, "--limit", "200")
    assert observed["focused"]["value"] == text, observed
    assert typed["snapshot"]["elements"] == observed["elements"], typed
    assert next(e for e in typed["snapshot"]["elements"] if e["ref"] == int(ref))["value"] == text
    print(f"Long Unicode input ({len(text)} scalars): {elapsed:.1f} ms; exact content verified")

    # One shell invocation, fail-fast, and only one final observation.
    run("set-value", "", "--ax-path", path, "--app", app, "--no-snapshot")
    import shlex
    group = [
        [cu, "--json", "--client-key", "latency-fixture", "click", "--ax-path", path,
         "--app", app, "--no-verify", "--no-snapshot"],
        [cu, "--json", "--client-key", "latency-fixture", "key", "end",
         "--app", app, "--no-verify", "--no-snapshot"],
        [cu, "--json", "--client-key", "latency-fixture", "type", "group中文😀",
         "--app", app, "--no-verify", "--no-snapshot"],
        [cu, "--json", "--client-key", "latency-fixture", "snapshot", app, "--limit", "200"],
    ]
    grouped = subprocess.run(["/bin/sh", "-ec", "\n".join(shlex.join(command) for command in group)],
                             capture_output=True, text=True, timeout=15)
    assert grouped.returncode == 0, grouped.stderr
    outputs = [json.loads(line) for line in grouped.stdout.splitlines()]
    assert len(outputs) == 4 and all("snapshot" not in d for d in outputs[:3]), outputs
    assert outputs[2]["effect_verified"] is None, outputs[2]
    assert outputs[-1]["focused"]["value"] == "group中文😀", outputs[-1]

    # Recovery options are accepted after the command too. Replaying an ID
    # must return the original result, not overwrite a subsequent user edit.
    command = ["set-value", str(other["ref"]), "once", "--ax-path", other["axPath"],
               "--app", app, "--request-id", f"late-id-{fixture.pid}"]
    first = run(*command)
    run("set-value", str(other["ref"]), "later", "--ax-path", other["axPath"], "--app", app)
    replay = run(*command)
    assert replay["replayed"] is True and replay["command_id"] == first["command_id"], replay
    final = run("snapshot", app, "--limit", "200")
    assert next(e for e in final["elements"] if e.get("title") == "other-input")["value"] == "later", final

    # A fresh client also receives a complete action snapshot, including
    # the field after ref 50. No extra full snapshot is needed to discover it.
    fresh = subprocess.run([cu, "--json", "--client-key", "latency-new-client",
                            "key", "escape", "--app", app], capture_output=True, text=True)
    assert fresh.returncode == 0, fresh.stderr
    full = json.loads(fresh.stdout)["snapshot"]
    assert "base_observation_id" not in full and len(full["elements"]) > 50, full
    assert any(e.get("title") == "other-input" for e in full["elements"]), full

    # A focused input beyond ref 100 must have the same identity in the
    # pre-type scan and the final snapshot, without falsely rejecting input.
    run("set-value", "", "--ax-path", other["axPath"], "--app", app, "--no-snapshot")
    run("click", "--ax-path", other["axPath"], "--app", app)
    typed = run("type", "late-field", "--app", app, "--no-paste")
    assert typed["effect_verified"] is True, typed
    assert typed["snapshot"]["focused"]["value"] == "late-field", typed

    # Compact observations use the same private full tree and refs. A shell
    # group edits two fields, then returns complete concise state without a baseline.
    import os
    env = dict(os.environ, CU_BIN=cu, CU_APP=app,
               COMPUTER_PILOT_CLIENT_KEY="compact-latency")
    helper = pathlib.Path(cu).resolve().parents[2] / "plugin/skills/computer-pilot/scripts/session.sh"

    def compact(*args):
        p = subprocess.run([cu, "--compact", *args], env=env,
                           capture_output=True, text=True, timeout=15)
        assert p.returncode == 0, p.stderr
        return json.loads(p.stdout)

    compact("snapshot", app, "--limit", "200")
    exact = "compact中文😀\n\"quoted\"\\path " * 40
    script = "\n".join([
        ". " + shlex.quote(str(helper)),
        shlex.join(["cu", "set-value", ref, exact]),
        shlex.join(["cu", "set-value", str(other["ref"]), "second-field"]),
        "cu snapshot",
    ])
    grouped = subprocess.run(["/bin/sh", "-ec", script], env=env,
                             capture_output=True, text=True, timeout=15)
    assert grouped.returncode == 0, grouped.stderr
    outputs = [json.loads(line) for line in grouped.stdout.splitlines()]
    current = outputs[-1]
    assert len(outputs) == 3 and "base_observation_id" not in current, outputs
    assert outputs[:2] == [{"ok": True, "method": "ax-set-value"}] * 2, outputs[:2]
    state = {e["ref"]: e for e in current["elements"]}
    full = run("snapshot", app, "--limit", "200")
    assert {r: e.get("value") for r, e in state.items()} == {
        e["ref"]: e.get("value") for e in full["elements"]}, current
    assert state[int(ref)]["value"] == exact
    assert state[other["ref"]]["value"] == "second-field"
    # Re-observing unchanged UI must still provide all targets and exact text.
    repeated = compact("snapshot", app, "--limit", "200")
    assert repeated["elements"] == current["elements"], repeated
    assert "base_observation_id" not in repeated and "removed_refs" not in repeated
    assert len(json.dumps(repeated)) < len(json.dumps(full)) * 0.6
    # An unchanged target from that complete compact snapshot is actionable.
    compact("set-value", str(other["ref"]), "from-complete-state", "--app", app,
            "--observation", repeated["observation_id"], "--no-snapshot")
    observed = compact("snapshot", app, "--limit", "200")
    assert next(e for e in observed["elements"] if e["ref"] == other["ref"])["value"] == "from-complete-state"
    # A full JSON observation remains available, including paths/geometry.
    assert all("axPath" in e and "width" in e for e in full["elements"])

    # Failing an action stops the sourced helper's group before the next edit.
    failed = subprocess.run(["/bin/sh", "-ec", "\n".join([
        ". " + shlex.quote(str(helper)), "cu click 99999",
        shlex.join(["cu", "set-value", ref, "must-not-run"]), "cu snapshot",
    ])], env=env, capture_output=True, text=True, timeout=15)
    assert failed.returncode != 0 and not failed.stdout, failed
    error = json.loads(failed.stderr)
    assert error["ok"] is False and error["code"] == "stale_observation", error
    final = run("snapshot", app, "--limit", "200")
    assert next(e for e in final["elements"] if e["ref"] == int(ref))["value"] == exact

    # Use the same task file for every kind of command and across new shells.
    # The configured binary must win even if another `cu` is on PATH.
    config = bundle.parent / "cu-env.sh"
    config.write_text("\n".join(
        f"export {key}={shlex.quote(env[key])}" for key in
        ("CU_BIN", "CU_APP", "COMPUTER_PILOT_CLIENT_KEY")
    ) + "\n. " + shlex.quote(str(helper)) + "\n")

    def session(*args, shell="/bin/sh", ok=True):
        script = ". " + shlex.quote(str(config)) + "\n" + shlex.join(["cu", *map(str, args)])
        p = subprocess.run([shell, "-ec", script], capture_output=True, text=True, timeout=15)
        d = json.loads(p.stdout or p.stderr)
        assert (p.returncode == 0) == ok and d["ok"] == ok, d
        return d

    full = session("--json", "snapshot", "--limit", "200")
    # Output flags must not silently switch to untargeted ordinary CLI calls.
    # Exercise real edits from independent shells, then read the assigned app.
    for shell in ("/bin/sh", "/bin/bash", "/bin/zsh"):
        scoped = session("--compact", "snapshot", shell=shell)
        assert scoped["pid"] == fixture.pid, scoped
        session("--compact", "set-value", other["ref"], shell, shell=shell)
        observed = session("--json", "snapshot", shell=shell)
        assert observed["pid"] == fixture.pid, observed
        assert next(e for e in observed["elements"] if e["ref"] == other["ref"])["value"] == shell
    session("set-value", other["ref"], "second-field")
    # Checkbox state belongs on the control in complete compact snapshots. The
    # independent callback label proves the actual control changed on/off.
    urgent = next(e for e in full["elements"] if e.get("title") == "Urgent")
    assert urgent["value"] == "1", urgent
    for value, label in (("0", "off"), ("1", "on")):
        action = compact("click", str(urgent["ref"]), "--app", app, "--limit", "200", "--no-verify")
        complete = action["snapshot"]
        assert "base_observation_id" not in complete, complete
        checkbox = next(e for e in complete["elements"] if e.get("title") == "Urgent")
        assert checkbox["value"] == value, complete
        assert any(e.get("value") == f"Urgent callback: {label}" for e in complete["elements"]), complete
        assert next(e for e in complete["elements"] if e["ref"] == int(ref))["value"] == exact
        assert "displays" not in complete and all("axPath" not in e for e in complete["elements"]), complete
    # Exercise the existing single-attribute fallback as well as batch decoding.
    fallback = subprocess.run([cu, "--json", "snapshot", app, "--limit", "200"],
                              env=dict(os.environ, CU_TEST_AX_BATCH_FAIL_ROLE="AXCheckBox",
                                       COMPUTER_PILOT_BROKER_CHILD="1"),
                              capture_output=True, text=True, timeout=15)
    assert fallback.returncode == 0, fallback.stderr
    assert next(e for e in json.loads(fallback.stdout)["elements"]
                if e.get("title") == "Urgent")["value"] == "1", fallback.stdout

    # Menu selection uses AXPress and fires the application's real handler.
    found = session("find", "--role", "popupbutton", "--first", shell="/bin/zsh")
    popup = found["match"]["ref"]
    session("perform", popup, "AXPress")
    opened = session("wait", "--text", "Proxy", "--timeout", "3")
    option = next(e for e in opened["snapshot"]["elements"]
                  if e["role"] == "menuitem" and e.get("title") == "Proxy")
    selected = session("perform", option["ref"], "AXPress", shell="/bin/bash")
    assert "available_actions" not in selected and "effect_advice" not in selected, selected
    chosen = session("snapshot")
    assert any(e.get("value") == "Choice: Proxy" for e in chosen["elements"]), chosen
    assert not any(e["role"] == "menuitem" for e in chosen["elements"]), chosen
    assert next(e for e in chosen["elements"] if e["ref"] == int(ref))["value"] == exact
    session("set-value", ref, exact, "--observation", chosen["observation_id"])
    # Menu transition waiting applies to click and path-based perform as well,
    # even with --no-snapshot. The next observation sees the completed callback.
    for label, use_path in (("Direct", False), ("Proxy", True)):
        session("perform", popup, "AXPress")
        opened = session("--json", "snapshot")
        option = next(e for e in opened["elements"] if e["role"] == "menuitem" and e.get("title") == label)
        if use_path:
            session("perform", "AXPress", "--ax-path", option["axPath"])
        else:
            session("click", option["ref"])
        chosen = session("snapshot")
        assert any(e.get("value") == f"Choice: {label}" for e in chosen["elements"]), chosen
        assert not any(e["role"] == "menuitem" for e in chosen["elements"]), chosen
    # Failure still exposes supported actions, without another why/snapshot.
    found = session("find", "--role", "popupbutton", "--first")
    rejected = session("perform", found["match"]["ref"], "AXBogusAction", ok=False)
    assert "AXPress" in rejected["diagnostics"]["available_actions"], rejected

    # Replace the entire view asynchronously. Refs returned by wait must be
    # usable directly, without a redundant snapshot to register them first.
    found = session("find", "--role", "button", "--title-equals", "Next page", "--first")
    session("click", found["match"]["ref"])
    ready = session("wait", "--text", "Ready for input", "--timeout", "3")
    # A disappeared ref returns the current page and registers it for ordinary
    # actions too. The stale write must not touch the replacement input.
    missing = session("set-value", other["ref"], "must-not-run",
                      "--observation", full["observation_id"], ok=False)
    assert missing["code"] == "stale_observation" and "no longer exists" in missing["error"], missing
    field = next(e for e in missing["diagnostics"]["snapshot"]["elements"] if e["role"] == "textfield")
    assert field["value"] == "Ready for input", field
    assert "axPath" not in field and missing["diagnostics"]["dispatched"] is False, missing
    session("set-value", field["ref"], "after-wait中文😀")
    # On timeout the last state remains actionable and is returned in full.
    expired = session("wait", "--text", "never present", "--timeout", "0", ok=False)
    assert "base_observation_id" not in expired["snapshot"], expired
    last = next(e for e in expired["snapshot"]["elements"] if e["role"] == "textfield")
    assert last["value"] == "after-wait中文😀", last
    session("set-value", last["ref"], "after-timeout", "--observation", expired["observation_id"])
    full = session("--json", "snapshot")
    assert full["client_key"] == "compact-latency" and full["pid"] == fixture.pid, full
    assert full["elements"][0]["value"] == "after-timeout" and "axPath" in full["elements"][0], full
    selected = session("key", "cmd+a")
    # NSTextField treats a newline as submit; exercise long event input without
    # submitting this single-line editor. AXValue above covers exact newlines.
    event_text = exact.replace("\n", " ")
    typed = session("type", event_text, "--no-paste")
    assert "combo" not in selected and "text" not in typed, (selected, typed)
    full = session("--json", "snapshot")
    assert full["elements"][0]["value"] == event_text, full

    # A different window with identical title/field/value must not accept the
    # previous window's ref. Identity comes from the same read as the tree.
    original = full
    other_window = next(e for e in original["elements"] if e.get("title") == "Other window")
    session("click", other_window["ref"])
    rejected = session("set-value", original["elements"][0]["ref"], "must-not-run",
                       "--observation", original["observation_id"], ok=False)
    assert rejected["code"] == "stale_observation" and "window changed" in rejected["error"], rejected
    switched = rejected["diagnostics"]["snapshot"]
    assert rejected["window_id"] != original["window_id"], rejected
    assert switched["elements"][0]["value"] == event_text, switched
    session("set-value", switched["elements"][0]["ref"], "recovered-window",
            "--observation", rejected["observation_id"])
    assert session("snapshot")["elements"][0]["value"] == "recovered-window"

    # Standard window controls must be identifiable without guessing an empty
    # button's meaning. Check both readers, then close only the owned second window.
    close = next(e for e in switched["elements"] if e.get("title") == "close button")
    fallback = subprocess.run([cu, "--json", "snapshot", app, "--limit", "200"],
                              env=dict(os.environ, CU_TEST_AX_BATCH_FAIL_ROLE="AXButton",
                                       COMPUTER_PILOT_BROKER_CHILD="1"),
                              capture_output=True, text=True, timeout=15)
    assert fallback.returncode == 0, fallback.stderr
    assert any(e.get("title") == close["title"] and e["ref"] == close["ref"]
               for e in json.loads(fallback.stdout)["elements"]), fallback.stdout
    session("--compact", "click", close["ref"])
    remaining = run("window", "list", "--app", app)
    assert len(remaining["windows"]) == 1, remaining
    assert session("snapshot")["elements"][0]["value"] == event_text

finally:
    fixture.terminate()
    fixture.wait(timeout=5)
PY
then
  _pass "grouping, exact Unicode input, target freshness, and complete snapshots"
else
  _fail "action latency behavior" "background fixture assertions failed"
fi
summary
