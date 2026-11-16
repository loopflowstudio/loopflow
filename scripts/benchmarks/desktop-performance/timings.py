#!/usr/bin/env python3
"""Report complete experience observations, with app internals as diagnostics.

uv run python scripts/benchmarks/desktop-performance/timings.py
uv run python scripts/benchmarks/desktop-performance/timings.py --json
uv run python scripts/benchmarks/desktop-performance/timings.py --home /path/to/home
uv run python scripts/benchmarks/desktop-performance/timings.py --input-trial /tmp/trial/report.json

Reads `<Machine>/desktop-cache/timings/{launches,reads}.ndjson`, which the app
appends to while it runs. Launch milliseconds count from kernel process start.
"""

from __future__ import annotations

import argparse
import html
import json
import math
import os
import statistics
from collections import Counter
from pathlib import Path

BUDGETS_MS = {"first_frame": 400, "usable_saved": 1000, "usable_fresh": 1000}
LAUNCH_METRICS = ["pre_main", "restored", "first_frame", "usable_saved", "usable_fresh", "fresh"]
EXPERIENCES = [
    ("open_app", "Open the app", "Launch requested", "Workspace visible and input usable"),
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
    ("first_word", "Get an answer", "Send pressed", "First response word visible"),
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


def _experience_trials(paths: list[Path]) -> dict:
    groups = {}
    errors = []
    # Aliases of a receipt are one observation, never extra samples.
    for path in dict.fromkeys(p.expanduser().resolve() for p in paths):
        try:
            report = json.loads(path.read_text())
            metadata = report["metadata"]
            dimensions = (
                "provider",
                "provider_version",
                "case",
                "conditions",
                "app_revision",
                "helper_revision",
                "machine",
                "data",
                "variant",
            )
            context = {key: metadata[key] for key in dimensions}
            if any(not isinstance(value, str) or not value for value in context.values()):
                raise ValueError(
                    "Trial conditions and build/provider identities must be nonempty text"
                )
            if context["provider"] not in ("codex", "claude") or context["case"] not in (
                "cached",
                "resume",
                "fresh",
            ):
                raise ValueError("Unknown provider or cached/resume/fresh case")
            key = json.dumps(context, sort_keys=True)
            group = groups.setdefault(key, {"context": context, "attempts": []})
            value = report["click_to_observed_response_ms"]
            observed = report["status"] == "observed; native captures require review"
            if observed and (
                type(value) not in (int, float) or not math.isfinite(value) or value < 0
            ):
                raise ValueError("Observed trial has no finite, nonnegative elapsed time")
            group["attempts"].append(
                {
                    "receipt": str(path),
                    "status": report["status"],
                    "ms": value if observed else None,
                }
            )
        except (OSError, ValueError, TypeError, KeyError) as error:
            errors.append({"receipt": str(path), "reason": str(error)})
    return {
        "groups": [
            {
                **group,
                **_stats([a["ms"] for a in group["attempts"] if a["ms"] is not None]),
                "unmeasured": sum(a["ms"] is None for a in group["attempts"]),
            }
            for group in groups.values()
            if group["attempts"]
        ],
        "errors": errors,
    }


def summarize(directory: Path, input_trials: list[Path] | None = None) -> dict:
    launches = _lines(directory / "launches.ndjson")
    reads = _lines(directory / "reads.ndjson")
    interactions = _lines(directory / "interactions.ndjson")
    versions = {}
    for app in dict.fromkeys(record.get("app", "") for record in launches + reads + interactions):
        mine = [record for record in reads if record.get("app", "") == app]
        versions[app] = {
            **_launches([record for record in launches if record.get("app", "") == app]),
            "reads": _durations(mine, "read", "verb"),
            "refreshes": _durations(mine, "refresh", "part"),
            "frame_waits": _durations(mine, "frame_wait", "part"),
            "frame_applies": _durations(mine, "frame_apply", "part"),
            "interactions": _interactions([r for r in interactions if r.get("app", "") == app]),
        }
    return {
        "source": str(directory),
        "experiences": _experience_trials(input_trials or []),
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
    lines = ["Everyday experiences — all-in time", ""]
    trials = report["experiences"]
    for identifier, label, start, end in EXPERIENCES:
        lines.append(f"{label}: {start} → {end}")
        if identifier == "open_task" and trials["groups"]:
            for group in trials["groups"]:
                lines.append(
                    f"  {json.dumps(group['context'], sort_keys=True)}: {_cell(group)}; "
                    f"unmeasured attempts {group['unmeasured']}; captures require review"
                )
        else:
            lines.append("  Unmeasured — no complete journey observations")
    lines.extend(f"Unavailable receipt: {e['receipt']}: {e['reason']}" for e in trials["errors"])
    lines += ["", "Internal timings below are diagnostics, not complete experience times."]
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
    trials = report["experiences"]
    maximum = max((g["median_ms"] or 0 for g in trials["groups"]), default=0) or 1
    for identifier, label, start, end in EXPERIENCES:
        heading = f"<strong>{escape(label)}</strong><p>{escape(start)} → {escape(end)}</p>"
        groups = trials["groups"] if identifier == "open_task" else []
        if not groups:
            experiences.append(
                f'<tr><td>{heading}</td><td colspan="6" class="missing">'
                "Unmeasured · no complete journey observations</td></tr>"
            )
        for group in groups:
            context = group["context"]
            case = escape(f"{context['provider']} · {context['case']} · {context['variant']}")
            numbers = "".join(
                f'<td class="num">{group[k]:,.1f}</td>'
                if group[k] is not None
                else '<td class="num">Unmeasured</td>'
                for k in ("median_ms", "p95_ms", "max_ms")
            )
            receipts = "".join(
                f'<li><a href="{escape(Path(a["receipt"]).as_uri())}">'
                f"{escape(a['status'])}</a></li>"
                for a in group["attempts"]
            )
            width = 100 * (group["median_ms"] or 0) / maximum
            experiences.append(
                f"<tr><td>{heading}</td><td>{case}<br>"
                f"Build {escape(context['app_revision'])}<br>"
                "Observed input response; captures require review"
                f"<details><summary>Conditions and receipts</summary>"
                f"<pre>{escape(json.dumps(context, indent=2))}</pre>"
                f"<ul>{receipts}</ul></details></td>"
                f"<td>{group['samples']} / {len(group['attempts'])} attempts<br>"
                f"{group['unmeasured']} unmeasured</td>{numbers}"
                f'<td><span style="width:{width:.2f}%"></span></td></tr>'
            )
    unavailable = "".join(
        f"<li>{escape(e['receipt'])}: {escape(e['reason'])}</li>" for e in trials["errors"]
    )
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
.label {{letter-spacing:2px; text-transform:uppercase; font-size:12px}}
.note {{background:#fff5d8; padding:18px; border-left:4px solid #bc8b2b; line-height:1.6}}
select {{font:inherit; padding:8px 16px; margin:12px}}
.table {{overflow:auto; background:white; border-radius:12px; border:1px solid #d5dfd4}}
table {{border-collapse:collapse; width:100%; font-size:13px}}
th {{background:#e2ebe1; text-align:left; padding:15px 12px; white-space:nowrap}}
td {{padding:12px; border-top:1px solid #e6ece4}}
.num {{text-align:right; font-variant-numeric:tabular-nums; white-space:nowrap}}
td:last-child {{min-width:110px}} span {{display:block; height:9px; background:#378267}}
.missing {{color:#6c766c}} td p {{margin:8px 0 0; color:#647469; line-height:1.5}}
pre {{white-space:pre-wrap; overflow-wrap:anywhere; line-height:1.6; font-size:12px}}
details {{background:white; margin:20px 0; padding:20px; border-radius:12px}}
summary {{cursor:pointer; font-weight:600}} [hidden] {{display:none}}
</style>
<main><div class="label">Loopflow · live observations</div>
<h1>The waits in everyday use</h1>
<p>From your action to a usable result · milliseconds, lower is faster</p>
<p class="note">Only complete journey observations belong in this table.
The retained launch/read/navigation journals do not establish these all-in times.
P95 needs 20 observed samples with matching conditions. Missing values stay unmeasured.</p>
<div class="table"><table><thead><tr><th>Experience · start → finish</th><th>Case</th>
<th>Observed / attempts</th><th>Median ms</th><th>P95 ms</th><th>Max ms</th><th>Median</th>
</tr></thead><tbody>{"".join(experiences)}</tbody></table></div>
<ul>{unavailable}</ul>
<p>Task-to-conversation observations include capture and observer delay, and hook/trust waits.
They time an unsent probe response, not model response latency. Captures need review;
they are not an automatic pass. Cases, builds, providers and host/data conditions stay separate.</p>
<details id="diagnostics"><summary>Diagnostics · what happened inside the app</summary>
<p>Real app journals · local Machine. These are internal phase timings, not all-in UX results.</p>
<p class="note">These are ordinary-use observations, including busy-host samples.
Host load was not recorded. P95 needs 20 completed samples.
Unfinished interactions may still be pending. Older versions have no interaction journal.
Builds sharing a version are pooled; this is not a controlled before/after comparison.</p>
<p class="note">Frame wait/apply rows contain only readings whose combined wait and apply
time reached 100 ms. Their percentiles describe that slow subset, not all readings.</p>
<label>App version <select id="version">{"".join(options)}</select></label>
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
<p>Source: {escape(report["source"])}. Journals rotate; this is the retained window.</p></main>
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
    parser.add_argument(
        "--input-trial",
        type=Path,
        action="append",
        default=[],
        help="Include an agent_input.py report.json as a Task-to-conversation observation; "
        "repeat per trial",
    )
    args = parser.parse_args()
    report = summarize(args.home.expanduser() / "desktop-cache" / "timings", args.input_trial)
    if args.html:
        args.html.write_text(render_html(report), encoding="utf-8")
        print(args.html)
        return 0
    print(json.dumps(report, indent=2) if args.json else render(report))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
