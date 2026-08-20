from __future__ import annotations

import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path

from scripts import lifecycle_scorecard as scorecard


def _performance_connection() -> sqlite3.Connection:
    connection = sqlite3.connect(":memory:")
    connection.row_factory = sqlite3.Row
    connection.executescript(
        """
        CREATE TABLE performance_evidence_authority (
            singleton INTEGER PRIMARY KEY,
            started_at INTEGER NOT NULL
        );
        CREATE TABLE tasks (id TEXT PRIMARY KEY, worktree TEXT NOT NULL);
        CREATE TABLE epochs (id TEXT PRIMARY KEY, task_id TEXT);
        CREATE TABLE runs (
            id TEXT PRIMARY KEY,
            epoch_id TEXT,
            cwd TEXT,
            started_at INTEGER,
            ended_at INTEGER,
            first_material_at INTEGER
        );
        CREATE TABLE task_prs (
            id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            merge_requested_at INTEGER,
            merged_at INTEGER,
            merge_tracking_complete INTEGER NOT NULL,
            repair_tracking_complete INTEGER NOT NULL,
            github_observation TEXT,
            merge_commit TEXT
        );
        CREATE TABLE task_pr_repair_incidents (
            task_pr_id TEXT NOT NULL,
            kind TEXT NOT NULL
        );
        """
    )
    return connection


def test_measured_row_preserves_missing_and_explicit_zero() -> None:
    budget = {"unit": "count", "p50": None, "p95": None, "maximum": 0.0}

    missing = scorecard.measured_row("repair", "Repair", None, [], 1, budget, 20)
    measured_zero = scorecard.measured_row("repair", "Repair", None, [0], 1, budget, 20)
    breach = scorecard.measured_row("repair", "Repair", None, [1], 1, budget, 20)

    assert (missing["measured"], missing["verdict"]) == (0, "unknown")
    assert (measured_zero["measured"], measured_zero["verdict"]) == (
        1,
        "collecting",
    )
    assert (breach["measured"], breach["verdict"]) == (1, "fail")


def test_lifecycle_loader_reads_authoritative_owner_facts(tmp_path: Path) -> None:
    repo = tmp_path / "loopflow"
    repo.mkdir()
    connection = _performance_connection()
    connection.execute("INSERT INTO performance_evidence_authority VALUES (1, 90)")
    connection.execute("INSERT INTO tasks VALUES ('task-1', ?)", (str(repo),))
    connection.execute("INSERT INTO epochs VALUES ('epoch-1', 'task-1')")
    connection.execute(
        "INSERT INTO runs VALUES ('run-1', 'epoch-1', ?, 100, 140, 112)",
        (str(repo),),
    )
    connection.execute(
        """
        INSERT INTO task_prs VALUES (
            'pr-1', 'task-1', 120, 150, 1, 1,
            '{"result":{"state":"fresh"}}', 'merge-sha'
        )
        """
    )
    connection.execute("INSERT INTO task_pr_repair_incidents VALUES ('pr-1', 'manual_git_repair')")

    evidence = scorecard.load_lifecycle(connection, repo, since=90)

    assert evidence is not None
    assert evidence["authority_started_at"] == 90
    assert evidence["task_runs"][0]["first_material_at"] == 112
    assert evidence["task_prs"][0]["merged_at"] == 150
    assert evidence["task_prs"][0]["merge_observation_complete"] is True
    assert evidence["task_prs"][0]["manual_git_repair"] == 1


def test_mixed_lifecycle_history_has_complete_json_coverage(tmp_path: Path) -> None:
    root = Path(__file__).resolve().parents[2]
    policy = json.loads(
        (root / "performance/budgets.json").read_text(encoding="utf-8")
    )
    generated_at = datetime(2026, 7, 22, tzinfo=timezone.utc)
    window_started_at = int(generated_at.timestamp()) - 14 * 24 * 60 * 60
    repo = tmp_path / "loopflow"
    repo.mkdir()
    worktrees = (repo, tmp_path / "loopflow.second")
    connection = _performance_connection()
    connection.execute(
        "INSERT INTO performance_evidence_authority VALUES (1, ?)",
        (window_started_at - 24 * 60 * 60,),
    )
    fresh = '{"result":{"state":"fresh"}}'
    for index, worktree in enumerate(worktrees, start=1):
        task_id = f"task-{index}"
        epoch_id = f"epoch-{index}"
        run_started_at = window_started_at + index * 200
        requested_at = window_started_at + index * 400
        connection.execute("INSERT INTO tasks VALUES (?, ?)", (task_id, str(worktree)))
        connection.execute("INSERT INTO epochs VALUES (?, ?)", (epoch_id, task_id))
        connection.execute(
            "INSERT INTO runs VALUES (?, ?, ?, ?, ?, ?)",
            (
                f"run-{index}",
                epoch_id,
                str(worktree),
                run_started_at,
                run_started_at + 100,
                run_started_at + index * 10,
            ),
        )
        connection.execute(
            "INSERT INTO task_prs VALUES (?, ?, ?, ?, 1, 1, ?, ?)",
            (
                f"pr-{index}",
                task_id,
                requested_at,
                requested_at + index * 100,
                fresh,
                f"merge-{index}",
            ),
        )
    for kind in ("avoidable_rebase_agent", "manual_git_repair"):
        connection.execute(
            "INSERT INTO task_pr_repair_incidents VALUES ('pr-2', ?)", (kind,)
        )
    lifecycle = scorecard.load_lifecycle(connection, repo, window_started_at)

    assert lifecycle is not None
    report = scorecard.build_report(policy, repo, generated_at, [], [], lifecycle)
    rows = {
        row["id"]: row
        for row in json.loads(json.dumps(report))["rows"]
        if row["id"]
        in {
            "task_first_progress_seconds",
            "land_to_merge_seconds",
            "avoidable_repairs",
            "manual_git_repairs",
        }
    }

    assert {metric: (row["eligible"], row["measured"]) for metric, row in rows.items()} == {
        "task_first_progress_seconds": (2, 2),
        "land_to_merge_seconds": (2, 2),
        "avoidable_repairs": (2, 2),
        "manual_git_repairs": (2, 2),
    }
    assert rows["avoidable_repairs"]["p50"] == 0
    assert rows["avoidable_repairs"]["p95"] == 1
    assert rows["manual_git_repairs"]["p50"] == 0
    assert rows["manual_git_repairs"]["p95"] == 1


def test_scorecard_is_owned_by_telemetry_flow_and_hidden_cli() -> None:
    root = Path(__file__).resolve().parents[2]
    flow = (root / ".lf/flows/telemetry-daily.yaml").read_text(encoding="utf-8")
    cli = (root / "rust/loopflow/src/lf/mod.rs").read_text(encoding="utf-8")

    assert flow.splitlines() == ["- op: doctor", "- op: __telemetry-scorecard"]
    assert '#[command(name = "__telemetry-scorecard", hide = true)]' in cli
    assert "Performance {" not in cli
