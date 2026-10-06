#!/usr/bin/env python3
"""Measure native desktop journeys and compare like-for-like observations.

uv run python scripts/desktop_performance.py run --output /tmp/desktop-baseline
uv run python scripts/desktop_performance.py run --output /tmp/after --baseline /tmp/baseline

# A commit from another process, to the row rendered, on a private copy of a Home.
uv run python scripts/desktop_performance.py write-visible \\
    --home /tmp/desktop-launch/home --repo ~/src/loopflow --output /tmp/write-visible
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import shutil
import signal
import sqlite3
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from types import FrameType
from typing import TextIO

REPO = Path(__file__).resolve().parent.parent
BUILD_COMMAND = [
    "swift",
    "build",
    "--package-path",
    "swift",
    "--build-tests",
    "-Xswiftc",
    "-gnone",
    "--jobs",
    "4",
]
ENDPOINTS = {
    "snapshot": [
        "Endpoint: in-process native bitmap capture with text verification.",
        "**Not compositor paint time. Frame-hitch evidence is unavailable.**",
        "",
        "Production workspace, router and local CLI reads over an isolated SQLite snapshot. "
        "Copied provider connections are refused. "
        "Cold workspace construction is not OS app launch. "
        "Session input readiness is unmeasured. "
        "Key-window and intermediate-sheet observations remain in each attempt.",
    ],
    "run": [
        "Endpoint: in-process native bitmap capture with text verification; "
        "Session return also waits for owned PTY replies.",
        "**Not compositor paint time. Frame-hitch evidence is unavailable.**",
        "",
        "Fixture data excludes CLI/registry discovery, network and provider startup. "
        "Three retained cat PTYs per population.",
    ],
    "write-visible": [
        "Endpoint: another process's commit to a private Home's store, through the real "
        "`lf monitor work --watch` reader, to native bitmap capture with text verification.",
        "**Not compositor paint time. Frame-hitch evidence is unavailable.**",
        "",
        "The interval starts when the writing process has exited. Rows are written with "
        "sqlite3, so no Exec, sync or provider is involved. Each write waits for an idle reader.",
    ],
}


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
        "journey_errors": [
            event for event in events if event["event"] in {"setup", "setup_or_journey"}
        ],
    }


def _comparison(current: dict, baseline: dict) -> dict:
    # Different source builds are the purpose of comparison; different measurement
    # contracts, host or population would make the latency delta misleading.
    for key in [
        "host",
        "build_mode",
        "command",
        "measurement_source",
        "snapshot",
        "repo",
        "issue",
        "journey",
        "inputs",
    ]:
        if current["metadata"].get(key) != baseline["metadata"].get(key):
            return {"available": False, "reason": f"Different {key}"}
    for key in ["population_version", "populations", "scenarios", "endpoint", "poll_interval_ms"]:
        if (current.get("plan") or {}).get(key) != (baseline.get("plan") or {}).get(key):
            return {"available": False, "reason": f"Different {key}"}
    for run in [current, baseline]:
        metadata = run["metadata"]
        if (
            metadata.get("exit_code") != 0
            or metadata["source_before"] != metadata["source_after"]
            or run["not_started"] != 0
            or run["journal_errors"]
            or any(attempt["outcome"] == "interrupted" for attempt in run["attempts"])
        ):
            return {
                "available": False,
                "reason": "Both runs need complete, source-stable observations",
            }
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
                "p50_delta_ms": group["p50_ms"] - before["p50_ms"]
                if group["p50_ms"] is not None and before["p50_ms"] is not None
                else None,
                "before_failure_rate": before["failure_rate"],
                "after_failure_rate": group["failure_rate"],
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
        and not summary["journey_errors"]
        and not errors
        and all(attempt["outcome"] == "passed" for attempt in summary["attempts"])
    )
    summary.update(
        metadata=metadata, status="complete" if complete else "incomplete", journal_errors=errors
    )
    if baseline:
        summary["comparison"] = _comparison(
            summary, json.loads((baseline / "report.json").read_text())
        )
    _write(output / "report.json", summary)
    snapshot_run = metadata.get("snapshot") is not None
    lines = [
        f"# Desktop measurements: {summary['status']}",
        "",
        *(ENDPOINTS["snapshot"] if snapshot_run else ENDPOINTS[metadata.get("journey", "run")]),
        f"Attempts: {len(summary['attempts'])}/{summary['expected_attempts']}; "
        f"not started: {summary['not_started']}.",
        "First interaction and warm samples are separate. "
        "p95 requires 20 successful samples. No budget is scored.",
        *(
            ["Targets, not gates: a created Task within 1000 ms p95, a Session row within 500 ms."]
            if metadata.get("journey") == "write-visible"
            else []
        ),
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
                p50 = f"{delta['p50_delta_ms']:+.2f}" if delta["p50_delta_ms"] is not None else "—"
                lines.append(
                    f"| {delta['population']} | {delta['scenario']} | {delta['state']} | "
                    f"{p50} | {p95} |"
                )
    if not complete:
        lines += [
            "",
            "See run.json, attempts.jsonl and native.log for failed, interrupted "
            "or unavailable observations.",
        ]
        lines.extend(f"- {error}" for error in errors)
    (output / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return summary


def _snapshot(source: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False, mode=0o700)
    database = output / "loopflow.db"
    with sqlite3.connect(source.resolve().as_uri() + "?mode=ro", uri=True) as source_db:
        with sqlite3.connect(database) as target:
            source_db.backup(target)
    database.chmod(0o600)
    with sqlite3.connect(database) as target:
        tables = [
            row[0] for row in target.execute("SELECT name FROM sqlite_master WHERE type='table'")
        ]
        counts = {
            name: target.execute(
                'SELECT count(*) FROM "' + name.replace('"', '""') + '"'
            ).fetchone()[0]
            for name in tables
        }
    _write(
        output / "snapshot.json",
        {
            "captured_at": datetime.now(timezone.utc).isoformat(),
            "sha256": hashlib.sha256(database.read_bytes()).hexdigest(),
            "counts": counts,
        },
    )


def _native_command(test_filter: str, environment: dict[str, str]) -> list[str]:
    swift = Path(subprocess.check_output(["xcrun", "--find", "swift"], text=True).strip())
    platform_path = Path(
        subprocess.check_output(
            ["xcrun", "--sdk", "macosx", "--show-sdk-platform-path"], text=True
        ).strip()
    )
    developer = platform_path / "Developer"
    environment["DYLD_FRAMEWORK_PATH"] = ":".join(
        str(developer / "Library" / name) for name in ["Frameworks", "PrivateFrameworks"]
    )
    environment["DYLD_LIBRARY_PATH"] = str(developer / "usr/lib")
    return [
        str(swift.parent.parent / "libexec/swift/pm/swiftpm-testing-helper"),
        "--test-bundle-path",
        str(
            REPO
            / "swift/.build/debug/LoopflowSwiftPackageTests.xctest"
            / "Contents/MacOS/LoopflowSwiftPackageTests"
        ),
        "--testing-library",
        "swift-testing",
        "--filter",
        test_filter,
    ]


def _run_process(
    command: list[str], environment: dict[str, str], log: TextIO, timeout: float
) -> tuple[int, str | None]:
    interrupted = False

    def _terminate(signum: int, frame: FrameType | None) -> None:
        nonlocal interrupted
        interrupted = True

    previous = signal.signal(signal.SIGTERM, _terminate)
    try:
        process = subprocess.Popen(
            command,
            cwd=REPO,
            env=environment,
            stdout=log,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        deadline = time.monotonic() + timeout
        outcome = None
        try:
            while not interrupted:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    outcome = "timeout"
                    break
                try:
                    process.wait(timeout=min(remaining, 0.1))
                    break
                except subprocess.TimeoutExpired:
                    pass
        except KeyboardInterrupt:
            interrupted = True
        if interrupted:
            outcome = "interrupted"
        if outcome:
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass
            # The leader can exit before its children. Stop the remaining group
            # even when wait() has already returned, so the journal stays frozen.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
        return process.returncode, outcome
    finally:
        signal.signal(signal.SIGTERM, previous)


def _private_home(home: Path, lf: Path) -> Path:
    """The store the benchmark writes rows into: never the one in use."""
    home = home.resolve()
    live = Path(os.environ.get("LF_HOME") or Path.home() / ".lf").resolve()
    if home == live:
        raise SystemExit(
            f"{home} is the Home in use. Copy it first: "
            "scripts/benchmarks/desktop-performance/launch.py writes <work>/home."
        )
    if not (home / "loopflow.db").exists():
        raise SystemExit(f"{home} has no loopflow.db")
    if not lf.exists():
        raise SystemExit(f"{lf} does not exist; build it or pass --lf")
    # The copy must already have the schema this lf reads; say so before a
    # five-minute wait for a reading that cannot come.
    environment = {k: v for k, v in os.environ.items() if not k.startswith(("LF_", "LOOPFLOW_"))}
    probe = subprocess.run(
        [str(lf), "wave", "list", "--json"],
        cwd=home,
        env={**environment, "LF_HOME": str(home)},
        capture_output=True,
        text=True,
    )
    if probe.returncode != 0:
        raise SystemExit(f"{lf} cannot read {home}:\n{probe.stderr.strip()}")
    return home


def _run(
    output: Path,
    samples: int,
    baseline: Path | None,
    snapshot: Path | None = None,
    binary: Path | None = None,
    repo: Path | None = None,
    issue: str | None = None,
    journey: str = "run",
    extra: dict[str, str] | None = None,
) -> int:
    output.mkdir(parents=True, exist_ok=False)
    extra = extra or {}
    test_filter = (
        "DesktopPerformanceTests.measureSnapshotTaskOpening"
        if snapshot
        else "DesktopPerformanceTests.measureWriteToVisible"
        if journey == "write-visible"
        else "DesktopPerformanceTests.measureExperiences"
    )
    metadata = {
        "schema": 1,
        "journey": journey,
        "inputs": extra,
        "started_at": datetime.now(timezone.utc).isoformat(),
        "host": {"name": platform.node(), "os": platform.platform(), "arch": platform.machine()},
        "build_mode": "SwiftPM debug -gnone",
        "build_command": BUILD_COMMAND,
        "command": None,
        "source_before": _sources(),
        "measurement_source": hashlib.sha256(
            b"".join(
                (REPO / path).read_bytes()
                for path in [
                    "swift/LoopflowTests/DesktopPerformanceTests.swift",
                    "swift/LoopflowTests/SessionActionFixtures.swift",
                    "tests/fixtures/dto/roadmap_snapshot.json",
                ]
            )
        ).hexdigest(),
        "population_source": "isolated DTO fixtures; no configured Home"
        if journey == "run"
        else "a private copy of a Home; the rows written are removed after each attempt",
        "resources_before": {
            "load_average": os.getloadavg(),
            "free_bytes": shutil.disk_usage(output).free,
        },
    }
    if snapshot:
        manifest = json.loads((snapshot / "snapshot.json").read_text())
        database = snapshot / "loopflow.db"
        if hashlib.sha256(database.read_bytes()).hexdigest() != manifest["sha256"]:
            raise ValueError("Snapshot changed since capture; create a new consistent snapshot")
        home = output / "home"
        home.mkdir(mode=0o700)
        shutil.copyfile(database, home / "loopflow.db")
        (home / "loopflow.db").chmod(0o600)
        metadata.update(
            population_source="isolated SQLite online backup",
            snapshot=manifest,
            binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
            repo=str(repo),
            issue=issue,
        )
    _write(output / "run.json", metadata)
    if sys.platform != "darwin":
        metadata.update(
            outcome="unavailable", reason="Native benchmark requires macOS", exit_code=None
        )
    else:
        environment = os.environ.copy()
        journal = "LF_DESKTOP_PERF_OUTPUT" if journey == "run" else "LF_DESKTOP_PERF_WRITE_OUTPUT"
        environment.pop("LF_DESKTOP_PERF_OUTPUT", None)
        environment.pop("LF_DESKTOP_PERF_WRITE_OUTPUT", None)
        environment.update(
            {journal: str(output / "attempts.jsonl")},
            LOOPFLOW_NATIVE_TESTS="1",
            LF_DESKTOP_PERF_SAMPLES=str(samples),
            **extra,
        )
        if snapshot:
            environment.update(
                LOOPFLOW_UI_TEST_MODE="live",
                LF_DESKTOP_TASK_SNAPSHOT=str(home),
                LF_DESKTOP_TASK_BINARY=str(binary),
                LF_DESKTOP_TASK_REPO=str(repo),
                LF_DESKTOP_TASK_ISSUE=issue,
            )
        # These are host-boundary time limits, never latency targets. The attempt
        # journal survives termination; only this invocation's process group stops.
        with (output / "native.log").open("w") as log:
            try:
                # SwiftPM can put its test child in a separate process group.
                # Both journeys build first, then own the native process directly.
                code, outcome = _run_process(BUILD_COMMAND, environment, log, timeout=600)
                if outcome:
                    metadata.update(outcome=outcome, exit_code=code)
                if code != 0 or outcome:
                    raise subprocess.CalledProcessError(code, BUILD_COMMAND)
                command = _native_command(test_filter, environment)
                metadata["command"] = command
                bundle = Path(command[2])
                metadata["test_binary_sha256"] = hashlib.sha256(bundle.read_bytes()).hexdigest()
                code, outcome = _run_process(
                    command,
                    environment,
                    log,
                    timeout=600 + samples * (150 if snapshot else 30),
                )
                metadata["exit_code"] = code
                if outcome:
                    metadata["outcome"] = outcome
            except (OSError, subprocess.SubprocessError) as error:
                metadata.setdefault("outcome", "unavailable")
                metadata.setdefault("exit_code", None)
                metadata["reason"] = str(error)
    metadata["source_after"] = _sources()
    metadata["resources_after"] = {
        "load_average": os.getloadavg(),
        "free_bytes": shutil.disk_usage(output).free,
    }
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
    run.add_argument("--snapshot", type=Path, help="Home snapshot created by the snapshot command")
    run.add_argument(
        "--lf", dest="binary", type=Path, help="Exact lf executable for snapshot reads"
    )
    run.add_argument("--repo", type=Path)
    run.add_argument("--issue", help="Task deep-link identifier, e.g. LOO-368")
    snapshot_command = commands.add_parser(
        "snapshot", help="Consistent private SQLite backup; preserves all rows"
    )
    snapshot_command.add_argument("--database", type=Path, required=True)
    snapshot_command.add_argument("--output", type=Path, required=True)
    written = commands.add_parser(
        "write-visible",
        help="Commit from another process to a private Home and time the row appearing",
    )
    written.add_argument(
        "--output", type=Path, required=True, help="New directory; never overwrites evidence"
    )
    written.add_argument(
        "--home", type=Path, required=True, help="A private copy of a Home; rows are written to it"
    )
    written.add_argument(
        "--repo", type=Path, required=True, help="Repository whose Waves the window shows"
    )
    written.add_argument(
        "--lf",
        type=Path,
        default=REPO / "target/release/lf",
        help="The lf whose reader is measured; its schema must match the copy",
    )
    written.add_argument(
        "--samples", type=int, default=21, help="Attempts per write; default 1 first + 20 warm"
    )
    written.add_argument("--baseline", type=Path)
    report = commands.add_parser(
        "report", help="Rebuild reports, including interrupted invocation evidence"
    )
    report.add_argument("output", type=Path)
    report.add_argument("--baseline", type=Path)
    args = parser.parse_args()
    if args.command == "snapshot":
        _snapshot(args.database, args.output)
        print(args.output / "snapshot.json")
        return 0
    if args.command != "report" and args.samples < 1:
        parser.error("--samples must be positive")
    if args.command == "run":
        if args.samples < 1:
            parser.error("--samples must be positive")
        if args.snapshot and not all([args.binary, args.repo, args.issue]):
            parser.error("--snapshot requires --lf, --repo and --issue")
        return _run(
            args.output.resolve(),
            args.samples,
            args.baseline,
            args.snapshot.resolve() if args.snapshot else None,
            args.binary.resolve() if args.binary else None,
            args.repo.resolve() if args.repo else None,
            args.issue,
        )
    if args.command == "write-visible":
        lf = args.lf.resolve()
        return _run(
            args.output.resolve(),
            args.samples,
            args.baseline,
            journey="write-visible",
            extra={
                "LF_DESKTOP_PERF_LF": str(lf),
                "LF_DESKTOP_PERF_HOME": str(_private_home(args.home, lf)),
                "LF_DESKTOP_PERF_REPO": str(args.repo.expanduser().resolve()),
            },
        )
    summary = _report(args.output, args.baseline)
    print(args.output / "report.md")
    return 0 if summary["status"] == "complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())
