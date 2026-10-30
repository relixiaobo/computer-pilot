#!/bin/bash
# Regression checks that never send input to the real desktop.
source "$(dirname "$0")/helpers.sh"

section "desktop safety — agent hosts reject implicit input"
for host in Codex ChatGPT; do
  # An invalid key is deliberate: even a broken guard cannot dispatch Escape.
  EXIT=0
  OUT=$(CU_TEST_FRONTMOST_OVERRIDE="$host" "$CU" --json key invalid-safety-probe --no-snapshot 2>&1) || EXIT=$?
  if [[ "$EXIT" -ne 0 && "$OUT" == *"refusing to send keys"* ]]; then
    _pass "$host refuses implicit keyboard input before dispatch"
  else
    _fail "$host implicit input guard" "$OUT"
  fi
done

section "desktop safety — fixture ownership and opt-in commands"
SAFETY_OUT=$(python3 - "$ROOT_DIR" <<'PY' 2>&1
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

root = Path(sys.argv[1])
with tempfile.TemporaryDirectory(prefix="cu-desktop-safety-") as tmp:
    base = Path(tmp)
    commands = base / "tests/commands"
    commands.mkdir(parents=True)
    for name in ("helpers.sh", "test_method_meta.sh", "verify_no_disruption.sh"):
        shutil.copy(root / "tests/commands" / name, commands / name)
    binary = base / "target/release/cu"
    binary.parent.mkdir(parents=True)
    calls = base / "calls.jsonl"
    binary.write_text('''#!/usr/bin/env python3
import json, os, sys
args = sys.argv[1:]
with open(os.environ["SAFETY_CALLS"], "a") as f:
    f.write(json.dumps(args) + "\\n")
if "key" in args:
    print(json.dumps({"ok": True, "method": "key-pid", "confidence": "medium", "advice": "verify"}))
else:
    print(json.dumps({"ok": True}))
''')
    binary.chmod(0o700)
    stub_bin = base / "bin"
    stub_bin.mkdir()
    for name in ("swift", "osascript", "pbcopy"):
        stub = stub_bin / name
        stub.write_text('#!/bin/sh\necho UNEXPECTED_OS_CALL >&2\nexit 99\n')
        stub.chmod(0o700)
    env = dict(os.environ, COMPUTER_PILOT_TEST_INTERACTIVE="0", SAFETY_CALLS=str(calls), CU=str(binary),
               PATH=str(stub_bin) + os.pathsep + os.environ["PATH"])
    def run(script):
        result = subprocess.run(["bash", "-c", script], cwd=commands, env=env,
                                text=True, capture_output=True, timeout=15)
        assert result.returncode == 0, result.stdout + result.stderr
        return result.stdout

    # A pre-existing TextEdit session must survive both setup and its EXIT trap.
    output = run('''source ./helpers.sh
pgrep() { return 0; }
_run_with_timeout() { echo UNEXPECTED_OS_CALL; return 99; }
open() { echo UNEXPECTED_OPEN; return 99; }
trap 'textedit_cleanup' EXIT
textedit_reset
echo UNEXPECTED_CONTINUATION
''')
    assert "preserving the user's session" in output and "UNEXPECTED" not in output, output
    print("existing TextEdit session is skipped without OS mutations")

    # Cleanup is limited to the known fixture document, and never force-kills
    # the app.
    output = run('''source ./helpers.sh
TEXTEDIT_FIXTURE_STARTED=1
TEXTEDIT_FIXTURE_NAME=fixture-42
pgrep() { return 0; }
kill() { echo UNEXPECTED_KILL; return 99; }
_run_with_timeout() { printf '%s\\n' "$@" >> "$SAFETY_CALLS.cleanup"; }
textedit_cleanup
[[ -z "$TEXTEDIT_FIXTURE_NAME" ]]
''')
    cleanup = Path(str(calls) + ".cleanup").read_text()
    assert "fixture-42" in cleanup and "document named" in cleanup, cleanup
    assert "close every" not in cleanup and "quit saving no" not in cleanup, cleanup
    assert "(count of documents) is 0 then quit" in cleanup and "UNEXPECTED" not in output, cleanup
    print("cleanup targets only the fixture document and preserves other documents")

    output = run("bash ./test_method_meta.sh")
    recorded = [json.loads(line) for line in calls.read_text().splitlines()]
    assert any("key" in args for args in recorded), recorded
    assert all("--app" in args for args in recorded), recorded
    assert "SKIP selector-less input" in output, output
    print("default method_meta never dispatches selector-less input")

    calls.unlink()
    output = run("bash ./verify_no_disruption.sh")
    assert "SKIP:" in output and not calls.exists(), output
    print("standalone disruption probe exits before invoking cu")
PY
) || { _fail "desktop safety regression" "$SAFETY_OUT"; summary; exit 1; }
echo "$SAFETY_OUT"
_pass "fixture ownership and default input isolation"

summary
