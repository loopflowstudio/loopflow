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
import signal
import statistics
import subprocess
import sys
import time
import uuid
from collections import Counter, defaultdict
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
    by_kind: dict[str, list[dict]] = defaultdict(list)
    for event in events:
        if event["event"].startswith("soak_"):
            by_kind[event["event"]].append(event)
    starts, ends, rounds = (by_kind[kind] for kind in ("soak_begin", "soak_end", "soak_round"))
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
        "phases": by_kind["soak_phase"],
        "end": ends[0] if len(ends) == 1 else None,
        "limits": [
            "Fixture reads, not CLI processes or SQLite query volume",
            "Soak retains cat PTYs; the separate reopen scenario uses synthetic native history",
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
        and metadata.get("cli_sha256") == metadata.get("cli_sha256_after")
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
    summary["cli_volume"] = _cli_volume(output / "cli-volume")
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
        "Three retained cat PTYs per population; "
        "separate native Session reopen uses a real CLI and owned provider stub.",
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
    volume = summary["cli_volume"]
    lines += [
        "",
        f"CLI volume: {volume['status']}; "
        f"processes started/ended {volume['processes_started']}/{volume['processes_ended']}; "
        f"observed SQLite totals {volume['observed_totals']}. "
        "Scope and partial receipts are retained in report.json.",
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


def _prepare_native_fixture(output: Path, cli: Path) -> Path:
    home = output / "native-home"
    native = home / "codex"
    bin_dir = home / "bin"
    bin_dir.mkdir(parents=True)
    native_id = str(uuid.uuid4())
    transcript = native / "sessions/2026/10/05" / f"rollout-time-{native_id}.jsonl"
    transcript.parent.mkdir(parents=True)
    history = json.dumps({"cwd": str(home), "message": "retained-history"}) + "\n"
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
    environment = {
        "HOME": str(home),
        "LF_HOME": str(home),
        "LF_BIN": str(cli),
        "CODEX_HOME": str(native),
        "CLAUDE_CONFIG_DIR": str(home / "claude"),
        "PATH": f"{bin_dir}:/usr/bin:/bin",
        "TMPDIR": str(home),
        "PERF_NATIVE_ID": native_id,
        "PERF_TRANSCRIPT": str(transcript),
        "LF_PERF_OUTPUT": str(output / "cli-volume"),
        "RUST_LOG": "off",
    }
    for args in (["resume", native_id], ["session", "list", "--all", "--history", "--json"]):
        process = subprocess.Popen(
            [str(cli), *args],
            cwd=home,
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
        try:
            stdout, stderr = process.communicate(timeout=60)
        except subprocess.TimeoutExpired:
            _stop(process)
            raise RuntimeError("Native fixture setup timed out") from None
        if process.returncode:
            raise RuntimeError(f"Native fixture setup failed: {stderr.decode(errors='replace')}")
    records = json.loads(stdout)
    if len(records) != 1:
        raise RuntimeError("Native fixture did not retain exactly one Session")
    fixture = output / "native-fixture.json"
    _write(
        fixture,
        {
            "cli": str(cli),
            "home": str(home),
            "environment": environment,
            "session_id": records[0]["id"],
            "native_id": native_id,
            "transcript": str(transcript),
            "history": history,
        },
    )
    return fixture


def _run_native(output: Path, samples: int, soak_seconds: int, cli: Path) -> dict:
    metadata: dict = {}
    environment = {key: value for key, value in os.environ.items() if not key.startswith("LF_")}
    isolated = output / "home"
    isolated.mkdir()
    volume = output / "cli-volume"
    volume.mkdir()
    environment.update(LF_HOME=str(isolated), HOME=str(isolated), LF_PERF_OUTPUT=str(volume))
    fixture = _prepare_native_fixture(output, cli)
    environment.update(
        LOOPFLOW_TEST_NATIVE_FIXTURE=str(fixture),
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
            return {"outcome": "unavailable", "reason": str(error), "exit_code": None}
        recorder = None
        recorder_attempted = False
        timeout = 600 + samples * 120 + soak_seconds
        try:
            deadline = time.monotonic() + timeout
            while process.poll() is None:
                if time.monotonic() >= deadline:
                    raise subprocess.TimeoutExpired(COMMAND, timeout)
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
    return metadata


def _run(output: Path, samples: int, baseline: Path | None, soak_seconds: int, cli: Path) -> int:
    output.mkdir(parents=True, exist_ok=False)
    metadata = {
        "schema": 1,
        "cli_sha256": hashlib.sha256(cli.read_bytes()).hexdigest(),
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
                    "rust/loopflow/src/performance.rs",
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
        try:
            metadata.update(_run_native(output, samples, soak_seconds, cli))
        except (OSError, RuntimeError, ValueError) as error:
            metadata.update(outcome="failed", reason=str(error), exit_code=None)
    metadata["source_after"] = _sources()
    metadata["cli_sha256_after"] = hashlib.sha256(cli.read_bytes()).hexdigest()
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
        "--cli", type=Path, required=True, help="Source CLI for the owned native Session fixture"
    )
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
    volume = commands.add_parser(
        "volume", help="Summarize instrumented CLI receipts; launches nothing"
    )
    volume.add_argument("directory", type=Path)
    args = parser.parse_args()
    if args.command == "volume":
        result = _cli_volume(args.directory)
        print(json.dumps(result, indent=2))
        return 0 if result["status"] == "complete" else 1
    if args.command == "run":
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
        )
    summary = _report(args.output, args.baseline)
    print(args.output / "report.md")
    return 0 if summary["status"] == "complete" else 1


if __name__ == "__main__":
    raise SystemExit(main())
