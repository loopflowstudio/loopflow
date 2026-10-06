#!/usr/bin/env python3
"""Measure native desktop journeys and compare like-for-like observations.

uv run python scripts/desktop_performance.py run --cli target/debug/lf --output /tmp/before
uv run python scripts/desktop_performance.py report /tmp/after --baseline /tmp/before
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
import tempfile
import time
import uuid
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path
from types import FrameType
from typing import Callable, TextIO

import yaml

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
        and metadata.get("repository_configs_unchanged", True)
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
        "snapshot",
        "repo",
        "issue",
        "repository_configs",
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


def _report(output: Path, baseline: Path | None) -> dict:
    metadata = json.loads((output / "run.json").read_text())
    events, errors = _read_events(output / "attempts.jsonl")
    summary = _summarize(events)
    errors.extend(summary["journal_errors"])
    if metadata.get("soak_seconds", 0):
        summary["soak"] = _soak(events, {"soak_seconds": metadata["soak_seconds"]})
    summary.update(metadata=metadata, journal_errors=errors)
    # Observed failures remain comparable, but cannot complete the journey.
    complete = _has_complete_observations(summary) and all(
        attempt["outcome"] == "passed" for attempt in summary["attempts"]
    )
    summary["status"] = "complete" if complete else "incomplete"
    summary["cli_volume"] = _cli_volume(output / "cli-volume")
    summary["fixture_setup_cli_volume"] = _cli_volume(output / "fixture-setup-cli-volume")
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
    (output / "report.md").write_text(
        _markdown(summary, scoped=(output / "fixture-setup-cli-volume").is_dir()),
        encoding="utf-8",
    )
    return summary


def _markdown(summary: dict, *, scoped: bool) -> str:
    snapshot_run = summary["metadata"].get("snapshot") is not None
    lines = [
        f"# Desktop measurements: {summary['status']}",
        "",
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


def _prepare_native_fixture(output: Path, cli: Path, repo: Path | None = None) -> Path:
    for directory in ("fixture-setup-cli-volume", "cli-volume"):
        (output / directory).mkdir()
    home = output / "native-home"
    checkout = home / "checkout"
    checkout.mkdir(parents=True)
    (checkout / "notes.txt").write_text("Original notes")
    native = home / "codex"
    bin_dir = home / "bin"
    bin_dir.mkdir(parents=True)
    native_id = str(uuid.uuid4())
    transcript = native / "sessions/2026/10/05" / f"rollout-time-{native_id}.jsonl"
    transcript.parent.mkdir(parents=True)
    history = json.dumps({"cwd": str(checkout), "message": "retained-history"}) + "\n"
    transcript.write_text(history)
    provider = bin_dir / "codex"
    provider.write_text(
        "#!/bin/sh\n"
        'if [ "$1" = --version ]; then exit 0; fi\n'
        'case " $* " in *" $PERF_NATIVE_ID "*) ;; *) exit 42;; esac\n'
        'grep -q retained-history "$PERF_TRANSCRIPT" || exit 43\n'
        'printf "native:%s:retained-history\\n" "$PERF_NATIVE_ID"\n'
        "while IFS= read -r input; do\n"
        '  [ "$input" = quit ] && exit 0\n'
        '  printf "reply:%s:%s\\n" "$PERF_NATIVE_ID" "$input"\n'
        "done\n"
    )
    provider.chmod(0o755)
    git = _benchmark_git()
    environment = {
        "HOME": str(home),
        "LF_HOME": str(home),
        "LF_BIN": str(cli),
        "CODEX_HOME": str(native),
        "CLAUDE_CONFIG_DIR": str(home / "claude"),
        "PATH": f"{bin_dir}:{git.parent}:/usr/bin:/bin",
        "TMPDIR": str(home),
        "PERF_NATIVE_ID": native_id,
        "PERF_TRANSCRIPT": str(transcript),
        "LF_PERF_OUTPUT": str(output / "fixture-setup-cli-volume"),
        "RUST_LOG": "off",
    }
    for args in (["init", "--quiet", str(checkout)], ["-C", str(checkout), "add", "notes.txt"]):
        subprocess.run([str(git), *args], env=environment, check=True, capture_output=True)
    _fixture_cli(cli, ["resume", native_id], checkout, environment)
    records = json.loads(
        _fixture_cli(
            cli, ["session", "list", "--all", "--history", "--json"], checkout, environment
        )
    )
    if len(records) != 1:
        raise RuntimeError("Native fixture did not retain exactly one Session")
    task_id, issue = _seed_native_task(home, checkout, repo or checkout, records[0]["id"])
    record = json.loads(
        _fixture_cli(
            cli,
            ["session", "bind", records[0]["id"], "--task", task_id, "--json"],
            checkout,
            environment,
        )
    )
    if task_id not in record["task_ids"]:
        raise RuntimeError("Rust membership omitted the owned fixture Task")
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
            "checkout": str(checkout),
            "repo": str(repo or checkout),
            "task_id": task_id,
            "issue": issue,
            "environment": environment,
            "session_id": records[0]["id"],
            "native_id": native_id,
            "transcript": str(transcript),
            "history": history,
        },
    )
    return fixture


def _seed_native_task(home: Path, checkout: Path, repo: Path, session: str) -> tuple[str, str]:
    """Seed owned planning facts; the public bind and shared Rust reader own membership."""
    project, task = (prefix + uuid.uuid4().hex for prefix in ("proj_", "task_"))
    wave = str(uuid.uuid4())
    issue = f"PERF-{int(task[-8:], 16)}"
    now = int(time.time())
    config_path = repo / ".lf/config.yaml"
    config = yaml.safe_load(config_path.read_text()) if config_path.is_file() else {}
    team = ((config or {}).get("pm") or {}).get("linear_team") or wave
    plan = dict(
        id=project,
        slug="desktop-performance",
        name="Owned performance fixture",
        summary="",
        metric_targets=[],
        flow="",
        status="started",
        krs=[],
        initiative_ids=[wave],
        team_ids=[team],
    )
    item = dict(
        id=task,
        identifier=issue,
        url=f"https://example.invalid/{issue}",
        name="Owned native Session",
        description="Synthetic performance fixture",
        rank=1,
        completed=False,
        project_id=project,
        project=plan["slug"],
        team_id=team,
        assignee=None,
    )
    with sqlite3.connect(home / "loopflow.db") as db:
        db.execute("PRAGMA foreign_keys=ON")
        db.execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?,?,?,?)",
            (wave, "desktop-performance", str(repo), now),
        )
        db.execute(
            """INSERT INTO projects(id,wave_id,external_project_id,created_at,
            project_slug,project_name,project_prompt_context,pm_snapshot_synced_at,updated_at)
            VALUES(?,?,?,?,?,?,?,?,?)""",
            (project, wave, project, now, plan["slug"], plan["name"], "", now, now),
        )
        db.execute(
            """INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,created_at,
            issue_title,issue_description,pm_snapshot_synced_at,pm_writeback_json,worktree,workspace_slug,updated_at)
            VALUES(?,?,?,?,?,?,?,?,?,?,?,?)""",
            (
                task,
                project,
                task,
                issue,
                now,
                item["name"],
                item["description"],
                now,
                '{"state":"current"}',
                str(checkout),
                "desktop-performance",
                now,
            ),
        )
        db.execute(
            "INSERT INTO pm_projects(repo,provider,id,observed_at,body) VALUES(?,?,?,?,?)",
            (str(repo), "linear", project, now, json.dumps(plan)),
        )
        db.execute(
            "INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) "
            "VALUES(?,?,?,?,?,?,?)",
            (str(repo), "linear", task, issue, project, now, json.dumps(item)),
        )
        for column, identity in (("wave_id", wave), ("project_id", project), ("task_id", task)):
            db.execute(
                f"INSERT INTO work_placements({column},home_id,placed_at) "
                "SELECT ?,id,? FROM homes WHERE route='local'",
                (identity, now),
            )
        db.execute("INSERT INTO pm_wave_sync VALUES(?,?,?,?)", (wave, "linear", wave, now))
        db.execute("INSERT INTO pm_wave_projects VALUES(?,?,?)", (wave, project, 0))
        db.execute("UPDATE agent_sessions SET repo=? WHERE id=?", (str(repo), session))
    return task, issue


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
                for path in [Path(repo) / ".lf/config.yaml", Path(repo) / "wave" / wave / "GOAL.md"]
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


def _native_command(test_filter: str, environment: dict[str, str]) -> list[str]:
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
            / "swift/.build/debug/LoopflowSwiftPackageTests.xctest"
            / "Contents/MacOS/LoopflowSwiftPackageTests"
        ),
        "--testing-library",
        "swift-testing",
        "--filter",
        test_filter,
    ]
    fixture_path = Path(environment["LOOPFLOW_TEST_NATIVE_FIXTURE"])
    fixture = json.loads(fixture_path.read_text())
    output = fixture_path.parent.resolve()
    app_home = output / "app-home"
    app_home.mkdir(exist_ok=True)
    environment.update(HOME=str(app_home), TMPDIR=str(app_home))
    parameters = {
        "HOME": str(output),
        "RECEIPTS": str(output),
        "REPO": fixture["repo"],
        "CODE": str(REPO),
        "CLI": fixture["cli"],
        "HELPER": command[0],
        "DISPLAY": "deny" if test_filter == "DesktopNativeSessionTests" else "allow",
        "GIT": environment["LOOPFLOW_TEST_GIT"],
        "PROVIDER": str(Path(fixture["home"]) / "bin/codex"),
    }
    policy = REPO / "scripts/desktop-performance.sb"
    if snapshot := environment.get("LF_DESKTOP_TASK_SNAPSHOT"):
        configs = _repository_configs(Path(snapshot) / "loopflow.db")
        policy_text = (
            policy.read_text()
            + "\n(allow file-read-data\n"
            + "".join("    (literal " + json.dumps(config["path"]) + ")\n" for config in configs)
            + ")\n"
        )
        policy = output / "snapshot-policy.sb"
        policy.write_text(policy_text)
    sandbox = ["/usr/bin/sandbox-exec"]
    for name, value in sorted(parameters.items()):
        sandbox += ["-D", f"{name}={value if name == 'DISPLAY' else Path(value).resolve()}"]
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
    cli: Path,
    snapshot: Path | None = None,
    repo: Path | None = None,
    issue: str | None = None,
) -> None:
    environment = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
    isolated = output / "home"
    isolated.mkdir(mode=0o700)
    environment.update(LF_HOME=str(isolated), HOME=str(isolated))
    fixture = _prepare_native_fixture(output, cli, repo)
    test_filter = "DesktopPerformanceTests.measureExperiences"
    if snapshot:
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
    )
    environment["LOOPFLOW_TEST_NATIVE_FIXTURE"] = str(fixture)
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
            code, outcome = _run_process(BUILD_COMMAND, environment, log, timeout=600)
            if code != 0 or outcome:
                metadata.update(outcome=outcome or "failed", exit_code=code)
                return
            command = _native_command(test_filter, environment)
            bundle_index = command.index("--test-bundle-path") + 1
            metadata["command"] = command[bundle_index - 2 :]
            metadata["sandbox_command"] = command
            metadata["test_binary_sha256"] = hashlib.sha256(
                Path(command[bundle_index]).read_bytes()
            ).hexdigest()
            code, outcome = _run_process(
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
        metadata["repository_configs_unchanged"] = metadata[
            "repository_configs"
        ] == _repository_configs(snapshot / "loopflow.db")
        if not metadata["repository_configs_unchanged"]:
            metadata["outcome"] = "repository configuration changed during measurement"


def _run(
    output: Path,
    samples: int,
    baseline: Path | None,
    soak_seconds: int,
    cli: Path,
    snapshot: Path | None = None,
    repo: Path | None = None,
    issue: str | None = None,
) -> int:
    output.mkdir(parents=True, exist_ok=False)
    metadata = {
        "schema": 1,
        "soak_seconds": soak_seconds,
        "cli_sha256": hashlib.sha256(cli.read_bytes()).hexdigest(),
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
                    "swift/LoopflowTests/DesktopNativeSessionFixture.swift",
                    "swift/LoopflowTests/SessionActionFixtures.swift",
                    "tests/fixtures/dto/roadmap_snapshot.json",
                    "tests/fixtures/dto/session_actions.json",
                    "tests/fixtures/dto/session_history_summary.json",
                    "tests/fixtures/dto/task_work.json",
                    "tests/fixtures/dto/task_comments.json",
                    "tests/fixtures/dto/context_report.json",
                    "tests/fixtures/dto/flow_catalog.json",
                    "scripts/desktop_performance.py",
                    "scripts/desktop-performance.sb",
                    "rust/loopflow/src/performance.rs",
                    "scripts/benchmarks/desktop-performance/record_live.py",
                ]
            )
        ).hexdigest(),
        "population_source": "isolated DTO fixtures; no configured Home",
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
    else:
        try:
            _run_native(output, metadata, samples, soak_seconds, cli, snapshot, repo, issue)
        except (OSError, RuntimeError, ValueError) as error:
            metadata.update(outcome="failed", reason=str(error))
            metadata.setdefault("exit_code", None)
    metadata["source_after"] = _sources()
    metadata["cli_sha256_after"] = hashlib.sha256(cli.read_bytes()).hexdigest()
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
        source = output / "copied-source"
        source.mkdir()
        repo = output / "repository"
        (repo / ".lf").mkdir(parents=True)
        (repo / ".lf/config.yaml").write_text("pm:\n  linear_team: " + str(uuid.uuid4()) + "\n")
        subprocess.run(
            [str(_benchmark_git()), "init", "--quiet", str(repo)], check=True, capture_output=True
        )
        _prepare_native_fixture(source, cli, repo)
        _snapshot(source / "native-home/loopflow.db", output / "snapshot")
        fixture = _prepare_native_fixture(output, cli, repo)
        environment = {"PATH": os.environ.get("PATH", "/usr/bin:/bin")}
        environment.update(
            LOOPFLOW_TEST_NATIVE_FIXTURE=str(fixture),
            LF_DESKTOP_TASK_SNAPSHOT=str(output / "snapshot"),
            LF_PERF_OUTPUT=str(output / "cli-volume"),
        )
        with tempfile.TemporaryDirectory(prefix="desktop-external-canary-") as directory:
            canary = Path(directory) / "provider"
            content = "#!/bin/sh\nexit 0\n"
            canary.write_text(content)
            canary.chmod(0o755)
            environment["LOOPFLOW_TEST_CANARY"] = str(canary)
            command = _native_command("DesktopNativeSessionTests", environment)
            result, failure = _run_process(command, environment, log, timeout=120)
            if mounted and not result and not failure:
                environment.update(LOOPFLOW_NATIVE_TESTS="1", SHELL="/bin/bash")
                command = _native_command(
                    "DesktopPerformanceTests.embeddedLaunchContract", environment
                )
                result, failure = _run_process(command, environment, log, timeout=60)
            if canary.read_text() != content:
                raise RuntimeError("Benchmark escaped its filesystem boundary")
        for directory in ("fixture-setup-cli-volume", "cli-volume"):
            _write(output / f"{directory}.json", _cli_volume(output / directory))
    print(output / "native.log")
    return 1 if result or failure else 0


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
        "--cli",
        "--lf",
        type=Path,
        required=True,
        help="Source CLI for the owned native Session fixture",
    )
    run.add_argument(
        "--soak-seconds",
        type=int,
        default=0,
        help="Append unattended idle/navigation/typing; 3600 for the acceptance soak",
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
        if args.snapshot and not all([args.repo, args.issue]):
            parser.error("--snapshot requires --repo and --issue")
        if args.soak_seconds < 0:
            parser.error("--soak-seconds cannot be negative")
        if args.samples < 1:
            parser.error("--samples must be positive")
        return _run(
            args.output.resolve(),
            args.samples,
            args.baseline,
            args.soak_seconds,
            args.cli.resolve(),
            args.snapshot.resolve() if args.snapshot else None,
            args.repo.resolve() if args.repo else None,
            args.issue,
        )
    summary = _report(args.output, args.baseline)
    print(args.output / "report.md")
    return 0 if summary["status"] == "complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())
