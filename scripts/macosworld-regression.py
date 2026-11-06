#!/usr/bin/env python3
"""Compare recorded macOSWorld result reports and render a Markdown dashboard.

The dashboard intentionally consumes recorded reports only.  Running the
benchmark remains a local/manual operation because macOSWorld needs a TCC
enabled desktop and may incur model API cost.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from typing import Any, Iterable


REQUIRED_FIELDS = (
    "date",
    "upstream_revision",
    "model",
    "language",
    "risk_set",
    "category",
    "max_steps",
    "binary_version",
    "binary_sha256",
    "passed",
    "total",
    "results",
)
COHORT_FIELDS = ("upstream_revision", "language", "risk_set", "category", "max_steps")
RESULT_FIELDS = ("task_id", "status", "grade", "steps", "input_tokens", "output_tokens")


class ReportError(ValueError):
    """A report cannot be safely included in a comparison."""


class Report:
    def __init__(self, path: Path, data: dict[str, Any]) -> None:
        self.path = path
        self.data = data

    @property
    def date(self) -> str:
        return self.data["date"]

    @property
    def model(self) -> str:
        return self.data["model"]

    @property
    def cohort(self) -> tuple[Any, ...]:
        return tuple(self.data[field] for field in COHORT_FIELDS)

    @property
    def results(self) -> list[dict[str, Any]]:
        return self.data["results"]


def _require_type(value: Any, expected: type | tuple[type, ...], label: str) -> None:
    if not isinstance(value, expected) or (expected is int and isinstance(value, bool)):
        names = ", ".join(t.__name__ for t in (expected if isinstance(expected, tuple) else (expected,)))
        raise ReportError(f"{label} must be {names}")


def _validate_result(result: Any, path: Path, index: int) -> dict[str, Any]:
    label = f"{path}: results[{index}]"
    _require_type(result, dict, label)
    missing = [field for field in RESULT_FIELDS if field not in result]
    if missing:
        raise ReportError(f"{label} missing field(s): {', '.join(missing)}")
    _require_type(result["task_id"], str, f"{label}.task_id")
    if not result["task_id"]:
        raise ReportError(f"{label}.task_id must not be empty")
    _require_type(result["status"], str, f"{label}.status")
    _require_type(result["grade"], bool, f"{label}.grade")
    for field in ("steps", "input_tokens", "output_tokens"):
        _require_type(result[field], int, f"{label}.{field}")
        if result[field] < 0:
            raise ReportError(f"{label}.{field} must be non-negative")
    return result


def load_report(path: str | Path) -> Report:
    """Load and validate one recorded macOSWorld report."""

    path = Path(path)
    try:
        data = json.loads(path.read_text())
    except FileNotFoundError as exc:
        raise ReportError(f"report not found: {path}") from exc
    except (OSError, json.JSONDecodeError) as exc:
        raise ReportError(f"cannot read JSON report {path}: {exc}") from exc

    _require_type(data, dict, str(path))
    missing = [field for field in REQUIRED_FIELDS if field not in data]
    if missing:
        raise ReportError(f"{path} missing field(s): {', '.join(missing)}")
    _require_type(data["date"], str, f"{path}.date")
    if len(data["date"]) != 10:
        raise ReportError(f"{path}.date must be YYYY-MM-DD")
    try:
        from datetime import date

        date.fromisoformat(data["date"])
    except ValueError as exc:
        raise ReportError(f"{path}.date must be YYYY-MM-DD") from exc
    for field in ("upstream_revision", "model", "language", "risk_set", "category", "binary_version", "binary_sha256"):
        _require_type(data[field], str, f"{path}.{field}")
        if not data[field]:
            raise ReportError(f"{path}.{field} must not be empty")
    _require_type(data["max_steps"], int, f"{path}.max_steps")
    _require_type(data["passed"], int, f"{path}.passed")
    _require_type(data["total"], int, f"{path}.total")
    for field in ("max_steps", "passed", "total"):
        if data[field] < 0:
            raise ReportError(f"{path}.{field} must be non-negative")
    _require_type(data["results"], list, f"{path}.results")
    if data["total"] != len(data["results"]):
        raise ReportError(f"{path}.total ({data['total']}) does not match results length ({len(data['results'])})")
    seen: set[str] = set()
    for index, result in enumerate(data["results"]):
        result = _validate_result(result, path, index)
        task_id = result["task_id"]
        if task_id in seen:
            raise ReportError(f"{path} contains duplicate task_id: {task_id}")
        seen.add(task_id)
    graded = sum(1 for result in data["results"] if result["grade"])
    if data["passed"] != graded:
        raise ReportError(f"{path}.passed ({data['passed']}) does not match graded passes ({graded})")
    if data["passed"] > data["total"]:
        raise ReportError(f"{path}.passed cannot exceed total")
    return Report(path=path, data=data)


def validate_comparison(reports: Iterable[Report]) -> list[Report]:
    """Validate a comparable series and return it in chronological order."""

    reports = list(reports)
    if len(reports) < 2:
        raise ReportError("at least two reports are required for a regression comparison")
    models = {report.model for report in reports}
    if len(models) != 1:
        raise ReportError("reports use different models; compare one model at a time")
    cohorts = {report.cohort for report in reports}
    if len(cohorts) != 1:
        raise ReportError("reports use different cohorts; upstream revision, language, risk set, category, and max_steps must match")
    dates = [report.date for report in reports]
    if len(set(dates)) != len(dates):
        raise ReportError("reports contain duplicate dates")
    reports = sorted(reports, key=lambda report: report.date)
    task_sets = {frozenset(result["task_id"] for result in report.results) for report in reports}
    if len(task_sets) != 1:
        raise ReportError("reports contain different task sets; compare the same fixed tasks")
    return reports


def _signed(value: int) -> str:
    return f"{value:+d}" if value else "0"


def _signed_pp(value: float) -> str:
    return f"{value:+.1f} pp" if value else "0.0 pp"


def _rate(report: Report) -> float:
    return report.data["passed"] / report.data["total"] * 100 if report.data["total"] else 0.0


def _result_map(report: Report) -> dict[str, dict[str, Any]]:
    return {result["task_id"]: result for result in report.results}


def render_markdown(reports: Iterable[Report], title: str = "macOSWorld regression dashboard") -> str:
    """Render a validated report series as a concise Markdown dashboard."""

    reports = validate_comparison(reports)
    first = reports[0]
    cohort = first.cohort
    cohort_text = "/".join(str(value) for value in cohort[1:4])
    lines = [
        f"# {title}",
        "",
        "This dashboard compares recorded results; it does not run macOSWorld.",
        "",
        f"- Model: `{first.model}`",
        f"- Cohort: `{cohort_text}`; max steps `{cohort[4]}`",
        f"- Upstream revision: `{cohort[0]}`",
        "",
        "| Date | Binary | Passed | Pass rate | Δ passed | Δ rate |",
        "|---|---|---:|---:|---:|---:|",
    ]
    previous: Report | None = None
    for report in reports:
        if previous is None:
            delta_passed, delta_rate = "—", "—"
        else:
            delta_passed = _signed(report.data["passed"] - previous.data["passed"])
            delta_rate = _signed_pp(_rate(report) - _rate(previous))
        lines.append(
            f"| {report.date} | `{report.data['binary_version']}` | "
            f"{report.data['passed']}/{report.data['total']} | {_rate(report):.1f}% | "
            f"{delta_passed} | {delta_rate} |"
        )
        previous = report

    lines.extend(["", "## Task status changes", ""])
    changes = []
    for before, after in zip(reports, reports[1:]):
        before_results, after_results = _result_map(before), _result_map(after)
        for task_id in sorted(after_results):
            old, new = before_results[task_id], after_results[task_id]
            old_state = (old["grade"], old["status"])
            new_state = (new["grade"], new["status"])
            if old_state != new_state:
                old_mark = "pass" if old["grade"] else "fail"
                new_mark = "pass" if new["grade"] else "fail"
                changes.append(f"| {before.date} → {after.date} | `{task_id}` | {old_mark} ({old['status']}) → {new_mark} ({new['status']}) |")
    if changes:
        lines.extend(["| From → to | Task | Result |", "|---|---|---|"])
        lines.extend(changes)
    else:
        lines.append("No task-level status changes.")
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reports", nargs="+", type=Path, help="recorded macOSWorld JSON reports")
    parser.add_argument("-o", "--output", type=Path, help="write Markdown to this file instead of stdout")
    parser.add_argument("--title", default="macOSWorld regression dashboard", help="Markdown heading")
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        markdown = render_markdown((load_report(path) for path in args.reports), title=args.title)
        if args.output:
            args.output.write_text(markdown)
        else:
            sys.stdout.write(markdown)
    except (ReportError, OSError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
