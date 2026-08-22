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
from typing import Any, Callable, Iterable, Mapping, Sequence, TextIO
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


def load_usage(connection: sqlite3.Connection, repo: Path, since: int) -> list[dict[str, Any]]:
    rows = connection.execute(
        """
        SELECT invocation.repo, invocation.provider,
               usage.total_input_tokens, usage.output_tokens, usage.cost_usd
          FROM agent_turns turn
          JOIN agent_invocations invocation ON invocation.id=turn.invocation_id
          LEFT JOIN turn_usage_samples usage
            ON usage.turn_id=turn.id
           AND usage.observed_at=(
               SELECT MAX(latest.observed_at) FROM turn_usage_samples latest
               WHERE latest.turn_id=turn.id
           )
         WHERE COALESCE(turn.ended_at, turn.started_at) >= ?
         ORDER BY turn.ended_at, turn.rowid
        """,
        (since,),
    )
    return [
        {
            "provider": row["provider"],
            "total_input_tokens": row["total_input_tokens"],
            "output_tokens": row["output_tokens"],
            "cost_usd": row["cost_usd"],
        }
        for row in rows
        if belongs_to_repo(row["repo"], repo)
    ]


def table_exists(connection: sqlite3.Connection, table: str) -> bool:
    return bool(
        connection.execute(
            "SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?", (table,)
        ).fetchone()
    )


def column_exists(connection: sqlite3.Connection, table: str, column: str) -> bool:
    return any(row["name"] == column for row in connection.execute(f"PRAGMA table_info({table})"))


def lifecycle_schema_available(connection: sqlite3.Connection) -> bool:
    return all(
        [
            table_exists(connection, "performance_evidence_authority"),
            table_exists(connection, "task_pr_repair_incidents"),
            column_exists(connection, "runs", "first_material_at"),
            column_exists(connection, "task_prs", "merged_at"),
            column_exists(connection, "task_prs", "merge_tracking_complete"),
            column_exists(connection, "task_prs", "repair_tracking_complete"),
        ]
    )


def fresh_github_observation(value: str | None) -> bool:
    if value is None:
        return False
    try:
        payload = json.loads(value)
    except json.JSONDecodeError:
        return False
    return payload.get("result", {}).get("state") == "fresh"


def load_lifecycle(connection: sqlite3.Connection, repo: Path, since: int) -> dict[str, Any] | None:
    if not lifecycle_schema_available(connection):
        return None
    authority = connection.execute(
        "SELECT started_at FROM performance_evidence_authority WHERE singleton=1"
    ).fetchone()
    if authority is None:
        return None

    runs = [
        dict(row)
        for row in connection.execute(
            """
            SELECT COALESCE(run.cwd, task.worktree) AS worktree,
                   run.started_at, run.ended_at, run.first_material_at
              FROM runs run
              JOIN epochs epoch ON epoch.id=run.epoch_id
              JOIN tasks task ON task.id=epoch.task_id
             WHERE epoch.task_id IS NOT NULL
               AND run.started_at IS NOT NULL
               AND run.ended_at >= ?
             ORDER BY run.ended_at, run.id
            """,
            (since,),
        )
        if belongs_to_repo(row["worktree"], repo)
    ]
    prs = []
    for row in connection.execute(
        """
        SELECT task.worktree, pr.merge_requested_at AS requested_at, pr.merged_at,
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
         WHERE pr.merge_requested_at IS NOT NULL
           AND pr.merge_commit IS NOT NULL
           AND (pr.merged_at >= ?
                OR (pr.merge_tracking_complete=1 AND pr.merged_at IS NULL))
         ORDER BY COALESCE(pr.merged_at, pr.merge_requested_at), pr.id
        """,
        (since,),
    ):
        if not belongs_to_repo(row["worktree"], repo):
            continue
        value = dict(row)
        value["merge_observation_complete"] = fresh_github_observation(
            value.pop("github_observation")
        )
        prs.append(value)
    return {
        "authority_started_at": authority["started_at"],
        "task_runs": runs,
        "task_prs": prs,
    }


def task_loop_trust_schema_available(connection: sqlite3.Connection) -> bool:
    return all(
        [
            lifecycle_schema_available(connection),
            table_exists(connection, "task_events"),
            column_exists(connection, "epochs", "terminal_at"),
            column_exists(connection, "tasks", "worktree"),
            column_exists(connection, "task_prs", "merge_mode"),
            column_exists(connection, "task_prs", "created_at"),
            column_exists(connection, "task_prs", "merged_at"),
            column_exists(connection, "task_prs", "abandoned_at"),
            column_exists(connection, "task_pr_repair_incidents", "occurred_at"),
        ]
    )


def task_loop_trust_observation(
    connection: sqlite3.Connection,
    repo: Path,
    window_started_at: datetime,
    window_ended_at: datetime,
) -> dict[str, Any]:
    identity = {
        "wave": "product",
        "metric_id": "task-loop-trust",
        "instrument": "lifecycle-scorecard",
    }
    source_window_start = window_started_at.isoformat().replace("+00:00", "Z")
    source_window_end = window_ended_at.isoformat().replace("+00:00", "Z")
    if not task_loop_trust_schema_available(connection):
        return {
            **identity,
            "kind": "unavailable",
            "source_as_of": source_window_end,
            "reason": "Task loop evidence schema is not available",
        }

    authority = connection.execute(
        "SELECT started_at FROM performance_evidence_authority WHERE singleton=1"
    ).fetchone()
    if authority is None or int(authority["started_at"]) > int(window_started_at.timestamp()):
        return {
            **identity,
            "kind": "unavailable",
            "source_as_of": source_window_end,
            "reason": "Task loop evidence authority does not cover the complete window",
        }

    rows = connection.execute(
        """
        SELECT task.worktree, epoch.state,
               NOT EXISTS (
                   SELECT 1 FROM task_prs pr
                   WHERE pr.task_id=task.id
                     AND pr.created_at <= epoch.terminal_at
                     AND (
                         (pr.merged_at IS NULL AND pr.abandoned_at IS NULL)
                         OR pr.merged_at >= epoch.created_at
                         OR pr.abandoned_at >= epoch.created_at
                     )
                     AND (pr.merge_mode IS NOT 'auto' OR pr.merge_commit IS NULL)
               ) AS all_prs_landed_unattended,
               EXISTS (
                   SELECT 1 FROM task_events event
                   WHERE event.task_id=task.id
                     AND event.created_at >= epoch.created_at
                     AND event.created_at <= epoch.terminal_at
                     AND json_extract(event.kind_json, '$.kind')='failed'
                     AND json_extract(event.kind_json, '$.resumable')=0
               ) AS actionable_non_convergence,
               EXISTS (
                   SELECT 1 FROM task_prs pr
                   JOIN task_pr_repair_incidents incident ON incident.task_pr_id=pr.id
                   WHERE pr.task_id=task.id
                     AND pr.created_at <= epoch.terminal_at
                     AND (
                         (pr.merged_at IS NULL AND pr.abandoned_at IS NULL)
                         OR pr.merged_at >= epoch.created_at
                         OR pr.abandoned_at >= epoch.created_at
                     )
                     AND incident.kind='manual_git_repair'
                     AND incident.occurred_at >= epoch.created_at
                     AND incident.occurred_at <= epoch.terminal_at
               ) AS manual_repair
          FROM epochs epoch
          JOIN tasks task ON task.id=epoch.task_id
         WHERE epoch.task_id IS NOT NULL
           AND epoch.state IN ('done', 'abandoned')
           AND epoch.terminal_at >= ?
           AND epoch.terminal_at <= ?
         ORDER BY epoch.terminal_at, epoch.id
        """,
        (int(window_started_at.timestamp()), int(window_ended_at.timestamp())),
    )
    eligible = [
        row
        for row in rows
        if row["worktree"] is not None and belongs_to_repo(row["worktree"], repo)
    ]
    successful = sum(
        1
        for row in eligible
        if not row["manual_repair"]
        and (
            (row["state"] == "done" and row["all_prs_landed_unattended"])
            or (row["state"] == "abandoned" and row["actionable_non_convergence"])
        )
    )
    return {
        **identity,
        "kind": "observed",
        "value": successful / len(eligible) if eligible else 0.0,
        "source_window_start": source_window_start,
        "source_window_end": source_window_end,
        "complete": bool(eligible),
        "eligible": len(eligible),
        "successful": successful,
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


def enforce_authority(
    row: dict[str, Any], lifecycle: Mapping[str, Any], since: int
) -> dict[str, Any]:
    started = int(lifecycle["authority_started_at"])
    if row["verdict"] != "fail" and since < started:
        stamp = datetime.fromtimestamp(started, timezone.utc).isoformat().replace("+00:00", "Z")
        row["verdict"] = "unknown"
        row["reason"] = (
            f"lifecycle evidence authority began {stamp}; the requested window is not fully covered"
        )
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
    policy: Mapping[str, Any], usage: Sequence[Mapping[str, Any]], provider: str | None
) -> list[dict[str, Any]]:
    samples = [row for row in usage if provider is None or row["provider"] == provider]
    suffix = f".{provider}" if provider else ""
    fields = [
        ("agent_total_input_tokens", "Agent total input / Turn", "total_input_tokens"),
        ("agent_output_tokens", "Agent output / Turn", "output_tokens"),
        ("agent_cost_usd", "Reported agent cost / Turn", "cost_usd"),
    ]
    return [
        measured_row(
            f"{metric}{suffix}",
            label,
            provider,
            [sample[field] for sample in samples if sample.get(field) is not None],
            len(samples),
            metric_budget(policy, metric),
            policy["minimum_p95_samples"],
        )
        for metric, label, field in fields
    ]


def build_report(
    policy: Mapping[str, Any],
    repo: Path,
    generated_at: datetime,
    usage: Sequence[Mapping[str, Any]],
    gates: Sequence[Mapping[str, Any]],
    lifecycle: Mapping[str, Any] | None,
) -> dict[str, Any]:
    since_time = generated_at - timedelta(days=int(policy["window_days"]))
    since = int(since_time.timestamp())
    minimum = int(policy["minimum_p95_samples"])
    if lifecycle is None:
        first = unknown_row(
            policy,
            "task_first_progress_seconds",
            "Task launch → first progress",
            "lifecycle evidence schema not yet promoted",
        )
    else:
        runs = [row for row in lifecycle["task_runs"] if row["ended_at"] >= since]
        values = [
            row["first_material_at"] - row["started_at"]
            for row in runs
            if row["first_material_at"] is not None
            and row["started_at"] <= row["first_material_at"] <= row["ended_at"]
        ]
        first = enforce_authority(
            measured_row(
                "task_first_progress_seconds",
                "Task launch → first progress",
                None,
                values,
                len(runs),
                metric_budget(policy, "task_first_progress_seconds"),
                minimum,
            ),
            lifecycle,
            since,
        )
    rows = [
        first,
        gate_row(
            policy,
            gates,
            "changed",
            "preland_changed_seconds",
            "Pre-land · changed",
            since_time,
        ),
        gate_row(
            policy,
            gates,
            "full",
            "preland_full_seconds",
            "Pre-land · full",
            since_time,
        ),
        *phase_rows(policy, gates, since_time),
    ]

    if lifecycle is None:
        rows.extend(
            [
                unknown_row(
                    policy,
                    "land_to_merge_seconds",
                    "Land request → merge",
                    "lifecycle evidence schema not yet promoted",
                ),
                unknown_row(
                    policy,
                    "avoidable_repairs",
                    "Avoidable repair",
                    "lifecycle evidence schema not yet promoted",
                ),
            ]
        )
    else:
        prs = [
            row
            for row in lifecycle["task_prs"]
            if (row["merged_at"] is not None and row["merged_at"] >= since)
            or (row["merge_tracking_complete"] and row["merged_at"] is None)
        ]
        merge_values = [
            row["merged_at"] - row["requested_at"]
            for row in prs
            if row["merge_observation_complete"]
            and row["merged_at"] is not None
            and row["merged_at"] >= row["requested_at"]
        ]
        rows.append(
            enforce_authority(
                measured_row(
                    "land_to_merge_seconds",
                    "Land request → merge",
                    None,
                    merge_values,
                    len(prs),
                    metric_budget(policy, "land_to_merge_seconds"),
                    minimum,
                ),
                lifecycle,
                since,
            )
        )
        repair_specs: list[tuple[str, str, Callable[[Mapping[str, Any]], bool]]] = [
            (
                "avoidable_repairs",
                "Avoidable repair",
                lambda row: bool(row["avoidable_rebase_agent"]),
            ),
        ]
        for metric, label, incident in repair_specs:
            values = [
                1 if incident(row) else 0
                for row in prs
                if row["merged_at"] is not None
                and row["merge_observation_complete"]
                and (incident(row) or row["repair_tracking_complete"])
            ]
            rows.append(
                enforce_authority(
                    measured_row(
                        metric,
                        label,
                        None,
                        values,
                        len(prs),
                        metric_budget(policy, metric),
                        minimum,
                    ),
                    lifecycle,
                    since,
                )
            )

    rows.append(
        unknown_row(
            policy,
            "credential_expiry_blocks",
            "Credential-expiry block",
            "provider account state has no incident history",
        )
    )
    if lifecycle is None:
        rows.append(
            unknown_row(
                policy,
                "manual_git_repairs",
                "Manual git repair",
                "lifecycle evidence schema not yet promoted",
            )
        )
    else:
        prs = lifecycle["task_prs"]
        values = [
            1 if row["manual_git_repair"] else 0
            for row in prs
            if row["merged_at"] is not None
            and row["merge_observation_complete"]
            and (row["manual_git_repair"] or row["repair_tracking_complete"])
        ]
        rows.append(
            enforce_authority(
                measured_row(
                    "manual_git_repairs",
                    "Manual git repair",
                    None,
                    values,
                    len(prs),
                    metric_budget(policy, "manual_git_repairs"),
                    minimum,
                ),
                lifecycle,
                since,
            )
        )
    rows.extend(
        [
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
            *usage_rows(policy, usage, None),
        ]
    )
    for provider in sorted({str(sample["provider"]) for sample in usage}):
        rows.extend(usage_rows(policy, usage, provider))
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
        policy = load_policy(repo)
        database = args.database
        generated_at = datetime.now(timezone.utc)
        since = int((generated_at - timedelta(days=policy["window_days"])).timestamp())
        with open_read_only(database) as connection:
            usage = load_usage(connection, repo, since)
            lifecycle = load_lifecycle(connection, repo, since)
            metric_observation = task_loop_trust_observation(
                connection,
                repo,
                generated_at - timedelta(days=7),
                generated_at,
            )
        report = build_report(policy, repo, generated_at, usage, load_gates(repo), lifecycle)
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
