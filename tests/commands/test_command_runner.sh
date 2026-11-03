#!/bin/bash
# Regression: diagnostic text must not corrupt counters or hide a failed suite.
source "$(dirname "$0")/helpers.sh"
section "command runner — failure accounting"
if python3 - "$ROOT_DIR" <<'PY'
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

with tempfile.TemporaryDirectory(prefix='cu-runner-') as tmp:
    root = Path(tmp)
    commands = root / 'tests/commands'
    commands.mkdir(parents=True)
    shutil.copy(Path(sys.argv[1]) / 'tests/commands/run_all.sh', commands)
    binary = root / 'target/release/cu'
    binary.parent.mkdir(parents=True)
    binary.write_text('#!/bin/sh\necho test\n')
    binary.chmod(0o700)
    cases = [
        ('echo "Results: 2 passed, 0 failed, 0 skipped / 2 total"\necho "Results: 2 passed, 0 failed, 0 skipped / 2 total"\n', 1),
        ('echo "Results: 2 passed, 0 failed, 0 skipped / 99 total"\n', 1),
        ('echo "open -a Fixture7974 failed"\necho "Results: 2 passed, 1 failed, 0 skipped / 3 total"\nexit 1\n', 1),
        ('echo "Fixture7974 failed"\nexit 7\n', 1),
        ('echo "Results: 2 passed, 0 failed, 0 skipped / 2 total"\nexit 7\n', 1),
        ('echo "Results: 2 passed, 0 failed, 0 skipped / 2 total"\nexit 0\n', 0),
    ]
    (commands / 'test_second.sh').write_text('echo SECOND_SUITE_RAN\necho "Results: 1 passed, 0 failed, 0 skipped / 1 total"\n')
    for script, expected in cases:
        (commands / 'test_first.sh').write_text(script)
        result = subprocess.run(['bash', str(commands / 'run_all.sh'), 'first', 'second'],
                                capture_output=True, text=True, timeout=10)
        assert result.returncode == expected, (script, result)
        assert 'SECOND_SUITE_RAN' in result.stdout, result
        assert f', {expected} failed,' in result.stdout.split('TOTAL: ')[-1], result
        assert 'syntax error' not in result.stdout + result.stderr, result
        assert ('Failed suites: first' in result.stdout) == bool(expected), result
PY
then
  _pass "numeric diagnostics, missing summaries and nonzero exits cannot hide failures"
else
  _fail "command runner accounting" "see isolated runner assertions above"
fi
summary
