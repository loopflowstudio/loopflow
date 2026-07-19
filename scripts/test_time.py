#!/usr/bin/env python3
"""Report aggregate agent time spent in tests and checks from local traces."""

from __future__ import annotations

import argparse
import json
import os
import sqlite3
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Optional


@dataclass(frozen=True)
class Interval:
    start_ms: int
    end_ms: int
    category: str


@dataclass
class Totals:
    commands: int = 0
    millis: int = 0


@dataclass
class Report:
    launches: int = 0
    wall_millis: int = 0
    activity_millis: int = 0
    full_millis: int = 0
    focused_millis: int = 0
    check_millis: int = 0
    normalized: int = 0
    raw: int = 0
    untimed: int = 0
    missing_artifacts: int = 0
    categories: dict[str, Totals] = field(default_factory=dict)
    skills: dict[str, Totals] = field(default_factory=dict)


CATEGORY_LABELS = {
    "gate_full": "gate · full",
    "gate_selected": "gate · selected",
    "rust_full": "rust · full",
    "rust_focused": "rust · focused",
    "python_full": "python · full",
    "python_focused": "python · focused",
    "swift_full": "swift · full",
    "swift_focused": "swift · focused",
    "other_test": "other tests",
    "check_build": "checks/builds",
}
FULL_CATEGORIES = {"gate_full", "rust_full", "python_full", "swift_full"}


def _default_db() -> Path:
    if path := os.environ.get("LF_DB_PATH"):
        return Path(path)
    root = Path(os.environ.get("LF_HOME", Path.home() / ".lf"))
    return root / "loopflow.db"


def _timestamp_ms(value: str) -> int:
    return int(datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp() * 1000)


def _classify_segment(command: str) -> Optional[str]:
    command = command.strip(" \t\r\n'\"()")
    inspection = ("rg ", "grep ", "git grep ", "sed ", "cat ", "head ", "tail ", "jq ")
    if command.startswith(inspection):
        return None
    if "scripts/test.py" in command:
        return "gate_full" if "--all" in command else "gate_selected"
    if "cargo nextest list" in command or "cargo test --no-run" in command:
        return "check_build"
    if "cargo nextest run" in command or "cargo test" in command:
        full = (
            "--workspace" in command
            or "--all-targets" in command
            or "cargo test --all" in command
            or command.endswith("cargo test")
            or command.endswith("cargo nextest run")
        )
        return "rust_full" if full else "rust_focused"
    if "pytest" in command:
        focused = "::" in command or " -k " in command or any(
            part.endswith(".py") for part in command.split()
        )
        return "python_focused" if focused else "python_full"
    if "swift test" in command or "xcodebuild test" in command:
        focused = "--filter" in command or "-only-testing" in command
        return "swift_focused" if focused else "swift_full"
    if any(
        marker in command
        for marker in (
            "npm test",
            "npm run test",
            "pnpm test",
            "vitest",
            "playwright test",
            "go test",
            "ctest",
            "make test",
            "just test",
            "bazel test",
        )
    ):
        return "other_test"
    if any(
        marker in command
        for marker in (
            "cargo fmt",
            "cargo clippy",
            "cargo check",
            "cargo build",
            "ruff ",
            "mypy ",
            "pyright",
            "npm run lint",
            "npm run build",
            "swift build",
            "xcodebuild build",
        )
    ):
        return "check_build"
    return None


def _classify(command: str) -> Optional[str]:
    command = command.lower()
    if " -lc " in command:
        command = command.split(" -lc ", 1)[1]
    for separator in ("&&", "||", ";"):
        command = command.replace(separator, "\n")
    return next(
        (category for line in command.splitlines() if (category := _classify_segment(line))),
        None,
    )


def _push(
    intervals: list[Interval], start_ms: int, end_ms: int, category: Optional[str]
) -> None:
    if category is not None and end_ms > start_ms:
        intervals.append(Interval(start_ms, end_ms, category))


def _normalized_intervals(lines: Iterable[str]) -> list[Interval]:
    starts: dict[str, tuple[int, str]] = {}
    intervals: list[Interval] = []
    for line in lines:
        try:
            record = json.loads(line)
            if record.get("payload", {}).get("type") != "conversation":
                continue
            event = record["payload"]["event"]
            item = event.get("item", {})
            if item.get("type") != "command":
                continue
            timestamp = _timestamp_ms(record["ts"])
            item_id = item["id"]
            command = item.get("command", [])
            command_text = " ".join(command) if isinstance(command, list) else str(command)
        except (KeyError, TypeError, ValueError, json.JSONDecodeError):
            continue
        if event.get("type") == "item_started":
            if category := _classify(command_text):
                starts[item_id] = (timestamp, category)
        elif event.get("type") == "item_completed":
            started = starts.pop(item_id, None)
            category = _classify(command_text) or (started[1] if started else None)
            duration = item.get("duration_ms")
            if isinstance(duration, int):
                start_ms = timestamp - duration
            elif started:
                start_ms = started[0]
            else:
                continue
            _push(intervals, start_ms, timestamp, category)
    return intervals


def _raw_intervals(lines: Iterable[str], provider: str) -> list[Interval]:
    starts: dict[str, tuple[int, str]] = {}
    intervals: list[Interval] = []
    for line in lines:
        try:
            wrapper = json.loads(line)
            if wrapper.get("stream") != "stdout":
                continue
            event = json.loads(wrapper["line"])
            timestamp = _timestamp_ms(wrapper["ts"])
        except (KeyError, TypeError, ValueError, json.JSONDecodeError):
            continue
        if provider == "codex":
            item = event.get("item", {})
            if item.get("type") != "command_execution" or not item.get("id"):
                continue
            item_id = item["id"]
            if event.get("type") == "item.started":
                if category := _classify(item.get("command", "")):
                    starts[item_id] = (timestamp, category)
            elif event.get("type") == "item.completed" and item_id in starts:
                start_ms, category = starts.pop(item_id)
                _push(intervals, start_ms, timestamp, category)
        elif provider == "claude":
            content = event.get("message", {}).get("content", [])
            if not isinstance(content, list):
                continue
            if event.get("type") == "assistant":
                for item in content:
                    if item.get("type") != "tool_use" or item.get("name") != "Bash":
                        continue
                    if category := _classify(item.get("input", {}).get("command", "")):
                        starts[item["id"]] = (timestamp, category)
            elif event.get("type") == "user":
                for item in content:
                    item_id = item.get("tool_use_id")
                    if item.get("type") == "tool_result" and item_id in starts:
                        start_ms, category = starts.pop(item_id)
                        _push(intervals, start_ms, timestamp, category)
    return intervals


def _read_lines(path: Path) -> list[str]:
    try:
        return path.read_text(errors="replace").splitlines()
    except OSError:
        return []


def _merge_millis(intervals: Iterable[Interval]) -> int:
    spans = sorted((interval.start_ms, interval.end_ms) for interval in intervals)
    total = 0
    current: Optional[tuple[int, int]] = None
    for start, end in spans:
        if current is None:
            current = (start, end)
        elif start <= current[1]:
            current = (current[0], max(current[1], end))
        else:
            total += current[1] - current[0]
            current = (start, end)
    if current:
        total += current[1] - current[0]
    return total


def _path_matches(value: str, expected: Optional[Path]) -> bool:
    return expected is None or Path(value).resolve() == expected.resolve()


def _build_report(
    db_path: Path,
    trace_root: Path,
    days: int,
    repo: Optional[Path],
    worktree: Optional[Path],
) -> Report:
    cutoff = int(datetime.now(timezone.utc).timestamp()) - days * 24 * 3600
    connection = sqlite3.connect(db_path)
    connection.row_factory = sqlite3.Row
    try:
        rows = connection.execute(
            """
            SELECT started_at, ended_at, repo, worktree, skill, provider,
                   conversation_path, provider_events_path
            FROM agent_launches
            WHERE started_at >= ? AND ended_at IS NOT NULL
            ORDER BY started_at
            """,
            (cutoff,),
        ).fetchall()
    finally:
        connection.close()

    report = Report()
    for row in rows:
        if not _path_matches(row["repo"], repo) or not _path_matches(row["worktree"], worktree):
            continue
        report.launches += 1
        report.wall_millis += max(0, row["ended_at"] - row["started_at"]) * 1000
        conversation = trace_root / row["conversation_path"]
        intervals = _normalized_intervals(_read_lines(conversation))
        source = "normalized" if intervals else "none"
        if not intervals and row["provider_events_path"]:
            provider_path = trace_root / row["provider_events_path"]
            intervals = _raw_intervals(_read_lines(provider_path), row["provider"])
            if intervals:
                source = "raw"
        if not conversation.is_file():
            report.missing_artifacts += 1
        if source == "normalized":
            report.normalized += 1
        elif source == "raw":
            report.raw += 1
        else:
            report.untimed += 1
            continue

        report.activity_millis += _merge_millis(intervals)
        report.full_millis += _merge_millis(
            interval for interval in intervals if interval.category in FULL_CATEGORIES
        )
        report.check_millis += _merge_millis(
            interval for interval in intervals if interval.category == "check_build"
        )
        report.focused_millis += _merge_millis(
            interval
            for interval in intervals
            if interval.category not in FULL_CATEGORIES and interval.category != "check_build"
        )
        for category in CATEGORY_LABELS:
            selected = [interval for interval in intervals if interval.category == category]
            if not selected:
                continue
            totals = report.categories.setdefault(category, Totals())
            totals.commands += len(selected)
            totals.millis += _merge_millis(selected)
        skill = row["skill"] or "(unattributed)"
        totals = report.skills.setdefault(skill, Totals())
        totals.commands += len(intervals)
        totals.millis += _merge_millis(intervals)
    return report


def _format_millis(millis: int) -> str:
    seconds = millis / 1000
    if seconds >= 3600:
        return f"{seconds / 3600:.1f}h"
    if seconds >= 60:
        return f"{seconds / 60:.1f}m"
    return f"{seconds:.1f}s"


def _render_report(report: Report, days: int, scope: str) -> str:
    percent = report.activity_millis / report.wall_millis * 100 if report.wall_millis else 0
    lines = [
        f"Test time · last {days} days · {scope}",
        (
            f"{report.launches} terminal launches · {_format_millis(report.wall_millis)} "
            f"captured wall · {_format_millis(report.activity_millis)} tests/checks "
            f"({percent:.1f}%)"
        ),
        (
            f"full {_format_millis(report.full_millis)} · "
            f"focused {_format_millis(report.focused_millis)} · "
            f"checks/builds {_format_millis(report.check_millis)}"
        ),
        "",
        "CATEGORY                 COMMANDS      TIME",
    ]
    for category, totals in sorted(
        report.categories.items(), key=lambda item: item[1].millis, reverse=True
    ):
        lines.append(
            f"{CATEGORY_LABELS[category]:<24} {totals.commands:>8} "
            f"{_format_millis(totals.millis):>9}"
        )
    lines.extend(["", "SKILL                    COMMANDS      TIME"])
    skills = sorted(report.skills.items(), key=lambda item: item[1].millis, reverse=True)
    for skill, totals in skills[:15]:
        label = skill if len(skill) <= 24 else f"{skill[:23]}…"
        lines.append(f"{label:<24} {totals.commands:>8} {_format_millis(totals.millis):>9}")
    if len(skills) > 15:
        lines.append(f"  … {len(skills) - 15} more skills")
    lines.extend(
        [
            "",
            (
                f"Capture: {report.normalized} normalized · {report.raw} raw fallback · "
                f"{report.untimed} without timed test/check commands · "
                f"{report.missing_artifacts} missing conversation artifacts"
            ),
            "Parallel intervals are merged per launch; category rows can overlap each other.",
            "Command text, prompts, and output are never printed.",
        ]
    )
    return "\n".join(lines)


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Aggregate test/check time from Loopflow's local agent traces."
    )
    parser.add_argument("--days", type=int, default=7)
    parser.add_argument("--repo", type=Path, help="scope to one main repository path")
    parser.add_argument("--worktree", type=Path, help="scope to one exact worktree path")
    parser.add_argument("--db", type=Path, default=_default_db(), help=argparse.SUPPRESS)
    parser.add_argument("--trace-root", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.days <= 0:
        parser.error("--days must be greater than zero")
    return args


def main() -> int:
    args = _parse_args()
    if not args.db.is_file():
        raise SystemExit(f"Loopflow ledger not found: {args.db}")
    trace_root = args.trace_root or args.db.parent / "traces"
    if args.worktree:
        scope = f"worktree {args.worktree.resolve()}"
    elif args.repo:
        scope = f"repo {args.repo.resolve()}"
    else:
        scope = "all repositories"
    report = _build_report(args.db, trace_root, args.days, args.repo, args.worktree)
    print(_render_report(report, args.days, scope))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
