#!/usr/bin/env python3
"""Record the running Loopflow app's own signposts, frame hitches and memory.

uv run python scripts/benchmarks/desktop-performance/record_live.py record --seconds 60
uv run python scripts/benchmarks/desktop-performance/record_live.py summarize <dir>

Nothing leaves the machine: signposts come from the unified log, hitches from a
local xctrace file, memory from `ps`. Interval names are the catalogue in
this directory's README.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import re
import shutil
import signal
import statistics
import subprocess
import sys
import threading
import time
import xml.etree.ElementTree as ElementTree
from datetime import datetime
from pathlib import Path

SUBSYSTEM = "studio.loopflow"
CATEGORY = "perf"
HITCH_TABLES = ("hitches", "potential-hangs")
LEGACY_METRIC = re.compile(r"metric=(\w+).*?value_ms=([\d.]+)")


# ---------------------------------------------------------------------------
# Recording


def _pid(process: str, pid: int | None) -> int:
    if pid:
        return pid
    found = subprocess.run(["pgrep", "-x", process], capture_output=True, text=True).stdout.split()
    if not found:
        raise SystemExit(f"{process} is not running; open the app first (or pass --pid)")
    return int(found[0])


def _log_show(pid: int, start: datetime, end: datetime, path: Path) -> None:
    # Two builds can run under the same process name (installed and demo), so filter by pid.
    fmt = "%Y-%m-%d %H:%M:%S"
    command = [
        "/usr/bin/log",
        "show",
        "--signpost",
        "--style",
        "ndjson",
        "--start",
        start.strftime(fmt),
        "--end",
        end.strftime(fmt),
        "--predicate",
        f'subsystem == "{SUBSYSTEM}" AND processID == {pid}',
    ]
    with path.open("w", encoding="utf-8") as out:
        subprocess.run(command, stdout=out, stderr=subprocess.DEVNULL, check=False)


def _xctrace(
    pid: int, seconds: int, trace: Path, template: str, instruments: tuple[str, ...] = ()
) -> str | None:
    if not shutil.which("xcrun"):
        return "xcrun is unavailable"
    command = [
        "xcrun",
        "xctrace",
        "record",
        "--template",
        template,
        "--attach",
        str(pid),
        "--time-limit",
        f"{seconds}s",
        "--output",
        str(trace),
    ]
    for instrument in instruments:
        command.extend(["--instrument", instrument])
    command.append("--no-prompt")
    # A tail window discards soak coverage. Bound resource use instead, and retain
    # an interrupted recording as failed evidence, never a shorter successful soak.
    temporary = trace.parent / "trace-tmp"
    temporary.mkdir(exist_ok=False)
    environment = dict(os.environ, TMPDIR=str(temporary.resolve()) + "/")
    initial_free = shutil.disk_usage(trace.parent).free
    reserve = 6 * 1024**3
    allowance = 2 * 1024**3
    receipt = {
        "command": command,
        "temporary_directory_requested": str(temporary.resolve()),
        "initial_free_bytes": initial_free,
        "reserve_bytes": reserve,
        "maximum_volume_consumption_bytes": allowance,
        "status": "starting",
    }
    receipt_path = trace.parent / "trace-recording.json"

    def save() -> None:
        receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")

    save()
    if initial_free < reserve + allowance:
        receipt.update(status="refused", reason="insufficient recording headroom")
        save()
        return str(receipt["reason"])
    started = time.monotonic()
    reason = None
    observed_paths: dict[str, int] = {}
    next_paths = started
    with (trace.parent / "xctrace.log").open("w") as log:
        process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT, env=environment)
        receipt["pid"] = process.pid
        try:
            while process.poll() is None:
                free = shutil.disk_usage(trace.parent).free
                receipt.update(
                    status="recording", free_bytes=free, elapsed_seconds=time.monotonic() - started
                )
                save()
                if free < reserve or initial_free - free >= allowance:
                    reason = "recording stopped at storage limit"
                    break
                if time.monotonic() - started > seconds + 30:
                    reason = "recording exceeded duration and shutdown allowance"
                    break
                if time.monotonic() >= next_paths:
                    # TMPDIR is only a request: Instruments can open its raw
                    # ktrace in the account temp directory. Retain exact open
                    # descriptors while this child is still ours to observe.
                    try:
                        opened = subprocess.run(
                            ["lsof", "-a", "-p", str(process.pid), "-Fn"],
                            capture_output=True,
                            text=True,
                            timeout=2,
                            check=False,
                        )
                        for line in opened.stdout.splitlines():
                            if line.startswith("n/") and line.endswith(".ktrace"):
                                path = Path(line[1:])
                                try:
                                    observed_paths[str(path)] = path.stat().st_size
                                except OSError:
                                    pass
                        receipt["observed_raw_trace_bytes"] = observed_paths
                    except (OSError, subprocess.TimeoutExpired) as error:
                        receipt["path_observation_error"] = str(error)
                    next_paths = time.monotonic() + 1
                time.sleep(0.25)
        finally:
            if process.poll() is None:
                # Only our unreaped child: never infer authority over Instruments
                # services or other recorders from their names or open paths.
                process.send_signal(signal.SIGINT)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            receipt.update(
                exit_code=process.returncode,
                reason=reason,
                status="failed" if reason or process.returncode else "finished",
            )
            save()
    if reason:
        return reason
    if process.returncode != 0 or not trace.exists():
        return "xctrace failed; see xctrace.log and trace-recording.json"
    return None


def _export(trace: Path, table: str) -> str:
    xpath = f'/trace-toc/run[@number="1"]/data/table[@schema="{table}"]'
    return subprocess.run(
        ["xcrun", "xctrace", "export", "--input", str(trace), "--xpath", xpath],
        capture_output=True,
        text=True,
        check=False,
    ).stdout


def _sample_resources(pid: int, seconds: int, path: Path, stop: threading.Event) -> None:
    with path.open("w", encoding="utf-8") as out:
        for _ in range(seconds):
            if stop.is_set():
                break
            reading = subprocess.run(
                ["ps", "-o", "rss=,%cpu=", "-p", str(pid)],
                capture_output=True,
                text=True,
            ).stdout.split()
            if len(reading) != 2:
                break
            out.write(
                json.dumps(
                    {"t": time.time(), "rss_kib": int(reading[0]), "cpu_percent": float(reading[1])}
                )
                + "\n"
            )
            out.flush()
            stop.wait(1)


def record(args: argparse.Namespace) -> Path:
    pid = _pid(args.process, args.pid)
    output = Path(args.output or f"/tmp/loopflow-live-{datetime.now():%Y%m%d-%H%M%S}")
    output.mkdir(parents=True, exist_ok=True)
    start = datetime.now()
    print(
        f"Recording {args.process} (pid {pid}) for {args.seconds}s into {output}. "
        "Use the app normally."
    )
    trace_error: str | None = "skipped (--no-xctrace)"
    stop = threading.Event()
    if args.xctrace:
        sampler = threading.Thread(
            target=_sample_resources,
            args=(pid, args.seconds, output / "rss.jsonl", stop),
            daemon=True,
        )
        sampler.start()
        trace_error = _xctrace(
            pid, args.seconds, output / "hitches.trace", args.template, tuple(args.instrument)
        )
        if trace_error:
            stop.set()
        sampler.join()
    else:
        _sample_resources(pid, args.seconds, output / "rss.jsonl", stop)
    end = datetime.now()
    time.sleep(2)  # logd flushes signposts a moment after they are emitted.
    _log_show(pid, start, end, output / "signposts.ndjson")
    trace = output / "hitches.trace"
    if trace.exists():
        toc = subprocess.run(
            ["xcrun", "xctrace", "export", "--input", str(trace), "--toc"],
            capture_output=True,
            text=True,
            check=False,
        )
        (output / "trace-toc.xml").write_text(toc.stdout, encoding="utf-8")
        for table in HITCH_TABLES:
            (output / f"{table}.xml").write_text(_export(trace, table), encoding="utf-8")
    (output / "run.json").write_text(
        json.dumps(
            {
                "process": args.process,
                "pid": pid,
                "seconds": args.seconds,
                "started_at": start.isoformat(),
                "ended_at": end.isoformat(),
                "xctrace": trace_error or "recorded",
                "template": args.template,
                "instruments": args.instrument,
                "host": _host(),
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    if args.phases and args.phases.exists():
        shutil.copyfile(args.phases, output / "phases.jsonl")
    return output


def _host() -> dict:
    version = subprocess.run(
        ["sw_vers", "-productVersion"], capture_output=True, text=True
    ).stdout.strip()
    chip = subprocess.run(
        ["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True
    ).stdout.strip()
    return {"macos": version, "cpu": chip}


# ---------------------------------------------------------------------------
# Summarizing


def _percentile(values: list[float], fraction: float) -> float:
    ordered = sorted(values)
    return ordered[max(0, math.ceil(len(ordered) * fraction) - 1)]


def _stamp(entry: dict) -> float:
    # "2026-09-26 11:36:50.079764-0700" — microseconds are enough for millisecond metrics.
    return datetime.strptime(entry["timestamp"], "%Y-%m-%d %H:%M:%S.%f%z").timestamp()


def intervals(lines: list[str]) -> tuple[list[dict], list[dict]]:
    """Pair begin/end signposts by (name, id). Returns (intervals, legacy log metrics)."""
    open_by_key: dict[tuple[str, int], dict] = {}
    closed: list[dict] = []
    legacy: list[dict] = []
    for line in lines:
        line = line.strip()
        if not line.startswith("{"):
            continue
        entry = json.loads(line)
        if entry.get("eventType") != "signpostEvent":
            match = LEGACY_METRIC.search(entry.get("eventMessage", ""))
            if match:
                legacy.append({"name": match.group(1), "ms": float(match.group(2))})
            continue
        key = (entry["signpostName"], entry["signpostID"])
        kind = entry.get("signpostType")
        message = entry.get("eventMessage", "")
        if kind == "begin":
            open_by_key[key] = {"name": key[0], "begin": _stamp(entry), "message": message}
        elif kind == "end" and key in open_by_key:
            started = open_by_key.pop(key)
            closed.append(
                {
                    "name": started["name"],
                    "scenario": _field(started["message"], "scenario")
                    or started["message"].strip()
                    or "-",
                    "ms": (_stamp(entry) - started["begin"]) * 1000,
                    "outcome": message or "ready",
                }
            )
    return closed, legacy


def _field(message: str, name: str) -> str | None:
    match = re.search(rf"{name}=(\S+)", message)
    return match.group(1) if match else None


def _trace_rows(output: Path, table: str, recorded: bool) -> list[dict] | None:
    path = output / f"{table}.xml"
    if not recorded or not path.exists():
        return None
    try:
        root = ElementTree.fromstring(path.read_text(encoding="utf-8"))
    except ElementTree.ParseError:
        return None
    if not any(schema.get("name") == table for schema in root.iter("schema")):
        return None

    rows = []
    values: dict[str, str] = {}

    def text(element: ElementTree.Element | None) -> str | None:
        # xctrace writes a repeated value once with id="N" and later as ref="N".
        if element is None:
            return None
        if element.text is not None:
            if element.get("id"):
                values[element.get("id", "")] = element.text
            return element.text
        return values.get(element.get("ref", ""))

    for row in root.iter("row"):
        start = text(row.find("start-time"))
        duration = text(row.find("duration"))
        if start is None or duration is None:
            continue
        rows.append(
            {
                "start_ms": int(start) / 1e6,
                "duration_ms": int(duration) / 1e6,
                "kind": text(row.find("hang-type")) or "hitch",
            }
        )
    return rows


def _idle(output: Path, frames: list[dict] | None, hangs: list[dict] | None) -> dict:
    phases = output / "phases.jsonl"
    result = {
        "requested_seconds": None,
        "covered_seconds": None,
        "hitch_ms_per_second": None,
        "hangs": None,
    }
    if not phases.exists():
        return result
    events = []
    for line in phases.read_text().splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("event") in {"soak_phase", "soak_end"} and "time" in event:
            events.append(event)
    intervals = [
        (a["time"], b["time"])
        for a, b in zip(events, events[1:])
        if a.get("phase") == "idle" and b["time"] >= a["time"]
    ]
    if not intervals:
        return result
    result["requested_seconds"] = sum(end - start for start, end in intervals)
    toc = output / "trace-toc.xml"
    if not toc.exists():
        return result
    try:
        root = ElementTree.fromstring(toc.read_text())
        info = root.find(".//run[@number='1']/info/run-info")
        if info is None:
            return result
        start_date, end_date = info.findtext("start-date"), info.findtext("end-date")
        if not start_date or not end_date:
            return result
        start = datetime.fromisoformat(start_date.replace("Z", "+00:00"))
        end = datetime.fromisoformat(end_date.replace("Z", "+00:00"))
        if start.tzinfo is None or end.tzinfo is None:
            return result
        origin, finish = start.timestamp(), end.timestamp()
    except (ElementTree.ParseError, ValueError):
        return result
    covered = [
        (max(a, origin), min(b, finish)) for a, b in intervals if min(b, finish) > max(a, origin)
    ]
    seconds = sum(b - a for a, b in covered)
    if seconds <= 0:
        return result
    result["covered_seconds"] = seconds

    def overlap(row: dict) -> float:
        begin = origin + row["start_ms"] / 1000
        end = begin + row["duration_ms"] / 1000
        return sum(max(0, min(b, end) - max(a, begin)) for a, b in covered)

    if frames is not None:
        result["hitch_ms_per_second"] = sum(overlap(row) * 1000 for row in frames) / seconds
    if hangs is not None:
        result["hangs"] = sum(overlap(row) > 0 for row in hangs)
    return result


def summarize(output: Path) -> dict:
    run = json.loads((output / "run.json").read_text(encoding="utf-8"))
    closed, legacy = intervals(
        (output / "signposts.ndjson").read_text(encoding="utf-8").splitlines()
        if (output / "signposts.ndjson").exists()
        else []
    )
    groups: dict[tuple[str, str], list[float]] = {}
    superseded: dict[tuple[str, str], int] = {}
    for interval in closed:
        key = (interval["name"], interval["scenario"])
        groups.setdefault(key, [])
        if interval["outcome"] == "superseded":
            superseded[key] = superseded.get(key, 0) + 1
            continue
        groups[key].append(interval["ms"])
    for metric in legacy:
        groups.setdefault((metric["name"], "log"), []).append(metric["ms"])
    rows = [
        {
            "name": name,
            "scenario": scenario,
            "n": len(values),
            "p50_ms": round(statistics.median(values), 2) if values else None,
            "p95_ms": round(_percentile(values, 0.95), 2) if len(values) >= 20 else None,
            "max_ms": round(max(values), 2) if values else None,
            "superseded": superseded.get((name, scenario), 0),
        }
        for (name, scenario), values in sorted(groups.items())
    ]
    rss = (
        [
            json.loads(line)
            for line in (output / "rss.jsonl").read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        if (output / "rss.jsonl").exists()
        else []
    )
    memory = {
        "samples": len(rss),
        "start_mib": round(rss[0]["rss_kib"] / 1024, 1) if rss else None,
        "end_mib": round(rss[-1]["rss_kib"] / 1024, 1) if rss else None,
        "max_mib": round(max(sample["rss_kib"] for sample in rss) / 1024, 1) if rss else None,
        "growth_mib": round((rss[-1]["rss_kib"] - rss[0]["rss_kib"]) / 1024, 1)
        if len(rss) > 1
        else None,
    }
    cpu_values = [sample["cpu_percent"] for sample in rss if "cpu_percent" in sample]
    cpu = {
        "samples": len(cpu_values),
        "p50_percent": statistics.median(cpu_values) if cpu_values else None,
        "p95_percent": _percentile(cpu_values, 0.95) if len(cpu_values) >= 20 else None,
        "max_percent": max(cpu_values) if cpu_values else None,
    }
    seconds = float(run["seconds"])
    recorded = run.get("xctrace") == "recorded"
    frames = _trace_rows(output, "hitches", recorded)
    hangs = _trace_rows(output, "potential-hangs", recorded)
    hitch_summary = {
        "recorded": frames is not None,
        "count": len(frames) if frames is not None else None,
        "hitch_ms_per_second": round(sum(frame["duration_ms"] for frame in frames) / seconds, 2)
        if frames is not None
        else None,
        "worst_ms": round(max((frame["duration_ms"] for frame in frames), default=0.0), 2)
        if frames is not None
        else None,
        "hangs": len(hangs) if hangs is not None else None,
        "worst_hang_ms": round(max((hang["duration_ms"] for hang in hangs), default=0.0), 2)
        if hangs is not None
        else None,
    }
    report = {
        "run": run,
        "intervals": rows,
        "memory": memory,
        "cpu": cpu,
        "hitches": hitch_summary,
        "idle": _idle(output, frames, hangs),
    }
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "report.md").write_text(_markdown(report), encoding="utf-8")
    return report


def _markdown(report: dict) -> str:
    run = report["run"]
    lines = [
        f"# Live desktop recording — {run['started_at'][:19]}",
        "",
        f"Process `{run['process']}` (pid {run['pid']}), {run['seconds']} s, "
        f"macOS {run['host']['macos']}, {run['host']['cpu']}. xctrace: {run['xctrace']}.",
        "Signposts are the app's own `studio.loopflow`/`perf` intervals. Navigation intervals "
        "end at a queued main-thread callback, not a render or presentation fence. "
        "Hitch time is Apple's Animation Hitches "
        "table for this process: ≤5 ms/s good, 5–10 warning, >10 critical.",
        "p95 requires 20 samples. CPU is ps's sampled process percentage; "
        "it excludes child processes and is not an interval CPU-time measurement.",
        "",
        "| Interval | Scenario | n | p50 ms | p95 ms | max ms | superseded |",
        "|---|---|---:|---:|---:|---:|---:|",
    ]
    for row in report["intervals"]:
        lines.append(
            f"| {row['name']} | {row['scenario']} | {row['n']} | "
            f"{row['p50_ms']} | {row['p95_ms']} | "
            f"{row['max_ms']} | {row['superseded']} |"
        )
    if not report["intervals"]:
        lines.append("| — | no signposts in the window | 0 | | | | |")
    hitch = report["hitches"]
    memory = report["memory"]
    cpu = report["cpu"]
    lines += [
        "",
        f"Hitches: {hitch['count']} ({hitch['hitch_ms_per_second']} ms/s, "
        f"worst {hitch['worst_ms']} ms)."
        if hitch["recorded"]
        else "Hitches: not recorded.",
        f"Potential hangs: {hitch['hangs']} (worst {hitch['worst_hang_ms']} ms)."
        if hitch["hangs"] is not None
        else "Potential hangs: not recorded.",
        f"RSS: start {memory['start_mib']} MiB, end {memory['end_mib']} MiB, "
        f"max {memory['max_mib']} MiB, "
        f"growth {memory['growth_mib']} MiB over {memory['samples']} samples.",
        f"CPU: p50 {cpu['p50_percent']}%, p95 {cpu['p95_percent']}%, max {cpu['max_percent']}% "
        f"over {cpu['samples']} samples."
        if cpu["samples"]
        else "CPU: not recorded.",
        "",
    ]
    idle = report["idle"]
    if idle["requested_seconds"] is not None:
        lines += [
            f"Idle trace coverage: {idle['covered_seconds']} / {idle['requested_seconds']} s; "
            f"hitches {idle['hitch_ms_per_second']} ms/s; potential hangs {idle['hangs']}. "
            "Missing trace clock or tables is unmeasured; partial coverage is not acceptance."
        ]
    return "\n".join(lines)


# ---------------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    commands = parser.add_subparsers(dest="command", required=True)
    rec = commands.add_parser("record", help="record the running app, then summarize")
    rec.add_argument("--seconds", type=int, default=60)
    rec.add_argument(
        "--process",
        default="Loopflow",
        help="process name: Loopflow (installed) or LoopflowMac (dev)",
    )
    rec.add_argument("--pid", type=int)
    rec.add_argument("--output")
    rec.add_argument("--phases", type=Path, help="Native soak journal for idle-only trace analysis")
    rec.add_argument(
        "--no-xctrace", dest="xctrace", action="store_false", help="signposts and RSS only"
    )
    rec.add_argument(
        "--template",
        default="Animation Hitches",
        help="xctrace template; 'Time Profiler' attributes main-thread hangs "
        "(open hitches.trace in Instruments)",
    )
    rec.add_argument(
        "--instrument", action="append", default=[], help="Additional xctrace instrument"
    )
    summ = commands.add_parser("summarize", help="rebuild report.json/report.md from a recording")
    summ.add_argument("directory")
    args = parser.parse_args(argv)
    output = record(args) if args.command == "record" else Path(args.directory)
    report = summarize(output)
    print((output / "report.md").read_text(encoding="utf-8"))
    print(f"Report: {output / 'report.md'}")
    if args.command == "record" and args.xctrace and report["run"]["xctrace"] != "recorded":
        return 1
    return 0 if report else 1


if __name__ == "__main__":
    sys.exit(main())
