#!/usr/bin/env python3
"""Compare two CLI+skill versions on an owned, multi-page native UI task.

Uses the same current agent harness for both arms (no historical harness sleeps).
Outputs complete transcripts, timing breakdowns, binary hashes, and saved data.
"""
import argparse
from collections import Counter
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import signal
import shutil
import tempfile
import statistics
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("agent_harness", ROOT / "tests/agent/run.py")
h = importlib.util.module_from_spec(spec)
spec.loader.exec_module(h)


def source_orders():
    return [dict(Customer=name, SKU=f"CP-{101+i}", Quantity=str(quantity), City=city,
                 Memo=f"订单 {i+1}：请核对商品编码和数量，使用可回收包装。收货前电话联系，周一至周五配送；包装内附装箱清单。Reference batch-{i+1:02d}; keep dry.")
            for i, (name, quantity, city) in enumerate([
                ("陈晨", 3, "上海"), ("Lin Wei", 12, "北京"), ("王芳", 8, "深圳"),
                ("Alex Chen", 25, "成都"), ("赵敏", 4, "苏州"), ("周宁", 16, "南京")])]


def verify_saved(path):
    initial = [dict(order, Priority=str(int(order["Quantity"]) >= 10).lower())
               for order in source_orders()]
    expected = [dict(order) for order in initial]
    expected[1]["City"] = "杭州"
    expected[4].update(Quantity="14", Priority="true")
    actual = json.loads(path.read_text()) if path.exists() else {}
    checks = {"all_initial_fields_exact": actual.get("initial") == initial,
              "all_final_fields_exact": actual.get("records") == expected,
              "both_corrections_saved": set(actual.get("edits", [])) == {2, 5},
              "archive_completed": actual.get("completed") is True}
    return {"passed": all(checks.values()), "checks": checks, "actual": actual}


def goal(pid):
    return f"""Use only the owned native test app pid:{pid}. Computer Pilot is installed and permissions are granted; start by observing this exact PID. Do not operate any other app or read benchmark files.
Complete this order-entry workflow:
1. For each of the 6 orders, read the Source Customer, SKU, Quantity, City, and Memo shown in the UI and copy all five values exactly into their matching editable fields. Set Priority on if Quantity >= 10, otherwise off. Save each order with Save and next.
2. After all six original orders are saved and the review page appears, edit order 2: change City to 杭州 and save the correction. Then edit order 5: change Quantity to 14, update Priority according to the same rule, and save the correction. Keep every other field unchanged.
3. Review the saved orders, click Finish archive, and confirm Archive complete: 6 orders before saying DONE.
The initial records must be saved before applying corrections. This app is not scriptable; use its native UI through cu. All controls are within the window. No real customer data or external messages are involved."""


def prepare_fixture(root):
    bundle = root / "OrderWorkflow.app/Contents"
    (bundle / "MacOS").mkdir(parents=True)
    executable = bundle / "MacOS/OrderWorkflow"
    subprocess.run(["swiftc", str(ROOT / "tests/fixtures/order-workflow.swift"), "-o", str(executable)], check=True)
    (bundle / "Info.plist").write_bytes(plistlib.dumps({
        "CFBundleExecutable": "OrderWorkflow", "CFBundleIdentifier": "test.computer-pilot.order-workflow",
        "CFBundleName": "OrderWorkflow", "CFBundlePackageType": "APPL", "LSUIElement": True}))
    source = root / "source.json"
    source.write_text(json.dumps(source_orders(), ensure_ascii=False))
    return executable, source


def configure(binary, directory):
    directory.mkdir(parents=True)
    # Unix-domain socket paths must stay below macOS's short path limit.
    os.environ["COMPUTER_PILOT_HOME"] = tempfile.mkdtemp(prefix="cu-long-broker-")
    h.CU = str(binary)
    h.AGENT_CLIENT_KEY = "long-workflow"
    h.AGENT_OUTPUT_DIR = str(directory / "output")


def stop_broker():
    response = h.cu_result(["--json", "status"])
    if response["ok"]:
        pid = json.loads(response["text"]).get("pid")
        if pid:
            os.kill(pid, signal.SIGTERM)


def ready(pid):
    for _ in range(40):
        result = h.cu_result(["snapshot", f"pid:{pid}", "--limit", "200"])
        if result["ok"] and json.loads(result["text"]).get("elements"):
            return
        time.sleep(.1)
    raise RuntimeError("owned app did not expose its UI")


def smoke(pid):
    def call(*args):
        result = h.cu_result(list(args))
        assert result["ok"], result
        return json.loads(result["text"])
    app = f"pid:{pid}"
    def edit_fields(values, priority):
        snap = call("snapshot", app)
        for key, value in values.items():
            field = next(e for e in snap["elements"] if e.get("title") == key and e["role"] == "textfield")
            call("set-value", value, "--ax-path", field["axPath"], "--app", app, "--no-snapshot")
        snap = call("snapshot", app)
        checkbox = next(e for e in snap["elements"] if e["role"] == "checkbox")
        priority_on = any(e.get("value") == "Priority status: on" for e in snap["elements"])
        if priority_on != priority:
            call("click", str(checkbox["ref"]), "--app", app)
    def click(title):
        snap = call("snapshot", app)
        el = next(e for e in snap["elements"] if e.get("title") == title)
        call("click", str(el["ref"]), "--app", app)
    for order in source_orders():
        edit_fields(order, int(order["Quantity"]) >= 10)
        click("Save and next")
    click("Edit order 2")
    edit_fields({"City": "杭州"}, True)
    click("Save correction")
    click("Edit order 5")
    edit_fields({"Quantity": "14"}, True)
    click("Save correction")
    click("Finish archive")


def summarize(runs):
    summary = {}
    keys = ["agent_loop_ms", "model_ms", "command_ms", "model_calls", "commands",
            "input_tokens", "output_tokens"]
    for arm in ("before", "after"):
        trials = [r for r in runs if r["arm"] == arm]
        passed = [r for r in trials if r["verified"] and r["agent_status"] == "done"]
        summary[arm] = dict(successes=len(passed), trials=len(trials))
        for label, selected in [("all_trial_medians", trials), ("successful_trial_medians", passed)]:
            metrics = [dict(r["timings"], input_tokens=r["input_tokens"], output_tokens=r["output_tokens"])
                       for r in selected]
            summary[arm][label] = {key: statistics.median(r[key] for r in metrics)
                                   for key in keys} if metrics else {}
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--baseline-ref", default="b9987a6")
    parser.add_argument("--candidate-ref", default="HEAD")
    parser.add_argument("--model", default=os.environ.get("AGENT_MODEL", "gpt-5.6-sol"))
    parser.add_argument("--reasoning", default="low")
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()
    root = ROOT / "test-results/long-workflow" / time.strftime("%Y%m%dT%H%M%S")
    root.mkdir(parents=True)
    executable, source = prepare_fixture(root)
    os.environ["AGENT_REASONING_EFFORT"] = args.reasoning
    os.environ["COMPUTER_PILOT_VISUAL_CURSOR"] = "0"
    versions = {}
    for name, binary, ref in [("before", args.baseline.resolve(), args.baseline_ref),
                              ("after", args.candidate.resolve(), args.candidate_ref)]:
        commit = subprocess.check_output(["git", "rev-parse", ref], cwd=ROOT, text=True).strip()
        skill = subprocess.check_output(["git", "show", f"{commit}:plugin/skills/computer-pilot/SKILL.md"], cwd=ROOT, text=True).split("---", 2)[-1].strip()
        versions[name] = dict(binary=str(binary), commit=commit,
                              sha256=hashlib.sha256(binary.read_bytes()).hexdigest(), skill=skill)
    report = dict(model=args.model, reasoning=args.reasoning, versions=versions, runs=[],
                  harness="same current harness for both arms; setup/independent verification excluded")
    (root / "task.txt").write_text(goal("<PID>"))
    original_prompt, original_call, original_cu = h.SYSTEM_PROMPT, h.call_llm, h.cu_result
    client = h.llm_client(h.is_claude_model(args.model)).with_options(timeout=90, max_retries=0) if not args.smoke else None
    h.llm_client = lambda _: client
    sequence = ["after"] if args.smoke else [arm for n in range(args.repeats) for arm in (("before", "after") if n % 2 == 0 else ("after", "before"))]
    counts = Counter()
    print(f"OUTPUT {root}", flush=True)
    for arm in sequence:
        counts[arm] += 1
        label = f"{arm}-{counts[arm]}"
        directory = root / label
        configure(versions[arm]["binary"], directory)
        saved = directory / "saved.json"
        fixture = subprocess.Popen([str(executable), str(source), str(saved)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            ready(fixture.pid)
            h.SYSTEM_PROMPT = original_prompt.replace(h._skill_content, versions[arm]["skill"])
            (directory / "system-prompt.txt").write_text(h.SYSTEM_PROMPT)
            print(f"START {label} pid:{fixture.pid}", flush=True)
            with (directory / "transcript.jsonl").open("w") as transcript:
                def record(item):
                    transcript.write(json.dumps(item, ensure_ascii=False) + "\n")
                    transcript.flush()
                def call_model(model, messages):
                    answer = original_call(model, messages)
                    record(dict(kind="model", text=answer[0], input_tokens=answer[1], output_tokens=answer[2]))
                    return answer
                def run_command(command):
                    result = original_cu(command)
                    record(dict(kind="command", argv=command, **result))
                    return result
                h.call_llm, h.cu_result = call_model, run_command
                if args.smoke:
                    smoke(fixture.pid)
                    result = dict(agent_status="smoke", timings={})
                else:
                    result = h.run_agent_task(dict(id=label, name="Six orders and two corrections", goal=goal(fixture.pid), verify=[]), args.model, max_steps=100, verbose=True)
                result["independent_verification"] = verify_saved(saved)
                result["verified"] = result["independent_verification"]["passed"]
                result.update(arm=arm, trial=counts[arm])
                report["runs"].append(result)
                (root / "results.json").write_text(json.dumps(report, ensure_ascii=False, indent=2))
                print("RESULT " + json.dumps({k: result[k] for k in ["arm", "trial", "agent_status", "verified", "timings"]}), flush=True)
        finally:
            h.call_llm, h.cu_result = original_call, original_cu
            fixture.terminate()
            fixture.wait(timeout=5)
            stop_broker()
            shutil.rmtree(os.environ["COMPUTER_PILOT_HOME"], ignore_errors=True)
    if not args.smoke:
        summary = summarize(report["runs"])
        report["summary"] = summary
        (root / "results.json").write_text(json.dumps(report, ensure_ascii=False, indent=2))
        print(json.dumps(summary, indent=2), flush=True)
    assert all(r["verified"] for r in report["runs"]), "some trials failed independent verification; retain them in the report"


if __name__ == "__main__":
    main()
