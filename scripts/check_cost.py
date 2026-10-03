#!/usr/bin/env python3
"""Summarize check time and submitted context by skill from local Run records."""

from __future__ import annotations

import argparse
import json
import re
import shlex
from collections import Counter, defaultdict
from datetime import datetime
from pathlib import Path
from typing import Any

from scripts.lifecycle_scorecard import canonical_repo

_CHECKS = {
    "cargo": {"build", "test", "nextest", "clippy", "fmt", "check"},
    "swift": {"build", "test"},
    "xcodebuild": {"build", "test", "build-for-testing", "test-without-building"},
    "pytest": None,
    "ruff": None,
}


def _timestamp(value: str) -> float:
    return datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp()


def _is_check(command: list[str]) -> bool:
    # Inspect shell words, not quoted search strings or tool output. This is a
    # command-family estimate: custom wrappers outside these names are unknown.
    if len(command) >= 3 and command[1] in {"-c", "-lc"}:
        source = command[2]
    elif len(command) == 1:
        words = shlex.split(command[0])
        if len(words) >= 3 and words[1] in {"-c", "-lc"}:
            source = words[2]
        else:
            source = command[0]
    else:
        source = shlex.join(command)
    if "<<" in source:
        raise ValueError("heredoc command batches are not classified")
    lexer = shlex.shlex(source, posix=True, punctuation_chars=";&|()\n")
    lexer.whitespace = " \t\r"
    lexer.whitespace_split = True
    words = list(lexer)
    at_command = True
    skip_value = False
    for index, word in enumerate(words):
        if word and all(char in ";&|()\n" for char in word):
            at_command = True
            skip_value = False
            continue
        if not at_command:
            continue
        if skip_value:
            skip_value = False
            continue
        if word in {"--project", "--directory", "--extra", "--with", "-n", "-u"}:
            skip_value = True
            continue
        name = Path(word).name
        if "=" in word or name in {"env", "time", "nice", "uv", "run"}:
            continue
        # Wrapper options are ignored until reaching its executable.
        if word.startswith("-"):
            continue
        following = words[index + 1 : index + 4]
        if name == "test_desktop.sh":
            return True
        if name in _CHECKS:
            subcommands = _CHECKS[name]
            if subcommands is None or any(part in subcommands for part in following):
                return True
        if name in {"python", "python3"} and following:
            if following[0].endswith("scripts/test.py") or following[:2] == ["-m", "pytest"]:
                return True
        if name in {"npm", "pnpm", "bun"} and any(
            part in {"test", "build", "lint", "typecheck"} for part in following
        ):
            return True
        at_command = False
    return False


def _span_seconds(spans: list[tuple[float, float]]) -> float:
    total = 0.0
    end = float("-inf")
    for start, stop in sorted(spans):
        total += max(0, stop - max(start, end))
        end = max(end, stop)
    return total


def _context(directory: Path) -> dict[str, int]:
    path = directory / "context.json"
    if not path.exists():
        return {"missing_context_runs": 1}
    context = json.loads(path.read_text())["context"]
    result: Counter[str] = Counter()
    for channel in ("system", "task"):
        value = context[channel]
        if value is None:
            continue
        result["context_tokens"] += value["tokens"]
        result["proof_word_mentions"] += len(re.findall(r"\bproof\b", value["text"], re.I))
        for asset in value["assets"]:
            if asset["kind"] == "scratch":
                result["scratch_tokens"] += asset["attributed_tokens"]
                text = value["text"].encode()[asset["byte_start"] : asset["byte_end"]].decode()
                for paragraph in text.split("\n\n"):
                    if re.search(r"\b(proof|verification|verified|pytest)\b", paragraph, re.I):
                        result["scratch_check_paragraph_words"] += len(paragraph.split())
            if asset["kind"] in {"skill_instructions", "operating_instructions"}:
                result["instruction_tokens"] += asset["attributed_tokens"]
    return dict(result)


def _command_cost(directory: Path, created: float, finished: float) -> Counter[str]:
    result: Counter[str] = Counter()
    starts: dict[str, float] = {}
    completed: dict[str, tuple[dict, float]] = {}
    for line in (directory / "events.jsonl").read_text().splitlines():
        event = json.loads(line)
        if event["type"] != "conversation":
            continue
        item = event["event"].get("item", {})
        if item.get("type") != "command":
            continue
        observed = _timestamp(event["observed_at"])
        if event["event"]["type"] == "item_started":
            starts.setdefault(item["id"], observed)
        elif event["event"]["type"] == "item_completed":
            completed[item["id"]] = item, observed
    spans: dict[str, list[tuple[float, float]]] = {"shell": [], "check": []}
    if not completed:
        result["runs_without_command_records"] += 1
    for identity, (item, observed) in completed.items():
        result["commands"] += 1
        try:
            check = _is_check(item["command"])
        except ValueError:
            result["unclassified_commands"] += 1
            check = False
        if check:
            result["check_commands"] += 1
            if item.get("exit_code") not in (None, 0):
                result["failed_check_commands"] += 1
        began = starts.get(identity)
        if began is None:
            result["commands_without_start"] += 1
            continue
        span = max(created, began), min(finished, observed)
        spans["shell"].append(span)
        if check:
            spans["check"].append(span)
    for kind, intervals in spans.items():
        result[f"{kind}_seconds"] += _span_seconds(intervals)
    return result


def summarize(runs: Path, repo: Path, since: str, until: str) -> dict[str, Any]:
    start, end = _timestamp(since), _timestamp(until)
    identities: dict[str, Path] = {}
    steps: dict[str, Counter[str]] = defaultdict(Counter)
    unavailable_repository_runs = 0
    for path in sorted(runs.glob("*/*/manifest.json")):
        manifest = json.loads(path.read_text())
        created = _timestamp(manifest["created_at"])
        if not start <= created < end or not manifest.get("repo"):
            continue
        source = manifest["repo"]
        if not Path(source).exists():
            unavailable_repository_runs += 1
            continue
        if source not in identities:
            identities[source] = canonical_repo(Path(source))
        if identities[source] != repo:
            continue
        step = manifest.get("skill") or "unattributed"
        result = steps[step]
        result["observed_runs"] += 1
        terminal = path.parent / "terminal.json"
        if not terminal.exists():
            result["unsettled_runs"] += 1
            continue
        finished = _timestamp(json.loads(terminal.read_text())["ended_at"])
        if finished > end:
            result["unfinished_at_cutoff_runs"] += 1
            continue
        result["settled_runs"] += 1
        result["run_seconds"] += max(0, finished - created)
        result.update(_context(path.parent))
        result.update(_command_cost(path.parent, created, finished))
    return {
        "schema_version": 1,
        "repository": str(repo),
        "unavailable_repository_runs": unavailable_repository_runs,
        "since": since,
        "until": until,
        "steps": {step: dict(values) for step, values in sorted(steps.items())},
        "method": "Missing repository paths are unattributable and excluded. "
        "Settled Runs in the start-time window, canonical repository identity. "
        "Shell/check seconds union observed command spans within each Run; overlapping Runs "
        "are separate work. Check classification is a shell-word command-family heuristic. "
        "Context tokens are captured channel/asset counts, not total provider consumption. "
        "No-command Runs and unsettled Runs are counted explicitly, not inferred idle.",
    }


def compare(baseline: dict[str, Any], current: dict[str, Any]) -> dict[str, Any]:
    comparisons = {}
    for step, after in current["steps"].items():
        before = baseline["steps"].get(step, {})
        if not before.get("run_seconds") or not after.get("run_seconds"):
            continue
        if any(
            report.get(gap)
            for report in (before, after)
            for gap in ("runs_without_command_records", "commands_without_start")
        ):
            continue
        old = 100 * before.get("check_seconds", 0) / before["run_seconds"]
        new = 100 * after.get("check_seconds", 0) / after["run_seconds"]
        comparisons[step] = {
            "baseline_runs": before["settled_runs"],
            "current_runs": after["settled_runs"],
            "baseline_check_percent": old,
            "current_check_percent": new,
            "change_percentage_points": new - old,
        }
    return comparisons


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=Path, required=True)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--since", required=True, help="ISO timestamp with UTC offset")
    parser.add_argument("--until", required=True, help="exclusive ISO cutoff with UTC offset")
    parser.add_argument("--baseline", type=Path, help="compare with a saved aggregate JSON report")
    args = parser.parse_args()
    report = summarize(args.runs, canonical_repo(args.repo), args.since, args.until)
    if args.baseline:
        report["comparison"] = compare(json.loads(args.baseline.read_text()), report)
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
