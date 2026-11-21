#!/usr/bin/env python3
"""Measure native desktop journeys and compare like-for-like observations.

uv run python scripts/desktop_performance.py run --output /tmp/before
uv run python scripts/desktop_performance.py report /tmp/after --baseline /tmp/before
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import selectors
import shutil
import signal
import sqlite3
import statistics
import subprocess
import sys
import tempfile
import time
from collections import Counter, defaultdict
from contextlib import ExitStack, contextmanager
from datetime import datetime, timezone
from pathlib import Path
from types import FrameType
from typing import Callable, TextIO

REPO = Path(__file__).resolve().parent.parent
RECORDER = REPO / "scripts/benchmarks/desktop-performance/record_live.py"
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
# Optimized comparisons retain testing support for the @testable imports.
OPTIMIZED_FLAGS = ["-c", "release", "-Xswiftc", "-enable-testing"]


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
            "scripts/desktop-performance.sb",
            "scripts/benchmarks/desktop-performance/record_live.py",
            "scripts/benchmarks/desktop-performance/claude",
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
    attempts = [
        ends.get(identity, {**start, "outcome": "interrupted", "duration_ms": None})
        for identity, start in starts.items()
    ]
    groups: dict[tuple, list[dict]] = defaultdict(list)
    for attempt in attempts:
        key = tuple(attempt[field] for field in ["metric", "scenario", "population", "state"])
        groups[key].append(attempt)
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
        "read_failures": [
            event
            for event in events
            if event["event"] == "read" and event.get("outcome") != "passed"
        ],
        "journey_errors": [
            event for event in events if event["event"] in {"setup", "setup_or_journey"}
        ],
    }


def _soak(events: list[dict], plan: dict | None) -> dict:
    seconds = (plan or {}).get("soak_seconds", 0)
    by_kind: dict[str, list[dict]] = defaultdict(list)
    for event in events:
        if event["event"].startswith("soak_"):
            by_kind[event["event"]].append(event)
    starts, ends, rounds = (by_kind[kind] for kind in ("soak_begin", "soak_end", "soak_round"))
    repeated_starts, repeated_ends = by_kind["soak_sample_begin"], by_kind["soak_sample_end"]
    complete = (
        len(repeated_starts) == len(repeated_ends)
        and {event["id"] for event in repeated_starts} == {event["id"] for event in repeated_ends}
        and all(event["outcome"] == "passed" for event in repeated_ends)
        and len(starts) == len(ends) == 1
        and ends[0]["elapsed_seconds"] >= seconds
        and ends[0]["preserved"]
        and len(rounds) >= 4
        and [e["round"] for e in rounds] == list(range(len(rounds)))
        and all(e["preserved"] for e in rounds)
    )
    return {
        "requested_seconds": seconds,
        "status": "not_requested" if not seconds else "complete" if complete else "incomplete",
        "begin": starts[0] if len(starts) == 1 else None,
        "rounds": rounds,
        "reopen_observations": repeated_starts + repeated_ends,
        "phases": by_kind["soak_phase"],
        "end": ends[0] if len(ends) == 1 else None,
        "limits": [
            "Fixture mode uses synthetic planning; snapshot mode includes real CLI/SQLite volume",
            "Snapshot soak reopens an owned Task-bound stub alongside observational copied records",
            "Bitmap/OCR and PTY echo, not key-to-glyph presentation",
        ],
    }


def _cli_volume(directory: Path) -> dict:
    processes = []
    errors = []
    counters = ("connections", "statements", "rows")
    for path in sorted(directory.glob("lf-*.jsonl")):
        events, malformed = _read_events(path)
        errors.extend(f"{path.name}: {error}" for error in malformed)
        valid = []
        for event in events:
            if (
                not isinstance(event, dict)
                or event.get("event") not in {"start", "sample", "end"}
                or any(type(event.get(key)) is not int or event[key] < 0 for key in counters)
                or not isinstance(event.get("pid"), int)
                or any(
                    not isinstance(event.get(key), (int, float)) or not math.isfinite(event[key])
                    for key in ("time", "elapsed_ms")
                )
            ):
                errors.append(f"{path.name}: invalid volume record")
                break
            if valid and (
                event["pid"] != valid[0]["pid"]
                or event["event"] == "start"
                or valid[-1]["event"] == "end"
                or event["elapsed_ms"] < valid[-1]["elapsed_ms"]
                or any(event[key] < valid[-1][key] for key in counters)
            ):
                errors.append(f"{path.name}: inconsistent volume sequence")
                break
            valid.append(event)
        if not valid or valid[0]["event"] != "start":
            errors.append(f"{path.name}: missing process start")
            continue
        processes.append(
            {
                "receipt": path.name,
                "pid": valid[0]["pid"],
                "complete": not malformed
                and len(valid) == len(events)
                and valid[-1]["event"] == "end",
                "samples": valid,
            }
        )
    complete = bool(processes) and not errors and all(p["complete"] for p in processes)
    return {
        "status": "complete" if complete else "partial" if processes or errors else "unmeasured",
        "processes_started": len(processes) if processes else None,
        "processes_ended": sum(p["complete"] for p in processes) if processes else None,
        "observed_totals": {
            key: sum(p["samples"][-1][key] for p in processes) if processes else None
            for key in counters
        },
        "sql_profile_ms": sum(p["samples"][-1]["sql_ms"] for p in processes)
        if processes
        and all(
            isinstance(p["samples"][-1].get("sql_ms"), (int, float))
            and math.isfinite(p["samples"][-1]["sql_ms"])
            for p in processes
        )
        else None,
        "processes": processes,
        "errors": errors,
        "limits": [
            "Only instrumented lf processes; excludes Git, providers and other executables",
            "SQLite statement starts (including PRAGMAs, writes and triggers), not only SELECTs",
            "Rows emitted by SQLite, not underlying rows scanned; main store connections only",
            "Cumulative one-second samples; missing end means counts are lower bounds",
            "Counters and a writer add overhead; use the same instrumentation in both builds",
        ],
    }


def _has_complete_observations(summary: dict) -> bool:
    metadata = summary["metadata"]
    return (
        metadata.get("exit_code") == 0
        and metadata.get("outcome") not in {"failed", "timeout", "interrupted"}
        and metadata.get("source_before") == metadata.get("source_after")
        and metadata.get("cli_sha256") == metadata.get("cli_sha256_after")
        and summary["expected_attempts"] is not None
        and summary["not_started"] == 0
        and summary["soak"]["status"] != "incomplete"
        and metadata.get("recorder_exit_code", 0) == 0
        and (
            not metadata.get("xctrace", False)
            or summary["soak"]["status"] == "not_requested"
            or (summary["soak"].get("trace_coverage") or {}).get("status") == "complete"
        )
        and metadata.get("repository_configs_unchanged", True)
        and metadata.get("repository_inputs_unchanged", True)
        and metadata.get("git_observations_complete", True)
        and not summary["journey_errors"]
        and not summary["journal_errors"]
        and all(attempt["outcome"] != "interrupted" for attempt in summary["attempts"])
    )


def _comparison(current: dict, baseline: dict) -> dict:
    # Different source builds are the purpose of comparison; different measurement
    # contracts, host or population would make the latency delta misleading.
    for key in [
        "host",
        "build_mode",
        "command",
        "measurement_source",
        "journey",
        "inputs",
        "xctrace",
        "snapshot",
        "repo",
        "issue",
        "repository_configs",
        "repository_inputs",
    ]:
        if current["metadata"].get(key) != baseline["metadata"].get(key):
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
    if not all(_has_complete_observations(run) for run in (current, baseline)):
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


def _soak_trace_coverage(soak: dict) -> dict:
    result = {"status": "unmeasured", "start_gap_seconds": None, "end_gap_seconds": None}
    resources = soak.get("resources") or {}
    bounds = resources.get("trace_bounds")
    start = (soak.get("begin") or {}).get("time")
    end = (soak.get("end") or {}).get("time")
    if not bounds or start is None or end is None or end <= start:
        return result
    start_gap = max(0, bounds[0] - start)
    end_gap = max(0, end - bounds[1])
    hitches = resources.get("hitches") or {}
    complete = (
        start_gap == 0
        and end_gap == 0
        and resources.get("run", {}).get("xctrace") == "recorded"
        and hitches.get("recorded") is True
        and hitches.get("hangs") is not None
    )
    return {
        "status": "complete" if complete else "incomplete",
        "start_gap_seconds": start_gap,
        "end_gap_seconds": end_gap,
    }


def _report(output: Path, baseline: Path | None) -> dict:
    metadata = json.loads((output / "run.json").read_text())
    events, errors = _read_events(output / "attempts.jsonl")
    summary = _summarize(events)
    errors.extend(summary["journal_errors"])
    if metadata.get("soak_seconds", 0):
        summary["soak"] = _soak(events, {"soak_seconds": metadata["soak_seconds"]})
    summary.update(metadata=metadata, journal_errors=errors)
    summary["cli_volume"] = _cli_volume(output / "cli-volume")
    summary["fixture_setup_cli_volume"] = _cli_volume(output / "fixture-setup-cli-volume")
    recording = output / "soak-resources" / "report.json"
    summary["soak"]["resources"] = json.loads(recording.read_text()) if recording.exists() else None
    summary["soak"]["memory_after_four_rounds_mib"] = None
    soak = summary["soak"]
    if len(soak["rounds"]) >= 4:
        before = (soak["begin"] or {}).get("rss_bytes")
        after = soak["rounds"][3].get("rss_bytes")
        if before is not None and after is not None:
            soak["memory_after_four_rounds_mib"] = (after - before) / (1024 * 1024)
    soak["trace_coverage"] = _soak_trace_coverage(soak)
    # Observed failures remain comparable, but cannot complete the journey.
    complete = _has_complete_observations(summary) and all(
        attempt["outcome"] == "passed" for attempt in summary["attempts"]
    )
    summary["status"] = "complete" if complete else "incomplete"
    if baseline:
        summary["comparison"] = _comparison(
            summary, json.loads((baseline / "report.json").read_text())
        )
    _write(output / "report.json", summary)
    (output / "report.md").write_text(
        _markdown(summary, scoped=(output / "fixture-setup-cli-volume").is_dir()),
        encoding="utf-8",
    )
    return summary


def _markdown(summary: dict, *, scoped: bool) -> str:
    metadata = summary["metadata"]
    snapshot_run = metadata.get("snapshot") is not None
    lines = [
        f"# Desktop measurements: {summary['status']}",
        "",
        *(
            [
                "Endpoint: another process's commit to a private Machine, through the real work "
                "reader, to native bitmap capture with text verification.",
                "**Not compositor paint time. Frame-hitch evidence is unavailable.**",
                "The interval starts when the sqlite3 writer exits; "
                "each write waits for an idle reader.",
            ]
            if metadata.get("journey") == "write-visible"
            else [
                "Endpoint: in-process native bitmap capture with text verification."
                + " Session return also waits for owned PTY replies.",
                "**Not compositor paint time. Soak trace evidence is reported separately.**",
                "",
                "Fixture data excludes CLI/registry discovery, network and provider startup. "
                "Three retained cat PTYs per population."
                if not snapshot_run
                else "Snapshot mode uses real CLI reads and Task links; "
                "copied Session connections are disabled; the same workspace reopens one owned "
                "Task-bound Session with a synthetic provider.",
            ]
        ),
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
    if failures := summary.get("read_failures"):
        lines += [
            "",
            f"Unavailable read observations: {len(failures)}. These remain acceptance gaps.",
        ]
        for reason, count in Counter(
            failure.get("reason", "unknown") for failure in failures
        ).items():
            lines.append(f"- {count} × {reason}")
    if comparison := summary.get("comparison"):
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
                p50 = f"{delta['p50_delta_ms']:+.2f}" if delta["p50_delta_ms"] is not None else "—"
                p95 = f"{delta['p95_delta_ms']:+.2f}" if delta["p95_delta_ms"] is not None else "—"
                p50 = f"{delta['p50_delta_ms']:+.2f}" if delta["p50_delta_ms"] is not None else "—"
                lines.append(
                    f"| {delta['population']} | {delta['scenario']} | {delta['state']} | "
                    f"{p50} | {p95} |"
                )
    if summary["soak"]["requested_seconds"]:
        soak = summary["soak"]
        lines += [
            "",
            f"Soak: {soak['status']}; requested {soak['requested_seconds']} s; "
            f"{len(soak['rounds'])} preserved rounds.",
            f"RSS growth after four rounds: {soak['memory_after_four_rounds_mib']} MiB.",
            f"Full-soak trace coverage: {soak['trace_coverage']['status']}; "
            f"opening gap {soak['trace_coverage']['start_gap_seconds']} s; "
            f"closing gap {soak['trace_coverage']['end_gap_seconds']} s.",
            "CPU, RSS, signposts and trace availability: soak-resources/report.json. "
            "Missing recording is unmeasured. "
            "Idle-only trace coverage is reported separately; partial coverage is not acceptance.",
        ]
    volume = summary["cli_volume"]
    scope = "Scenario" if scoped else "Unscoped"
    lines += [
        "",
        f"{scope} CLI volume: {volume['status']}; "
        f"processes started/ended {volume['processes_started']}/{volume['processes_ended']}; "
        f"observed SQLite totals {volume['observed_totals']}. "
        "Scope and partial receipts are retained in report.json.",
        (
            "Fixture setup is excluded and reported separately in fixture_setup_cli_volume. "
            "Scenario totals cover only instrumented commands actually executed."
            if scoped
            else "No separate setup receipts; setup/scenario attribution is unmeasured."
        ),
    ]
    if summary["status"] != "complete":
        lines += [
            "",
            "See run.json, attempts.jsonl and native.log for failed, interrupted "
            "or unavailable observations.",
        ]
        lines.extend(f"- {error}" for error in summary["journal_errors"])
    return "\n".join(lines) + "\n"


def _stop(process: subprocess.Popen) -> None:
    # Every caller creates an owned process group. Its leader can exit while
    # descendants still hold pipes or write receipts, so poll() cannot settle it.
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    process.wait()


def _benchmark_git() -> Path:
    if sys.platform == "darwin":
        return Path(subprocess.check_output(["xcrun", "--find", "git"], text=True).strip())
    return Path(shutil.which("git") or "/usr/bin/git")


def _fixture_cli(cli: Path, args: list[str], checkout: Path, environment: dict[str, str]) -> str:
    process = subprocess.Popen(
        [str(cli), *args],
        cwd=checkout,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    try:
        stdout, stderr = process.communicate(timeout=60)
    except BaseException as error:
        # Binding and discovery own their descendants just as resume does.
        _stop(process)
        if isinstance(error, subprocess.TimeoutExpired):
            raise RuntimeError(f"Native fixture {args[0]} timed out") from error
        raise
    if process.returncode:
        raise RuntimeError(f"Native fixture {args[0]} failed: {stderr}")
    return stdout


@contextmanager
def _fixture_server(cli: Path, repo: Path, environment: dict[str, str], prefix=()):
    with ExitStack() as owned:
        log = owned.enter_context((repo.parent / "server.log").open("a"))

        def start(command, stdout):
            process = subprocess.Popen(
                [*prefix, cli, *command],
                cwd=repo,
                env=environment,
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=log,
                start_new_session=True,
            )
            owned.callback(_stop, process)
            return process

        holder = start(["holder", "run"], subprocess.PIPE)
        ready = owned.enter_context(selectors.DefaultSelector())
        owned.callback(holder.stdout.close)
        ready.register(holder.stdout, selectors.EVENT_READ)
        if not ready.select(60) or not holder.stdout.readline():
            raise RuntimeError(f"Private terminal holder unavailable: {log.name}")
        server = start(["server", "run"], log)
        deadline = time.monotonic() + 60
        while True:
            status = subprocess.run(
                [cli, "server", "status"],
                cwd=repo,
                env=environment,
                stdin=subprocess.DEVNULL,
                capture_output=True,
                timeout=10,
            )
            if status.returncode == 0 and json.loads(status.stdout)["ready"]:
                break
            if server.poll() is not None or time.monotonic() >= deadline:
                raise RuntimeError(f"Private Machine server unavailable: {log.name}")
            time.sleep(0.05)
        yield


def _prepare_native_fixture(output: Path, cli: Path) -> Path:
    for directory in ("fixture-setup-cli-volume", "cli-volume"):
        (output / directory).mkdir()
    home = output.resolve() / "native-machine"
    repo = home / "repo"
    checkout = home / "checkout"
    git = _benchmark_git()
    project = (
        home
        / "claude/projects"
        / "".join(c if c.isascii() and c.isalnum() else "-" for c in str(checkout))
    )
    project.mkdir(parents=True)
    repo.mkdir()
    (home / "lf").mkdir(mode=0o700)
    (home / "bin").mkdir()
    provider = home / "bin/claude"
    shutil.copyfile(REPO / "scripts/benchmarks/desktop-performance/claude", provider)
    provider.chmod(0o755)
    environment = {
        "HOME": str(home),
        "LF_HOME": str(home / "lf"),
        "LF_BIN": str(cli),
        "CODEX_HOME": str(home / "codex"),
        "CLAUDE_CONFIG_DIR": str(home / "claude"),
        "BENCHMARK_PROJECT": str(project),
        "PATH": f"{home / 'bin'}:{git.parent}:/usr/bin:/bin",
        "TMPDIR": str(home),
        "LF_PERF_OUTPUT": str(output / "fixture-setup-cli-volume"),
        "RUST_LOG": "off",
    }
    (repo / ".lf").mkdir()
    (repo / ".lf/config.yaml").write_text("agent: claude\n")
    (repo / "README.md").write_text("Benchmark checkout\n")
    for args in (
        ["init", "-q", "-b", "main"],
        ["config", "user.name", "Desktop Benchmark"],
        ["config", "user.email", "benchmark@example.invalid"],
        ["add", "."],
        ["commit", "-qm", "Benchmark fixture"],
        ["worktree", "add", "-qb", "benchmark", str(checkout)],
    ):
        subprocess.run([git, *args], cwd=repo, env=environment, check=True)
    with _fixture_server(cli, repo, environment):
        task = json.loads(
            _fixture_cli(
                cli,
                ["task", "create", "--worktree", str(checkout), "Benchmark task", "--json"],
                repo,
                environment,
            )
        )["task"]
        session = json.loads(
            _fixture_cli(
                cli,
                ["session", "resume", "--task", task["id"], "--prepare", "--json"],
                repo,
                environment,
            )
        )
        _fixture_cli(cli, ["session", "resume", session["id"]], checkout, environment)
    (transcript,) = project.glob("*.jsonl")
    # Setup processes have exited before publishing the scenario environment.
    # Separate destinations retain partial setup receipts without subtracting
    # cumulative counters or attributing setup work to the measured journey.
    environment["LF_PERF_OUTPUT"] = str(output / "cli-volume")
    fixture = output / "native-fixture.json"
    _write(
        fixture,
        {
            "cli": str(cli),
            "home": str(home),
            "checkout": task["worktree"],
            "repo": str(repo),
            "task_id": task["id"],
            "issue": task["identifier"],
            "environment": environment,
            "session_id": session["id"],
            "native_id": transcript.stem,
            "transcript": str(transcript),
            "history": transcript.read_text(),
        },
    )
    return fixture


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


def _table_facts(db: sqlite3.Connection, table: str) -> dict:
    quoted = '"' + table.replace('"', '""') + '"'
    digest = hashlib.sha256()
    count = 0
    # Include SQLite row identity, storage types, payloads and duplicate rows.
    for row in db.execute(f"SELECT _rowid_, * FROM {quoted} ORDER BY _rowid_"):
        encoded = repr(row).encode("utf-8")
        digest.update(len(encoded).to_bytes(8, "big"))
        digest.update(encoded)
        count += 1
    return {"rows": count, "sha256": digest.hexdigest()}


def _prepare_snapshot(snapshot: Path, schema_database: Path, output: Path) -> None:
    """Populate a fresh fixture schema with every retained row; never open source writable."""
    source = (snapshot / "loopflow.db").resolve()
    manifest = json.loads((snapshot / "snapshot.json").read_text())
    source_hash = hashlib.sha256(source.read_bytes()).hexdigest()
    if source_hash != manifest["sha256"]:
        raise ValueError("Snapshot changed since capture")
    schema_database = schema_database.resolve()
    schema_hash = hashlib.sha256(schema_database.read_bytes()).hexdigest()
    with (
        sqlite3.connect(source.as_uri() + "?mode=ro", uri=True) as original,
        sqlite3.connect(schema_database.as_uri() + "?mode=ro", uri=True) as schema,
    ):
        query = "SELECT type,name,sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY type,name"
        original_schema = original.execute(query).fetchall()
        target_schema = schema.execute(query).fetchall()
        if [r for r in original_schema if r[0] != "index"] != [
            r for r in target_schema if r[0] != "index"
        ]:
            raise ValueError("Fixture preparation supports index-only schema differences")
        tables = [name for kind, name, _ in target_schema if kind == "table"]
        for table in tables:
            quoted = '"' + table.replace('"', '""') + '"'
            if any(
                row[1].lower() == "_rowid_"
                for row in original.execute(f"PRAGMA table_info({quoted})")
            ):
                raise ValueError("Fixture table shadows SQLite row identity")
            original.execute(f"SELECT _rowid_ FROM {quoted} LIMIT 0")
        output.mkdir(parents=True, exist_ok=False, mode=0o700)
        database = output / "loopflow.db"
        with sqlite3.connect(database, uri=True) as target:
            target.execute("ATTACH DATABASE ? AS retained", (source.as_uri() + "?mode=ro",))
            # Load before installing triggers, so importing facts cannot mint new facts.
            for kind, name, sql in target_schema:
                if kind == "table" and name != "sqlite_sequence":
                    target.execute(sql)
            for table in sorted(tables, key=lambda name: name == "sqlite_sequence"):
                quoted = '"' + table.replace('"', '""') + '"'
                if table == "sqlite_sequence":
                    target.execute(f"DELETE FROM {quoted}")
                target.execute(
                    f"INSERT INTO {quoted}(_rowid_, "
                    + ",".join(
                        '"' + row[1].replace('"', '""') + '"'
                        for row in original.execute(f"PRAGMA table_info({quoted})")
                    )
                    + f") SELECT _rowid_, * FROM retained.{quoted}"
                )
            for kind, _, sql in target_schema:
                if kind != "table":
                    target.execute(sql)
            facts = {table: _table_facts(original, table) for table in tables}
            if facts != {table: _table_facts(target, table) for table in tables}:
                raise RuntimeError("Prepared fixture changed historical content")
            if target.execute(query).fetchall() != target_schema:
                raise RuntimeError("Prepared fixture does not match candidate schema")
        database.chmod(0o600)
    if source_hash != hashlib.sha256(source.read_bytes()).hexdigest():
        raise RuntimeError("Retained snapshot changed during fixture preparation")
    if schema_hash != hashlib.sha256(schema_database.read_bytes()).hexdigest():
        raise RuntimeError("Schema fixture changed during preparation")
    _write(
        output / "snapshot.json",
        {
            "captured_at": manifest["captured_at"],
            "sha256": hashlib.sha256(database.read_bytes()).hexdigest(),
            "counts": {table: fact["rows"] for table, fact in facts.items()},
            "preparation": {
                "source": manifest,
                "schema_sha256": schema_hash,
                "tables": facts,
                "index_changes": {
                    "removed": [r[1] for r in original_schema if r not in target_schema],
                    "added": [r[1] for r in target_schema if r not in original_schema],
                },
            },
        },
    )


def _repository_configs(database: Path) -> list[dict]:
    with sqlite3.connect(database.resolve().as_uri() + "?mode=ro", uri=True) as db:
        paths = sorted(
            {
                str(path.resolve())
                for repo, wave in db.execute("SELECT repo,name FROM waves")
                for path in [
                    Path(repo) / ".lf/config.yaml",
                    Path(repo) / ".lf/wave" / wave / "GOAL.md",
                ]
            }
        )
    return [
        {
            "path": path,
            "sha256": hashlib.sha256(Path(path).read_bytes()).hexdigest()
            if Path(path).is_file()
            else None,
        }
        for path in paths
    ]


def _git_outcomes(path: Path) -> list[dict]:
    processes: dict[str, dict] = {}
    for line in path.read_text().splitlines() if path.exists() else []:
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            processes[f"malformed-{len(processes)}"] = {"argv": [], "exit": None}
            continue
        sid = event.get("sid")
        if event.get("event") == "start":
            processes[sid] = {"argv": event["argv"], "exit": None}
        elif sid in processes and event.get("event") == "error":
            processes[sid].setdefault("errors", []).append(event["msg"])
        elif sid in processes and event.get("event") == "def_repo":
            processes[sid]["worktree"] = event["worktree"]
        elif sid in processes and event.get("event") == "exit":
            processes[sid]["exit"] = event["code"]
            if "t_abs" in event:
                processes[sid]["ms"] = event["t_abs"] * 1000
    return list(processes.values())


def _input_hash(path: Path) -> str | None:
    if path.is_symlink():
        return "symlink:" + os.readlink(path)
    if not path.is_file():
        return None
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return f"{path.stat().st_mode:o}:" + digest.hexdigest()


def _checkout_inputs(database: Path, prefix: list[str], policy: Path, git: Path) -> dict:
    """Observe exact snapshot checkouts without granting writes or their parent directories."""
    with sqlite3.connect(database.resolve().as_uri() + "?mode=ro", uri=True) as db:
        table = (
            "task_checkouts"
            if db.execute(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_checkouts'"
            ).fetchone()
            else "tasks"
        )
        checkouts = sorted(
            {
                row[0]
                for row in db.execute(f"SELECT worktree FROM {table} UNION SELECT repo FROM waves")
                if row[0]
            }
        )
    files: set[Path] = set()
    trees: set[Path] = set()
    identities = []
    existing = []
    for name in checkouts:
        checkout = Path(name)
        identities.append(
            {
                "path": name,
                "resolved": str(checkout.resolve()),
                "exists": checkout.is_dir(),
                "identity": [checkout.stat().st_dev, checkout.stat().st_ino]
                if checkout.exists()
                else None,
            }
        )
        marker = checkout / ".git"
        files.add(marker)
        if not marker.exists():
            continue
        existing.append(checkout)
        trees.add(checkout.resolve())
        if marker.is_file():
            text = marker.read_text().strip()
            if not text.startswith("gitdir: "):
                raise ValueError(f"Invalid Git marker: {marker}")
            directory = (checkout / text[8:]).resolve()
        else:
            directory = marker.resolve()
        common_file = directory / "commondir"
        common = (
            (directory / common_file.read_text().strip()).resolve()
            if common_file.exists()
            else directory
        )
        for root in {directory, common}:
            for name in [
                "HEAD",
                "index",
                "commondir",
                "gitdir",
                "config",
                "config.worktree",
                "packed-refs",
                "shallow",
                "info/exclude",
                "info/attributes",
            ]:
                files.add(root / name)
            for name in ["refs", "objects"]:
                folder = root / name
                trees.add(folder)
                if folder.exists():
                    files.update(path for path in folder.rglob("*") if path.is_file())
    files.update(Path(item["path"]) for item in _repository_configs(database))
    original = policy.read_text()

    def write_policy() -> None:
        # Git status reads tracked content and per-directory ignore files throughout
        # each selected checkout. No parent directory or credential Machine is granted.
        rules = [" (subpath " + json.dumps(str(path)) + ")\n" for path in sorted(trees)]
        roots = tuple(str(tree) + "/" for tree in trees)
        rules += [
            " (literal " + json.dumps(str(path.parent.resolve() / path.name)) + ")\n"
            for path in sorted(files)
            if not path.is_dir() and not str(path).startswith(roots)
        ]
        policy.write_text(
            original + ("\n(allow file-read-data\n" + "".join(rules) + ")\n" if rules else "")
        )

    write_policy()
    env = {
        "HOME": str(policy.parent),
        "PATH": str(git.parent) + ":/usr/bin:/bin",
        "GIT_OPTIONAL_LOCKS": "0",
        "GIT_CONFIG_NOSYSTEM": "1",
    }
    listings = []
    for checkout in existing:
        tracked = subprocess.check_output(
            prefix + [str(git), "-C", str(checkout), "ls-files", "-z", "--cached"],
            env=env,
            timeout=30,
        )
        for name in tracked.decode().split("\0"):
            if name:
                path = checkout / name
                if not path.is_symlink() and not path.resolve().is_relative_to(checkout.resolve()):
                    raise ValueError(f"Tracked input escapes checkout: {path}")
                files.add(path)
    write_policy()
    for checkout in existing:
        result = subprocess.run(
            prefix
            + [
                str(git),
                "-C",
                str(checkout),
                "ls-files",
                "-z",
                "--cached",
                "--others",
                "--exclude-standard",
            ],
            env=env,
            capture_output=True,
            timeout=30,
            check=True,
        )
        names = sorted(set(result.stdout.decode().split("\0")) - {""})
        listings.append({"path": str(checkout), "names": names})
        for name in names:
            if Path(name).name == ".gitignore":
                files.add(checkout / name)
    write_policy()
    return {
        "checkouts": identities,
        "listings": listings,
        "files": [
            {"path": str(path.absolute()), "sha256": _input_hash(path)}
            for path in sorted(files)
            if not path.is_dir()
        ],
    }


def _test_command(
    test_filter: str, environment: dict[str, str], optimized: bool = False
) -> list[str]:
    environment["LOOPFLOW_TEST_GIT"] = str(_benchmark_git())
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
    command = [
        str(swift.parent.parent / "libexec/swift/pm/swiftpm-testing-helper"),
        "--test-bundle-path",
        str(
            REPO
            / f"swift/.build/{'release' if optimized else 'debug'}/LoopflowSwiftPackageTests.xctest"
            / "Contents/MacOS/LoopflowSwiftPackageTests"
        ),
        "--testing-library",
        "swift-testing",
        "--filter",
        test_filter,
    ]
    return command


def _transport_directory(lf_home: Path) -> Path:
    """Where lf puts Session transport sockets for `lf_home` (transport::directory)."""
    digest = hashlib.sha256(os.fsencode(lf_home.resolve())).hexdigest()[:16]
    directory = Path("/tmp").resolve() / f"lf-{os.geteuid()}-{digest}"
    directory.mkdir(mode=0o700, exist_ok=True)
    return directory


def _native_command(
    test_filter: str, environment: dict[str, str], output: Path, optimized: bool = False
) -> list[str]:
    command = _test_command(test_filter, environment, optimized)
    fixture_path = environment.get("LOOPFLOW_TEST_NATIVE_FIXTURE")
    fixture = json.loads(Path(fixture_path).read_text()) if fixture_path else None
    app_home = output / "app-home"
    app_home.mkdir(exist_ok=True)
    environment.update(HOME=str(app_home), TMPDIR=str(app_home))
    parameters = {
        "HOME": str(output),
        "RECEIPTS": str(output),
        # The private benchmark checkout already lies under HOME.
        "REPO": environment.get("LF_DESKTOP_TASK_REPO", str(output)),
        "CODE": str(REPO),
        "CLI": fixture["cli"] if fixture else "/bin/cat",
        "HELPER": command[0],
        "DISPLAY": "deny" if test_filter == "DesktopNativeSessionTests" else "allow",
        "GIT": environment["LOOPFLOW_TEST_GIT"],
        "PROVIDER": str(Path(fixture["home"]) / "bin/claude") if fixture else "/bin/cat",
        "TRANSPORT": str(_transport_directory(Path(fixture["environment"]["LF_HOME"])))
        if fixture
        else str(app_home),
    }
    policy = REPO / "scripts/desktop-performance.sb"
    if snapshot := environment.get("LF_DESKTOP_TASK_SNAPSHOT"):
        configs = _repository_configs(Path(snapshot) / "loopflow.db")
        policy_text = policy.read_text()
        if configs:
            rules = "".join(
                "    (literal " + json.dumps(config["path"]) + ")\n" for config in configs
            )
            policy_text += "\n(allow file-read-data\n" + rules + ")\n"
        policy = output / "snapshot-policy.sb"
        policy.write_text(policy_text)
    sandbox = ["/usr/bin/sandbox-exec"]
    for name, value in sorted(parameters.items()):
        sandbox += ["-D", f"{name}={value if name == 'DISPLAY' else Path(value).resolve()}"]
    if snapshot:
        inputs = _checkout_inputs(
            Path(snapshot) / "loopflow.db",
            sandbox + ["-f", str(policy)],
            policy,
            Path(environment["LOOPFLOW_TEST_GIT"]),
        )
        (output / "repository-inputs.json").write_text(json.dumps(inputs, indent=2) + "\n")
    # sandbox-exec is SIP-protected and strips DYLD_* from its environment.
    # Set only the test framework paths after crossing that boundary.
    loader = [
        f"{name}={environment[name]}" for name in ("DYLD_FRAMEWORK_PATH", "DYLD_LIBRARY_PATH")
    ]
    return sandbox + [
        "-f",
        str(policy),
        "/usr/bin/env",
        *loader,
        *command,
    ]


def _run_fixture_process(command, environment, log, timeout, observe=None):
    path = environment.get("LOOPFLOW_TEST_NATIVE_FIXTURE")
    if not path:
        return _run_process(command, environment, log, timeout, observe)
    fixture = json.loads(Path(path).read_text())
    sandbox = command[: command.index("/usr/bin/env")]
    with _fixture_server(
        Path(fixture["cli"]),
        Path(fixture["repo"]),
        {
            **fixture["environment"],
            "BENCHMARK_INTERACTIVE": environment.get("LOOPFLOW_NATIVE_TESTS", ""),
        },
        sandbox,
    ):
        return _run_process(command, environment, log, timeout, observe)


def _run_process(
    command: list[str],
    environment: dict[str, str],
    log: TextIO,
    timeout: float,
    observe: Callable[[], None] | None = None,
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
                if observe is not None:
                    observe()
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
        except BaseException:
            _stop(process)
            raise
        if interrupted:
            outcome = "interrupted"
        if outcome:
            _stop(process)
        return process.returncode, outcome
    finally:
        signal.signal(signal.SIGTERM, previous)


def _run_native(
    output: Path,
    metadata: dict,
    samples: int,
    soak_seconds: int,
    cli: Path | None,
    snapshot: Path | None = None,
    repo: Path | None = None,
    issue: str | None = None,
    xctrace: bool = True,
    optimized: bool = False,
) -> None:
    environment = {
        key: value for key, value in os.environ.items() if not key.startswith(("LF_", "LOOPFLOW_"))
    }
    isolated = output / "home"
    isolated.mkdir(mode=0o700)
    environment.update(LF_HOME=str(isolated), HOME=str(isolated))
    test_filter = "DesktopPerformanceTests.measureExperiences"
    if snapshot:
        if cli is None:
            raise ValueError("Snapshot measurements require the source CLI")
        environment["LOOPFLOW_TEST_NATIVE_FIXTURE"] = str(_prepare_native_fixture(output, cli))
        manifest = json.loads((snapshot / "snapshot.json").read_text())
        database = snapshot / "loopflow.db"
        if hashlib.sha256(database.read_bytes()).hexdigest() != manifest["sha256"]:
            raise ValueError("Snapshot changed since capture; create a new consistent snapshot")
        metadata["repository_configs"] = _repository_configs(database)
        shutil.copyfile(database, isolated / "loopflow.db")
        (isolated / "loopflow.db").chmod(0o600)
        environment.update(
            LOOPFLOW_UI_TEST_MODE="live",
            LF_DESKTOP_TASK_SNAPSHOT=str(isolated),
            LF_DESKTOP_TASK_BINARY=str(cli),
            LF_DESKTOP_TASK_REPO=str(repo),
            LF_DESKTOP_TASK_ISSUE=issue,
        )
        test_filter = "DesktopPerformanceTests.measureSnapshotTaskOpening"
    environment.update(
        LOOPFLOW_NATIVE_TESTS="1",
        LF_DESKTOP_PERF_OUTPUT=str(output / "attempts.jsonl"),
        LF_DESKTOP_PERF_SAMPLES=str(samples),
        LF_DESKTOP_PERF_SOAK_SECONDS=str(soak_seconds),
        LF_PERF_OUTPUT=str(output / "cli-volume"),
        GIT_TRACE2_EVENT=str(output / "git-events.jsonl"),
    )
    if not snapshot:
        (output / "cli-volume").mkdir()
    recorder = None
    recorder_attempted = False
    with (output / "native.log").open("w") as log:

        def observe() -> None:
            nonlocal recorder, recorder_attempted
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
                                *([] if xctrace else ["--no-xctrace"]),
                            ],
                            stdout=log,
                            stderr=subprocess.STDOUT,
                            start_new_session=True,
                        )
                    except OSError as error:
                        metadata["recorder_error"] = str(error)
            if recorder_attempted and (recorder is None or recorder.poll() is not None):
                (output / "resources-finished").touch(exist_ok=True)

        try:
            build = BUILD_COMMAND + (OPTIMIZED_FLAGS if optimized else [])
            code, outcome = _run_process(build, environment, log, timeout=900)
            if code != 0 or outcome:
                metadata.update(outcome=outcome or "failed", exit_code=code)
                return
            if not _measurement_preflight(output, metadata):
                return
            command = _native_command(test_filter, environment, output, optimized)
            bundle_index = command.index("--test-bundle-path") + 1
            metadata["command"] = command[bundle_index - 2 :]
            metadata["sandbox_command"] = command
            metadata["test_binary_sha256"] = hashlib.sha256(
                Path(command[bundle_index]).read_bytes()
            ).hexdigest()
            code, outcome = _run_fixture_process(
                command,
                environment,
                log,
                timeout=600 + samples * 150 + soak_seconds,
                observe=observe,
            )
            metadata.update(exit_code=code, outcome=outcome)
            if recorder is not None:
                try:
                    recorder.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    pass
        finally:
            if recorder is not None:
                _stop(recorder)
                metadata["recorder_exit_code"] = recorder.returncode
    if snapshot:
        inputs_path = output / "repository-inputs.json"
        metadata["repository_inputs"] = json.loads(inputs_path.read_text())
        policy = output / "snapshot-policy.sb"
        # Rebuild the exact read boundary, including checkout disappearance/recreation,
        # file membership and aliases, rather than hashing only still-existing files.
        policy.write_text((REPO / "scripts/desktop-performance.sb").read_text())
        prefix = command[: command.index("/usr/bin/env")]
        after = _checkout_inputs(isolated / "loopflow.db", prefix, policy, _benchmark_git())
        (output / "repository-inputs-after.json").write_text(json.dumps(after, indent=2) + "\n")
        metadata["repository_inputs_unchanged"] = metadata["repository_inputs"] == after
        outcomes = _git_outcomes(output / "git-events.jsonl")
        metadata["git_outcomes"] = outcomes
        metadata["git_observations_complete"] = bool(outcomes) and all(
            row["exit"] == 0 for row in outcomes
        )
        metadata["repository_configs_unchanged"] = metadata[
            "repository_configs"
        ] == _repository_configs(snapshot / "loopflow.db")
        if not metadata["repository_configs_unchanged"]:
            metadata["outcome"] = "repository configuration changed during measurement"


def _private_home(home: Path, lf: Path) -> Path:
    """The store the benchmark writes rows into: never the one in use."""
    home = home.resolve()
    live = Path(os.environ.get("LF_HOME") or Path.home() / ".lf").resolve()
    if home == live:
        raise SystemExit(
            f"{home} is the Machine in use. Copy it first: "
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
    soak_seconds: int,
    cli: Path | None,
    snapshot: Path | None = None,
    repo: Path | None = None,
    issue: str | None = None,
    xctrace: bool = True,
    optimized: bool = False,
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
        "soak_seconds": soak_seconds,
        "xctrace": xctrace,
        "cli_sha256": hashlib.sha256(cli.read_bytes()).hexdigest() if cli else None,
        "started_at": datetime.now(timezone.utc).isoformat(),
        "host": {"name": platform.node(), "os": platform.platform(), "arch": platform.machine()},
        "build_mode": f"SwiftPM {'release' if optimized else 'debug'} -gnone",
        "build_command": BUILD_COMMAND + (OPTIMIZED_FLAGS if optimized else []),
        "command": None,
        "source_before": _sources(),
        "measurement_source": hashlib.sha256(
            b"".join(
                (REPO / path).read_bytes()
                for path in [
                    "swift/LoopflowTests/DesktopPerformanceTests.swift",
                    "swift/LoopflowTests/DesktopNativeSessionFixture.swift",
                    "swift/LoopflowTests/SessionActionFixtures.swift",
                    "tests/fixtures/dto/roadmap_snapshot.json",
                    "tests/fixtures/dto/session_actions.json",
                    "tests/fixtures/dto/session_history_summary.json",
                    "tests/fixtures/dto/task_work.json",
                    "tests/fixtures/dto/task_comments.json",
                    "tests/fixtures/dto/context_report.json",
                    "tests/fixtures/dto/graph_catalog.json",
                    "scripts/desktop_performance.py",
                    "scripts/desktop-performance.sb",
                    "scripts/benchmarks/desktop-performance/claude",
                    "rust/loopflow-execution/src/performance.rs",
                    "scripts/benchmarks/desktop-performance/record_live.py",
                ]
            )
        ).hexdigest(),
        "population_source": "isolated DTO fixtures; no configured Machine"
        if journey == "run"
        else "a private copy of a Machine; the rows written are removed after each attempt",
        "resources_before": {
            "load_average": os.getloadavg(),
            "free_bytes": shutil.disk_usage(output).free,
        },
    }
    if snapshot:
        metadata.update(
            population_source="isolated SQLite online backup",
            snapshot=json.loads((snapshot / "snapshot.json").read_text()),
            repo=str(repo),
            issue=issue,
        )
    _write(output / "run.json", metadata)
    if sys.platform != "darwin":
        metadata.update(
            outcome="unavailable", reason="Native benchmark requires macOS", exit_code=None
        )
    elif journey == "write-visible":
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
        # These are host-boundary time limits, never latency targets. The attempt
        # journal survives termination; only this invocation's process group stops.
        with (output / "native.log").open("w") as log:
            try:
                # SwiftPM can put its test child in a separate process group.
                # Both journeys build first, then own the native process directly.
                code, outcome = _run_process(
                    metadata["build_command"], environment, log, timeout=600
                )
                if outcome:
                    metadata.update(outcome=outcome, exit_code=code)
                if code != 0 or outcome:
                    raise subprocess.CalledProcessError(code, BUILD_COMMAND)
                if not _measurement_preflight(output, metadata):
                    raise RuntimeError(metadata["reason"])
                command = _test_command(test_filter, environment, optimized)
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
            except (OSError, RuntimeError, subprocess.SubprocessError) as error:
                metadata.setdefault("outcome", "unavailable")
                metadata.setdefault("exit_code", None)
                metadata["reason"] = str(error)
    else:
        try:
            _run_native(
                output,
                metadata,
                samples,
                soak_seconds,
                cli,
                snapshot,
                repo,
                issue,
                xctrace,
                optimized,
            )
        except (OSError, RuntimeError, ValueError) as error:
            metadata.update(outcome="failed", reason=str(error))
            metadata.setdefault("exit_code", None)
    metadata["source_after"] = _sources()
    metadata["resources_after"] = {
        "load_average": os.getloadavg(),
        "free_bytes": shutil.disk_usage(output).free,
    }
    metadata["cli_sha256_after"] = hashlib.sha256(cli.read_bytes()).hexdigest() if cli else None
    _write(output / "run.json", metadata)
    summary = _report(output, baseline)
    print(f"{summary['status']}: {output / 'report.md'}")
    return 0 if summary["status"] == "complete" else 1


def _verify_native_fixture(output: Path, cli: Path, *, mounted: bool = False) -> int:
    output.mkdir(parents=True, exist_ok=False)
    with (output / "native.log").open("w") as log:
        result, failure = _run_process(BUILD_COMMAND, os.environ.copy(), log, timeout=600)
        if result or failure:
            return 1
        # Snapshot reads and the owned Session come from different Machines.
        source = output / "copied-source"
        source.mkdir()
        copied = json.loads(_prepare_native_fixture(source, cli).read_text())
        _snapshot(Path(copied["environment"]["LF_HOME"]) / "loopflow.db", output / "snapshot")
        fixture = _prepare_native_fixture(output, cli)
        environment = {"PATH": os.environ.get("PATH", "/usr/bin:/bin")}
        environment.update(
            LOOPFLOW_TEST_NATIVE_FIXTURE=str(fixture),
            LF_DESKTOP_TASK_SNAPSHOT=str(output / "snapshot"),
            LF_DESKTOP_TASK_REPO=copied["repo"],
            LF_PERF_OUTPUT=str(output / "cli-volume"),
        )
        with tempfile.TemporaryDirectory(prefix="desktop-external-canary-") as directory:
            canary = Path(directory) / "provider"
            content = "#!/bin/sh\nexit 0\n"
            canary.write_text(content)
            canary.chmod(0o755)
            environment["LOOPFLOW_TEST_CANARY"] = str(canary)
            command = _native_command("DesktopNativeSessionTests", environment, output)
            result, failure = _run_fixture_process(command, environment, log, timeout=120)
            if mounted and not result and not failure:
                environment.update(LOOPFLOW_NATIVE_TESTS="1", SHELL="/bin/bash")
                command = _native_command(
                    "DesktopPerformanceTests.embeddedLaunchContract", environment, output
                )
                result, failure = _run_fixture_process(command, environment, log, timeout=60)
            if mounted and not result and not failure:
                environment.update(
                    LOOPFLOW_REOPEN_PROOF="1",
                    LF_DESKTOP_PERF_OUTPUT=str(output / "reopen.jsonl"),
                )
                command = _native_command(
                    "DesktopPerformanceTests.repeatedNativeReopen", environment, output
                )
                result, failure = _run_fixture_process(command, environment, log, timeout=180)
            if canary.read_text() != content:
                raise RuntimeError("Benchmark escaped its filesystem boundary")
        for directory in ("fixture-setup-cli-volume", "cli-volume"):
            _write(output / f"{directory}.json", _cli_volume(output / directory))
    print(output / "native.log")
    return 1 if result or failure else 0


def _terminal_feedback(output: Path, baseline: Path | None, mounted: bool) -> int:
    output.mkdir(parents=True, exist_ok=False)
    # No provider transcript or account is needed for the contained renderer fixture.
    lines = ["\x1b[2J\x1b[Hterminal feedback", "error alpha", "error 界 e\u0301"]
    for index in range(16):
        lines.append(f"\x1b[38;5;{index}mANSI {index:02d} error\x1b[0m")
    lines += [
        "\x1b[38;2;68;34;34mdark red\x1b[0m \x1b[38;2;34;68;34mdark green\x1b[0m",
        "\x1b[2mfaint error\x1b[0m \x1b[7minverse\x1b[0m \x1b[8mconcealed\x1b[0m",
        "\x1b]133;A\x07$ \x1b]133;B\x07false\r\n\x1b]133;C\x07failed output\r\n\x1b]133;D;1\x07",
        "\x1b]133;A\x07$ \x1b]133;B\x07",
        "feedback-ready",
    ]
    payload = ("\r\n".join(lines) + "\r\n").encode()
    fixture_input = output / "terminal-feedback.ansi"
    fixture_input.write_bytes(payload)
    metadata = {
        "source": _sources(),
        "artifact_manifest_sha256": hashlib.sha256(
            (REPO / "swift/Package.swift").read_bytes()
        ).hexdigest(),
        "input_sha256": hashlib.sha256(payload).hexdigest(),
        "configuration": "isolated default Ghostty configuration; minimum-contrast=3",
        "viewport_points": [800, 480],
        "visible_panes": 1,
        "font_points": 13,
        "scenarios": ["idle", "typing", "copy", "find", "retained-return"],
        "build": "release with testing enabled",
        "mounted": mounted,
        "presented_pixels": "unmeasured; action_to_draw_call_ms is a diagnostic only",
        "gpu_time": "unmeasured; collect with native Instruments for demo",
        "status": "fixture prepared",
    }
    if baseline:
        prior = json.loads((baseline / "terminal-feedback.json").read_text())
        metadata["baseline"] = str(baseline.resolve())
        metadata["comparable_fixture"] = all(
            prior.get(key) == metadata[key]
            for key in (
                "input_sha256",
                "configuration",
                "viewport_points",
                "visible_panes",
                "font_points",
                "build",
            )
        )
    result = 0
    if mounted:
        with (output / "native.log").open("w") as log:
            result, failure = _run_process(
                BUILD_COMMAND + OPTIMIZED_FLAGS, os.environ.copy(), log, timeout=600
            )
            if not result and not failure:
                environment = {"PATH": os.environ.get("PATH", "/usr/bin:/bin")}
                environment.update(
                    LOOPFLOW_NATIVE_TESTS="1",
                    LOOPFLOW_TERMINAL_FEEDBACK_INPUT=str(fixture_input),
                    LOOPFLOW_TERMINAL_FEEDBACK_OUTPUT=str(output / "observations.json"),
                )
                command = _native_command(
                    "TerminalFeedbackTests", environment, output, optimized=True
                )
                result, failure = _run_process(
                    ["/usr/bin/time", "-l", *command], environment, log, timeout=120
                )
            metadata["status"] = "failed" if result or failure else "native diagnostics complete"
            metadata["failure"] = failure
            result = 1 if result or failure else 0
    _write(output / "terminal-feedback.json", metadata)
    print(output / "terminal-feedback.json")
    return result


def _measurement_preflight(output: Path, metadata: dict) -> bool:
    code = _preflight(output / "preflight")
    report = json.loads((output / "preflight/preflight.json").read_text())
    metadata["preflight"] = report
    if code:
        metadata.update(outcome="unavailable", exit_code=None, reason=report["reason"])
    return code == 0


def _host_load(processes: str) -> dict:
    # ps reports 100% per core. Keep executable names, never command arguments.
    rows = []
    for line in processes.splitlines():
        fields = line.strip().split(maxsplit=1)
        if len(fields) != 2:
            continue
        try:
            cpu = float(fields[0])
        except ValueError:
            continue
        if math.isfinite(cpu) and cpu >= 0:
            rows.append((cpu, Path(fields[1]).name))
    return {
        "cpu_percent": round(sum(cpu for cpu, _ in rows), 2),
        "top": [
            {"cpu_percent": cpu, "executable": name} for cpu, name in sorted(rows, reverse=True)[:8]
        ],
    }


def _preflight(output: Path, seconds: int = 60) -> int:
    """Observe this host before a trial; never interrupt unrelated processes."""
    output.mkdir(parents=True, exist_ok=False)
    rows = []
    reason = None
    for _ in range(seconds):
        try:
            processes = subprocess.check_output(["ps", "-A", "-o", "pcpu=,comm="], text=True)
        except (OSError, subprocess.SubprocessError):
            reason = "Host CPU observation unavailable; no trial started"
            break
        load = _host_load(processes)
        load.update(time=datetime.now(timezone.utc).isoformat(), load_average=os.getloadavg())
        rows.append(load)
        # This is the external load before starting the benchmark. Deliberate
        # five/twenty-agent fixture load belongs to a later measurement window.
        if load["cpu_percent"] > 10:
            reason = (
                "Background process CPU exceeds 0.1 core; "
                "trial rejected without stopping other work"
            )
            break
        if len(rows) < seconds:
            time.sleep(1)
    if seconds < 60 and reason is None:
        reason = "Diagnostic sample only; fewer than 60 quiet samples"
    native = None
    if reason is None:
        # These APIs only inspect access; neither requests a permission dialog.
        probe_command = [
            "xcrun",
            "swift",
            "-e",
            "import AppKit; import CoreGraphics; import ApplicationServices; "
            r'print("display=\(NSScreen.screens.count) '
            r'capture=\(CGPreflightScreenCaptureAccess()) input=\(AXIsProcessTrusted())")',
        ]
        probe_log = output / "native-probe.log"
        try:
            with probe_log.open("w") as log:
                code, outcome = _run_process(probe_command, os.environ.copy(), log, timeout=60)
            capabilities = probe_log.read_text().strip()
            native = {"exit_code": code, "outcome": outcome, "capabilities": capabilities}
            if code or outcome or "display=0" in capabilities or "display=" not in capabilities:
                reason = "Native display preflight unavailable; no permissions requested"
        except OSError as error:
            native = {"exit_code": None, "reason": str(error)}
            reason = "Native capability probe unavailable; no permissions requested"
    report = {
        "schema": 1,
        "host": platform.node(),
        "os": platform.platform(),
        "logical_cpus": os.cpu_count(),
        "samples": rows,
        "native": native,
        "status": "rejected" if reason else "ready",
        "reason": reason,
        "quiet_threshold_cpu_percent": 10,
        "limits": [
            "ps CPU is a recent average, not a frame-time measurement",
            "Thermal state, swap and whole-trial background load still need capture",
            "Capture/input trust applies to the probe identity; the signed app may differ",
            "Mounted controls and owned-window bitmap capture need no external "
            "HID/screen-capture access",
        ],
    }
    _write(output / "preflight.json", report)
    print(output / "preflight.json")
    return 1 if reason else 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    commands = parser.add_subparsers(dest="command", required=True)
    preflight = commands.add_parser(
        "preflight", help="Observe load and native access on this host; never stops other work"
    )
    preflight.add_argument("--output", type=Path, required=True)
    preflight.add_argument("--seconds", type=int, default=60)
    feedback = commands.add_parser(
        "terminal-feedback",
        help="Prepare paired terminal fixtures and optional contained native diagnostics",
    )
    feedback.add_argument("--output", type=Path, required=True)
    feedback.add_argument("--baseline", type=Path)
    feedback.add_argument("--mounted", action="store_true")
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
        "--cli",
        "--lf",
        type=Path,
        help="Source CLI for the snapshot’s owned native Session fixture",
    )
    run.add_argument(
        "--soak-seconds",
        type=int,
        default=0,
        help="Append unattended idle/navigation/typing; 3600 for the acceptance soak",
    )
    run.add_argument(
        "--no-xctrace",
        action="store_true",
        help="Diagnostic RSS/signposts only; leaves hitch/hang coverage unmeasured",
    )
    run.add_argument(
        "--optimized",
        action="store_true",
        help="Release-configuration Swift; never compared with debug runs",
    )
    run.add_argument("--snapshot", type=Path)
    run.add_argument("--repo", type=Path)
    run.add_argument("--issue")
    verify = commands.add_parser(
        "verify-fixture", help="Headless combined snapshot/native isolation proof"
    )
    verify.add_argument("--lf", type=Path, required=True)
    verify.add_argument("--output", type=Path, required=True)
    verify.add_argument(
        "--mounted",
        action="store_true",
        help="Also verify production Ghostty shell launches in an owned native window",
    )
    snapshot_command = commands.add_parser("snapshot", help="Consistent private SQLite backup")
    snapshot_command.add_argument("--database", type=Path, required=True)
    snapshot_command.add_argument("--output", type=Path, required=True)
    prepare = commands.add_parser(
        "prepare-snapshot", help="Copy all retained facts into a fresh fixture schema"
    )
    prepare.add_argument("--snapshot", type=Path, required=True)
    prepare.add_argument("--schema-database", type=Path, required=True)
    prepare.add_argument("--output", type=Path, required=True)
    written = commands.add_parser(
        "write-visible",
        help="Commit from another process to a private Machine and time the row appearing",
    )
    written.add_argument(
        "--output", type=Path, required=True, help="New directory; never overwrites evidence"
    )
    written.add_argument(
        "--home",
        type=Path,
        required=True,
        help="A private copy of a Machine; rows are written to it",
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
    volume = commands.add_parser(
        "volume", help="Summarize instrumented CLI receipts; launches nothing"
    )
    volume.add_argument("directory", type=Path)
    args = parser.parse_args()
    if args.command == "preflight":
        if args.seconds < 1:
            parser.error("--seconds must be positive")
        return _preflight(args.output.resolve(), args.seconds)
    if args.command == "terminal-feedback":
        return _terminal_feedback(args.output.resolve(), args.baseline, args.mounted)
    if args.command == "verify-fixture":
        return _verify_native_fixture(
            args.output.resolve(), args.lf.resolve(), mounted=args.mounted
        )
    if args.command == "prepare-snapshot":
        _prepare_snapshot(args.snapshot, args.schema_database, args.output)
        print(args.output / "snapshot.json")
        return 0
    if args.command == "snapshot":
        _snapshot(args.database, args.output)
        print(args.output / "snapshot.json")
        return 0
    if args.command == "volume":
        result = _cli_volume(args.directory)
        print(json.dumps(result, indent=2))
        return 0 if result["status"] == "complete" else 1
    if args.command == "run":
        if args.snapshot and not all([args.repo, args.issue, args.cli]):
            parser.error("--snapshot requires --repo, --issue and --lf")
        if args.soak_seconds < 0:
            parser.error("--soak-seconds cannot be negative")
        if args.samples < 1:
            parser.error("--samples must be positive")
        return _run(
            args.output.resolve(),
            args.samples,
            args.baseline,
            args.soak_seconds,
            args.cli.resolve() if args.cli else None,
            args.snapshot.resolve() if args.snapshot else None,
            args.repo.resolve() if args.repo else None,
            args.issue,
            not args.no_xctrace,
            args.optimized,
        )
    if args.command == "write-visible":
        if args.samples < 1:
            parser.error("--samples must be positive")
        lf = args.lf.resolve()
        return _run(
            args.output.resolve(),
            args.samples,
            args.baseline,
            soak_seconds=0,
            cli=lf,
            xctrace=False,
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
