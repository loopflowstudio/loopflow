#!/usr/bin/env python3
"""Measure the Systems wave's north-star numbers and rank PR-pipeline bottlenecks.

North stars: billing, prod uptime, green-on-main, test time. This tool measures
what's observable from the repo and GitHub today (green-on-main, CI job time,
and — with --local — local suite time), ranks the bottleneck that gates every
PR, and emits a JSON snapshot for trending.

    uv run python scripts/measure_perf.py                 # CI snapshot + bottleneck
    uv run python scripts/measure_perf.py --local         # also time local suites
    uv run python scripts/measure_perf.py --json out.json # write a trend snapshot
"""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import time
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Optional

CI_WORKFLOW = "ci.yml"
REPO_ROOT = Path(__file__).resolve().parent.parent
BUDGET_FILE = REPO_ROOT / "deploy" / "budget.json"
# GitHub gives each repo a 10 GB Actions cache before it evicts the LRU entry.
GH_CACHE_CEILING_GB = 10.0

# Local suites, keyed by name -> command. Timed only under --local.
LOCAL_SUITES: dict[str, list[str]] = {
    "rust-test": ["cargo", "test", "--all", "--quiet"],
    "python-test": ["uv", "run", "pytest", "python/tests/", "-q"],
    "swift-test": ["swift", "test", "--package-path", "swift"],
}


def _gh_json(args: list[str]) -> object:
    out = subprocess.run(["gh", *args], capture_output=True, text=True, cwd=REPO_ROOT)
    if out.returncode != 0:
        raise RuntimeError(f"gh {' '.join(args)} failed: {out.stderr.strip()}")
    return json.loads(out.stdout)


def _seconds(started: Optional[str], completed: Optional[str]) -> float:
    if not started or not completed:
        return 0.0

    def fmt(t: str) -> datetime:
        return datetime.fromisoformat(t.replace("Z", "+00:00"))

    return (fmt(completed) - fmt(started)).total_seconds()


@dataclass
class JobStat:
    name: str
    samples: list[float] = field(default_factory=list)

    @property
    def median(self) -> float:
        return statistics.median(self.samples) if self.samples else 0.0

    @property
    def p90(self) -> float:
        if not self.samples:
            return 0.0
        ordered = sorted(self.samples)
        return ordered[min(len(ordered) - 1, int(0.9 * len(ordered)))]


def ci_bottlenecks(workflow: str, limit: int) -> list[JobStat]:
    """Median duration per CI job across recent successful runs, slowest first."""
    runs = _gh_json(
        [
            "run",
            "list",
            "--workflow",
            workflow,
            "--status",
            "success",
            "--limit",
            str(limit),
            "--json",
            "databaseId",
        ]
    )
    stats: dict[str, JobStat] = {}
    for run in runs:
        detail = _gh_json(["run", "view", str(run["databaseId"]), "--json", "jobs"])
        for job in detail["jobs"]:
            dur = _seconds(job.get("startedAt"), job.get("completedAt"))
            if dur <= 0:
                continue
            stats.setdefault(job["name"], JobStat(job["name"])).samples.append(dur)
    return sorted(stats.values(), key=lambda s: s.median, reverse=True)


def green_on_main(workflow: str, limit: int) -> tuple[float, int]:
    """Success rate and current green streak for CI on main."""
    runs = _gh_json(
        [
            "run",
            "list",
            "--branch",
            "main",
            "--workflow",
            workflow,
            "--limit",
            str(limit),
            "--json",
            "conclusion",
        ]
    )
    conclusions = [r["conclusion"] for r in runs if r["conclusion"]]
    if not conclusions:
        return 0.0, 0
    rate = sum(c == "success" for c in conclusions) / len(conclusions)
    streak = 0
    for c in conclusions:  # most-recent first
        if c == "success":
            streak += 1
        else:
            break
    return rate, streak


def local_test_times() -> dict[str, float]:
    times: dict[str, float] = {}
    for name, cmd in LOCAL_SUITES.items():
        start = time.monotonic()
        proc = subprocess.run(cmd, cwd=REPO_ROOT, capture_output=True, text=True)
        elapsed = time.monotonic() - start
        times[name] = elapsed if proc.returncode == 0 else -elapsed  # negative = failed
    return times


def _fmt(seconds: float) -> str:
    return f"{seconds:.0f}s" if abs(seconds) < 90 else f"{seconds / 60:.1f}m"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--limit", type=int, default=8, help="recent runs to sample")
    parser.add_argument("--local", action="store_true", help="also time local suites")
    parser.add_argument("--json", type=Path, help="write a trend snapshot here")
    args = parser.parse_args()

    rate, streak = green_on_main(CI_WORKFLOW, args.limit)
    jobs = ci_bottlenecks(CI_WORKFLOW, args.limit)
    critical = jobs[0] if jobs else None

    print("Systems north stars")
    print(
        f"  green on main : {rate * 100:.0f}% over last {args.limit} runs, {streak} green in a row"
    )
    print("  billing       : (not wired — read deploy/budget.json + Actions minutes)")
    print("  prod uptime   : (not wired — needs a host health endpoint)")
    print()
    print("CI critical path (jobs run in parallel — the slowest job gates every PR):")
    for j in jobs:
        marker = "  <- bottleneck" if j is critical else ""
        print(f"  {_fmt(j.median):>6}  median  {_fmt(j.p90):>6} p90  {j.name}{marker}")

    local = {}
    if args.local:
        print("\nLocal suite time:")
        local = local_test_times()
        for name, secs in sorted(local.items(), key=lambda kv: -abs(kv[1])):
            state = "FAILED " if secs < 0 else ""
            print(f"  {_fmt(abs(secs)):>6}  {state}{name}")

    if args.json:
        snapshot = {
            "green_on_main_rate": rate,
            "green_streak": streak,
            "ci_critical_path": critical.name if critical else None,
            "ci_critical_seconds": critical.median if critical else None,
            "ci_jobs": {j.name: round(j.median, 1) for j in jobs},
            "local_suite_seconds": {k: round(v, 1) for k, v in local.items()},
        }
        args.json.write_text(json.dumps(snapshot, indent=2) + "\n")
        print(f"\nWrote snapshot to {args.json}")

    if critical:
        print(
            f"\nGrind target: {critical.name} at {_fmt(critical.median)} "
            f"is the slowest gate on a green PR."
        )


if __name__ == "__main__":
    main()
