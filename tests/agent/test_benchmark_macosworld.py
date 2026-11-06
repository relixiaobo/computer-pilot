"""The macOSWorld dashboard compares only compatible, complete reports."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("macosworld_regression", ROOT / "scripts/macosworld-regression.py")
dashboard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dashboard)


def report(date, passed, *, model="test-model", category="sys_apps", results=None):
    results = results or [
        {"task_id": "task-a", "status": "done", "grade": True, "steps": 2, "input_tokens": 10, "output_tokens": 2},
        {"task_id": "task-b", "status": "fail", "grade": False, "steps": 3, "input_tokens": 11, "output_tokens": 3},
    ]
    return {
        "date": date,
        "upstream_revision": "abc123",
        "model": model,
        "language": "en",
        "risk_set": "safe_low",
        "category": category,
        "max_steps": 12,
        "binary_version": f"cu 0.9.{date[-1]}",
        "binary_sha256": (date.replace("-", "") + "0" * 64)[:64],
        "passed": passed,
        "total": len(results),
        "results": results,
    }


class MacOSWorldDashboardTests(unittest.TestCase):
    def write(self, directory, name, data):
        path = Path(directory) / name
        path.write_text(json.dumps(data))
        return path

    def load_pair(self, first, second):
        with tempfile.TemporaryDirectory() as directory:
            paths = [self.write(directory, f"{index}.json", item) for index, item in enumerate((first, second))]
            return [dashboard.load_report(path) for path in paths]

    def test_render_includes_rate_deltas_and_task_change(self):
        first = report("2026-11-01", 1)
        changed = report("2026-11-02", 2, results=[
            {"task_id": "task-a", "status": "done", "grade": True, "steps": 2, "input_tokens": 10, "output_tokens": 2},
            {"task_id": "task-b", "status": "done", "grade": True, "steps": 3, "input_tokens": 11, "output_tokens": 3},
        ])
        reports = self.load_pair(first, changed)
        output = dashboard.render_markdown(reports)
        self.assertIn("1/2", output)
        self.assertIn("2/2", output)
        self.assertIn("20261101", output)
        self.assertIn("+1", output)
        self.assertIn("+50.0 pp", output)
        self.assertIn("task-b", output)
        self.assertIn("fail (fail) → pass (done)", output)

    def test_comparison_rejects_mixed_model_cohort_and_task_set(self):
        base = report("2026-11-01", 1)
        cases = [
            report("2026-11-02", 1, model="other-model"),
            report("2026-11-02", 1, category="productivity"),
            report("2026-11-02", 1, results=[{**base["results"][0], "task_id": "other"}, base["results"][1]]),
        ]
        for other in cases:
            with self.subTest(other=other):
                with self.assertRaises(dashboard.ReportError):
                    dashboard.validate_comparison(self.load_pair(base, other))

    def test_loader_rejects_missing_fields_duplicate_tasks_and_bad_totals(self):
        base = report("2026-11-01", 1)
        with tempfile.TemporaryDirectory() as directory:
            missing = dict(base)
            del missing["model"]
            with self.assertRaisesRegex(dashboard.ReportError, "missing field"):
                dashboard.load_report(self.write(directory, "missing.json", missing))
            duplicate = dict(base)
            duplicate["results"] = [base["results"][0], base["results"][0]]
            with self.assertRaisesRegex(dashboard.ReportError, "duplicate task_id"):
                dashboard.load_report(self.write(directory, "duplicate.json", duplicate))
            bad_total = dict(base)
            bad_total["total"] = 3
            with self.assertRaisesRegex(dashboard.ReportError, "does not match results length"):
                dashboard.load_report(self.write(directory, "total.json", bad_total))
            bad_digest = dict(base)
            bad_digest["binary_sha256"] = "not-a-digest"
            with self.assertRaisesRegex(dashboard.ReportError, "64-character hexadecimal"):
                dashboard.load_report(self.write(directory, "digest.json", bad_digest))

    def test_cli_writes_output_file(self):
        with tempfile.TemporaryDirectory() as directory:
            first = self.write(directory, "first.json", report("2026-11-01", 1))
            second = self.write(directory, "second.json", report("2026-11-02", 1))
            output = Path(directory) / "dashboard.md"
            self.assertEqual(dashboard.main([str(first), str(second), "--output", str(output)]), 0)
            self.assertIn("macOSWorld regression dashboard", output.read_text())


if __name__ == "__main__":
    unittest.main()
