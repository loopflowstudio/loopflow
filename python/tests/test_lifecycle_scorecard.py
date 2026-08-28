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
            merge_commit TEXT,
            abandoned_at INTEGER
        );
        CREATE TABLE task_pr_repair_incidents (
            task_pr_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            occurred_at INTEGER NOT NULL
        );
        """
    )
    return connection


def _task_loop_connection() -> sqlite3.Connection:
    connection = _performance_connection()
    connection.executescript(
        """
        ALTER TABLE epochs ADD COLUMN state TEXT;
        ALTER TABLE epochs ADD COLUMN created_at INTEGER;
        ALTER TABLE epochs ADD COLUMN terminal_at INTEGER;
        ALTER TABLE task_prs ADD COLUMN merge_mode TEXT;
        ALTER TABLE task_prs ADD COLUMN created_at INTEGER;
        CREATE TABLE task_events (
            task_id TEXT NOT NULL,
            kind_json TEXT NOT NULL,
            created_at INTEGER NOT NULL
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


def test_usage_loader_reads_latest_turn_sample_without_inventing_missing_values(
    tmp_path: Path,
) -> None:
    repo = tmp_path / "loopflow"
    repo.mkdir()
    connection = sqlite3.connect(":memory:")
    connection.row_factory = sqlite3.Row
    connection.executescript(
        """
        CREATE TABLE agent_invocations (
            id TEXT PRIMARY KEY,
            repo TEXT NOT NULL,
            provider TEXT NOT NULL
        );
        CREATE TABLE agent_turns (
            id TEXT PRIMARY KEY,
            invocation_id TEXT NOT NULL,
            started_at INTEGER NOT NULL,
            ended_at INTEGER
        );
        CREATE TABLE turn_usage_samples (
            turn_id TEXT NOT NULL,
            observed_at INTEGER NOT NULL,
            total_input_tokens INTEGER,
            output_tokens INTEGER,
            cost_usd REAL
        );
        """
    )
    connection.execute(
        "INSERT INTO agent_invocations VALUES ('invocation-1', ?, 'codex')",
        (str(repo),),
    )
    connection.execute(
        "INSERT INTO agent_turns VALUES ('turn-1', 'invocation-1', 100, 120)"
    )
    connection.execute(
        "INSERT INTO agent_turns VALUES ('turn-2', 'invocation-1', 100, 130)"
    )
    connection.execute(
        "INSERT INTO turn_usage_samples VALUES ('turn-1', 110, 10, 2, 0.1)"
    )
    connection.execute(
        "INSERT INTO turn_usage_samples VALUES ('turn-1', 120, 20, 4, 0.2)"
    )

    usage = scorecard.load_usage(connection, repo, since=90)

    assert usage == [
        {
            "provider": "codex",
            "total_input_tokens": 20,
            "output_tokens": 4,
            "cost_usd": 0.2,
        },
        {
            "provider": "codex",
            "total_input_tokens": None,
            "output_tokens": None,
            "cost_usd": None,
        },
    ]


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
            '{"result":{"state":"fresh"}}', 'merge-sha', NULL
        )
        """
    )
    connection.execute(
        "INSERT INTO task_pr_repair_incidents VALUES ('pr-1', 'manual_git_repair', 130)"
    )

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
            "INSERT INTO task_prs VALUES (?, ?, ?, ?, 1, 1, ?, ?, NULL)",
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
            "INSERT INTO task_pr_repair_incidents VALUES ('pr-2', ?, ?)",
            (kind, window_started_at + 500),
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


def test_task_loop_trust_emits_one_exact_window_observation(tmp_path: Path) -> None:
    repo = tmp_path / "loopflow"
    repo.mkdir()
    connection = _task_loop_connection()
    started = datetime(2026, 7, 15, tzinfo=timezone.utc)
    ended = datetime(2026, 7, 22, tzinfo=timezone.utc)
    connection.execute(
        "INSERT INTO performance_evidence_authority VALUES (1, ?)",
        (int(started.timestamp()) - 1,),
    )
    for index, state in enumerate(("done", "done", "abandoned", "done"), start=1):
        task_id = f"task-{index}"
        epoch_id = f"epoch-{index}"
        connection.execute(
            "INSERT INTO tasks (id, worktree) VALUES (?, ?)",
            (task_id, str(repo)),
        )
        connection.execute(
            "INSERT INTO epochs (id, task_id, state, created_at, terminal_at) VALUES (?, ?, ?, ?, ?)",
            (
                epoch_id,
                task_id,
                state,
                int(started.timestamp()) + index,
                int(ended.timestamp()) - index,
            ),
        )
    connection.execute(
        "INSERT INTO task_prs (id, task_id, merge_commit, merge_tracking_complete, repair_tracking_complete, merge_mode, created_at) VALUES ('pr-1', 'task-1', 'merge-1', 1, 1, 'auto', ?)",
        (int(started.timestamp()) + 10,),
    )
    connection.execute(
        "INSERT INTO task_prs (id, task_id, merge_commit, merge_tracking_complete, repair_tracking_complete, merge_mode, created_at) VALUES ('pr-2', 'task-2', 'merge-2', 1, 1, 'user', ?)",
        (int(started.timestamp()) + 10,),
    )
    connection.execute(
        "INSERT INTO task_events VALUES ('task-3', ?, ?)",
        ('{"kind":"failed","error":"bounded attempts exhausted","resumable":false}', int(ended.timestamp()) - 4),
    )
    connection.execute(
        "INSERT INTO task_prs (id, task_id, merge_commit, merge_tracking_complete, repair_tracking_complete, merge_mode, created_at) VALUES ('pr-4', 'task-4', 'merge-4', 1, 1, 'auto', ?)",
        (int(started.timestamp()) + 10,),
    )
    connection.execute(
        "INSERT INTO task_pr_repair_incidents VALUES ('pr-4', 'manual_git_repair', ?)",
        (int(ended.timestamp()) - 5,),
    )

    observation = scorecard.task_loop_trust_observation(
        connection, repo, started, ended
    )

    assert observation == {
        "wave": "product",
        "metric_id": "task-loop-trust",
        "instrument": "lifecycle-scorecard",
        "kind": "observed",
        "value": 0.5,
        "source_window_start": "2026-07-15T00:00:00Z",
        "source_window_end": "2026-07-22T00:00:00Z",
        "complete": True,
        "eligible": 4,
        "successful": 2,
    }


def test_task_loop_trust_without_eligible_tasks_is_unavailable(
    tmp_path: Path,
) -> None:
    repo = tmp_path / "loopflow"
    repo.mkdir()
    connection = _task_loop_connection()
    started = datetime(2026, 7, 15, tzinfo=timezone.utc)
    ended = datetime(2026, 7, 22, tzinfo=timezone.utc)
    connection.execute(
        "INSERT INTO performance_evidence_authority VALUES (1, ?)",
        (int(started.timestamp()) - 1,),
    )

    observation = scorecard.task_loop_trust_observation(
        connection, repo, started, ended
    )

    assert observation == {
        "wave": "product",
        "metric_id": "task-loop-trust",
        "instrument": "lifecycle-scorecard",
        "kind": "unavailable",
        "source_as_of": "2026-07-22T00:00:00Z",
        "reason": "No eligible settled Tasks in the source window",
    }


def test_task_loop_trust_counts_a_pr_that_settles_after_its_epoch_starts(
    tmp_path: Path,
) -> None:
    repo = tmp_path / "loopflow"
    repo.mkdir()
    connection = _task_loop_connection()
    started = datetime(2026, 7, 15, tzinfo=timezone.utc)
    ended = datetime(2026, 7, 22, tzinfo=timezone.utc)
    started_at = int(started.timestamp())
    ended_at = int(ended.timestamp())
    connection.execute(
        "INSERT INTO performance_evidence_authority VALUES (1, ?)",
        (started_at - 1,),
    )
    for index in (1, 2):
        connection.execute(
            "INSERT INTO tasks (id, worktree) VALUES (?, ?)",
            (f"task-{index}", str(repo)),
        )
        connection.execute(
            "INSERT INTO epochs (id, task_id, state, created_at, terminal_at) VALUES (?, ?, 'done', ?, ?)",
            (f"epoch-{index}", f"task-{index}", started_at + 100, ended_at - 100),
        )
    connection.execute(
        "INSERT INTO task_prs (id, task_id, merge_commit, merged_at, merge_tracking_complete, repair_tracking_complete, merge_mode, created_at) VALUES ('pr-1', 'task-1', 'merge-1', ?, 1, 1, 'auto', ?)",
        (ended_at - 200, started_at + 10),
    )
    connection.execute(
        "INSERT INTO task_prs (id, task_id, merge_commit, merged_at, merge_tracking_complete, repair_tracking_complete, merge_mode, created_at) VALUES ('pr-2', 'task-2', 'merge-2', ?, 1, 1, 'user', ?)",
        (ended_at - 200, started_at + 10),
    )

    observation = scorecard.task_loop_trust_observation(
        connection, repo, started, ended
    )

    assert observation["eligible"] == 2
    assert observation["successful"] == 1
    assert observation["value"] == 0.5


def test_task_loop_trust_ignores_manual_repair_from_an_earlier_epoch(
    tmp_path: Path,
) -> None:
    repo = tmp_path / "loopflow"
    repo.mkdir()
    connection = _task_loop_connection()
    started = datetime(2026, 7, 15, tzinfo=timezone.utc)
    ended = datetime(2026, 7, 22, tzinfo=timezone.utc)
    started_at = int(started.timestamp())
    ended_at = int(ended.timestamp())
    epoch_started_at = started_at + 200
    epoch_ended_at = ended_at - 100
    connection.execute(
        "INSERT INTO performance_evidence_authority VALUES (1, ?)",
        (started_at - 1,),
    )
    connection.execute("INSERT INTO tasks VALUES ('task-1', ?)", (str(repo),))
    connection.execute(
        "INSERT INTO epochs (id, task_id, state, created_at, terminal_at) VALUES ('epoch-1', 'task-1', 'done', ?, ?)",
        (epoch_started_at, epoch_ended_at),
    )
    connection.execute(
        "INSERT INTO task_prs (id, task_id, merge_commit, merged_at, merge_tracking_complete, repair_tracking_complete, merge_mode, created_at) VALUES ('pr-1', 'task-1', 'merge-1', ?, 1, 1, 'auto', ?)",
        (epoch_ended_at - 1, started_at + 10),
    )
    connection.execute(
        "INSERT INTO task_pr_repair_incidents VALUES ('pr-1', 'manual_git_repair', ?)",
        (epoch_started_at - 1,),
    )

    observation = scorecard.task_loop_trust_observation(
        connection, repo, started, ended
    )

    assert observation["eligible"] == 1
    assert observation["successful"] == 1
    assert observation["value"] == 1.0


def test_scorecard_is_owned_by_telemetry_flow_and_hidden_cli() -> None:
    root = Path(__file__).resolve().parents[2]
    flow = (root / ".lf/flows/telemetry-daily.yaml").read_text(encoding="utf-8")
    cli = (root / "rust/loopflow/src/lf/mod.rs").read_text(encoding="utf-8")

    assert flow.splitlines() == ["- op: doctor", "- op: __telemetry-scorecard"]
    assert '#[command(name = "__telemetry-scorecard", hide = true)]' in cli
    assert "Performance {" not in cli
