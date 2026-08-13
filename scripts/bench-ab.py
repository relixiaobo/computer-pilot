#!/usr/bin/env python3
"""Interleaved A/B benchmark for two `cu` binaries.

Why interleaved: AX IPC latency depends on what the target app is doing, and
system load drifts over seconds. Running A's whole batch then B's whole batch
produced 3x variance on the reference machine and yielded a sign-flipped
result. Alternating within one loop cancels that drift.

Why the validation: a binary that errors out early looks *faster*. Every sample
is checked for exit code 0 and `ok: true`, and both binaries' semantic output is
compared before any timing is reported. A benchmark that cannot prove the two
binaries do the same thing is not a benchmark.

Usage:
    scripts/bench-ab.py BASELINE_BIN CANDIDATE_BIN -- snapshot Finder --limit 50
    scripts/bench-ab.py BASELINE_BIN CANDIDATE_BIN -n 40 --compare-key elements \
        --timeout 20 --output finder-samples.json \
        -- snapshot Code --limit 200

By default only the AX path is measured (COMPUTER_PILOT_BROKER_CHILD=1). Pass
--via-broker to measure the full round trip; each arm then gets its own
COMPUTER_PILOT_HOME so the two Brokers cannot be shared or restarted between
samples.
"""

import argparse
import json
from math import isfinite
import os
from pathlib import Path
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time


DEFAULT_TIMEOUT_SECONDS = 30.0
BROKER_CLEANUP_TIMEOUT_SECONDS = 10.0
DEFAULT_OUTPUT = Path("bench-ab-samples.json")


def sample_count(value):
    try:
        count = int(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be an integer") from error
    if count < 2:
        raise argparse.ArgumentTypeError("must be at least 2")
    return count


def positive_seconds(value):
    try:
        seconds = float(value)
    except ValueError as error:
        raise argparse.ArgumentTypeError("must be a number") from error
    if not isfinite(seconds) or seconds <= 0:
        raise argparse.ArgumentTypeError("must be finite and greater than 0")
    return seconds


def argument_parser():
    parser = argparse.ArgumentParser(
        usage="%(prog)s [options] BASELINE_BIN CANDIDATE_BIN -- CU_ARGS...",
        description="Interleaved A/B benchmark for two cu binaries.",
        epilog=(
            "Example: %(prog)s ./cu-main ./cu-candidate -n 40 "
            "-- snapshot Finder --limit 50"
        ),
    )
    parser.add_argument("baseline", help="path to the baseline cu binary")
    parser.add_argument("candidate", help="path to the candidate cu binary")
    parser.add_argument(
        "-n",
        type=sample_count,
        default=30,
        help="samples per binary; must be at least 2 (default: 30)",
    )
    parser.add_argument(
        "--compare-key",
        default="elements",
        help="top-level JSON key compared for semantic equivalence; "
        "'-' disables the comparison (default: elements)",
    )
    parser.add_argument(
        "--via-broker",
        action="store_true",
        help="measure the full Broker round trip instead of the AX path alone",
    )
    parser.add_argument(
        "--timeout",
        type=positive_seconds,
        default=DEFAULT_TIMEOUT_SECONDS,
        metavar="SECONDS",
        help=f"deadline for each cu invocation (default: {DEFAULT_TIMEOUT_SECONDS:g})",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT,
        metavar="PATH",
        help=f"new JSON sample file; existing files are refused (default: {DEFAULT_OUTPUT})",
    )
    return parser


def parse_args(argv=None):
    # Split on the first bare `--` ourselves. argparse.REMAINDER is greedy and
    # would swallow our own options once the two positionals are filled.
    raw = list(sys.argv[1:] if argv is None else argv)
    parser = argument_parser()
    if "--" not in raw:
        # Let argparse service help before enforcing the cu-argument separator.
        # Previously `bench-ab.py --help` failed with exit 2 because this check
        # ran before argparse saw the help action.
        if "-h" in raw or "--help" in raw:
            parser.parse_args(raw)
        parser.error("pass the cu arguments after a bare `--`")
    split = raw.index("--")
    own, cu_argv = raw[:split], raw[split + 1 :]

    args = parser.parse_args(own)
    if not cu_argv:
        parser.error("pass the cu arguments after the bare `--`")
    args.argv = cu_argv
    return args


def build_env(via_broker, home=None):
    env = dict(os.environ)
    env.setdefault("COMPUTER_PILOT_CLIENT_KEY", "cu-bench")
    if via_broker:
        env.pop("COMPUTER_PILOT_BROKER_CHILD", None)
        # Each arm MUST get its own COMPUTER_PILOT_HOME. The Broker is keyed by
        # socket path under that home and `ensure_running` reuses any live
        # Broker whose protocol+version match (src/broker.rs:581). Two builds
        # of the same version therefore share one Broker process -- so a change
        # to the *resident Broker* (fsync policy, poll granularity) would be
        # invisible, and both arms would silently measure the same code.
        # Worse, if the versions differ, every sample would tear down and
        # restart the other arm's Broker.
        env["COMPUTER_PILOT_HOME"] = home
    else:
        env["COMPUTER_PILOT_BROKER_CHILD"] = "1"
    return env


def stop_broker(binary, env):
    """Best-effort shutdown of the Broker this arm started."""
    try:
        completed = subprocess.run(
            [binary, "--json", "status"],
            capture_output=True,
            text=True,
            env=env,
            timeout=BROKER_CLEANUP_TIMEOUT_SECONDS,
        )
        pid = json.loads(completed.stdout).get("pid")
        if isinstance(pid, int) and pid > 1:
            os.kill(pid, signal.SIGTERM)
    except Exception:
        pass


def run_once(binary, argv, env, timeout):
    """Return (elapsed_ms, parsed_json). Raises on any failure."""
    started = time.perf_counter()
    try:
        completed = subprocess.run(
            [binary, "--json", *argv],
            capture_output=True,
            text=True,
            env=env,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired:
        raise RuntimeError(f"{binary} timed out after {timeout:g}s") from None
    except OSError as error:
        raise RuntimeError(f"could not execute {binary}: {error}") from None
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    if completed.returncode != 0:
        detail = (completed.stderr or completed.stdout).strip() or "no output"
        raise RuntimeError(
            f"{binary} exited {completed.returncode}: {detail[:400]}"
        )
    try:
        payload = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"{binary} produced non-JSON stdout: {error}") from error
    if not isinstance(payload, dict):
        raise RuntimeError(
            f"{binary} produced a JSON {type(payload).__name__}; expected an object"
        )
    if payload.get("ok") is not True:
        raise RuntimeError(f"{binary} returned ok!=true: {completed.stdout[:400]}")
    return elapsed_ms, payload


# Per-run identifiers and timings that legitimately differ every invocation.
VOLATILE_FIELDS = {
    "command_id",
    "command_status",
    "observation_id",
    "ax_generation",
    "settle_ms",
    "elapsed_ms",
    "warmup_ms",
    "ready_in_ms",
}
# Live geometry. Split out of the strict comparison on purpose -- see below.
GEOMETRY_FIELDS = ("x", "y", "width", "height")


def semantic_projection(payload, key):
    """Split the compared subset into (strict_identity, geometry).

    Full byte-equality is the WRONG equivalence criterion against a live UI: it
    cannot tell "the candidate changed semantics" from "the candidate sampled
    the tree a few microseconds earlier". Measured case: an optimization that
    moves the batch attribute read 2-3 IPC calls earlier produced, on Finder
    --limit 200, identical ref/role/title/value/axPath for all 200 elements but
    2 differing `width` values -- Finder was mid-relayout on its list-view
    columns. Byte-equality called that a regression. It was not one.

    So identity is compared strictly, and geometry is reported separately as
    drift rather than treated as a failure.
    """
    if key == "-":
        return None, None
    if key not in payload:
        raise RuntimeError(
            f"--compare-key {key!r} absent from output; use '-' to disable"
        )
    subset = payload[key]
    if isinstance(subset, dict):
        subset = {k: v for k, v in subset.items() if k not in VOLATILE_FIELDS}
        return json.dumps(subset, sort_keys=True), None
    if isinstance(subset, list) and subset and isinstance(subset[0], dict):
        identity = [
            {
                k: v
                for k, v in item.items()
                if k not in GEOMETRY_FIELDS and k not in VOLATILE_FIELDS
            }
            for item in subset
        ]
        geometry = [[item.get(f) for f in GEOMETRY_FIELDS] for item in subset]
        return json.dumps(identity, sort_keys=True), geometry
    return json.dumps(subset, sort_keys=True), None


# A candidate may sample geometry a few microseconds off the baseline, so exact
# equality is too strict (see semantic_projection). But geometry is NOT cosmetic:
# it drives CGEvent click coordinates, `cu nearest`, `--region`, and annotated
# screenshot offsets. Excluding it outright let a payload with every button moved
# 1000px and sized 0x0 pass as "identical" -- verified, not hypothetical.
# So: bound the drift instead of ignoring it.
GEOMETRY_ABS_TOLERANCE = 2.0  # points; covers sub-frame relayout, not a real move
# A bracket only counts as "clean" if the baseline held geometry still too. A
# wide baseline band is self-defeating: with A1.x=0 and A2.x=1000 the band
# [0,1000] accepts a candidate at x=500 -- verified. So the band is only
# admissible evidence when it is narrow to begin with.
GEOMETRY_STABILITY_THRESHOLD = 4.0  # points of A1<->A2 spread still called "still"


def geometry_stable(a1, a2):
    """True if the baseline held geometry still enough for the band to mean anything."""
    if a1 is None or a2 is None:
        return True  # nothing to compare (non-element payload)
    if len(a1) != len(a2):
        return False
    for va1, va2 in zip(a1, a2):
        for x1, x2 in zip(va1, va2):
            if not (isinstance(x1, (int, float)) and isinstance(x2, (int, float))):
                return False
            if not (isfinite(x1) and isfinite(x2)):
                return False
            if abs(x1 - x2) > GEOMETRY_STABILITY_THRESHOLD:
                return False
    return True


def geometry_verdict(a1, cand, a2):
    """Classify candidate geometry against the two baseline observations.

    Returns (ok, drifted_count, violations). `a1`/`a2` bracket the candidate in
    time, so [min, max] per field is the range the UI legitimately occupied
    during the candidate call; anything outside that band by more than the
    tolerance is the candidate's own doing.
    """
    if a1 is None or cand is None or a2 is None:
        return True, None, []
    if not (len(a1) == len(cand) == len(a2)):
        return False, None, ["element count differs between arms"]

    def usable(value):
        # bool is an int subclass; exclude it explicitly.
        return (
            isinstance(value, (int, float))
            and not isinstance(value, bool)
            and isfinite(value)
        )

    drifted, violations = 0, []
    for index, (va1, vc, va2) in enumerate(zip(a1, cand, a2)):
        if vc != va1:
            drifted += 1
        for field, x1, c, x2 in zip(GEOMETRY_FIELDS, va1, vc, va2):
            # A missing / null / NaN field must FAIL, not skip. Skipping was an
            # exploitable hole: a candidate that dropped every geometry key (or
            # emitted null/NaN) passed the band check outright -- verified.
            if not usable(c):
                violations.append(
                    f"element[{index}].{field}: candidate value {c!r} is missing, "
                    "null, or non-finite"
                )
                continue
            if not (usable(x1) and usable(x2)):
                violations.append(
                    f"element[{index}].{field}: baseline value is missing or "
                    "non-finite; cannot establish a band"
                )
                continue
            low, high = min(x1, x2), max(x1, x2)
            if c < low - GEOMETRY_ABS_TOLERANCE or c > high + GEOMETRY_ABS_TOLERANCE:
                violations.append(
                    f"element[{index}].{field}: candidate {c} outside baseline band "
                    f"[{low}, {high}] +/-{GEOMETRY_ABS_TOLERANCE}"
                )
            # A control that had extent and now has none is never "drift".
            if field in ("width", "height") and c == 0 and x1 != 0:
                violations.append(f"element[{index}].{field}: collapsed to 0 (baseline {x1})")
        if len(violations) >= 5:
            violations.append("... further geometry violations suppressed")
            break
    return not violations, drifted, violations


def summarize(label, samples):
    return (
        f"  {label:10s} median {statistics.median(samples):7.1f} ms"
        f"   p25 {statistics.quantiles(samples, n=4, method='inclusive')[0]:7.1f}"
        f"   min {min(samples):7.1f}   n={len(samples)}"
    )


def validate_output_path(path):
    if os.path.lexists(path):
        raise RuntimeError(f"refusing to overwrite output file: {path}")
    if not path.parent.is_dir():
        raise RuntimeError(f"output directory does not exist: {path.parent}")


def write_samples(path, payload):
    temp_path = None
    try:
        descriptor, temp_name = tempfile.mkstemp(
            prefix=f".{path.name}.", dir=path.parent
        )
        temp_path = Path(temp_name)
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=2)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.link(temp_path, path)
    except FileExistsError:
        # The hard link publishes atomically and refuses a destination created
        # after validate_output_path ran, closing the preflight race.
        raise RuntimeError(f"refusing to overwrite output file: {path}") from None
    except OSError as error:
        raise RuntimeError(f"could not write output file {path}: {error}") from None
    finally:
        if temp_path is not None:
            temp_path.unlink(missing_ok=True)


def main():
    args = parse_args()
    broker_arms = []
    try:
        validate_output_path(args.output)
        if args.via_broker:
            baseline_home = tempfile.mkdtemp(prefix="cu-bench-base-")
            baseline_env = build_env(True, baseline_home)
            broker_arms.append((args.baseline, baseline_home, baseline_env))

            candidate_home = tempfile.mkdtemp(prefix="cu-bench-cand-")
            candidate_env = build_env(True, candidate_home)
            broker_arms.append((args.candidate, candidate_home, candidate_env))
            print(
                f"broker isolation: baseline={baseline_home} "
                f"candidate={candidate_home}"
            )
        else:
            baseline_env = build_env(False)
            candidate_env = build_env(False)

        return run_benchmark(args, baseline_env, candidate_env)
    except (OSError, RuntimeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    finally:
        for binary, home, env in broker_arms:
            stop_broker(binary, env)
            shutil.rmtree(home, ignore_errors=True)


def run_benchmark(args, baseline_env, candidate_env):
    # Equivalence gate, control-first.
    #
    # A single cross-binary comparison against a LIVE UI is not a valid test:
    # the target app mutates between the two calls, so drift masquerades as a
    # binary difference. Measured on Finder --limit 200, baseline-vs-baseline
    # disagreed on 2 of 3 attempts -- including one pair with identical byte
    # length but different content. So: first establish whether the target is
    # stable at all, and only then attribute any difference to the candidate.
    if args.compare_key != "-":
        def project(binary, env):
            _, payload = run_once(binary, args.argv, env, args.timeout)
            return semantic_projection(payload, args.compare_key)

        # Repeated A-B-A bracketing.
        #
        # Two ordering mistakes were made and fixed here; both produced FALSE
        # accusations against a byte-identical patch, so the reasoning is
        # recorded rather than just the result.
        #
        #   A-A-B  -- leaves the candidate call outside the proven-stable
        #             window. Drift during B is blamed on the candidate.
        #   A-B-A  -- brackets B correctly, but a single round still fails when
        #             a drifting value happens to return to its prior state, so
        #             A1 == A2 coincidentally while B saw the excursion.
        #
        # Sound rule: run K brackets. A bracket is "clean" iff A1 == A2 (the UI
        # held still across that candidate call). One clean bracket in which B
        # also matches is sufficient proof the binaries agree. Only if EVERY
        # clean bracket disagrees is this a real regression. No clean bracket at
        # all means the target is simply not quiescent -- which is a statement
        # about the target, never about the candidate.
        #
        # Observed on Finder --limit 200: baseline disagreed with itself on all
        # 3 of 3 consecutive pairs, because Finder recomputes list-view column
        # widths continuously (220.0 -> 209.0, x 877 -> 875).
        # A single agreeing bracket is not proof: with a drifting target the
        # odds of one lucky match are high. Require several, and only count a
        # bracket as clean when the baseline held BOTH identity and geometry
        # still -- otherwise the band it establishes is too wide to mean
        # anything (r8-#6).
        BRACKETS = 8
        REQUIRED_AGREEING = 3
        clean, agreed, size, drift, geo_violations = 0, 0, 0, None, []
        for _ in range(BRACKETS):
            a1, ga1 = project(args.baseline, baseline_env)
            b, gb = project(args.candidate, candidate_env)
            a2, ga2 = project(args.baseline, baseline_env)
            if a1 != a2 or not geometry_stable(ga1, ga2):
                continue
            clean += 1
            size = len(a1)
            if b != a1:
                continue
            geo_ok, bracket_drift, violations = geometry_verdict(ga1, gb, ga2)
            if geo_ok:
                agreed += 1
                drift = bracket_drift if drift is None else max(drift, bracket_drift or 0)
                if agreed >= REQUIRED_AGREEING:
                    break
            else:
                geo_violations = violations
        if clean == 0:
            print(
                f"ABORT: target UI is not quiescent -- across {BRACKETS} brackets the "
                "BASELINE never held both identity and geometry still.\n"
                "        Equivalence cannot be proven against a moving target, and "
                "any difference here says NOTHING about the candidate.\n"
                "        Pick a quiescent target/limit, or pass --compare-key - to "
                "skip the gate and verify equivalence separately.",
                file=sys.stderr,
            )
            return 3
        if agreed == 0 and geo_violations:
            print(
                f"ABORT: candidate geometry left the baseline band ({clean} clean "
                f"bracket(s) of {BRACKETS}):\n        "
                + "\n        ".join(geo_violations)
                + "\n        Geometry drives click coordinates, `cu nearest`, "
                "--region and annotation offsets -- this is a real regression.",
                file=sys.stderr,
            )
            return 2
        if agreed == 0:
            print(
                f"ABORT: binaries disagree on the IDENTITY of {args.compare_key!r} in "
                f"all {clean} clean bracket(s) -- this is a real candidate regression.",
                file=sys.stderr,
            )
            return 2
        if agreed < REQUIRED_AGREEING:
            print(
                f"ABORT: only {agreed} agreeing bracket(s) of the {REQUIRED_AGREEING} "
                f"required ({clean} clean of {BRACKETS}). One lucky match is not proof "
                "against a target this noisy -- rerun on a quieter target.",
                file=sys.stderr,
            )
            return 3
        note = (
            f"identity+geometry agree in {agreed}/{REQUIRED_AGREEING} required "
            f"baseline-bracketed rounds ({clean} clean of {BRACKETS}, {size} bytes)"
        )
        if drift:
            note += (
                f"; max {drift} element(s) drifted within the band "
                f"+/-{GEOMETRY_ABS_TOLERANCE}pt"
            )
        print(f"equivalence: {note}")

    # Warm-up, discarded: pays AX bridge cold-start (200-500ms) and, under
    # --via-broker, starts each arm's own Broker so sampling never includes
    # broker spawn time.
    for binary, env in (
        (args.baseline, baseline_env),
        (args.candidate, candidate_env),
    ):
        run_once(binary, args.argv, env, args.timeout)
        run_once(binary, args.argv, env, args.timeout)

    base_samples, cand_samples = [], []
    baseline_arm = (args.baseline, baseline_env, base_samples)
    candidate_arm = (args.candidate, candidate_env, cand_samples)
    for i in range(args.n):
        # Alternate leader each iteration so neither binary systematically
        # inherits the other's cache/scheduling state.
        order = (
            (baseline_arm, candidate_arm)
            if i % 2 == 0
            else (candidate_arm, baseline_arm)
        )
        for binary, env, samples in order:
            elapsed, _ = run_once(binary, args.argv, env, args.timeout)
            samples.append(elapsed)

    base_median = statistics.median(base_samples)
    cand_median = statistics.median(cand_samples)
    delta = 100.0 * (cand_median - base_median) / base_median

    # Raw samples so a reviewer can recompute or plot rather than trust a summary.
    raw = {
        "baseline": args.baseline,
        "candidate": args.candidate,
        "argv": args.argv,
        "mode": "via-broker" if args.via_broker else "ax-path-only",
        "timeout_seconds": args.timeout,
        "baseline_ms": base_samples,
        "candidate_ms": cand_samples,
    }
    write_samples(args.output, raw)

    mode = "via broker" if args.via_broker else "AX path only"
    print(f"\ncu {' '.join(args.argv)}   ({mode})")
    print(summarize("baseline", base_samples))
    print(summarize("candidate", cand_samples))
    print(f"  median delta: {delta:+.1f}%")
    print(f"  raw samples: {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
