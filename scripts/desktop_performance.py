#!/usr/bin/env python3
"""Measure native desktop journeys and compare like-for-like observations.

uv run python scripts/desktop_performance.py run --output /tmp/desktop-baseline
uv run python scripts/desktop_performance.py run --output /tmp/after --baseline /tmp/baseline
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import signal
import statistics
import subprocess
import sys
import time
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
RECORDER = REPO / "scripts/benchmarks/desktop-performance/record_live.py"
COMMAND = [
    "swift",
    "test",
    "--package-path",
    "swift",
    "-Xswiftc",
    "-gnone",
    "--jobs",
    "4",
    "--no-parallel",
    "--filter",
    "DesktopPerformanceTests",
]


def _sources() -> str:
    paths = subprocess.check_output(
        [
            "git",
            "ls-files",
            "-cz",
            "--others",
            "--exclude-standard",
            "--",
            "swift",
            "tests/fixtures/dto",
            "scripts/desktop_performance.py",
            "scripts/benchmarks/desktop-performance/record_live.py",
        ],
        cwd=REPO,
    ).split(b"\0")
    digest = hashlib.sha256()
    for name in sorted(set(paths) - {b""}):
        path = REPO / os.fsdecode(name)
        digest.update(name + b"\0")
        digest.update(path.read_bytes() if path.is_file() else b"<deleted>")
    return digest.hexdigest()


def _write(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _read_events(path: Path) -> tuple[list[dict], list[str]]:
    events = []
    errors = []
    if not path.exists():
        return events, ["Native runner produced no journal."]
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            errors.append(f"Incomplete journal record at line {number}.")
    return events, errors


def _summarize(events: list[dict]) -> dict:
    plans = [event for event in events if event["event"] == "plan"]
    plan = plans[0] if len(plans) == 1 else None
    errors = [] if plan else ["Expected exactly one observation plan."]
    planned = (
        {
            (population, scenario, attempt)
            for population in plan["populations"]
            for scenario in plan["scenarios"]
            for attempt in range(plan["samples"])
        }
        if plan
        else set()
    )
    starts: dict[str, dict] = {}
    ends: dict[str, dict] = {}
    observed = set()
    subject_fields = ["metric", "scenario", "population", "attempt", "state"]
    for event in events:
        kind = event["event"]
        if kind not in {"begin", "end"}:
            continue
        identity = event["id"]
        records = starts if kind == "begin" else ends
        if identity in records:
            errors.append(f"Duplicate {kind} for {identity}.")
        records[identity] = event
        if kind == "begin":
            subject = tuple(event[field] for field in ["population", "scenario", "attempt"])
            if subject not in planned or subject in observed:
                errors.append(f"Unplanned or repeated observation: {identity}.")
            observed.add(subject)
            state = "first_interaction" if event["attempt"] == 0 else "warm"
            if event["state"] != state:
                errors.append(f"Incorrect sampling state for {identity}.")
        elif identity not in starts or any(
            event[field] != starts[identity][field] for field in subject_fields
        ):
            errors.append(f"Result has no matching preceding observation: {identity}.")
    attempts = []
    for identity, start in starts.items():
        attempts.append(
            ends.get(identity, {**start, "outcome": "interrupted", "duration_ms": None})
        )
    groups: dict[tuple, list[dict]] = {}
    for attempt in attempts:
        key = tuple(attempt[field] for field in ["metric", "scenario", "population", "state"])
        groups.setdefault(key, []).append(attempt)
    summaries = []
    for key, values in sorted(groups.items()):
        durations = sorted(value["duration_ms"] for value in values if value["outcome"] == "passed")
        summaries.append(
            {
                **dict(zip(["metric", "scenario", "population", "state"], key)),
                "attempted": len(values),
                "passed": len(durations),
                "failure_rate": (len(values) - len(durations)) / len(values),
                "outcomes": dict(Counter(value["outcome"] for value in values)),
                "p50_ms": statistics.median(durations) if durations else None,
                "p95_ms": durations[math.ceil(len(durations) * 0.95) - 1]
                if len(durations) >= 20
                else None,
            }
        )
    return {
        "plan": plan,
        "attempts": attempts,
        "groups": summaries,
        "expected_attempts": len(planned) if plan else None,
        "not_started": len(planned - observed) if plan else None,
        "journal_errors": errors,
        "soak": _soak(events, plan),
        "journey_errors": [
            event for event in events if event["event"] in {"setup", "setup_or_journey"}
        ],
    }


def _soak(events: list[dict], plan: dict | None) -> dict:
    seconds = (plan or {}).get("soak_seconds", 0)
    starts = [e for e in events if e["event"] == "soak_begin"]
    ends = [e for e in events if e["event"] == "soak_end"]
    rounds = [e for e in events if e["event"] == "soak_round"]
    complete = (
        len(starts) == len(ends) == 1
        and ends[0]["elapsed_seconds"] >= seconds
        and ends[0]["preserved"]
        and len(rounds) >= 4
        and [e["round"] for e in rounds] == list(range(len(rounds)))
        and all(e["preserved"] for e in rounds)
    )
    return {
        "requested_seconds": seconds,
        "status": "not_requested" if not seconds else "complete" if complete else "incomplete",
        "rounds": rounds,
        "phases": [e for e in events if e["event"] == "soak_phase"],
        "end": ends[0] if len(ends) == 1 else None,
        "limits": [
            "Fixture reads, not CLI processes or SQLite query volume",
            "Retained cat PTYs, not native provider startup or identity",
            "Bitmap/OCR and PTY echo, not key-to-glyph presentation",
        ],
    }


def _comparison(current: dict, baseline: dict) -> dict:
    # Different source builds are the purpose of comparison; different measurement
    # contracts, host or population would make the latency delta misleading.
    for key in ["host", "build_mode", "command", "measurement_source"]:
        if current["metadata"][key] != baseline["metadata"][key]:
            return {"available": False, "reason": f"Different {key}"}
    for key in [
        "population_version",
        "populations",
        "scenarios",
        "endpoint",
        "poll_interval_ms",
        "soak_seconds",
    ]:
        if (current.get("plan") or {}).get(key) != (baseline.get("plan") or {}).get(key):
            return {"available": False, "reason": f"Different {key}"}
    if current["status"] != "complete" or baseline["status"] != "complete":
        return {"available": False, "reason": "Both runs need complete, source-stable observations"}
    previous = {
        (g["metric"], g["scenario"], g["population"], g["state"]): g for g in baseline["groups"]
    }
    deltas = []
    for group in current["groups"]:
        key = (group["metric"], group["scenario"], group["population"], group["state"])
        before = previous.get(key)
        if before is None:
            return {"available": False, "reason": "Different scenario groups"}
        deltas.append(
            {
                **dict(zip(["metric", "scenario", "population", "state"], key)),
                "p50_delta_ms": group["p50_ms"] - before["p50_ms"],
                "p95_delta_ms": (group["p95_ms"] - before["p95_ms"])
                if group["p95_ms"] is not None and before["p95_ms"] is not None
                else None,
            }
        )
    return {
        "available": True,
        "baseline_source": baseline["metadata"]["source_before"],
        "deltas": deltas,
    }


def _report(output: Path, baseline: Path | None) -> dict:
    metadata = json.loads((output / "run.json").read_text())
    events, errors = _read_events(output / "attempts.jsonl")
    summary = _summarize(events)
    errors.extend(summary["journal_errors"])
    complete = (
        metadata.get("exit_code") == 0
        and metadata.get("source_before") == metadata.get("source_after")
        and summary["expected_attempts"] is not None
        and summary["not_started"] == 0
        and summary["soak"]["status"] != "incomplete"
        and not summary["journey_errors"]
        and not errors
        and all(attempt["outcome"] == "passed" for attempt in summary["attempts"])
    )
    summary.update(
        metadata=metadata, status="complete" if complete else "incomplete", journal_errors=errors
    )
    recording = output / "soak-resources" / "report.json"
    summary["soak"]["resources"] = json.loads(recording.read_text()) if recording.exists() else None
    summary["soak"]["memory_after_four_rounds_mib"] = None
    rss_path = output / "soak-resources" / "rss.jsonl"
    if rss_path.exists() and len(summary["soak"]["rounds"]) >= 4:
        rss = [json.loads(line) for line in rss_path.read_text().splitlines()]
        boundary = summary["soak"]["rounds"][3]["time"]
        after = next((row for row in rss if row["t"] >= boundary), None)
        if rss and after and rss[0]["t"] < boundary:
            summary["soak"]["memory_after_four_rounds_mib"] = (
                after["rss_kib"] - rss[0]["rss_kib"]
            ) / 1024
    if baseline:
        summary["comparison"] = _comparison(
            summary, json.loads((baseline / "report.json").read_text())
        )
    _write(output / "report.json", summary)
    lines = [
        f"# Desktop measurements: {summary['status']}",
        "",
        "Endpoint: in-process native bitmap capture with text verification; "
        "Session return also waits for owned PTY replies.",
        "**Not compositor paint time. Frame-hitch evidence is unavailable.**",
        "",
        "Fixture data excludes CLI/registry discovery, network and provider startup. "
        "Three retained cat PTYs per population.",
        f"Attempts: {len(summary['attempts'])}/{summary['expected_attempts']}; "
        f"not started: {summary['not_started']}.",
        "First interaction and warm samples are separate. "
        "p95 requires 20 successful samples. No budget is scored.",
        "",
        "| Population | Scenario | State | Pass/attempt | p50 ms | p95 ms |",
        "|---|---|---|---:|---:|---:|",
    ]
    for group in summary["groups"]:
        p50 = f"{group['p50_ms']:.2f}" if group["p50_ms"] is not None else "—"
        p95 = f"{group['p95_ms']:.2f}" if group["p95_ms"] is not None else "—"
        lines.append(
            f"| {group['population']} | {group['scenario']} | {group['state']} | "
            f"{group['passed']}/{group['attempted']} | {p50} | {p95} |"
        )
    if baseline:
        comparison = summary["comparison"]
        lines += ["", "## Comparison", ""]
        if not comparison["available"]:
            lines.append(comparison["reason"])
        else:
            lines += [
                "Positive deltas are slower. These observations do not establish causality.",
                "",
                "| Population | Scenario | State | Δ p50 ms | Δ p95 ms |",
                "|---|---|---|---:|---:|",
            ]
            for delta in comparison["deltas"]:
                p95 = f"{delta['p95_delta_ms']:+.2f}" if delta["p95_delta_ms"] is not None else "—"
                lines.append(
                    f"| {delta['population']} | {delta['scenario']} | {delta['state']} | "
                    f"{delta['p50_delta_ms']:+.2f} | {p95} |"
                )
    if summary["soak"]["requested_seconds"]:
        soak = summary["soak"]
        lines += [
            "",
            f"Soak: {soak['status']}; requested {soak['requested_seconds']} s; "
            f"{len(soak['rounds'])} preserved rounds.",
            f"RSS growth after four rounds: {soak['memory_after_four_rounds_mib']} MiB.",
            "CPU, RSS, signposts and trace availability: soak-resources/report.json. "
            "Missing recording is unmeasured. "
            "Idle-only trace coverage is reported separately; partial coverage is not acceptance.",
        ]
    if not complete:
        lines += [
            "",
            "See run.json, attempts.jsonl and native.log for failed, interrupted "
            "or unavailable observations.",
        ]
        lines.extend(f"- {error}" for error in errors)
    (output / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return summary


def _stop(process: subprocess.Popen) -> None:
    if process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()


def _run(output: Path, samples: int, baseline: Path | None, soak_seconds: int = 0) -> int:
    output.mkdir(parents=True, exist_ok=False)
    metadata = {
        "schema": 1,
        "started_at": datetime.now(timezone.utc).isoformat(),
        "host": {"name": platform.node(), "os": platform.platform(), "arch": platform.machine()},
        "build_mode": "SwiftPM debug -gnone",
        "command": COMMAND,
        "source_before": _sources(),
        "measurement_source": hashlib.sha256(
            b"".join(
                (REPO / path).read_bytes()
                for path in [
                    "swift/LoopflowTests/DesktopPerformanceTests.swift",
                    "swift/LoopflowTests/SessionActionFixtures.swift",
                    "tests/fixtures/dto/roadmap_snapshot.json",
                    "tests/fixtures/dto/session_actions.json",
                    "tests/fixtures/dto/session_history_summary.json",
                    "tests/fixtures/dto/task_work.json",
                    "tests/fixtures/dto/task_comments.json",
                    "tests/fixtures/dto/context_report.json",
                    "tests/fixtures/dto/flow_catalog.json",
                    "scripts/desktop_performance.py",
                    "scripts/benchmarks/desktop-performance/record_live.py",
                ]
            )
        ).hexdigest(),
        "population_source": "isolated DTO fixtures; no configured Home",
    }
    _write(output / "run.json", metadata)
    if sys.platform != "darwin":
        metadata.update(
            outcome="unavailable", reason="Native benchmark requires macOS", exit_code=None
        )
    else:
        environment = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
        isolated = output / "home"
        isolated.mkdir()
        environment.update(LF_HOME=str(isolated), HOME=str(isolated))
        environment.update(
            LOOPFLOW_NATIVE_TESTS="1",
            LF_DESKTOP_PERF_OUTPUT=str(output / "attempts.jsonl"),
            LF_DESKTOP_PERF_SAMPLES=str(samples),
            LF_DESKTOP_PERF_SOAK_SECONDS=str(soak_seconds),
        )
        # These are host-boundary time limits, never latency targets. The attempt
        # journal survives termination; only this invocation's process group stops.
        with (output / "native.log").open("w") as log:
            try:
                process = subprocess.Popen(
                    COMMAND,
                    cwd=REPO,
                    env=environment,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    start_new_session=True,
                )
            except OSError as error:
                metadata.update(outcome="unavailable", reason=str(error), exit_code=None)
                process = None
            if process is not None:
                try:
                    deadline = time.monotonic() + 600 + samples * 120 + soak_seconds
                    recorder = None
                    recorder_attempted = False
                    while process.poll() is None:
                        if time.monotonic() >= deadline:
                            raise subprocess.TimeoutExpired(COMMAND, deadline)
                        if soak_seconds and not recorder_attempted:
                            events, _ = _read_events(output / "attempts.jsonl")
                            began = next((e for e in events if e["event"] == "soak_begin"), None)
                            if began:
                                recorder_attempted = True
                                try:
                                    recorder = subprocess.Popen(
                                        [
                                            sys.executable,
                                            str(RECORDER),
                                            "record",
                                            "--pid",
                                            str(began["pid"]),
                                            "--seconds",
                                            str(soak_seconds),
                                            "--output",
                                            str(output / "soak-resources"),
                                            "--phases",
                                            str(output / "attempts.jsonl"),
                                        ],
                                        stdout=log,
                                        stderr=subprocess.STDOUT,
                                        start_new_session=True,
                                    )
                                except OSError as error:
                                    metadata["recorder_error"] = str(error)
                        if recorder is not None and recorder.poll() is not None:
                            (output / "resources-finished").touch(exist_ok=True)
                        elif recorder_attempted and recorder is None:
                            (output / "resources-finished").touch(exist_ok=True)
                        time.sleep(0.25)
                    metadata["exit_code"] = process.returncode
                    if recorder is not None:
                        try:
                            metadata["recorder_exit_code"] = recorder.wait(timeout=30)
                        except subprocess.TimeoutExpired:
                            _stop(recorder)
                            metadata["recorder_exit_code"] = recorder.returncode
                except (subprocess.TimeoutExpired, KeyboardInterrupt) as error:
                    metadata["outcome"] = (
                        "timeout" if isinstance(error, subprocess.TimeoutExpired) else "interrupted"
                    )
                    _stop(process)
                    metadata["exit_code"] = process.returncode
                finally:
                    if recorder is not None:
                        _stop(recorder)
    metadata["source_after"] = _sources()
    _write(output / "run.json", metadata)
    summary = _report(output, baseline)
    print(f"{summary['status']}: {output / 'report.md'}")
    return 0 if summary["status"] == "complete" else 1


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    commands = parser.add_subparsers(dest="command", required=True)
    run = commands.add_parser(
        "run", help="Run both native journeys against fixed small and large populations"
    )
    run.add_argument(
        "--output", type=Path, required=True, help="New directory; never overwrites evidence"
    )
    run.add_argument(
        "--samples",
        type=int,
        default=21,
        help="Attempts per scenario/population; default 1 first + 20 warm",
    )
    run.add_argument("--baseline", type=Path)
    run.add_argument(
        "--soak-seconds",
        type=int,
        default=0,
        help="Append unattended idle/navigation/typing; 3600 for the acceptance soak",
    )
    report = commands.add_parser(
        "report", help="Rebuild reports, including interrupted invocation evidence"
    )
    report.add_argument("output", type=Path)
    report.add_argument("--baseline", type=Path)
    args = parser.parse_args()
    if args.command == "run":
        if args.soak_seconds < 0:
            parser.error("--soak-seconds cannot be negative")
        if args.samples < 1:
            parser.error("--samples must be positive")
        return _run(args.output.resolve(), args.samples, args.baseline, args.soak_seconds)
    summary = _report(args.output, args.baseline)
    print(args.output / "report.md")
    return 0 if summary["status"] == "complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())
