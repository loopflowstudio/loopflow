#!/usr/bin/env python3
"""Report the timings Desktop recorded for its real launches and `lf` reads.

uv run python scripts/benchmarks/desktop-performance/timings.py
uv run python scripts/benchmarks/desktop-performance/timings.py --json
uv run python scripts/benchmarks/desktop-performance/timings.py --home /path/to/home

Reads `<Home>/desktop-cache/timings/{launches,reads}.ndjson`, which the app
appends to while it runs. Launch milliseconds count from kernel process start.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import statistics
from collections import Counter
from pathlib import Path

BUDGETS_MS = {"first_frame": 400, "usable_saved": 1000, "usable_fresh": 1000}
LAUNCH_METRICS = ["pre_main", "restored", "first_frame", "usable_saved", "usable_fresh", "fresh"]


def _lines(path: Path) -> list[dict]:
    if not path.exists():
        return []
    records = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(record, dict) and isinstance(record.get("ms"), (int, float)):
            records.append(record)
    return records


def _stats(values: list[float]) -> dict:
    return {
        "samples": len(values),
        "median_ms": round(statistics.median(values), 1) if values else None,
        "p95_ms": round(sorted(values)[math.ceil(len(values) * 0.95) - 1], 1)
        if len(values) >= 20
        else None,
        "max_ms": round(max(values), 1) if values else None,
    }


def _launches(records: list[dict]) -> dict:
    launches: dict[str, dict] = {}
    for record in records:
        launches.setdefault(record.get("launch", ""), {})[record.get("event")] = record
    values: dict[str, list[float]] = {name: [] for name in LAUNCH_METRICS}
    cache: Counter = Counter()
    failed: Counter = Counter()
    for events in launches.values():
        for event, record in events.items():
            name = {"launch": "pre_main", "usable": f"usable_{record.get('source')}"}.get(
                event, event
            )
            if name in values:
                values[name].append(record["ms"])
        if "restored" in events:
            cache[events["restored"].get("cache")] += 1
        if "refresh_failed" in events:
            failed[events["refresh_failed"].get("part")] += 1
    metrics = {name: _stats(samples) for name, samples in values.items()}
    for name, budget in BUDGETS_MS.items():
        metrics[name]["budget_ms"] = budget
    return {
        "launches": len(launches),
        "cache": dict(cache),
        "metrics": metrics,
        "never_first_frame": sum("first_frame" not in events for events in launches.values()),
        "never_usable": sum("usable" not in events for events in launches.values()),
        "never_fresh": sum("fresh" not in events for events in launches.values()),
        "refresh_failed_before_fresh": dict(failed),
    }


def _durations(records: list[dict], event: str, key: str) -> dict:
    rows = [record for record in records if record.get("event") == event]
    report = {}
    for name in sorted({row.get(key, "") for row in rows}):
        named = [row for row in rows if row.get(key, "") == name]
        report[name] = {
            **_stats([row["ms"] for row in named if row.get("ok")]),
            "failed": sum(not row.get("ok") for row in named),
        }
    return report


def summarize(directory: Path) -> dict:
    launches = _lines(directory / "launches.ndjson")
    reads = _lines(directory / "reads.ndjson")
    versions = {}
    for app in dict.fromkeys(record.get("app", "") for record in launches + reads):
        mine = [record for record in reads if record.get("app", "") == app]
        versions[app] = {
            **_launches([record for record in launches if record.get("app", "") == app]),
            "reads": _durations(mine, "read", "verb"),
            "refreshes": _durations(mine, "refresh", "part"),
        }
    return {
        "source": str(directory),
        "endpoints": {
            "first_frame": "first window content committed to the render server; not on-glass presentation",
            "usable": "outline rows observed, next main-queue callback; saved or fresh by source",
            "fresh": "every part of the workspace read by this launch",
        },
        "not_measured": ["CPU", "memory", "main-thread stalls", "on-glass presentation"],
        "versions": versions,
    }


def _cell(stats: dict) -> str:
    def ms(value: float | None) -> str:
        return "—" if value is None else f"{value:.0f}"

    return f"{stats['samples']:>7} {ms(stats['median_ms']):>9} {ms(stats['p95_ms']):>9} {ms(stats['max_ms']):>9}"


def render(report: dict) -> str:
    lines = [f"Desktop timings from {report['source']}"]
    if not report["versions"]:
        return lines[0] + "\nNo launches recorded yet. Open Desktop, then run this again."
    header = f"{'':24} {'samples':>7} {'median ms':>9} {'p95 ms':>9} {'max ms':>9}"
    for app, version in report["versions"].items():
        cache = (
            ", ".join(f"{name} {count}" for name, count in sorted(version["cache"].items()))
            or "none"
        )
        lines += [
            "",
            f"app {app}: {version['launches']} launches; saved workspace: {cache}",
            header,
        ]
        for name, stats in version["metrics"].items():
            budget = f"  budget {stats['budget_ms']}" if "budget_ms" in stats else ""
            lines.append(f"{name:24} {_cell(stats)}{budget}")
        failed = ", ".join(
            f"{part} {count}"
            for part, count in sorted(version["refresh_failed_before_fresh"].items())
        )
        lines.append(
            f"launches that never reached: first frame {version['never_first_frame']}, "
            f"usable {version['never_usable']}, fresh {version['never_fresh']}; "
            f"refresh failed before fresh: {failed or 'none'}"
        )
        for title, key in (("lf read", "reads"), ("refresh", "refreshes")):
            if version[key]:
                lines += [
                    "",
                    f"{title:24} {'ok':>7} {'median ms':>9} {'p95 ms':>9} {'max ms':>9} {'failed':>7}",
                ]
            for name, stats in version[key].items():
                lines.append(f"{name:24} {_cell(stats)} {stats['failed']:>7}")
    lines += [
        "",
        "p95 needs 20 samples. A launch still running, or quit early, counts as never reaching what it lacks.",
        "Not measured: " + ", ".join(report["not_measured"]) + ".",
    ]
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--home", type=Path, default=Path(os.environ.get("LF_HOME") or Path.home() / ".lf")
    )
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    report = summarize(args.home.expanduser() / "desktop-cache" / "timings")
    print(json.dumps(report, indent=2) if args.json else render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
