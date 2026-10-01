#!/usr/bin/env python3
"""Measure which launch context changes outcomes by replaying recorded steps without it."""

from __future__ import annotations

import argparse
import json
import os
import re
import sqlite3
import statistics
import subprocess
import time
import uuid
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any, Callable

from scripts.check_cost import _is_check, _timestamp

STEPS = ("implement", "compress", "loop-decide")
TRIMMED_MEMORY_TOKENS = 4_000

# ── Recorded evidence ────────────────────────────────────────────────────────


def _request_key(manifest: dict[str, Any]) -> str | None:
    # The recorded cohort predates the rename of `launch` to `exec`.
    return next((key for key in ("exec", "launch") if manifest.get(key)), None)


def _block(tag: str) -> re.Pattern[str]:
    return re.compile(rf"^<{tag}(?: [^>\n]*)?>\n.*?^</{tag}>\n?", re.M | re.S)


_SCRATCH_FILE = re.compile(r'^<lf:file path="(scratch/[^"]+)">\n(.*?)^</lf:file>\n?', re.M | re.S)


def _events(directory: Path) -> list[dict[str, Any]]:
    path = directory / "events.jsonl"
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line.strip()]


def _commands(events: list[dict[str, Any]]) -> list[str]:
    """Shell commands from Codex command items and Claude Bash tool calls."""
    commands = []
    for event in events:
        if event["type"] == "conversation":
            item = event["event"].get("item", {})
            if event["event"]["type"] == "item_completed" and item.get("type") == "command":
                commands.append(" ".join(item["command"]))
        elif event["type"] == "provider_output" and '"tool_use"' in event.get("line", ""):
            try:
                message = json.loads(event["line"]).get("message", {})
            except json.JSONDecodeError:
                continue
            for part in message.get("content", []):
                if part.get("type") == "tool_use" and "command" in part.get("input", {}):
                    commands.append(part["input"]["command"])
    return commands


def _final_text(events: list[dict[str, Any]]) -> str | None:
    final = None
    for event in events:
        if event["type"] == "text":
            final = event["text"]
        elif event["type"] == "conversation":
            item = event["event"].get("item", {})
            if item.get("type") == "message" and item.get("text"):
                final = item["text"]
    return final


def observe(directory: Path) -> dict[str, Any]:
    """What one settled record shows about its turn; absent evidence stays None."""
    manifest = json.loads((directory / "manifest.json").read_text())
    events = _events(directory)
    terminal = directory / "terminal.json"
    receipt = json.loads(terminal.read_text()) if terminal.exists() else None
    usage = next((e["usage"] for e in reversed(events) if e["type"] == "usage"), {})
    commands = _commands(events)
    checks = []
    for command in commands:
        try:
            if _is_check([command]):
                checks.append(command)
        except ValueError:
            continue
    final = _final_text(events)
    # The verdict the agent tried to record; failed recording still states its intent.
    verdicts = [
        match.group(1)
        for command in commands
        for match in re.finditer(r"\blf flow (?:decide\s+)?(?!decide\b)(\w+)", command)
    ]
    stated = re.search(r"\b(advance|iterate|blocked|retry)\b", (final or "")[:200], re.I)
    return {
        "outcome": receipt["outcome"] if receipt else None,
        "minutes": (_timestamp(receipt["ended_at"]) - _timestamp(manifest["created_at"])) / 60
        if receipt
        else None,
        "peak_input_tokens": usage.get("peak_input_tokens"),
        "output_tokens": usage.get("output_tokens"),
        "cost_usd": usage.get("cost_usd"),
        "commands": len(commands),
        "checks": checks,
        "decision": (verdicts or [stated.group(1) if stated else None])[0].lower()
        if verdicts or stated
        else None,
        "final_text": final,
    }


def _own_scratch(path: str, worktree: str) -> bool:
    slug = Path(worktree).name.split(".", 1)[-1]
    return Path(path).name in {f"{slug}.md", "questions.md"}


def source_tokens(directory: Path, manifest: dict[str, Any]) -> dict[str, int] | None:
    """Captured token weight of each launch source, from the record's own asset ranges."""
    path = directory / "context.json"
    if not path.exists():
        return None
    context = json.loads(path.read_text())["context"]
    weights: Counter[str] = Counter()
    for channel in ("system", "task"):
        value = context[channel]
        if value is None:
            continue
        weights["submitted"] += value["tokens"]
        for asset in value["assets"]:
            kind = asset["kind"]
            if kind == "scratch":
                own = _own_scratch(asset["label"], str(manifest.get("worktree") or ""))
                kind = "scratch_own" if own else "scratch_older"
            elif kind in {"operating_instructions", "surface_instructions", "skill_instructions"}:
                kind = "instructions"
            weights[kind] += asset["attributed_tokens"]
        if channel == "task":
            steers = _block("lf:steers").search(value["text"])
            if steers:
                # Steers ride inside the Task goal asset; weigh them by byte share.
                weights["steers"] += round(
                    value["tokens"] * len(steers.group().encode()) / len(value["text"].encode())
                )
    return dict(weights)


def replayable(runs: Path) -> list[tuple[Path, dict[str, Any]]]:
    records = []
    for path in sorted(runs.glob("*/*/manifest.json")):
        manifest = json.loads(path.read_text())
        if manifest.get("skill") in STEPS and _request_key(manifest):
            records.append((path.parent, manifest))
    return records


def _spread(values: list[float]) -> dict[str, float] | None:
    if not values:
        return None
    ordered = sorted(values)
    return {
        "median": statistics.median(ordered),
        "p90": ordered[min(len(ordered) - 1, round(0.9 * (len(ordered) - 1)))],
        "sum": sum(ordered),
    }


def census(runs: Path, since: str | None = None) -> dict[str, Any]:
    steps: dict[str, dict[str, Any]] = {}
    grouped: dict[str, list[tuple[Path, dict[str, Any]]]] = defaultdict(list)
    for directory, manifest in replayable(runs):
        if since and _timestamp(manifest["created_at"]) < _timestamp(since):
            continue
        grouped[manifest["skill"]].append((directory, manifest))
    for step, records in sorted(grouped.items()):
        sources: dict[str, list[float]] = defaultdict(list)
        observed: dict[str, list[float]] = defaultdict(list)
        counts: Counter[str] = Counter()
        for directory, manifest in records:
            counts["records"] += 1
            events = _events(directory)
            if any(e["type"] == "user_input" and e.get("op") != "initial" for e in events):
                counts["resumed_turns"] += 1
            weights = source_tokens(directory, manifest)
            if weights is None:
                counts["missing_context"] += 1
            else:
                for source, tokens in weights.items():
                    sources[source].append(tokens)
            turn = observe(directory)
            for measure in ("minutes", "peak_input_tokens", "output_tokens", "cost_usd"):
                if turn[measure] is not None:
                    observed[measure].append(turn[measure])
            if "loopflow-progress" in manifest[_request_key(manifest)]["task_prompt"]:
                counts["agent_comment_markers"] += 1
        submitted = sum(sources["submitted"])
        steps[step] = {
            **counts,
            "sources": {
                source: {
                    "present_in": len(values),
                    "share_of_submitted": sum(values) / submitted if submitted else None,
                    **(_spread(values) or {}),
                }
                for source, values in sorted(sources.items())
            },
            "turns": {
                measure: {"n": len(values), **(_spread(values) or {})}
                for measure, values in sorted(observed.items())
            },
        }
    return {
        "schema_version": 1,
        "since": since,
        "steps": steps,
        "method": "Replayable implement, compress and loop-decide launches. Source tokens are the "
        "record's captured asset weights (cl100k_base); steers are a byte share of the task "
        "channel. Peak input and output tokens exist only where the provider reported them. "
        "Native resumed history is never part of a recorded launch request.",
    }


# ── Arms ─────────────────────────────────────────────────────────────────────


def _drop(tag: str) -> Callable[[str, dict[str, Any]], str]:
    return lambda prompt, _manifest: _block(tag).sub("", prompt)


def _trim_memory(prompt: str, _manifest: dict[str, Any]) -> str:
    def trim(match: re.Match[str]) -> str:
        opening, body, closing = match.group(1), match.group(2), match.group(3)
        # cl100k averages about four bytes a token on this prose.
        keep = TRIMMED_MEMORY_TOKENS * 4 // 2
        if len(body) <= 2 * keep:
            return match.group()
        notice = "\n\n[Excerpt only. Read the complete Wave memory file before relying on it.]\n\n"
        return f"{opening}{body[:keep]}{notice}{body[-keep:]}{closing}"

    return re.sub(
        r"^(<lf:wave-memory>\n)(.*?)(^</lf:wave-memory>\n?)", trim, prompt, flags=re.M | re.S
    )


def _own_scratch_only(prompt: str, manifest: dict[str, Any]) -> str:
    worktree = str(manifest.get("worktree") or "")
    return _SCRATCH_FILE.sub(
        lambda match: match.group() if _own_scratch(match.group(1), worktree) else "", prompt
    )


ARMS: dict[str, Callable[[str, dict[str, Any]], str]] = {
    "baseline": lambda prompt, _manifest: prompt,
    "no-memory": _drop("lf:wave-memory"),
    "trim-memory": _trim_memory,
    "no-scratch": _drop("lf:scratch"),
    "own-scratch": _own_scratch_only,
    "no-steers": _drop("lf:steers"),
}


def applicable_arms(manifest: dict[str, Any]) -> list[str]:
    prompt = manifest[_request_key(manifest)]["task_prompt"]
    return [
        arm for arm, edit in ARMS.items() if arm == "baseline" or edit(prompt, manifest) != prompt
    ]


# ── Disposable replay ────────────────────────────────────────────────────────


def _git(cwd: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=cwd, check=True, capture_output=True, text=True
    ).stdout.strip()


def launch_commit(manifest: dict[str, Any]) -> str | None:
    """HEAD of the source worktree when the step launched, if still recoverable."""
    worktree = Path(manifest["cwd"])
    created = _timestamp(manifest["created_at"])
    if worktree.is_dir():
        log = _git(worktree, "reflog", "--date=unix", "--format=%H %gd", "HEAD")
        for line in log.splitlines():
            sha, selector = line.split(" ", 1)
            moved = re.search(r"@\{(\d+)\}", selector)
            if moved and int(moved.group(1)) <= created:
                return sha
    base = re.search(
        r"^Base commit: ([0-9a-f]{40})$", manifest[_request_key(manifest)]["task_prompt"], re.M
    )
    # A Task's first step starts at its base; later steps without a reflog are unknown.
    first_visit = not any((manifest.get("flow") or {}).get("iterations") or [[]])
    return base.group(1) if base and first_visit and manifest["skill"] == "implement" else None


def _stage(
    source: dict[str, Any], arm: str, repo: Path, commit: str, out: Path
) -> tuple[Path, Path, str, str]:
    checkout, home = out / "checkout", out / "home"
    subprocess.run(
        ["git", "clone", "--quiet", "--no-checkout", str(repo), str(checkout)], check=True
    )
    _git(checkout, "remote", "remove", "origin")
    _git(checkout, "checkout", "--quiet", "--detach", commit)
    key = _request_key(source)
    prompt = source[key]["task_prompt"]
    for path, body in _SCRATCH_FILE.findall(prompt):
        target = checkout / path
        if not target.exists():
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(body)
    _git(checkout, "add", "--all", "--force", "scratch")
    _git(
        checkout, "-c", "user.name=replay", "-c", "user.email=replay@localhost",
        "commit", "--quiet", "--allow-empty", "--message", "replay start",
    )  # fmt: skip
    start = _git(checkout, "rev-parse", "HEAD")

    # Stage the variant in the current manifest shape, outside any Flow or caller.
    identity = f"run_{uuid.uuid4().hex}"
    variant = {
        name: value
        for name, value in source.items()
        if name not in {"run_id", "parent_run_id", "launch", "exec"}
    }
    variant.update(
        artifact_key=identity,
        caller_artifact_key=None,
        exec={**source[key], "task_prompt": ARMS[arm](prompt, source)},
        flow=None,
        context=None,
        cwd=str(checkout),
        repo=str(checkout),
        worktree=str(checkout),
    )
    record = home / "runs" / identity.removeprefix("run_")[:2] / identity
    record.mkdir(parents=True)
    (record / "manifest.json").write_text(json.dumps(variant))
    (record / "prepared").touch()
    (home / "accounts").symlink_to(Path.home() / ".lf" / "accounts")
    return checkout, home, identity, start


def _admit_account(lf: str, environment: dict[str, str], home: Path, source: dict) -> None:
    """Give the disposable Home the recorded login's routing row; credentials stay put."""
    request = source[_request_key(source)]
    provider = request["agent"].split(":", 1)[0]
    if request.get("account_id") is None:
        return
    # Any store-opening command creates and migrates the Home's database.
    subprocess.run(
        [lf, "account", "--cached", "--json"], env=environment, capture_output=True, check=True
    )
    with sqlite3.connect(home / "loopflow.db", uri=True) as store:
        store.execute("ATTACH ? AS real", (f"file:{Path.home() / '.lf' / 'loopflow.db'}?mode=ro",))
        store.execute(
            "INSERT INTO provider_accounts SELECT * FROM real.provider_accounts"
            " WHERE provider = ? AND account_id = ?",
            (provider, request["account_id"]),
        )


def replay(
    runs: Path,
    run: str,
    arm: str,
    repo: Path,
    out: Path,
    lf: str,
    minutes: float,
    commit: str | None,
) -> dict[str, Any]:
    directory = runs / run.removeprefix("run_")[:2] / run
    source = json.loads((directory / "manifest.json").read_text())
    commit = commit or launch_commit(source)
    if commit is None:
        raise SystemExit(f"{run}: launch commit is not recoverable; pass --commit")
    out = out / run / arm
    checkout, home, identity, start = _stage(source, arm, repo, commit, out)
    environment = {
        name: value
        for name, value in os.environ.items()
        if not name.startswith(("LF_", "LOOPFLOW_", "CLAUDE", "CODEX"))
    }
    # Nested `lf` calls resolve to the same binary and Home, so they cannot reach
    # the real store and its PM grant. The binary must honor LF_HOME for its store.
    lf = str(Path(lf).resolve())
    environment["PATH"] = f"{Path(lf).parent}{os.pathsep}{environment.get('PATH', '')}"
    environment["LF_HOME"] = str(home)
    _admit_account(lf, environment, home, source)
    began = time.monotonic()
    with (out / "replay.log").open("w") as log:
        try:
            status: int | str = subprocess.run(
                [lf, "replay", identity],
                cwd=checkout,
                env=environment,
                stdout=log,
                stderr=subprocess.STDOUT,
                timeout=minutes * 60,
            ).returncode
        except subprocess.TimeoutExpired:
            status = "timeout"
    _git(checkout, "add", "--all")
    changed = {}
    for line in _git(checkout, "diff", "--cached", "--numstat", start).splitlines():
        added, removed, path = line.split("\t")
        changed[path] = [int(added) if added != "-" else 0, int(removed) if removed != "-" else 0]
    request = source[_request_key(source)]
    variant = ARMS[arm](request["task_prompt"], source)
    result = {
        "run": run,
        "arm": arm,
        "skill": source["skill"],
        "agent": request["agent"],
        "commit": commit,
        "exit": status,
        "wall_minutes": (time.monotonic() - began) / 60,
        "prompt_bytes": len(variant.encode()),
        "prompt_bytes_removed": len(request["task_prompt"].encode()) - len(variant.encode()),
        "changed": changed,
        "turn": _replayed_turn(out),
    }
    (out / "result.json").write_text(json.dumps(result, indent=2))
    return result


# ── Comparison ───────────────────────────────────────────────────────────────


def _overlap(left: dict[str, Any], right: dict[str, Any]) -> float | None:
    union = set(left) | set(right)
    return len(set(left) & set(right)) / len(union) if union else None


def _replayed_turn(out: Path) -> dict[str, Any] | None:
    """The child Run `lf replay` created beside the staged (still prepared) record."""
    children = [
        path.parent
        for path in (out / "home" / "runs").glob("*/*/manifest.json")
        if not (path.parent / "prepared").exists()
    ]
    return observe(children[0]) if len(children) == 1 else None


def report(runs: Path, out: Path, repeat: Path | None = None) -> dict[str, Any]:
    """Compare arms per record; a repeat baseline measures run-to-run noise."""
    steps = []
    for directory in sorted(path for path in out.iterdir() if path.is_dir()):
        arm_dirs = {path.parent.name: path.parent for path in directory.glob("*/result.json")}
        if repeat and (repeat / directory.name / "baseline" / "result.json").exists():
            arm_dirs["baseline-repeat"] = repeat / directory.name / "baseline"
        arms = {
            arm: json.loads((path / "result.json").read_text())
            for arm, path in sorted(arm_dirs.items())
        }
        baseline = arms.get("baseline")
        original = observe(runs / directory.name.removeprefix("run_")[:2] / directory.name)
        rows = {}
        for arm, result in arms.items():
            turn = _replayed_turn(arm_dirs[arm]) or {}
            rows[arm] = {
                "exit": result["exit"],
                "prompt_bytes_removed": result["prompt_bytes_removed"],
                "minutes": turn.get("minutes"),
                "peak_input_tokens": turn.get("peak_input_tokens"),
                "output_tokens": turn.get("output_tokens"),
                "cost_usd": turn.get("cost_usd"),
                "commands": turn.get("commands"),
                "checks": len(turn.get("checks") or []),
                "decision": turn.get("decision"),
                "files_changed": len(result["changed"]),
                "lines_changed": sum(a + r for a, r in result["changed"].values()),
                "file_overlap_with_baseline": _overlap(result["changed"], baseline["changed"])
                if baseline
                else None,
                "final_text": turn.get("final_text"),
            }
        steps.append(
            {
                "run": directory.name,
                "skill": next(iter(arms.values()))["skill"] if arms else None,
                "original": {**original, "checks": len(original["checks"])},
                "arms": rows,
            }
        )
    return {"schema_version": 1, "steps": steps}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=Path, default=Path.home() / ".lf" / "runs")
    commands = parser.add_subparsers(dest="command", required=True)
    weigh = commands.add_parser(
        "census", help="weigh each source across recorded launches; no spend"
    )
    weigh.add_argument("--since", help="ISO timestamp with UTC offset")
    arms = commands.add_parser("arms", help="list the arms a record's prompt supports")
    arms.add_argument("run")
    launch = commands.add_parser(
        "replay", help="replay one record under one arm; spends provider usage"
    )
    launch.add_argument("run")
    launch.add_argument("arm", choices=sorted(ARMS))
    launch.add_argument("--out", type=Path, required=True, help="directory for disposable Homes")
    launch.add_argument("--repo", type=Path, default=Path.cwd(), help="repository to clone")
    launch.add_argument(
        "--lf",
        default=str(Path.cwd() / "target" / "debug" / "lf"),
        help="lf binary that keeps its store in LF_HOME; it also serves nested calls",
    )
    launch.add_argument("--minutes", type=float, default=45)
    launch.add_argument("--commit", help="checkout commit when the reflog cannot supply it")
    compare = commands.add_parser("report", help="compare replayed arms with baseline and original")
    compare.add_argument("--out", type=Path, required=True)
    compare.add_argument("--repeat", type=Path, help="second baseline replays of the same records")
    args = parser.parse_args()
    if args.command == "census":
        result = census(args.runs, args.since)
    elif args.command == "arms":
        directory = args.runs / args.run.removeprefix("run_")[:2] / args.run
        manifest = json.loads((directory / "manifest.json").read_text())
        result = {"arms": applicable_arms(manifest), "commit": launch_commit(manifest)}
    elif args.command == "replay":
        result = replay(
            args.runs, args.run, args.arm, args.repo, args.out, args.lf, args.minutes, args.commit
        )
    else:
        result = report(args.runs, args.out, args.repeat)
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
