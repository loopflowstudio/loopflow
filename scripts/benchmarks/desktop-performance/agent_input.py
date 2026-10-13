"""Capture one owned Codex/Claude Task-click-to-draft-response trial.

Start recording before clicking the Task. Mark visible input, the unsent probe,
and its visible response with saved native captures. Never probe retained drafts.
Raw local evidence stays in the output directory; no app or provider is launched.
"""

from __future__ import annotations

import argparse
import collections
import html
import json
import re
import shutil
import subprocess
import time
import xml.etree.ElementTree as ET
from datetime import datetime
from pathlib import Path

REQUIRED = {
    "provider",
    "provider_version",
    "case",
    "conditions",
    "app_revision",
    "helper_revision",
    "task",
    "session",
    "machine",
    "data",
    "variant",
}
PHASES = (
    "input_visible",
    "probe_sent",
    "input_response",
    "hook_review_begin",
    "hook_review_end",
    "failed",
)
LIMITS = [
    "Surface creation, focus and key-to-draw are intermediate events, never provider readiness.",
    "Endpoint captures are observer evidence; marker timestamps include capture/observer delay.",
    "Time Profiler samples local CPU; missing symbols and provider internals remain unknown.",
    "Process CPU deltas do not attribute lock, I/O, subprocess or network waits. "
    "Inspect the trace alongside wall gaps.",
    "All-process sampling and 100 ms process polling add overhead; "
    "compare the same recorder settings.",
]


def _json(path: Path) -> dict:
    return json.loads(path.read_text())


def _write(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n")


def _append(path: Path, value: dict) -> None:
    with path.open("a") as output:
        output.write(json.dumps(value) + "\n")


def _command(args: list[str], output: Path) -> int:
    with output.open("w") as handle:
        return subprocess.run(args, stdout=handle, stderr=subprocess.STDOUT, check=False).returncode


def _metadata(path: Path) -> dict:
    value = _json(path)
    missing = REQUIRED - value.keys()
    if missing or any(not value[key] for key in REQUIRED):
        raise ValueError(f"Nonempty metadata required: {sorted(missing or REQUIRED)}")
    if value["provider"] not in ("codex", "claude") or value["case"] not in (
        "cached",
        "resume",
        "fresh",
    ):
        raise ValueError("Choose codex/claude and cached/resume/fresh; run each separately")
    return value


def mark(directory: Path, phase: str, evidence: Path) -> None:
    if not evidence.is_file():
        raise ValueError("A saved native capture or failure receipt is required")
    # Reference existing evidence, preserving the time of observation rather than
    # manufacturing a provider endpoint from Ghostty's first draw.
    _append(
        directory / "markers.ndjson",
        {
            "phase": phase,
            "wall": time.time(),
            "monotonic": time.monotonic(),
            "evidence": str(evidence.resolve()),
        },
    )


def _processes(root: int, extra: list[int], seen: set[int]) -> list[dict]:
    result = subprocess.run(
        ["ps", "-axo", "pid=,ppid=,time=,comm="], capture_output=True, text=True, check=True
    )
    rows = []
    for line in result.stdout.splitlines():
        words = line.split(maxsplit=3)
        if len(words) != 4:
            continue
        pid, parent, cpu, command = words
        rows.append(
            {"pid": int(pid), "parent": int(parent), "cpu": cpu, "name": Path(command).name}
        )
    seen.update([root, *extra])
    while True:
        children = {row["pid"] for row in rows if row["parent"] in seen}
        if children <= seen:
            break
        seen.update(children)
    return [row for row in rows if row["pid"] in seen]


def record(
    directory: Path,
    metadata: Path,
    pid: int,
    extra: list[int],
    seconds: int,
    lf_events: Path | None = None,
) -> None:
    value = _metadata(metadata)
    directory.mkdir(mode=0o700, parents=True, exist_ok=False)
    _write(
        directory / "metadata.json",
        value | {"desktop_pid": pid, "provider_pids": extra, "seconds": seconds},
    )
    started = time.time()
    trace = directory / "cpu.trace"
    command = [
        "xcrun",
        "xctrace",
        "record",
        "--template",
        "Time Profiler",
        "--all-processes",
        "--time-limit",
        f"{seconds}s",
        "--output",
        str(trace),
        "--no-prompt",
    ]
    log = (directory / "recorder.log").open("w")
    profiler = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT)
    seen: set[int] = set()
    interrupted = False
    try:
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            _append(
                directory / "processes.ndjson",
                {"wall": time.time(), "processes": _processes(pid, extra, seen)},
            )
            time.sleep(0.1)
    except KeyboardInterrupt:
        interrupted = True
    finally:
        if interrupted and profiler.poll() is None:
            profiler.terminate()
        try:
            status = profiler.wait(timeout=30)
        except subprocess.TimeoutExpired:
            profiler.kill()
            status = profiler.wait()
        log.close()
        ended = time.time()
        _write(
            directory / "capture.json",
            {
                "start": started,
                "end": ended,
                "interrupted": interrupted,
                "xctrace_status": status,
                "pids": sorted(seen),
                "command": command,
            },
        )
        predicate = f'subsystem == "studio.loopflow" AND processID == {pid}'
        _command(
            [
                "/usr/bin/log",
                "show",
                "--signpost",
                "--style",
                "ndjson",
                "--start",
                datetime.fromtimestamp(started).strftime("%Y-%m-%d %H:%M:%S"),
                "--end",
                datetime.fromtimestamp(ended).strftime("%Y-%m-%d %H:%M:%S"),
                "--predicate",
                predicate,
            ],
            directory / "signposts.ndjson",
        )
        if lf_events is not None and lf_events.exists():
            shutil.copyfile(lf_events, directory / "lf-events.ndjson")
        if status == 0:
            _command(
                ["xcrun", "xctrace", "export", "--input", str(trace), "--toc"],
                directory / "toc.xml",
            )
            _command(
                [
                    "xcrun",
                    "xctrace",
                    "export",
                    "--input",
                    str(trace),
                    "--xpath",
                    '/trace-toc/run/data/table[@schema="time-profile"]',
                ],
                directory / "cpu.xml",
            )
        summarize(directory)


def _lines(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line.startswith("{")]


def events(directory: Path) -> list[dict]:
    rows = _lines(directory / "markers.ndjson") + _lines(directory / "lf-events.ndjson")
    for line in _lines(directory / "signposts.ndjson"):
        message = line.get("eventMessage", "")
        if "phase=" not in message or "wall=" not in message:
            if line.get("signpostName") == "lf" and line.get("timestamp"):
                rows.append(
                    {
                        "phase": "lf_" + line.get("signpostType", "unknown"),
                        "wall": datetime.fromisoformat(line["timestamp"]).timestamp(),
                        "interval": line.get("signpostID"),
                        "detail": message,
                    }
                )
            continue
        fields = dict(re.findall(r"(phase|trace|task|session|wall)=([^ ]+)", message))
        fields["wall"] = float(fields["wall"])
        rows.append(fields)
    return sorted(rows, key=lambda row: row["wall"])


def folded_stacks(path: Path, pids: set[int]) -> collections.Counter[str]:
    """Read xctrace's time-profile export, including shared element references."""
    root = ET.parse(path).getroot()
    ids = {node.attrib["id"]: node for node in root.iter() if "id" in node.attrib}

    def resolve(node: ET.Element) -> ET.Element:
        return ids.get(node.get("ref"), node)

    stacks: collections.Counter[str] = collections.Counter()
    for row in root.iter("row"):
        state = next((resolve(node) for node in row.iter("thread-state")), None)
        if state is None or (state.text or state.get("fmt")) != "Running":
            continue
        # Process may be a column or nested in the sampled thread.
        process = next((resolve(node) for node in row.iter("process")), None)
        if process is None:
            thread = next((resolve(node) for node in row.iter("thread")), None)
            if thread is not None:
                process = next((resolve(node) for node in thread.iter("process")), None)
        if process is None:
            continue
        pid_node = process.find("pid")
        if pid_node is None or int(resolve(pid_node).text or "-1") not in pids:
            continue
        backtrace = next((resolve(node) for node in row.iter("backtrace")), None)
        if backtrace is None:
            continue
        frames = [
            resolve(node).get("name") or resolve(node).get("fmt") or "[unsymbolicated]"
            for node in backtrace.iter("frame")
        ]
        if not frames:
            continue
        name = process.get("fmt", "process")
        # xctrace exports leaf first. Counts are samples, never invented CPU ms.
        stack = ";".join([name, *reversed(frames)]).replace("\n", " ")
        stacks[stack] += 1
    return stacks


def flamegraph(stacks: collections.Counter[str], path: Path) -> None:
    total = sum(stacks.values())
    if not total:
        return
    nodes: dict[tuple[str, ...], int] = collections.Counter()
    for stack, count in stacks.items():
        frames = tuple(stack.split(";"))
        for depth in range(1, len(frames) + 1):
            nodes[frames[:depth]] += count
    children: dict[tuple[str, ...], list[tuple[str, ...]]] = collections.defaultdict(list)
    for node in sorted(nodes):
        children[node[:-1]].append(node)
    height = 40 + 20 * max(map(len, nodes))
    svg = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="{height}" '
        f'viewBox="0 0 1200 {height}">',
        '<text x="10" y="18">Local CPU samples; hover for stacks. '
        "Wall waits are not CPU samples.</text>",
    ]

    def draw(parent: tuple[str, ...], x: float) -> None:
        for node in children[parent]:
            width = nodes[node] / total * 1180
            y = height - 20 * len(node)
            title = html.escape(" → ".join(node) + f" ({nodes[node]} samples)")
            svg.append(
                f'<g><title>{title}</title><rect x="{x:.2f}" y="{y}" width="{width:.2f}" '
                f'height="19" fill="hsl({len(node) * 31 % 60},75%,70%)" stroke="white"/>'
            )
            if width > 70:
                svg.append(
                    f'<text x="{x + 3:.2f}" y="{y + 14}" font-size="11">'
                    f"{html.escape(node[-1][: int(width / 7)])}</text>"
                )
            svg.append("</g>")
            draw(node, x)
            x += width

    draw((), 10)
    path.write_text("\n".join(svg + ["</svg>"]))


def _cpu_seconds(value: str) -> float:
    days, _, clock = value.rpartition("-")
    parts = clock.split(":")
    seconds = sum(float(part) * 60**index for index, part in enumerate(reversed(parts)))
    return seconds + (int(days) * 86400 if days else 0)


def _cpu_deltas(directory: Path) -> dict[int, float]:
    readings: dict[int, list[float]] = collections.defaultdict(list)
    for sample in _lines(directory / "processes.ndjson"):
        for process in sample["processes"]:
            readings[process["pid"]].append(_cpu_seconds(process["cpu"]))
    return {pid: max(0, values[-1] - values[0]) for pid, values in readings.items()}


def summarize(directory: Path) -> dict:
    metadata = _metadata(directory / "metadata.json")
    timeline = events(directory)
    timeline = [
        row
        for row in timeline
        if (not row.get("session") or row["session"] == metadata["session"])
        and (not row.get("task") or row["task"] == metadata["task"])
    ]
    clicks = [
        row
        for row in timeline
        if row["phase"] == "task_click" and row.get("task") == metadata["task"]
    ]
    proof = {
        phase: [
            row
            for row in timeline
            if row["phase"] == phase and Path(row.get("evidence", "")).is_file()
        ]
        for phase in PHASES
    }
    elapsed = None
    status = "unproved"
    # A single trial per recording prevents an earlier or superseded click from
    # receiving another conversation's endpoint. Reject partial/failed evidence.
    if len(clicks) == 1 and not proof["failed"] and all(len(proof[p]) == 1 for p in PHASES[:3]):
        click, visible, probe, response = clicks[0], *(proof[p][0] for p in PHASES[:3])
        keys = [
            row
            for row in timeline
            if row["phase"] == "key_received"
            and row.get("session") == metadata["session"]
            and probe["wall"] <= row["wall"] <= response["wall"]
        ]
        draws = [
            row
            for row in timeline
            if row["phase"] == "key_draw"
            and row.get("session") == metadata["session"]
            and keys
            and keys[0]["wall"] <= row["wall"] <= response["wall"]
        ]
        if click["wall"] <= visible["wall"] <= probe["wall"] <= response["wall"] and draws:
            elapsed = (response["wall"] - click["wall"]) * 1000
            status = "observed; native captures require review"
    if proof["failed"] or any(row["phase"].endswith("_failed") for row in timeline):
        status = "failed"
    capture = _json(directory / "capture.json") if (directory / "capture.json").exists() else None
    if capture is None or (
        capture["interrupted"]
        or not clicks
        or any(
            not capture["start"] <= row["wall"] <= capture["end"]
            for row in timeline
            if row["phase"] in ("task_click", "input_response")
        )
    ):
        elapsed = None
        status = "unproved; recording does not cover the full interval"
    stacks: collections.Counter[str] = collections.Counter()
    cpu_error = None
    if (directory / "cpu.xml").exists():
        try:
            stacks = folded_stacks(
                directory / "cpu.xml", set(_json(directory / "capture.json")["pids"])
            )
        except (ET.ParseError, ValueError, KeyError) as error:
            cpu_error = str(error)
    if not stacks:
        cpu_error = (
            cpu_error
            or "No attributable CPU stacks; inspect recorder.log, toc.xml and the raw trace"
        )
    (directory / "cpu.folded").write_text("".join(f"{s} {n}\n" for s, n in sorted(stacks.items())))
    flamegraph(stacks, directory / "cpu.svg")
    pairs = (
        ("primary_begin", "primary_returned"),
        ("connect_prepare_begin", "connect_prepare_returned"),
        ("launch_lock_begin", "launch_lock_acquired"),
        ("provider_spawn_begin", "provider_spawn_returned"),
        ("hook_review_begin", "hook_review_end"),
    )
    durations = {}
    for begin, end in pairs:
        starts = [row for row in timeline if row["phase"] == begin]
        ends = [row for row in timeline if row["phase"] == end]
        durations[begin.removesuffix("_begin")] = (
            (ends[0]["wall"] - starts[0]["wall"]) * 1000
            if len(starts) == len(ends) == 1 and ends[0]["wall"] >= starts[0]["wall"]
            else None
        )
    report = {
        "observed_cpu_seconds_by_pid": _cpu_deltas(directory),
        "phase_durations_ms": durations,
        "metadata": metadata,
        "status": status,
        "click_to_observed_response_ms": elapsed,
        "timeline": timeline,
        "cpu_samples": sum(stacks.values()),
        "cpu_unavailable": cpu_error,
        "limitations": LIMITS,
    }
    _write(directory / "report.json", report)
    lines = [
        f"{metadata['provider']} / {metadata['case']} / {metadata['variant']}",
        status,
        f"Click → observed draft response: {elapsed if elapsed is not None else 'unmeasured'} ms",
        "",
        "Wall timeline (milliseconds from the Task click):",
    ]
    origin = clicks[0]["wall"] if len(clicks) == 1 else None
    lines.extend(
        f"{(row['wall'] - origin) * 1000:.1f}  {row['phase']}  {row.get('session', '')}"
        if origin is not None
        else f"{row['wall']}  {row['phase']}"
        for row in timeline
    )
    lines += [
        "",
        "Measured phases (ms; overlapping phases are not additive):",
        *[
            f"{name}: {duration if duration is not None else 'unmeasured'}"
            for name, duration in durations.items()
        ],
    ]
    lines += ["", f"CPU samples: {sum(stacks.values())}; {cpu_error or 'cpu.svg'}", *LIMITS]
    (directory / "report.md").write_text("\n".join(lines) + "\n")
    return report


def compare(before: Path, after: Path) -> dict:
    baseline, candidate = summarize(before), summarize(after)
    keys = ("provider", "provider_version", "case", "conditions", "machine", "data")
    mismatches = [key for key in keys if baseline["metadata"][key] != candidate["metadata"][key]]
    if mismatches:
        raise ValueError(f"Incomparable trials: {mismatches}")
    old, new = baseline["click_to_observed_response_ms"], candidate["click_to_observed_response_ms"]
    return {
        "before": str(before),
        "after": str(after),
        "provider": candidate["metadata"]["provider"],
        "case": candidate["metadata"]["case"],
        "before_ms": old,
        "after_ms": new,
        "delta_ms": new - old if old is not None and new is not None else None,
        "limitations": LIMITS,
        "statuses": [baseline["status"], candidate["status"]],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    capture = commands.add_parser("record")
    capture.add_argument("--metadata", type=Path, required=True)
    capture.add_argument("--pid", type=int, required=True)
    capture.add_argument("--provider-pid", type=int, action="append", default=[])
    capture.add_argument("--seconds", type=int, choices=range(1, 301), default=60)
    capture.add_argument("--output", type=Path, required=True)
    capture.add_argument("--lf-events", type=Path)
    marker = commands.add_parser("mark")
    marker.add_argument("directory", type=Path)
    marker.add_argument("phase", choices=PHASES)
    marker.add_argument("--evidence", type=Path, required=True)
    summary = commands.add_parser("summarize")
    summary.add_argument("directory", type=Path)
    comparison = commands.add_parser("compare")
    comparison.add_argument("before", type=Path)
    comparison.add_argument("after", type=Path)
    args = parser.parse_args()
    if args.command == "record":
        record(
            args.output, args.metadata, args.pid, args.provider_pid, args.seconds, args.lf_events
        )
    elif args.command == "mark":
        mark(args.directory, args.phase, args.evidence)
    elif args.command == "compare":
        print(json.dumps(compare(args.before, args.after), indent=2))
    else:
        summarize(args.directory)


if __name__ == "__main__":
    main()
