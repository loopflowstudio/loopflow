#!/usr/bin/env python3
"""Measure Desktop startup from captured `lf` reads, without a display.

uv run python scripts/benchmarks/desktop-performance/startup.py capture --repo ~/src/loopflow --output /tmp/startup-capture
uv run python scripts/benchmarks/desktop-performance/startup.py run --capture /tmp/startup-capture --output /tmp/startup-run

`capture` runs the app's read-only startup verbs once against the current Home
and records their output and wall time; the capture holds private planning
text, so keep it out of the repository. `run` replays it in-process and writes
`journal.ndjson`, `report.json` and `report.md`. The endpoint is the model
holding outline content, not a rendered frame.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import statistics
import subprocess
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
BUDGET_USABLE_MS = 1000


def _lf(args: list[str], cwd: Path) -> tuple[str, float]:
    start = time.monotonic()
    done = subprocess.run(["lf", *args], cwd=cwd, capture_output=True, text=True, check=True)
    return done.stdout, (time.monotonic() - start) * 1000


def capture(repo: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    reads = []

    def record(verb: str, name: str, args: list[str]) -> str:
        text, ms = _lf(args, repo)
        (output / name).write_text(text, encoding="utf-8")
        reads.append({"verb": verb, "file": name, "ms": round(ms, 1), "bytes": len(text.encode())})
        return text

    home = json.loads(record("home", "home.json", ["home", "id", "--json"]))["id"]
    record("wave", "waves.json", ["wave", "list", "--all", "--current", "--json"])
    record("roadmap", "roadmap.json", ["roadmap", "--all", "--json"])
    record("activity", "activity.json", ["activity", "--since", "7d", "--limit", "50", "--json"])
    after, page = None, 0
    while True:
        args = ["session", "list", "--json", "--page", "--limit", "100"]
        if after:
            args += ["--after", after]
        after = json.loads(record("session", f"sessions-{page}.json", args))["next"]
        page += 1
        if not after:
            break
    version = subprocess.run(["lf", "--version"], capture_output=True, text=True).stdout.strip()
    manifest = {"repo": str(repo), "homeId": home, "lf": version, "reads": reads}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    for read in reads:
        print(f"{read['verb']:9} {read['ms']:9.0f} ms {read['bytes']:8} B")


def _p95(values: list[float]) -> float | None:
    return round(sorted(values)[math.ceil(len(values) * 0.95) - 1], 1) if len(values) >= 20 else None


def summarize(output: Path, manifest: dict) -> dict:
    records = [json.loads(line) for line in (output / "journal.ndjson").read_text().splitlines() if line]
    scenarios = []
    for name in dict.fromkeys(record["scenario"] for record in records):
        rows = [record for record in records if record["scenario"] == name]

        def stats(key: str) -> dict:
            values = [row[key] for row in rows if row[key] is not None]
            return {
                "samples": len(values),
                "median_ms": round(statistics.median(values), 1) if values else None,
                "p95_ms": _p95(values),
                "max_ms": round(max(values), 1) if values else None,
            }

        usable = stats("usable_ms")
        scenarios.append({
            "scenario": name,
            "attempted": len(rows),
            "usable_from": sorted({row["usable_from"] for row in rows}),
            "status": sorted({row["status"] for row in rows}),
            "content_kept": all(row["content_kept"] for row in rows),
            "init_main_thread": stats("init_main_thread_ms"),
            "usable": usable,
            "settled": stats("settled_ms"),
            "reads_finished_until_usable": rows[-1]["reads_finished_until_usable"],
            "reads_until_settled": rows[-1]["reads_until_settled"],
            "usable_within_budget": usable["max_ms"] is not None and usable["max_ms"] <= BUDGET_USABLE_MS,
        })
    report = {
        "endpoint": "model holds outline content (in-process); not a rendered frame",
        "not_measured": ["first frame", "process launch and pre-main", "CPU", "memory", "main-thread stalls after init"],
        "budget_usable_ms": BUDGET_USABLE_MS,
        "capture": {key: manifest[key] for key in ("lf", "reads")},
        "scenarios": scenarios,
    }
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    lines = [
        "| Scenario | Samples | Usable from | Usable median / p95 / max ms | Init on main thread median / p95 ms "
        "| Settled median ms | Status | `lf` reads before usable | `lf` reads until settled |",
        "|---|---|---|---|---|---|---|---|---|",
    ]

    def counts(reads: dict) -> str:
        return ", ".join(f"{verb} {count}" for verb, count in sorted(reads.items())) or "none"

    for row in scenarios:
        lines.append(
            f"| {row['scenario']} | {row['usable']['samples']} | {'/'.join(row['usable_from'])} "
            f"| {row['usable']['median_ms']} / {row['usable']['p95_ms']} / {row['usable']['max_ms']} "
            f"| {row['init_main_thread']['median_ms']} / {row['init_main_thread']['p95_ms']} "
            f"| {row['settled']['median_ms']} | {'/'.join(row['status'])} "
            f"| {counts(row['reads_finished_until_usable'] or {})} | {counts(row['reads_until_settled'])} |"
        )
    (output / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return report


def run(capture_dir: Path, output: Path, samples: int) -> int:
    output.mkdir(parents=True, exist_ok=True)
    environment = {
        **os.environ,
        "LF_DESKTOP_STARTUP_OUTPUT": str(output / "journal.ndjson"),
        "LF_DESKTOP_STARTUP_CAPTURE": str(capture_dir),
        "LF_DESKTOP_STARTUP_SAMPLES": str(samples),
    }
    command = [str(REPO / "scripts/test_desktop.sh"), "-Xswiftc", "-gnone", "--filter", "DesktopStartupTests"]
    done = subprocess.run(command, cwd=REPO, env=environment)
    if done.returncode != 0:
        return done.returncode
    summarize(output, json.loads((capture_dir / "manifest.json").read_text()))
    print((output / "report.md").read_text())
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    capturing = commands.add_parser("capture")
    capturing.add_argument("--repo", type=Path, required=True)
    capturing.add_argument("--output", type=Path, required=True)
    running = commands.add_parser("run")
    running.add_argument("--capture", type=Path, required=True)
    running.add_argument("--output", type=Path, required=True)
    running.add_argument("--samples", type=int, default=20)
    args = parser.parse_args()
    if args.command == "capture":
        capture(args.repo.expanduser().resolve(), args.output)
        return 0
    return run(args.capture, args.output, args.samples)


if __name__ == "__main__":
    raise SystemExit(main())
