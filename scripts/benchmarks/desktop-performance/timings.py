#!/usr/bin/env python3
"""Report complete experience observations, with app internals as diagnostics.

uv run python scripts/benchmarks/desktop-performance/timings.py
uv run python scripts/benchmarks/desktop-performance/timings.py --json
uv run python scripts/benchmarks/desktop-performance/timings.py --home /path/to/home

Reads daily interaction journals and launch/read diagnostics under
`<Machine>/desktop-cache/timings/`. Launch diagnostics count from kernel process start.
Start's creation and conversation setup steps come from `<Machine>/logs/lf.log`.
"""

from __future__ import annotations

import argparse
import html
import json
import math
import os
import re
import statistics
from collections import Counter
from pathlib import Path

LAUNCH_METRICS = ["pre_main", "restored", "first_frame", "usable_saved", "usable_fresh", "fresh"]
EXPERIENCES = [
    ("start_task", "Start → typing", "Start pressed", "Conversation input accepts typing"),
    ("complete_task", "Complete → row gone", "Complete pressed", "Completed Task row absent"),
    ("open_app", "Open the app", "App process starts", "Workspace visible and input usable"),
    ("open_task", "Open a Task’s conversation", "Task clicked", "Conversation responds to typing"),
    (
        "return_session",
        "Return to a conversation",
        "Session clicked",
        "Conversation responds to typing",
    ),
    (
        "new_conversation",
        "Start a conversation",
        "New conversation requested",
        "Composer accepts typing",
    ),
    ("first_word", "Get an answer", "Send pressed", "New response text visibly observed"),
    (
        "find_work",
        "Find work",
        "Search or filter changed",
        "Matching results visible and selectable",
    ),
    ("open_file", "Open a file", "File clicked", "Contents visible and editor accepts input"),
    (
        "reconnect",
        "Recover a connection",
        "Reconnect begins",
        "Conversation restored and input usable",
    ),
]


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


# A creation step under its request's span, or a conversation setup step.
_STEP = re.compile(
    r'(?:task_creation\{request_id="(?P<creation>[^"]+)"\}\S*?creation_step\{step="(?P<server>\w+)"\}'
    r'|conversation_setup\{request_id="(?P<setup>[^"]+)" step="(?P<conversation>\w+)"\})'
    r".*?elapsed_ms=(?P<ms>[0-9.]+)"
)


def _machine_steps(log: Path | None) -> dict[str, list[dict]]:
    """Creation and setup step timings in lf.log, by lower-case request ID."""
    steps: dict[str, list[dict]] = {}
    if log is None or not log.exists():
        return steps
    with log.open(encoding="utf-8", errors="replace") as lines:
        for line in lines:
            if "step finished" not in line:
                continue
            match = _STEP.search(line)
            if not match:
                continue
            request = match["creation"] or match["setup"]
            source = "server" if match["creation"] else "conversation"
            steps.setdefault(request.lower(), []).append(
                {
                    "source": source,
                    "step": match["server"] or match["conversation"],
                    "ms": float(match["ms"]),
                }
            )
    return steps


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


def _interactions(records: list[dict]) -> dict:
    observations = {}
    for record in records:
        observations[record["observation"]] = record
    groups = {}
    for record in observations.values():
        key = f"{record['name']} / {record['scenario']}"
        groups.setdefault(key, []).append(record)
    return {
        key: {
            **_stats([r["ms"] for r in values if r["phase"] == "ready"]),
            "superseded": sum(r["phase"] == "superseded" for r in values),
            "unfinished": sum(r["phase"] == "begin" for r in values),
        }
        for key, values in sorted(groups.items())
    }


def _journal(directory: Path, log: Path | None = None) -> dict:
    machine_steps = _machine_steps(log)
    attempts = {}
    errors = []
    stalls = []
    validations = {}
    validation_path = directory / "interaction-validations.ndjson"
    if validation_path.exists():
        for number, line in enumerate(validation_path.read_text().splitlines(), 1):
            try:
                row = json.loads(line)
                if row["verdict"] not in ("accepted", "rejected") or not row["reason"]:
                    raise ValueError("Validation needs a verdict and reason")
                validations[row["observation"]] = {**row, "receipt": str(validation_path)}
            except (KeyError, TypeError, ValueError) as error:
                errors.append(
                    {"receipt": str(validation_path), "line": number, "reason": str(error)}
                )
    for path in sorted((directory / "interactions").glob("*.ndjson")):
        for number, line in enumerate(path.read_text().splitlines(), 1):
            try:
                row = json.loads(line)
                if not isinstance(row, dict):
                    raise ValueError("Journal row must be an object")
                if row.get("event") == "main_thread_stall":
                    if (
                        type(row.get("ms")) not in (int, float)
                        or not math.isfinite(row["ms"])
                        or row["ms"] < 0
                    ):
                        raise ValueError("Invalid stall duration")
                    stalls.append(row)
                    continue
                if row["schema"] != 1:
                    raise ValueError("Unknown interaction schema")
                if (
                    not isinstance(row.get("build"), dict)
                    or not isinstance(row.get("context"), dict)
                    or not isinstance(row.get("work"), dict)
                ):
                    raise ValueError("Missing build, context or work object")
                if row.get("phase") not in (
                    "input",
                    "acknowledged",
                    "usable",
                    "outcome",
                ) or not isinstance(row.get("evidence"), str):
                    raise ValueError("Invalid interaction phase or evidence")
                if not all(
                    isinstance(row.get(key), str) for key in ("observation", "name", "scenario")
                ):
                    raise ValueError("Invalid interaction identity")
                repository = row["context"].get("repository")
                if isinstance(repository, str):
                    row["context"]["repository"] = repository.rstrip("/") or "/"
                attempt = attempts.setdefault(
                    row["observation"],
                    {
                        "name": row["name"],
                        "scenario": row["scenario"],
                        "build": row["build"],
                        "context": row["context"],
                        "events": [],
                        "receipt": str(path),
                    },
                )
                if any(attempt[key] != row[key] for key in ("name", "scenario", "build")):
                    raise ValueError("Interaction identity changed")
                attempt["events"].append(row)
                if attempt["name"] == "open_app" and row["context"].get("repository"):
                    attempt["context"] = row["context"]
            except (KeyError, TypeError, ValueError) as error:
                errors.append({"receipt": str(path), "line": number, "reason": str(error)})
    groups = {}
    for observation, attempt in attempts.items():
        revision = attempt["build"].get("revision")
        known_revision = bool(
            revision and revision != "unknown" and not revision.endswith("-dirty")
        )
        build = {k: v for k, v in attempt["build"].items() if k != "launch" or not known_revision}
        # Unknown builds remain per-launch; never pool before/after by app version.
        kinds = {
            e["evidence"]
            for e in attempt["events"]
            if e["phase"] in ("acknowledged", "usable")
            and e["evidence"] in ("captured_frame", "presented_frame")
        }
        method = (
            "capture_upper_bound"
            if "captured_frame" in kinds
            else "presented_frame"
            if kinds
            else "unverified"
        )
        key = (
            attempt["name"],
            attempt["scenario"],
            json.dumps(build, sort_keys=True),
            method,
            attempt["context"].get("repository"),
            attempt["context"].get("machine"),
        )
        group = groups.setdefault(
            key,
            {
                "name": attempt["name"],
                "scenario": attempt["scenario"],
                "build": build,
                "measurement": method,
                "comparable_build": known_revision,
                "repository": attempt["context"].get("repository"),
                "machine": attempt["context"].get("machine"),
                "attempts": [],
            },
        )
        events = attempt["events"]
        outcomes = [e for e in events if e["phase"] == "outcome"]
        original_outcome = outcomes[-1].get("outcome") if outcomes else "unfinished"
        validation = validations.get(observation)
        outcome = (
            "rejected" if validation and validation["verdict"] == "rejected" else original_outcome
        )
        phases = {}
        for phase in ("acknowledged", "usable"):
            values = [
                e.get("ms")
                for e in events
                if e["phase"] == phase and e["evidence"] in ("presented_frame", "captured_frame")
            ]
            value = values[-1] if values else None
            phases[phase] = (
                value
                if (type(value) in (float, int) and math.isfinite(value) and value >= 0)
                else None
            )
        ordered = (
            phases["acknowledged"] is not None
            and phases["usable"] is not None
            and phases["acknowledged"] <= phases["usable"]
        )
        complete = (
            outcome == "completed"
            and len(outcomes) == 1
            and ordered
            and known_revision
            and any(
                e["phase"] == "input"
                and (
                    e["evidence"] == "input_event"
                    or (attempt["name"] == "open_app" and e["evidence"] == "process_start")
                )
                for e in events
            )
        )
        # The terminal elapsed time includes the original input's queueing.
        terminal = outcomes[-1] if outcomes else None
        end_at = terminal.get("at") if terminal else None
        elapsed = terminal.get("ms") if terminal else None
        attributed_stalls = None
        if all(type(v) in (int, float) and math.isfinite(v) for v in (end_at, elapsed)):
            start_at = end_at - elapsed / 1000
            attributed_stalls = [
                stall
                for stall in stalls
                if stall.get("build") == attempt["build"]
                and type(stall.get("at")) in (int, float)
                and stall["at"] >= start_at
                and stall["at"] - stall["ms"] / 1000 <= end_at
            ]
        group["attempts"].append(
            {
                "observation": observation,
                "outcome": outcome,
                "journal_outcome": original_outcome,
                "validation": validation,
                "measured": complete,
                "phases": phases,
                "work": (outcomes[-1] if outcomes else events[-1])["work"],
                "context": attempt["context"],
                "stalls": attributed_stalls,
                "machine_steps": machine_steps.get(observation.lower(), []),
                "receipt": attempt["receipt"],
            }
        )
    result = []
    for group in groups.values():
        samples = [a for a in group["attempts"] if a["measured"]]
        steps = {}
        for attempt in samples:
            for step in attempt["machine_steps"]:
                steps.setdefault(f"{step['source']} {step['step']}", []).append(step["ms"])
        result.append(
            {
                **group,
                "machine_steps": {name: _stats(values) for name, values in steps.items()},
                "samples": len(samples),
                "outcomes": dict(Counter(a["outcome"] for a in group["attempts"])),
                "acknowledged": _stats([a["phases"]["acknowledged"] for a in samples]),
                "usable": _stats([a["phases"]["usable"] for a in samples]),
                "stall_count": len(
                    {
                        (stall["at"], stall["ms"], stall["build"].get("launch"))
                        for attempt in samples
                        for stall in attempt["stalls"]
                    }
                )
                if samples and all(a["stalls"] is not None for a in samples)
                else None,
            }
        )
    return {
        "groups": result,
        "errors": errors,
        "stalls": stalls,
        "stall_summary": _stats([row["ms"] for row in stalls]),
        "stall_threshold_counts": {
            str(threshold): sum(row["ms"] >= threshold for row in stalls)
            for threshold in (50, 100, 250)
        },
    }


def summarize(directory: Path, log: Path | None = None) -> dict:
    launches = _lines(directory / "launches.ndjson")
    reads = _lines(directory / "reads.ndjson")
    interactions = _lines(directory / "interactions.ndjson")
    versions = {}
    revisions = {r["launch"]: r.get("rev") for r in launches if r.get("event") == "launch"}

    def build(record: dict) -> tuple:
        return record.get("app", ""), record.get("rev") or revisions.get(record.get("launch"))

    for app, revision in dict.fromkeys(build(r) for r in launches + reads + interactions):
        mine = [r for r in reads if build(r) == (app, revision)]
        key = f"{app} @ {revision}" if revision else f"{app} (unattributed)"
        versions[key] = {
            "app": app,
            "revision": revision,
            **_launches([r for r in launches if build(r) == (app, revision)]),
            "reads": _durations(mine, "read", "verb"),
            "refreshes": _durations(mine, "refresh", "part"),
            "frame_waits": _durations(mine, "frame_wait", "part"),
            "frame_applies": _durations(mine, "frame_apply", "part"),
            "interactions": _interactions([r for r in interactions if build(r) == (app, revision)]),
        }
    return {
        "source": str(directory),
        "experiences": _journal(directory, log),
        "endpoints": {
            "first_frame": "first window content committed to the render server; "
            "not on-glass presentation",
            "usable": "outline rows observed, next main-queue callback; saved or fresh by source",
            "fresh": "every part of the workspace read by this launch",
            "interactions": "existing navigation signpost endpoints; next main-queue callback "
            "after observed view state, not layout, pixels or validated provider input",
        },
        "frames": "readings that took 100 ms or more to show: time queued behind the main actor, "
        "then time to apply",
        "not_measured": ["CPU", "memory", "host load", "on-glass presentation"],
        "versions": versions,
    }


def _cell(stats: dict) -> str:
    def ms(value: float | None) -> str:
        return "—" if value is None else f"{value:.0f}"

    return (
        f"{stats['samples']:>7} {ms(stats['median_ms']):>9} "
        f"{ms(stats['p95_ms']):>9} {ms(stats['max_ms']):>9}"
    )


def _experience_text(report: dict) -> list[str]:
    lines = ["Everyday experiences — input → acknowledged → usable → outcome", ""]
    for identifier, label, start, end in EXPERIENCES:
        lines.append(f"{label}: {start} → {end}")
        groups = [g for g in report["experiences"]["groups"] if g["name"] == identifier]
        if not groups:
            lines.append("  Unmeasured — no interaction journal observations")
        for group in groups:
            machine = f" / {group['machine']} Machine" if group["machine"] else ""
            lines.append(
                f"  {group['build']} / {group['scenario']}{machine}: "
                f"{group['samples']} measured; outcomes {group['outcomes']}"
            )
            for phase in ("acknowledged", "usable"):
                lines.append(f"    {phase}: {_cell(group[phase])}")
            for name, stats in group["machine_steps"].items():
                lines.append(f"      {name}: {_cell(stats)}")
    lines += [
        "",
        "App-state/commit callbacks cannot supply presented-frame latencies.",
        "CLI, read and refresh timings below are diagnostics, not complete experiences.",
    ]
    return lines


def render(report: dict) -> str:
    lines = _experience_text(report) + ["", f"Desktop timings from {report['source']}"]
    if not report["versions"]:
        return "\n".join(lines) + "\nNo launches recorded yet. Open Desktop, then run this again."
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
            budget = f"  historical budget {stats['budget_ms']}" if "budget_ms" in stats else ""
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
        for title, key in (
            ("lf read", "reads"),
            ("refresh", "refreshes"),
            ("slow reading: waited", "frame_waits"),
            ("slow reading: applied", "frame_applies"),
        ):
            if version[key]:
                lines += [
                    "",
                    f"{title:24} {'ok':>7} {'median ms':>9} {'p95 ms':>9} "
                    f"{'max ms':>9} {'failed':>7}",
                ]
            for name, stats in version[key].items():
                lines.append(f"{name:24} {_cell(stats)} {stats['failed']:>7}")
        for name, stats in version["interactions"].items():
            lines.append(
                f"{name:24} {_cell(stats)}; superseded {stats['superseded']}; "
                f"unfinished {stats['unfinished']} (may still be pending)"
            )
    lines += [
        "",
        "p95 needs 20 samples. A launch still running, or quit early, "
        "counts as never reaching what it lacks.",
        "Not measured: " + ", ".join(report["not_measured"]) + ".",
    ]
    return "\n".join(lines)


def render_html(report: dict) -> str:
    def escape(value: object) -> str:
        return html.escape(str(value), quote=True)

    experiences = []
    journal = report["experiences"]
    definitions = list(EXPERIENCES)
    known = {d[0] for d in definitions}
    labels = {
        "open_wave": "Open a Wave",
        "workspace_layout": "Arrange the workspace",
        "typing_echo": "See typed input",
    }
    for group in journal["groups"]:
        if group["name"] not in known:
            known.add(group["name"])
            definitions.append(
                (
                    group["name"],
                    labels.get(group["name"], group["name"]),
                    "Input event",
                    "Presented acknowledgment and usable content",
                )
            )
    for identifier, label, start, end in definitions:
        groups = [g for g in journal["groups"] if g["name"] == identifier]
        heading = f"<strong>{escape(label)}</strong><p>{escape(start)} → {escape(end)}</p>"
        if not groups:
            experiences.append(
                f'<tr><td>{heading}</td><td colspan="6" class="missing">'
                "Unmeasured · no interaction journal observations</td></tr>"
            )
        for group in groups:
            build = group["build"]
            numbers = []
            for phase in ("acknowledged", "usable"):
                for key in ("median_ms", "p95_ms"):
                    value = group[phase][key]
                    numbers.append(f"{value:,.1f}" if value is not None else "Unmeasured")
            outcomes = ", ".join(f"{k}: {v}" for k, v in group["outcomes"].items())
            details = escape(json.dumps(group["attempts"], indent=2))
            experiences.append(
                f"<tr><td>{heading}</td><td>{escape(build.get('platform'))}<br>"
                f"{escape(build.get('app'))} @ {escape(build.get('revision'))}<br>"
                f"{escape(group['repository'] or 'repository not recorded')}<br>"
                f"{escape(group['scenario'])}<br>{escape(group['measurement'])}"
                "<details><summary>Outcomes, work and context</summary>"
                f"<pre>{details}</pre></details></td><td>{group['samples']} / "
                f"{len(group['attempts'])}<br>{escape(outcomes)}</td>"
                + "".join(f'<td class="num">{v}</td>' for v in numbers)
                + "</tr>"
            )
    # One workflow chart per build and measurement source; never pool revisions.
    workflows = {}
    for group in journal["groups"]:
        if group["usable"]["median_ms"] is not None:
            key = (
                group["name"],
                group["scenario"],
                json.dumps(group["build"], sort_keys=True),
                group["measurement"],
            )
            workflows.setdefault(key, []).append(group)
    # Keep repositories and builds comparable within a workflow without making
    # subsecond navigation disappear beside multi-second Task creation.
    largest = {}
    for (name, scenario, *_), groups in workflows.items():
        key = (name, scenario)
        largest[key] = max(largest.get(key, 1), *(g["usable"]["median_ms"] for g in groups))
    ceilings = {}
    for key, value in largest.items():
        magnitude = 10 ** math.floor(math.log10(max(value, 1)))
        ceilings[key] = next(
            step * magnitude for step in (1, 2, 5, 10) if step * magnitude >= value
        )
    repositories = sorted({g["repository"] for g in journal["groups"] if g["repository"]})
    charts = []
    experience_labels = {name: label for name, label, *_ in definitions}
    for (name, scenario, _, measurement), groups in sorted(workflows.items()):
        ceiling = ceilings[(name, scenario)]
        title = (
            "Board: "
            + scenario.removeprefix("board_").replace("_to_", " → ").replace("_", " ").title()
            if scenario.startswith("board_")
            else experience_labels[name] + " · " + scenario.replace("_", " ")
        )
        title = {
            "start_to_typing": "Start → typing",
            "new_task_to_typing": "New Task: Start → typing",
            "complete_to_row_gone": "Complete → row gone",
            "process_start_to_typing": "App process start → typing",
            "new_session_to_typing": "New conversation → typing",
            "refresh_to_typing": "Refresh → typing",
            "file_to_editor": "Open file → focused editor (two actions)",
            "file_to_visible_editor": "Open file → contents usable",
            "send_to_response_observed": "Send → response observed",
        }.get(scenario, title)
        bars = []
        by_repository = {(g["repository"], g["machine"]): g for g in groups}
        rows = set(by_repository) | {
            (r, None) for r in repositories if not any(key[0] == r for key in by_repository)
        }
        for repository, machine in sorted(rows, key=lambda r: (r[0] or "", r[1] or "")):
            group = by_repository.get((repository, machine))
            repo = Path((repository or "").rstrip("/")).name or "Repository not recorded"
            label = {"loopflow": "Loopflow", "manabot": "Manabot", "kata": "Kata"}.get(repo, repo)
            if machine:
                label += f" · {machine}"
            if group is None:
                bars.append(
                    f'<div class="bar-row"><div class="repo">{escape(label)}</div>'
                    '<div class="track"></div><div class="bar-value">'
                    "<small>Unmeasured</small></div></div>"
                )
                continue
            value = group["usable"]["median_ms"]
            stall_count = group["stall_count"]
            stall_label = f" · {stall_count} stalls" if stall_count is not None else ""
            bars.append(
                f'<div class="bar-row"><div class="repo">{escape(label)}</div>'
                '<div class="track">'
                f'<div class="bar" style="width:{100 * value / ceiling:.3f}%"></div></div>'
                f'<div class="bar-value">{value:,.1f}'
                f"<small>ms · n={group['samples']}{stall_label}</small></div></div>"
            )
        source = (
            "Capture-inclusive upper bound"
            if measurement == "capture_upper_bound"
            else "Presented frame"
        )
        charts.append(
            f'<section class="workflow"><h2>{escape(title)}</h2>'
            "<p>Click the section → matching list visible and selectable</p>"
            if scenario.startswith("board_")
            else f'<section class="workflow"><h2>{escape(title)}</h2><p>Input → usable result</p>'
        )
        charts[-1] += (
            "".join(bars) + f'<div class="axis"><span>0</span><span>{ceiling / 2:g}</span>'
            f"<span>{ceiling:g} ms</span></div>"
            + f'<p class="chart-source">{escape(source)}</p></section>'
        )
    measured_names = {key[0] for key in workflows}
    for name, label, start, end in EXPERIENCES:
        if name not in measured_names:
            charts.append(
                f'<section class="workflow"><h2>{escape(label)}</h2>'
                f"<p>{escape(start)} → {escape(end)}</p>"
                '<p class="empty">Unmeasured · no completed observations yet</p></section>'
            )
    if not charts:
        charts.append(
            '<p class="empty">No completed workflow measurements yet. See coverage below.</p>'
        )

    stall_summary = journal["stall_summary"]
    stall_text = (
        f"{stall_summary['samples']} main-thread busy turns of at least 50 ms. "
        f"Median: {stall_summary['median_ms']} ms; max: {stall_summary['max_ms']} ms. "
        "This is a thresholded diagnostic, not a frame-hitch distribution."
    )
    stall_counts = journal["stall_threshold_counts"]
    maximum_stall = stall_summary["max_ms"]
    maximum_stall_label = f"{maximum_stall:,.0f} ms" if maximum_stall is not None else "Unmeasured"
    unavailable = "".join(f"<li>{escape(e)}</li>" for e in journal["errors"])
    rows = []
    options = []
    versions = report["versions"]
    for app, version in versions.items():
        options.append(f'<option value="{escape(app)}">{escape(app)}</option>')
        for section in (
            "metrics",
            "interactions",
            "reads",
            "refreshes",
            "frame_waits",
            "frame_applies",
        ):
            measurements = version[section]
            scale = max((s["median_ms"] or 0 for s in measurements.values()), default=0) or 1
            for name, stats in measurements.items():
                values = [
                    f"{stats[k]:,.1f}" if stats[k] is not None else "Unmeasured"
                    for k in ("median_ms", "p95_ms", "max_ms")
                ]
                width = 100 * (stats["median_ms"] or 0) / scale
                issues = (
                    ", ".join(
                        f"{label}: {stats[label]}"
                        for label in ("failed", "superseded", "unfinished")
                        if stats.get(label)
                    )
                    or "—"
                )
                rows.append(
                    f'<tr data-version="{escape(app)}"><td>{escape(section)}</td>'
                    f"<td>{escape(name)}</td><td>{stats['samples']}</td>"
                    + "".join(f'<td class="num">{v}</td>' for v in values)
                    + f'<td>{issues}</td><td><span style="width:{width:.2f}%"></span></td></tr>'
                )
    details = escape(render(report))
    endpoints = escape(json.dumps(report["endpoints"], indent=2))
    return f"""<!doctype html>
<html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Loopflow · live performance</title>
<style>
body {{font:15px system-ui; color:#203c32; background:#f3f5ef;
max-width:1320px; margin:48px auto; padding:0 24px}}
h1 {{font-size:38px; letter-spacing:-1.3px; margin-bottom:8px}}
h2 {{font-size:23px; font-weight:600; margin:0 0 8px}}
.intro {{font-size:18px; color:#52665e; margin-bottom:36px}}
.charts {{display:grid; grid-template-columns:repeat(auto-fit,minmax(min(100%,460px),1fr));
gap:24px}}
.workflow {{background:white; padding:28px; border:1px solid #d5dfd4; border-radius:14px}}
.workflow h2 {{text-transform:none}} .workflow p {{color:#647469; line-height:1.5; margin:0 0 30px}}
.bar-row {{display:grid; grid-template-columns:80px minmax(70px,1fr) 86px; gap:12px;
align-items:center; margin:22px 0}}
.repo {{font-weight:600}} .track {{height:28px; background:#edf1ea; border-radius:4px}}
.bar {{height:100%; background:#378267; border-radius:4px}}
.bar-value {{font-size:21px; text-align:right; font-variant-numeric:tabular-nums}}
.bar-value small {{display:block; font-size:11px; color:#647469; margin-top:3px}}
.axis {{display:flex; justify-content:space-between; margin:0 98px 24px 92px;
color:#647469; font-size:11px}}
.axis span {{display:inline; height:auto; background:none}}
.workflow p.chart-source {{font-size:12px; margin:28px 0 0}}
.stalls {{margin:32px 0; padding:24px 28px; background:#e2ebe1; border-radius:14px}}
.stalls p {{color:#52665e}} .stall-counts {{display:flex; flex-wrap:wrap; gap:28px 48px}}
.stall-counts strong {{display:block; font-size:28px; font-variant-numeric:tabular-nums}}
.stall-counts span {{display:block; background:none; height:auto; margin-top:6px; color:#52665e}}
.below-fold {{margin-top:64px; padding-top:28px; border-top:1px solid #d5dfd4}}
@media(max-width:560px) {{
body {{margin:24px auto; padding:0 14px}} .workflow {{padding:20px}}
.bar-row {{grid-template-columns:65px minmax(40px,1fr) 80px; gap:8px}}
.axis {{margin-left:73px; margin-right:88px}}
}}
.note {{background:#fff5d8; padding:18px; border-left:4px solid #bc8b2b; line-height:1.6}}
select {{font:inherit; padding:8px 16px; margin:12px}}
.table {{overflow:auto; background:white; border-radius:12px; border:1px solid #d5dfd4}}
table {{border-collapse:collapse; width:100%; font-size:13px}}
th {{background:#e2ebe1; text-align:left; padding:15px 12px; white-space:nowrap}}
td {{padding:12px; border-top:1px solid #e6ece4}}
.num {{text-align:right; font-variant-numeric:tabular-nums; white-space:nowrap}}
td:last-child {{min-width:110px}} td > span {{display:block; height:9px; background:#378267}}
.missing {{color:#6c766c}} td p {{margin:8px 0 0; color:#647469; line-height:1.5}}
pre {{white-space:pre-wrap; overflow-wrap:anywhere; line-height:1.6; font-size:12px}}
details {{background:white; margin:20px 0; padding:20px; border-radius:12px}}
summary {{cursor:pointer; font-weight:600}} [hidden] {{display:none}}
</style>
<main>
<h1>Live app performance</h1>
<p class="intro">How long until the result is usable?<br>
Median journey time · milliseconds · shorter bars are faster.</p>
<div class="charts">{"".join(charts)}</div>
<p>Each workflow has its own labeled scale, shared across repositories and builds.
Capture cost is included. P95 needs 20 samples per case.</p>
<aside class="stalls" aria-label="Main-thread stalls">
<h2>Main-thread stalls</h2>
<p>Recorded busy turns across the retained journal window.</p>
<div class="stall-counts">
<div><strong>{stall_counts["50"]:,}</strong><span>50 ms or longer</span></div>
<div><strong>{stall_counts["100"]:,}</strong><span>100 ms or longer</span></div>
<div><strong>{stall_counts["250"]:,}</strong><span>250 ms or longer</span></div>
<div><strong>{maximum_stall_label}</strong><span>Longest busy turn</span></div></div>
</aside>
<section class="below-fold"><h2>Measurement details</h2>
<p>Builds, acknowledgment times, outcomes and coverage.</p>
<details id="experience-details"><summary>Samples, timing stages and unmeasured workflows</summary>
<p class="note">These times come from the app journal, starting at the input event’s timestamp.
Acknowledgment and usability need frame evidence. Native capture observations are upper bounds
including image acquisition cost; they are grouped separately from presentation callbacks.
State callbacks and unfinished waits do not enter percentiles.
P95 needs 20 completed samples per build and case. Missing measurements remain unmeasured.</p>
<div class="table"><table><thead><tr><th>Experience · start → finish</th><th>Case</th>
<th>Measured / attempts</th><th>Ack median ms</th><th>Ack P95 ms</th>
<th>Usable median ms</th><th>Usable P95 ms</th>
</tr></thead><tbody>{"".join(experiences)}</tbody></table></div>
<ul>{unavailable}</ul>
<p>{escape(stall_text)}</p>
<p>Agent clicks provide the stimulus; app event/frame timestamps provide the measurement.
The earlier observer-inclusive pilot is diagnostic only and is not a reactivity baseline.</p>
</details>
<details id="diagnostics"><summary>Diagnostics · what happened inside the app</summary>
<p>Real app journals · local Machine. These are internal phase timings, not all-in UX results.</p>
<p class="note">These are ordinary-use observations, including busy-host samples.
Host load was not recorded. P95 needs 20 completed samples.
Unfinished interactions may still be pending. Older versions have no interaction journal.
Records are grouped by build revision. Old records without a revision are marked unattributed;
they cannot establish a before/after comparison.</p>
<p class="note">Frame wait/apply rows contain only readings whose combined wait and apply
time reached 100 ms. Their percentiles describe that slow subset, not all readings.</p>
<label>App build <select id="version">{"".join(options)}</select></label>
<p>Median bars share a scale within each metric category and version.
Exact tail values remain in the table.</p>
<div class="table"><table><thead><tr><th>Category</th><th>Measurement</th>
<th>Completed</th><th>Median ms</th><th>P95 ms</th><th>Max ms</th>
<th>Other outcomes</th><th>Median</th>
</tr></thead><tbody>{"".join(rows)}</tbody></table></div>
<details><summary>Endpoints and measurement limits</summary><pre>{endpoints}</pre></details>
<details><summary>All versions, launch failures and raw summary</summary>
<pre>{details}</pre></details>
</details>
<p>Source: {escape(report["source"])}. Journals rotate; this is the retained window.</p>
</section></main>
<script>
const selector = document.getElementById('version');
selector.selectedIndex = selector.options.length - 1;
function filterRows() {{
  for (const row of document.querySelectorAll('tr[data-version]')) {{
    row.hidden = row.dataset.version !== selector.value;
  }}
}}
selector.addEventListener('change', filterRows); filterRows();
</script></html>"""


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--home", type=Path, default=Path(os.environ.get("LF_HOME") or Path.home() / ".lf")
    )
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--html", type=Path, help="Write a standalone chart and table")
    args = parser.parse_args()
    home = args.home.expanduser()
    report = summarize(home / "desktop-cache" / "timings", home / "logs" / "lf.log")
    if args.html:
        args.html.write_text(render_html(report), encoding="utf-8")
        print(args.html)
        return 0
    print(json.dumps(report, indent=2) if args.json else render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
