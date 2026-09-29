#!/usr/bin/env python3
"""Generate the lifecycle scorecard and typed Project metric inputs."""

from __future__ import annotations

import argparse
import io
import json
import math
import sqlite3
import subprocess
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence, TextIO
from urllib.parse import quote

SCHEMA_VERSION = 1
POLICY_PATH = Path("performance/budgets.json")


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--json", action="store_true", help="emit structured JSON")
    parser.add_argument(
        "--envelope",
        action="store_true",
        help=argparse.SUPPRESS,
    )
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path.cwd(),
        help="repository checkout to report (default: current directory)",
    )
    parser.add_argument(
        "--database",
        type=Path,
        required=True,
        help="read this Rust-resolved Loopflow Home database",
    )
    parser.add_argument(
        "--runs",
        type=Path,
        required=True,
        help="RunSnapshot JSON supplied by the Rust Run reader",
    )
    return parser.parse_args(argv)


def git_path(repo: Path, *args: str) -> Path | None:
    result = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "--path-format=absolute", *args],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    value = Path(result.stdout.strip())
    return value if value.is_absolute() else (repo / value).resolve()


def canonical_repo(repo: Path) -> Path:
    repo = repo.resolve()
    common = git_path(repo, "--git-common-dir")
    if common is not None and common.name == ".git":
        return common.parent.resolve()
    top = git_path(repo, "--show-toplevel")
    return top.resolve() if top is not None else repo


def git_common_dir(repo: Path) -> Path:
    return git_path(repo, "--git-common-dir") or repo / ".git"


def open_read_only(database: Path) -> sqlite3.Connection:
    database = database.expanduser().resolve()
    if not database.is_file():
        raise FileNotFoundError(f"Loopflow evidence database does not exist: {database}")
    uri = f"file:{quote(str(database))}?mode=ro"
    connection = sqlite3.connect(uri, uri=True)
    connection.row_factory = sqlite3.Row
    return connection


def load_policy(repo: Path) -> dict[str, Any]:
    path = repo / POLICY_PATH
    policy = json.loads(path.read_text(encoding="utf-8"))
    if policy.get("schema_version") != 1:
        raise ValueError(f"unsupported performance budget schema {policy.get('schema_version')}")
    return policy


def belongs_to_repo(value: str, repo: Path) -> bool:
    candidate = Path(value).expanduser().resolve()
    if candidate == repo:
        return True
    return candidate.parent == repo.parent and candidate.name.startswith(f"{repo.name}.")


def load_runs(path: Path, repo: Path, since: int, until: int) -> list[dict[str, Any]]:
    runs = json.loads(path.read_text(encoding="utf-8"))
    return [
        run
        for run in runs
        if run["repo"] is not None
        and belongs_to_repo(run["repo"], repo)
        and run["ended"] is not None
        and since <= run["ended"] <= until
    ]


def fresh_github_observation(value: str | None) -> bool:
    if value is None:
        return False
    try:
        payload = json.loads(value)
    except json.JSONDecodeError:
        return False
    return payload.get("result", {}).get("state") == "fresh"


def load_lifecycle(
    connection: sqlite3.Connection, repo: Path, since: int, until: int
) -> list[dict[str, Any]]:
    prs = []
    for row in connection.execute(
        """
        SELECT wave.repo, pr.created_at, pr.publication_requested_at,
               pr.merge_requested_at AS requested_at, pr.merged_at,
               pr.merge_tracking_complete, pr.repair_tracking_complete,
               pr.github_observation,
               EXISTS(SELECT 1 FROM task_pr_repair_incidents incident
                        WHERE incident.task_pr_id=pr.id
                          AND incident.kind='avoidable_rebase_agent')
                   AS avoidable_rebase_agent,
               EXISTS(SELECT 1 FROM task_pr_repair_incidents incident
                        WHERE incident.task_pr_id=pr.id
                          AND incident.kind='manual_git_repair')
                   AS manual_git_repair
          FROM task_prs pr
          JOIN tasks task ON task.id=pr.task_id
          JOIN projects project ON project.id=task.project_id
          JOIN waves wave ON wave.id=project.wave_id
         WHERE pr.merge_requested_at IS NOT NULL
           AND pr.merge_commit IS NOT NULL
           AND COALESCE(pr.merged_at, pr.updated_at) BETWEEN ? AND ?
         ORDER BY COALESCE(pr.merged_at, pr.updated_at), pr.id
        """,
        (since, until),
    ):
        if not belongs_to_repo(row["repo"], repo):
            continue
        value = dict(row)
        value["merge_observation_complete"] = fresh_github_observation(
            value.pop("github_observation")
        )
        prs.append(value)
    return prs


def task_loop_trust_observation(window_ended_at: datetime) -> dict[str, Any]:
    return {
        "wave": "product",
        "metric_id": "task-loop-trust",
        "instrument": "lifecycle-scorecard",
        "kind": "unavailable",
        "source_as_of": window_ended_at.isoformat().replace("+00:00", "Z"),
        "reason": "Current Work records do not identify complete Task-loop intervals",
    }


def parse_time(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00")).astimezone(timezone.utc)


def load_gates(repo: Path) -> list[dict[str, Any]]:
    root = git_common_dir(repo) / "loopflow/pre-land/runs"
    gates: list[dict[str, Any]] = []
    for kind in ("changed", "full"):
        for path in sorted((root / kind).glob("*.json")):
            gate = json.loads(path.read_text(encoding="utf-8"))
            if gate.get("schema") not in (1, 2, 3):
                raise ValueError(
                    f"unsupported pre-land evidence schema {gate.get('schema')} in {path}"
                )
            gates.append(gate)
    return gates


def percentile(values: Sequence[float], quantile: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    rank = max(1, math.ceil(quantile * len(ordered)))
    return ordered[rank - 1]


def metric_budget(policy: Mapping[str, Any], metric: str) -> dict[str, Any]:
    try:
        return dict(policy["metrics"][metric])
    except KeyError as error:
        raise ValueError(f"performance budget '{metric}' is missing") from error


def measured_row(
    metric: str,
    label: str,
    provider: str | None,
    values: Iterable[float | int],
    eligible: int,
    budget: Mapping[str, Any],
    minimum: int,
) -> dict[str, Any]:
    samples = sorted(float(value) for value in values)
    p50 = percentile(samples, 0.50)
    p95 = percentile(samples, 0.95)
    breach = any(
        limit is not None and value is not None and value > limit
        for limit, value in ((budget.get("p50"), p50), (budget.get("p95"), p95))
    ) or (budget.get("maximum") is not None and any(value > budget["maximum"] for value in samples))
    if breach:
        verdict, reason = "fail", "observed value exceeds budget"
    elif eligible == 0:
        verdict, reason = "unknown", "no eligible evidence in window"
    elif len(samples) < eligible:
        verdict = "unknown"
        reason = f"{eligible - len(samples)} of {eligible} eligible samples are missing"
    elif len(samples) < minimum:
        verdict = "collecting"
        reason = f"{len(samples)} samples; p95 requires {minimum}"
    elif all(budget.get(key) is None for key in ("p50", "p95", "maximum")):
        verdict, reason = "unbudgeted", "no budget configured for this measurement"
    else:
        verdict, reason = "pass", None
    return {
        "id": metric,
        "label": label,
        "provider": provider,
        "eligible": eligible,
        "measured": len(samples),
        "p50": p50,
        "p95": p95,
        "budget": dict(budget),
        "verdict": verdict,
        "reason": reason,
    }


def unknown_row(policy: Mapping[str, Any], metric: str, label: str, reason: str) -> dict[str, Any]:
    row = measured_row(
        metric, label, None, [], 0, metric_budget(policy, metric), policy["minimum_p95_samples"]
    )
    row["reason"] = reason
    return row


def gate_in_window(gate: Mapping[str, Any], since: datetime) -> bool:
    value = gate.get("finished_at")
    return isinstance(value, str) and parse_time(value) >= since


def gate_row(
    policy: Mapping[str, Any],
    gates: Sequence[Mapping[str, Any]],
    kind: str,
    metric: str,
    label: str,
    since: datetime,
) -> dict[str, Any]:
    eligible = [gate for gate in gates if gate.get("kind") == kind and gate_in_window(gate, since)]
    values = [
        sum(float(phase["elapsed_s"]) for phase in gate.get("phases", []))
        for gate in eligible
        if gate.get("status") == "passed"
        and all(phase.get("status") == "passed" for phase in gate.get("phases", []))
    ]
    return measured_row(
        metric,
        label,
        None,
        values,
        len(eligible),
        metric_budget(policy, metric),
        policy["minimum_p95_samples"],
    )


def phase_rows(
    policy: Mapping[str, Any], gates: Sequence[Mapping[str, Any]], since: datetime
) -> list[dict[str, Any]]:
    rows = []
    for metric, budget in sorted(policy["metrics"].items()):
        if not metric.startswith("preland_phase."):
            continue
        phase_name = metric.removeprefix("preland_phase.")
        phases = [
            phase
            for gate in gates
            if gate_in_window(gate, since)
            for phase in gate.get("phases", [])
            if phase.get("phase") == phase_name
        ]
        values = [phase["elapsed_s"] for phase in phases if phase.get("status") != "not_run"]
        rows.append(
            measured_row(
                metric,
                f"Pre-land phase · {phase_name}",
                None,
                values,
                len(phases),
                budget,
                policy["minimum_p95_samples"],
            )
        )
    return rows


def resource_row(
    policy: Mapping[str, Any],
    gates: Sequence[Mapping[str, Any]],
    metric: str,
    label: str,
    field: str,
    since: datetime,
) -> dict[str, Any]:
    eligible = [gate for gate in gates if gate_in_window(gate, since)]
    values = []
    for gate in eligible:
        resources = gate.get("resources") or {}
        if resources.get(field) is not None:
            values.append(resources[field])
    return measured_row(
        metric,
        label,
        None,
        values,
        len(eligible),
        metric_budget(policy, metric),
        policy["minimum_p95_samples"],
    )


def usage_rows(
    policy: Mapping[str, Any], runs: Sequence[Mapping[str, Any]], provider: str | None
) -> list[dict[str, Any]]:
    samples = [run for run in runs if provider is None or run["harness"] == provider]
    suffix = f".{provider}" if provider else ""
    fields = [
        ("run_total_input_tokens", "Total input / Run", "total_input_tokens"),
        ("run_output_tokens", "Output / Run", "output_tokens"),
        ("run_cost_usd", "Reported cost / Run", "cost_usd"),
    ]
    rows = []
    for metric, label, field in fields:
        values = [
            run["usage"][field]
            for run in samples
            if run["usage"][field] is not None
            and run["usage"]["gaps"] == 0
            and run["evidence_gaps"] == 0
            and run["usage"]["streams"] == run["usage"]["final_streams"]
        ]
        rows.append(
            measured_row(
                f"{metric}{suffix}",
                label,
                provider,
                values,
                len(samples),
                metric_budget(policy, metric),
                policy["minimum_p95_samples"],
            )
        )
    rows.append(
        measured_row(
            f"run_elapsed_seconds{suffix}",
            "Elapsed / Run",
            provider,
            [run["ended"] - run["started"] for run in samples if run["ended"] >= run["started"]],
            len(samples),
            metric_budget(policy, "run_elapsed_seconds"),
            policy["minimum_p95_samples"],
        )
    )
    return rows


def build_report(
    policy: Mapping[str, Any],
    repo: Path,
    generated_at: datetime,
    runs: Sequence[Mapping[str, Any]],
    gates: Sequence[Mapping[str, Any]],
    prs: Sequence[Mapping[str, Any]],
) -> dict[str, Any]:
    since_time = generated_at - timedelta(days=int(policy["window_days"]))
    minimum = int(policy["minimum_p95_samples"])
    rows = [
        unknown_row(
            policy,
            "task_first_progress_seconds",
            "Task launch → first progress",
            "Run records do not record the first material progress boundary",
        ),
        gate_row(
            policy, gates, "changed", "preland_changed_seconds", "Pre-land · changed", since_time
        ),
        gate_row(policy, gates, "full", "preland_full_seconds", "Pre-land · full", since_time),
        *phase_rows(policy, gates, since_time),
    ]
    for metric, label, start in [
        ("task_pr_to_merge_seconds", "Task PR creation → merge", "created_at"),
        ("publication_to_merge_seconds", "Publication request → merge", "publication_requested_at"),
        ("land_to_merge_seconds", "Land request → merge", "requested_at"),
    ]:
        values = [
            pr["merged_at"] - pr[start]
            for pr in prs
            if pr["merge_tracking_complete"]
            and pr["merge_observation_complete"]
            and pr["merged_at"] is not None
            and pr[start] is not None
            and pr["merged_at"] >= pr[start]
        ]
        rows.append(
            measured_row(
                metric,
                label,
                None,
                values,
                len(prs),
                metric_budget(policy, metric),
                minimum,
            )
        )
    for metric, label, field in [
        ("avoidable_repairs", "Avoidable repair", "avoidable_rebase_agent"),
        ("manual_git_repairs", "Manual git repair", "manual_git_repair"),
    ]:
        values = [
            int(bool(pr[field]))
            for pr in prs
            if pr["merge_observation_complete"]
            and pr["merged_at"] is not None
            and (pr[field] or pr["repair_tracking_complete"])
        ]
        rows.append(
            measured_row(
                metric,
                label,
                None,
                values,
                len(prs),
                metric_budget(policy, metric),
                minimum,
            )
        )
    rows.extend(
        [
            unknown_row(
                policy,
                "credential_expiry_blocks",
                "Credential-expiry block",
                "provider account state has no incident history",
            ),
            resource_row(
                policy,
                gates,
                "build_disk_bytes",
                "Build artifacts / gate",
                "build_disk_bytes",
                since_time,
            ),
            resource_row(
                policy,
                gates,
                "preland_cpu_seconds",
                "Pre-land child CPU",
                "cpu_seconds",
                since_time,
            ),
            *usage_rows(policy, runs, None),
        ]
    )
    for provider in sorted({str(run["harness"]) for run in runs}):
        rows.extend(usage_rows(policy, runs, provider))
    return {
        "schema_version": SCHEMA_VERSION,
        "repo": repo.name,
        "window_started_at": since_time.isoformat().replace("+00:00", "Z"),
        "window_ended_at": generated_at.isoformat().replace("+00:00", "Z"),
        "window_days": policy["window_days"],
        "minimum_p95_samples": minimum,
        "rows": rows,
    }


def format_value(value: float, unit: str) -> str:
    if unit == "seconds":
        return f"{value:.1f}s"
    if unit == "tokens":
        return f"{round(value):,}"
    if unit == "usd":
        return f"${value:.2f}"
    if unit == "bytes":
        return f"{value / 1024**3:.1f}GiB"
    return f"{value:.0f}"


def value_budget(value: float | None, budget: float | None, unit: str) -> str:
    left = "—" if value is None else format_value(value, unit)
    right = "—" if budget is None else format_value(budget, unit)
    return f"{left} / {right}"


def print_report(report: Mapping[str, Any], output: TextIO = sys.stdout) -> None:
    print(
        f"Lifecycle scorecard · {report['repo']} · {report['window_days']} days "
        f"through {report['window_ended_at']}\n",
        file=output,
    )
    print(
        f"{'MEASURE':<38}  {'COVERAGE':>10}  {'P50 / BUDGET':>17}  "
        f"{'P95 / BUDGET':>17}  {'VERDICT':>10}",
        file=output,
    )
    for row in report["rows"]:
        label = row["label"]
        if row["provider"]:
            label = f"{label} · {row['provider']}"
        label = label if len(label) <= 38 else f"{label[:37]}…"
        budget = row["budget"]
        upper_budget = budget.get("p95")
        if upper_budget is None:
            upper_budget = budget.get("maximum")
        print(
            f"{label:<38}  {row['measured']}/{row['eligible']:>8}  "
            f"{value_budget(row['p50'], budget.get('p50'), budget['unit']):>17}  "
            f"{value_budget(row['p95'], upper_budget, budget['unit']):>17}  "
            f"{row['verdict'].upper():>10}",
            file=output,
        )
        if row["reason"]:
            print(f"  {row['reason']}", file=output)


def format_report(report: Mapping[str, Any]) -> str:
    output = io.StringIO()
    print_report(report, output)
    return output.getvalue()


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        repo = canonical_repo(args.repo)
        policy = load_policy(args.repo)
        database = args.database
        generated_at = datetime.now(timezone.utc)
        since = int((generated_at - timedelta(days=policy["window_days"])).timestamp())
        until = int(generated_at.timestamp())
        runs = load_runs(args.runs, repo, since, until)
        with open_read_only(database) as connection:
            prs = load_lifecycle(connection, repo, since, until)
        metric_observation = task_loop_trust_observation(generated_at)
        report = build_report(policy, repo, generated_at, runs, load_gates(repo), prs)
    except (FileNotFoundError, OSError, ValueError, sqlite3.Error) as error:
        print(f"lifecycle-scorecard: {error}", file=sys.stderr)
        return 1
    if args.envelope:
        print(
            json.dumps(
                {
                    "report": report,
                    "metric_observations": [metric_observation],
                    "text": format_report(report),
                },
                sort_keys=True,
            )
        )
    elif args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print_report(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
