#!/usr/bin/env python3
"""Launch the release app for real against a private copy of the Home and report its own timings.

uv run python scripts/benchmarks/desktop-performance/launch.py run --work /tmp/desktop-launch --output /tmp/desktop-launch/run
uv run python scripts/benchmarks/desktop-performance/launch.py run --work /tmp/desktop-launch --output /tmp/run2 --built --saved 5 --uncached 1 --failed 1

`run` copies the Home's database with SQLite's backup (a read of the live
file and its write-ahead log), builds the release binary into a private app
bundle with its own bundle id, and opens it in the background once per sample.
The bundle's `lf` forwards only the startup reads to the installed `lf` under
the copied Home and refuses everything else, so nothing is launched, repaired
or written in the live Home. `--home <other work>/home` copies another run's
Home instead, so a baseline and a candidate read the same data. The numbers are
the app's launch journal; `timings.py --home <work>/home` reads the same files.
Needs a logged-in desktop session. Windows appear behind your work and close
again.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import sqlite3
import subprocess
import time
from collections import Counter
from contextlib import closing
from pathlib import Path

from timings import BUDGETS_MS, _stats

REPO = Path(__file__).resolve().parents[3]
SWIFT = REPO / "swift"
BUNDLE_ID = "com.loopflow.mac.bench"
READS = '"home id"|"wave list"|"roadmap "*|"session list"|"activity "*|"ps "*|"ps "'
SHIM = f"""#!/bin/sh
# Forward the app's startup reads to the real lf; refuse everything else.
case "$1 $2" in
  {READS}) ;;
  *) echo "denied $1 $2" >> "$LF_BENCH_LOG"; echo "benchmark lf: refused $1 $2" >&2; exit 1 ;;
esac
echo "$1 $2" >> "$LF_BENCH_LOG"
case " $LF_BENCH_FAIL " in *" $1 "*) echo "benchmark lf: forced failure" >&2; exit 1 ;; esac
exec "$LF_BENCH_LF" "$@"
"""
# What each scenario waits for before the app is closed.
SCENARIOS = {
    "uncached": {"saved": False, "fail": "", "until": "fresh"},
    "saved": {"saved": True, "fail": "", "until": "fresh"},
    "saved_refresh_fails": {"saved": True, "fail": "roadmap session", "until": "refresh_failed"},
}


def _sh(cmd: list[str], **kwargs) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, check=True, **kwargs)


def _lf() -> str:
    path = shutil.which("lf")
    if not path:
        raise SystemExit("lf is not on PATH")
    return path


def snapshot(home: Path, source: Path) -> None:
    """Copy the Home's database; the copy appears only once it is complete."""
    target = home / "loopflow.db"
    if target.exists():
        return
    home.mkdir(parents=True, exist_ok=True)
    partial = home / "loopflow.db.partial"
    partial.unlink(missing_ok=True)
    try:
        # mode=ro still reads the write-ahead log, so a live Home copies whole.
        with (
            closing(
                sqlite3.connect(f"{(source / 'loopflow.db').resolve().as_uri()}?mode=ro", uri=True)
            ) as live,
            closing(sqlite3.connect(partial)) as copy,
        ):
            live.backup(copy)
    except sqlite3.Error as error:
        partial.unlink(missing_ok=True)
        raise SystemExit(f"could not copy {source / 'loopflow.db'}: {error}") from error
    partial.rename(target)
    if (source / "config.yaml").exists():
        shutil.copy(source / "config.yaml", home)


def bundle(app: Path) -> None:
    _sh(["swift", "build", "-c", "release", "--product", "LoopflowMac"], cwd=SWIFT)
    contents = app / "Contents"
    (contents / "MacOS").mkdir(parents=True, exist_ok=True)
    (contents / "Resources").mkdir(exist_ok=True)
    shutil.copy(SWIFT / ".build/release/LoopflowMac", contents / "MacOS/Loopflow")
    shutil.copy(SWIFT / "LoopflowMac/Info.plist", contents)
    shutil.copy(SWIFT / "LoopflowMac/AppIcon.icns", contents / "Resources")
    _sh(
        [
            "plutil",
            "-replace",
            "CFBundleIdentifier",
            "-string",
            BUNDLE_ID,
            str(contents / "Info.plist"),
        ]
    )
    _sh(
        [
            "plutil",
            "-replace",
            "CFBundleName",
            "-string",
            "Loopflow Bench",
            str(contents / "Info.plist"),
        ]
    )
    shim = contents / "MacOS/lf"
    shim.write_text(SHIM, encoding="utf-8")
    shim.chmod(0o755)
    _sh(["codesign", "--force", "--deep", "--sign", "-", str(app)], capture_output=True)
    source = _sh(
        ["git", "describe", "--always", "--dirty", "--abbrev=9"],
        cwd=REPO,
        capture_output=True,
        text=True,
    )
    (app / "Contents/Resources/source").write_text(source.stdout.strip(), encoding="utf-8")


def _records(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def _pid(executable: Path) -> int | None:
    found = subprocess.run(
        ["pgrep", "-f", str(executable)], capture_output=True, text=True
    ).stdout.split()
    return int(found[0]) if found else None


def launch(app: Path, home: Path, repo: Path, scenario: str, timeout: float) -> dict:
    spec = SCENARIOS[scenario]
    cache = home / "desktop-cache"
    timings = cache / "timings"
    if not spec["saved"]:
        (cache / "workspace.json").unlink(missing_ok=True)
    elif not (cache / "workspace.json").exists():
        raise SystemExit(f"{scenario} needs a saved workspace; run an uncached sample first")
    shutil.rmtree(timings, ignore_errors=True)
    log = home / "lf-calls.log"
    log.write_text("")
    env = {
        "LF_HOME": str(home),
        "LF_DB_PATH": str(home / "loopflow.db"),
        "LF_BENCH_LOG": str(log),
        "LF_BENCH_LF": _lf(),
        "LF_BENCH_FAIL": spec["fail"],
    }
    command = ["open", "-n", "-g"]
    for key, value in env.items():
        command += ["--env", f"{key}={value}"]
    _sh([*command, str(app), "--args", "--repo", str(repo)])
    executable = app / "Contents/MacOS/Loopflow"
    deadline = time.monotonic() + timeout
    events: dict[str, dict] = {}
    while time.monotonic() < deadline:
        time.sleep(0.25)
        events = {record["event"]: record for record in _records(timings / "launches.ndjson")}
        if spec["until"] in events and "usable" in events:
            break
    time.sleep(1)  # let the save that follows the last read land
    pid = _pid(executable)
    usage = (
        subprocess.run(
            ["ps", "-o", "rss=,cputime=", "-p", str(pid)], capture_output=True, text=True
        ).stdout.split()
        if pid
        else []
    )
    if pid:
        os.kill(pid, 15)
        while _pid(executable):
            time.sleep(0.1)
    events = {record["event"]: record for record in _records(timings / "launches.ndjson")}
    calls = Counter(log.read_text().splitlines())
    minutes, _, seconds = usage[1].partition(":") if usage else ("0", "", "0")
    return {
        "scenario": scenario,
        "pre_main": events.get("launch", {}).get("ms"),
        "restored": events.get("restored", {}).get("ms"),
        "cache": events.get("restored", {}).get("cache"),
        "first_frame": events.get("first_frame", {}).get("ms"),
        "usable": events.get("usable", {}).get("ms"),
        "usable_source": events.get("usable", {}).get("source"),
        "fresh": events.get("fresh", {}).get("ms"),
        "refresh_failed": events.get("refresh_failed", {}).get("part"),
        "reached": spec["until"] in events and "usable" in events,
        "reads": [
            {key: record[key] for key in ("verb", "part", "ms", "ok") if key in record}
            for record in _records(timings / "reads.ndjson")
        ],
        "lf_calls": {
            name: count for name, count in calls.items() if not name.startswith("denied ")
        },
        "lf_refused": {
            name[7:]: count for name, count in calls.items() if name.startswith("denied ")
        },
        "rss_mb": round(int(usage[0]) / 1024, 1) if usage else None,
        "cpu_s": round(int(minutes) * 60 + float(seconds), 2) if usage else None,
        "saved_workspace_kept": (cache / "workspace.json").exists(),
    }


def summarize(rows: list[dict], manifest: dict) -> dict:
    scenarios = {}
    for name in dict.fromkeys(row["scenario"] for row in rows):
        mine = [row for row in rows if row["scenario"] == name]
        metrics = {
            key: _stats([row[key] for row in mine if row[key] is not None])
            for key in ("pre_main", "restored", "first_frame", "usable", "fresh", "rss_mb", "cpu_s")
        }
        reads: dict[str, list[float]] = {}
        failed: Counter = Counter()
        for row in mine:
            for read in row["reads"]:
                label = read.get("verb") or f"refresh {read.get('part')}"
                if read["ok"]:
                    reads.setdefault(label, []).append(read["ms"])
                else:
                    failed[label] += 1
        scenarios[name] = {
            "launches": len(mine),
            "reached_endpoint": sum(row["reached"] for row in mine),
            "cache": dict(Counter(row["cache"] for row in mine)),
            "usable_source": dict(Counter(row["usable_source"] for row in mine)),
            "metrics": metrics,
            "reads": {label: _stats(values) for label, values in sorted(reads.items())},
            "failed_reads": dict(failed),
            "lf_calls": dict(sum((Counter(row["lf_calls"]) for row in mine), Counter())),
            "lf_refused": dict(sum((Counter(row["lf_refused"]) for row in mine), Counter())),
            "saved_workspace_kept": all(row["saved_workspace_kept"] for row in mine),
        }
    return {
        **manifest,
        "budgets_ms": {
            "first_frame": BUDGETS_MS["first_frame"],
            "usable": BUDGETS_MS["usable_saved"],
        },
        "endpoints": {
            "first_frame": "first window content committed to the render server; not on-glass presentation",
            "usable": "outline rows observed, next main-queue callback",
            "fresh": "every part of the workspace read by this launch",
        },
        "not_measured": [
            "main-thread stalls",
            "on-glass presentation",
            "launches with cold OS file caches",
        ],
        "scenarios": scenarios,
    }


def render(report: dict) -> str:
    def ms(value: float | None) -> str:
        return "—" if value is None else f"{value:.0f}"

    lines = [
        "# Desktop launch, rendered",
        "",
        f"Release build of `{report['source']}`, `{report['lf']}`, {report['host']}. "
        f"Home copy: {report['database_mb']} MB database.",
        f"Budgets: first frame {report['budgets_ms']['first_frame']} ms, usable {report['budgets_ms']['usable']} ms. "
        "Milliseconds from kernel process start; p95 needs 20 samples.",
    ]
    for name, scenario in report["scenarios"].items():
        lines += [
            "",
            f"## {name}",
            "",
            f"{scenario['launches']} launches, {scenario['reached_endpoint']} reached the endpoint; "
            f"saved workspace {scenario['cache']}; usable from {scenario['usable_source']}.",
            "",
            "| | samples | median | p95 | max |",
            "|---|---|---|---|---|",
        ]
        for key, stats in scenario["metrics"].items():
            unit = "" if key in ("rss_mb", "cpu_s") else " ms"
            if key in ("rss_mb", "cpu_s"):
                cells = [
                    str(stats[field]) if stats[field] is not None else "—"
                    for field in ("median_ms", "p95_ms", "max_ms")
                ]
            else:
                cells = [ms(stats[field]) for field in ("median_ms", "p95_ms", "max_ms")]
            lines.append(f"| {key}{unit} | {stats['samples']} | " + " | ".join(cells) + " |")
        for label, stats in scenario["reads"].items():
            lines.append(
                f"| `{label}` ms | {stats['samples']} | {ms(stats['median_ms'])} | {ms(stats['p95_ms'])} | {ms(stats['max_ms'])} |"
            )
        lines += [
            "",
            f"`lf` processes across these launches: {scenario['lf_calls']}; "
            f"failed reads: {scenario['failed_reads'] or 'none'}; refused: {scenario['lf_refused'] or 'none'}; "
            f"saved workspace kept: {scenario['saved_workspace_kept']}.",
        ]
    lines += ["", "Not measured: " + ", ".join(report["not_measured"]) + "."]
    return "\n".join(lines) + "\n"


def run(args: argparse.Namespace) -> int:
    work, output = args.work.expanduser(), args.output.expanduser()
    home, app = work / "home", work / "Loopflow Bench.app"
    snapshot(home, args.home.expanduser())
    if not args.built:
        bundle(app)
    output.mkdir(parents=True, exist_ok=True)
    rows = []
    plan = [
        ("uncached", args.uncached),
        ("saved", args.saved),
        ("saved_refresh_fails", args.failed),
    ]
    with (output / "journal.ndjson").open("w", encoding="utf-8") as journal:
        for scenario, count in plan:
            for index in range(count):
                row = launch(app, home, args.repo.expanduser(), scenario, args.timeout)
                rows.append(row)
                journal.write(json.dumps(row) + "\n")
                journal.flush()
                print(
                    f"{scenario} {index + 1}/{count}: first_frame {row['first_frame']} usable {row['usable']} fresh {row['fresh']}"
                )
    manifest = {
        "source": (app / "Contents/Resources/source").read_text(encoding="utf-8"),
        "lf": _sh([_lf(), "--version"], capture_output=True, text=True).stdout.strip(),
        "host": _sh(
            ["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True
        ).stdout.strip(),
        "database_mb": round((home / "loopflow.db").stat().st_size / 1e6),
    }
    report = summarize(rows, manifest)
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (output / "report.md").write_text(render(report), encoding="utf-8")
    print(render(report))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="command", required=True)
    runner = sub.add_parser("run")
    runner.add_argument(
        "--work", type=Path, required=True, help="holds the Home copy and the app bundle"
    )
    runner.add_argument("--output", type=Path, required=True)
    runner.add_argument(
        "--home",
        type=Path,
        default=Path(os.environ.get("LF_HOME") or Path.home() / ".lf"),
        help="Home to copy; another run's <work>/home compares two builds over one copy",
    )
    runner.add_argument("--repo", type=Path, default=Path.home() / "src/loopflow")
    runner.add_argument("--uncached", type=int, default=3)
    runner.add_argument("--saved", type=int, default=20)
    runner.add_argument("--failed", type=int, default=3)
    runner.add_argument("--built", action="store_true", help="reuse the bundle already in --work")
    runner.add_argument("--timeout", type=float, default=120)
    runner.set_defaults(func=run)
    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
