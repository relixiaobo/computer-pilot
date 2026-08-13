#!/bin/bash
# Test: scripts/bench-ab.py validates inputs, subprocesses, equivalence, and output safety.
source "$(dirname "$0")/helpers.sh"

BENCH="$ROOT_DIR/scripts/bench-ab.py"
SANDBOX=$(mktemp -d "${TMPDIR:-/tmp}/cu-bench-ab.XXXXXX")
cleanup_sandbox() { rm -rf "$SANDBOX"; }
trap cleanup_sandbox EXIT

FAKE="$SANDBOX/fake-cu"
cat >"$FAKE" <<'PY'
#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys
import time

count_file = os.environ.get("FAKE_CU_COUNT_FILE")
if count_file:
    with open(count_file, "a", encoding="utf-8") as handle:
        handle.write("1\n")

home_file = os.environ.get("FAKE_CU_HOME_FILE")
if home_file:
    with open(home_file, "a", encoding="utf-8") as handle:
        handle.write(os.environ.get("COMPUTER_PILOT_HOME", "") + "\n")

mode = os.environ.get("FAKE_CU_MODE", "success")
if mode == "timeout":
    time.sleep(float(os.environ.get("FAKE_CU_SLEEP_SECONDS", "1")))
elif mode == "exit":
    print("synthetic command failure", file=sys.stderr)
    raise SystemExit(7)
elif mode == "nonjson":
    print("not json")
    raise SystemExit(0)
elif mode == "notok":
    print(json.dumps({"ok": False, "error": "synthetic rejection"}))
    raise SystemExit(0)

title = "stable"
if mode == "mismatch" and "candidate" in Path(sys.argv[0]).name:
    title = "changed"
print(
    json.dumps(
        {
            "ok": True,
            "elements": [
                {
                    "ref": 1,
                    "role": "button",
                    "title": title,
                    "x": 10,
                    "y": 20,
                    "width": 30,
                    "height": 40,
                }
            ],
        }
    )
)
PY
chmod +x "$FAKE"
ln -s "$FAKE" "$SANDBOX/baseline-cu"
ln -s "$FAKE" "$SANDBOX/candidate-cu"
BASELINE="$SANDBOX/baseline-cu"
CANDIDATE="$SANDBOX/candidate-cu"

run_bench() {
  EXIT=0
  OUT=$("$BENCH" "$@" 2>"$SANDBOX/stderr") || EXIT=$?
  ERR=$(cat "$SANDBOX/stderr")
}

section "bench-ab arguments"

run_bench --help
if [[ "$EXIT" -eq 0 && "$OUT" == *"--timeout"* && "$OUT" == *"--output"* ]]; then
  _pass "help works without the cu argument separator"
else
  _fail "help works without the cu argument separator" \
    "exit=$EXIT stdout=${OUT:0:200} stderr=${ERR:0:200}"
fi

run_bench "$BASELINE" "$CANDIDATE" -n 1 -- snapshot Fake
if [[ "$EXIT" -eq 2 && "$ERR" == *"must be at least 2"* ]]; then
  _pass "sample count rejects values that cannot produce a quartile"
else
  _fail "sample count rejects values that cannot produce a quartile" \
    "exit=$EXIT stderr=${ERR:0:240}"
fi

run_bench "$BASELINE" "$CANDIDATE" --timeout 0 -- snapshot Fake
if [[ "$EXIT" -eq 2 && "$ERR" == *"greater than 0"* ]]; then
  _pass "per-command timeout must be positive"
else
  _fail "per-command timeout must be positive" "exit=$EXIT stderr=${ERR:0:240}"
fi

section "bench-ab subprocess failures"

export FAKE_CU_MODE=exit
run_bench "$BASELINE" "$CANDIDATE" -n 2 --output "$SANDBOX/exit.json" -- snapshot Fake
if [[ "$EXIT" -eq 1 && "$ERR" == *"exited 7"* && "$ERR" == *"synthetic command failure"* ]]; then
  _pass "non-zero cu exit stops the benchmark with context"
else
  _fail "non-zero cu exit stops the benchmark with context" "exit=$EXIT stderr=${ERR:0:240}"
fi

export FAKE_CU_MODE=nonjson
run_bench "$BASELINE" "$CANDIDATE" -n 2 --output "$SANDBOX/nonjson.json" -- snapshot Fake
if [[ "$EXIT" -eq 1 && "$ERR" == *"produced non-JSON stdout"* ]]; then
  _pass "non-JSON cu output is rejected"
else
  _fail "non-JSON cu output is rejected" "exit=$EXIT stderr=${ERR:0:240}"
fi

export FAKE_CU_MODE=notok
run_bench "$BASELINE" "$CANDIDATE" -n 2 --output "$SANDBOX/notok.json" -- snapshot Fake
if [[ "$EXIT" -eq 1 && "$ERR" == *"returned ok!=true"* && "$ERR" == *"synthetic rejection"* ]]; then
  _pass "ok=false cu output is rejected"
else
  _fail "ok=false cu output is rejected" "exit=$EXIT stderr=${ERR:0:240}"
fi

export FAKE_CU_MODE=timeout
export FAKE_CU_SLEEP_SECONDS=0.2
run_bench "$BASELINE" "$CANDIDATE" -n 2 --timeout 0.05 \
  --output "$SANDBOX/timeout.json" -- snapshot Fake
if [[ "$EXIT" -eq 1 && "$ERR" == *"timed out after 0.05s"* ]]; then
  _pass "every measured cu invocation has a deadline"
else
  _fail "every measured cu invocation has a deadline" "exit=$EXIT stderr=${ERR:0:240}"
fi
unset FAKE_CU_SLEEP_SECONDS

section "bench-ab equivalence and output"

export FAKE_CU_MODE=mismatch
run_bench "$BASELINE" "$CANDIDATE" -n 2 --output "$SANDBOX/mismatch.json" -- snapshot Fake
if [[ "$EXIT" -eq 2 && "$ERR" == *"binaries disagree on the IDENTITY"* ]]; then
  _pass "semantic disagreement aborts before timing is reported"
else
  _fail "semantic disagreement aborts before timing is reported" "exit=$EXIT stderr=${ERR:0:260}"
fi

export FAKE_CU_MODE=success
export FAKE_CU_COUNT_FILE="$SANDBOX/invocations"
printf 'keep me\n' >"$SANDBOX/existing.json"
run_bench "$BASELINE" "$CANDIDATE" -n 2 --output "$SANDBOX/existing.json" -- snapshot Fake
if [[ "$EXIT" -eq 1 && "$ERR" == *"refusing to overwrite"* \
  && "$(cat "$SANDBOX/existing.json")" == "keep me" \
  && ! -e "$FAKE_CU_COUNT_FILE" ]]; then
  _pass "existing output is refused before either binary runs"
else
  INVOCATIONS=0
  if [[ -f "$FAKE_CU_COUNT_FILE" ]]; then
    INVOCATIONS=$(wc -l <"$FAKE_CU_COUNT_FILE")
  fi
  DETAILS="exit=$EXIT stderr=${ERR:0:200} contents=$(cat "$SANDBOX/existing.json")"
  _fail "existing output is refused before either binary runs" \
    "$DETAILS invocations=$INVOCATIONS"
fi

unset FAKE_CU_COUNT_FILE
run_bench "$FAKE" "$FAKE" -n 2 --output "$SANDBOX/samples.json" -- snapshot Fake --limit 1
if [[ "$EXIT" -eq 0 ]] && python3 - "$SANDBOX/samples.json" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)
assert data["baseline"] == data["candidate"]
assert data["argv"] == ["snapshot", "Fake", "--limit", "1"]
assert data["mode"] == "ax-path-only"
assert data["timeout_seconds"] == 30.0
assert len(data["baseline_ms"]) == 2
assert len(data["candidate_ms"]) == 2
PY
  [[ "$(stat -f '%Lp' "$SANDBOX/samples.json")" == "600" ]]
then
  _pass "successful self-comparison writes separate samples atomically with mode 0600"
else
  _fail "successful self-comparison writes separate samples atomically with mode 0600" \
    "exit=$EXIT stdout=${OUT:0:240} stderr=${ERR:0:240}"
fi

export FAKE_CU_HOME_FILE="$SANDBOX/broker-homes"
run_bench "$FAKE" "$FAKE" -n 2 --via-broker --compare-key - \
  --output "$SANDBOX/broker-samples.json" -- snapshot Fake
HOME_COUNT=$(sort -u "$FAKE_CU_HOME_FILE" | sed '/^$/d' | wc -l | tr -d ' ')
HOMES_REMOVED=true
while IFS= read -r BENCH_HOME; do
  [[ -z "$BENCH_HOME" || ! -e "$BENCH_HOME" ]] || HOMES_REMOVED=false
done < <(sort -u "$FAKE_CU_HOME_FILE")
if [[ "$EXIT" -eq 0 && "$HOME_COUNT" -eq 2 && "$HOMES_REMOVED" == "true" ]]; then
  _pass "broker mode isolates and cleans both arms when binary paths match"
else
  _fail "broker mode isolates and cleans both arms when binary paths match" \
    "exit=$EXIT homes=$HOME_COUNT removed=$HOMES_REMOVED stdout=${OUT:0:240} stderr=${ERR:0:240}"
fi
unset FAKE_CU_HOME_FILE

summary
