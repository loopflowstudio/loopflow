from __future__ import annotations

import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path

from scripts import lifecycle_scorecard as scorecard

ROOT = Path(__file__).resolve().parents[2]
NOW = datetime(2026, 9, 28, tzinfo=timezone.utc)
UNTIL = int(NOW.timestamp())
SINCE = UNTIL - 14 * 86400


def _policy() -> dict:
    return json.loads((ROOT / "performance/budgets.json").read_text())


def _connection(repo: Path, database: str = ":memory:") -> sqlite3.Connection:
    connection = sqlite3.connect(database)
    connection.row_factory = sqlite3.Row
    connection.executescript("""
        CREATE TABLE waves (id TEXT PRIMARY KEY, repo TEXT NOT NULL);
        CREATE TABLE projects (id TEXT PRIMARY KEY, wave_id TEXT NOT NULL);
        CREATE TABLE tasks (id TEXT PRIMARY KEY, project_id TEXT NOT NULL);
        CREATE TABLE task_prs (
            id TEXT PRIMARY KEY, task_id TEXT NOT NULL,
            created_at INTEGER, publication_requested_at INTEGER,
            merge_requested_at INTEGER, merged_at INTEGER, updated_at INTEGER,
            merge_tracking_complete INTEGER, repair_tracking_complete INTEGER,
            github_observation TEXT, merge_commit TEXT
        );
        CREATE TABLE task_pr_repair_incidents (
            task_pr_id TEXT, kind TEXT, occurred_at INTEGER
        );
    """)
    connection.execute("INSERT INTO waves VALUES (?, ?)", ("wave", str(repo)))
    connection.execute("INSERT INTO projects VALUES ('project', 'wave')")
    connection.execute("INSERT INTO tasks VALUES ('task', 'project')")
    return connection


def _pr(connection: sqlite3.Connection, identity: str, **overrides: object) -> None:
    values = dict(
        id=identity,
        task_id="task",
        created_at=SINCE + 10,
        publication_requested_at=SINCE + 100,
        merge_requested_at=SINCE + 200,
        merged_at=SINCE + 300,
        updated_at=SINCE + 310,
        merge_tracking_complete=1,
        repair_tracking_complete=1,
        github_observation='{"result":{"state":"fresh"}}',
        merge_commit="merge",
    )
    values.update(overrides)
    columns = ", ".join(values)
    placeholders = ", ".join("?" for _ in values)
    connection.execute(
        f"INSERT INTO task_prs ({columns}) VALUES ({placeholders})", tuple(values.values())
    )


def _run(repo: Path, **overrides: object) -> dict:
    run = dict(
        id="run",
        repo=str(repo),
        started=SINCE + 20,
        ended=SINCE + 80,
        task_pr_id=None,
        first_provider_attempt_at=None,
        harness="codex",
        evidence_gaps=0,
        usage=dict(
            streams=1,
            final_streams=1,
            gaps=0,
            total_input_tokens=100,
            output_tokens=10,
            cost_usd=None,
        ),
    )
    run.update(overrides)
    run["observed_at"] = run.pop("started")
    ended = run.pop("ended")
    run["recorded_at"] = ended
    run["recorded_outcome"] = "completed" if ended is not None else None
    return run


def test_measured_row_preserves_missing_and_explicit_zero() -> None:
    budget = {"unit": "count", "p50": None, "p95": None, "maximum": 0.0}
    missing = scorecard.measured_row("repair", "Repair", None, [], 1, budget, 20)
    zero = scorecard.measured_row("repair", "Repair", None, [0], 1, budget, 20)
    breach = scorecard.measured_row("repair", "Repair", None, [1], 1, budget, 20)
    assert (missing["measured"], missing["verdict"]) == (0, "unknown")
    assert (zero["measured"], zero["verdict"]) == (1, "collecting")
    assert (breach["measured"], breach["verdict"]) == (1, "fail")


def test_complete_samples_without_a_budget_do_not_claim_a_performance_pass() -> None:
    row = scorecard.measured_row(
        "duration",
        "Duration",
        None,
        [10] * 20,
        20,
        {"unit": "seconds", "p50": None, "p95": None, "maximum": None},
        20,
    )
    assert row["measured"] == 20
    assert row["verdict"] == "unbudgeted"


def test_run_window_includes_long_runs_and_excludes_foreign_unfinished_and_future(
    tmp_path: Path,
) -> None:
    path = tmp_path / "runs.json"
    runs = [
        _run(tmp_path, id="long", started=SINCE - 100),
        _run(tmp_path, id="boundary", started=SINCE - 60, ended=SINCE),
        _run(tmp_path, id="unfinished", ended=None),
        _run(tmp_path, id="old", ended=SINCE - 1),
        _run(tmp_path, id="future", ended=UNTIL + 1),
        _run(tmp_path / "other", id="foreign"),
    ]
    path.write_text(json.dumps(runs))
    selected = scorecard.load_history(path, tmp_path)
    assert [run["id"] for run in selected] == ["long", "boundary", "unfinished", "old", "future"]
    report = scorecard.build_report(
        _policy(), tmp_path, NOW.replace(microsecond=500000), selected, [], []
    )
    rows = {row["id"]: row for row in report["rows"]}
    assert rows["session_elapsed_seconds"]["eligible"] == 2
    assert rows["session_elapsed_seconds"]["p50"] == 60
    assert rows["session_elapsed_seconds"]["p95"] == 180


def test_recorded_attempt_joins_exact_pr_across_run_windows(tmp_path: Path) -> None:
    connection = _connection(tmp_path)
    _pr(connection, "first", created_at=SINCE - 100)
    _pr(connection, "second")
    _pr(connection, "prepared")
    _pr(connection, "historical")
    runs = [
        _run(tmp_path, task_pr_id="first", first_provider_attempt_at=SINCE - 60, ended=SINCE - 1),
        _run(tmp_path, task_pr_id="first", first_provider_attempt_at=SINCE + 20),
        _run(tmp_path, task_pr_id="second", first_provider_attempt_at=SINCE + 100, ended=None),
        _run(tmp_path, task_pr_id="prepared"),
        _run(tmp_path, first_provider_attempt_at=SINCE + 20),
    ]
    report = scorecard.build_report(
        _policy(),
        tmp_path,
        NOW,
        runs,
        [],
        scorecard.load_lifecycle(connection, tmp_path, SINCE, UNTIL),
    )
    row = next(row for row in report["rows"] if row["id"] == "recorded_attempt_to_merge_seconds")
    assert (row["eligible"], row["measured"], row["p50"], row["p95"]) == (4, 2, 200, 360)
    assert row["verdict"] == "unknown"
    assert "2 of 4" in row["reason"]
    assert "lower bound" in row["reason"]


def test_recorded_attempt_requires_ordered_boundaries_and_merge_evidence(tmp_path: Path) -> None:
    connection = _connection(tmp_path)
    _pr(connection, "before")
    _pr(connection, "after")
    _pr(connection, "missing", merged_at=None)
    _pr(connection, "partial", github_observation='{"result":{"state":"partial"}}')
    _pr(connection, "untracked", merge_tracking_complete=0)
    _pr(connection, "usage-gap")
    runs = [
        _run(tmp_path, task_pr_id="before", first_provider_attempt_at=SINCE),
        _run(tmp_path, task_pr_id="after", first_provider_attempt_at=SINCE + 400),
        *[
            _run(tmp_path, task_pr_id=pr, first_provider_attempt_at=SINCE + 20)
            for pr in ("missing", "partial", "untracked", "usage-gap")
        ],
    ]
    runs[-1]["usage"]["gaps"] = 1
    report = scorecard.build_report(
        _policy(),
        tmp_path,
        NOW,
        runs,
        [],
        scorecard.load_lifecycle(connection, tmp_path, SINCE, UNTIL),
    )
    row = next(row for row in report["rows"] if row["id"] == "recorded_attempt_to_merge_seconds")
    assert (row["eligible"], row["measured"], row["p50"]) == (6, 1, 280)


def test_usage_keeps_missing_nonfinal_and_gapped_receipts_unknown(tmp_path: Path) -> None:
    complete = _run(tmp_path)
    nonfinal = _run(tmp_path)
    nonfinal["usage"]["final_streams"] = 0
    usage_gap = _run(tmp_path)
    usage_gap["usage"]["gaps"] = 1
    recording_gap = _run(tmp_path, evidence_gaps=1)
    rows = {
        row["id"]: row
        for row in scorecard.usage_rows(
            _policy(),
            [complete, nonfinal, usage_gap, recording_gap],
            None,
        )
    }
    assert rows["session_total_input_tokens"]["eligible"] == 4
    assert rows["session_total_input_tokens"]["measured"] == 1
    assert rows["session_total_input_tokens"]["p50"] == 100
    assert rows["session_cost_usd"]["measured"] == 0
    assert rows["session_cost_usd"]["p50"] is None
    assert rows["session_cost_usd"]["verdict"] == "unknown"
    assert rows["session_elapsed_seconds"]["measured"] == 4


def test_current_pr_owners_measure_intervals_and_preserve_missing_coverage(tmp_path: Path) -> None:
    connection = _connection(tmp_path)
    _pr(connection, "complete")
    _pr(connection, "missing", merged_at=None)
    _pr(connection, "conflict", github_observation='{"result":{"state":"partial"}}')
    _pr(connection, "untracked", merge_tracking_complete=0, repair_tracking_complete=0)
    _pr(connection, "old", merged_at=SINCE - 1)
    _pr(connection, "future", merged_at=UNTIL + 1)
    _pr(connection, "open", merge_commit=None)
    connection.execute("INSERT INTO waves VALUES (?, ?)", ("other", str(tmp_path / "other")))
    connection.execute("INSERT INTO projects VALUES ('other', 'other')")
    connection.execute("INSERT INTO tasks VALUES ('other', 'other')")
    _pr(connection, "foreign", task_id="other")
    prs = scorecard.load_lifecycle(connection, tmp_path, SINCE, UNTIL)
    report = scorecard.build_report(_policy(), tmp_path, NOW, [], [], prs)
    rows = {row["id"]: row for row in report["rows"]}
    for metric, expected in [
        ("task_pr_to_merge_seconds", 290),
        ("publication_to_merge_seconds", 200),
        ("land_to_merge_seconds", 100),
    ]:
        row = rows[metric]
        assert (row["eligible"], row["measured"], row["p50"]) == (4, 1, expected)
        assert row["verdict"] == "unknown"
    assert rows["manual_git_repairs"]["measured"] == 1
    assert rows["manual_git_repairs"]["p50"] == 0
    assert rows["task_first_progress_seconds"]["verdict"] == "unknown"


def test_recorded_repair_breaches_even_when_other_coverage_is_missing(tmp_path: Path) -> None:
    connection = _connection(tmp_path)
    _pr(connection, "repair", repair_tracking_complete=0)
    _pr(connection, "missing", merged_at=None)
    connection.execute(
        "INSERT INTO task_pr_repair_incidents VALUES ('repair', 'manual_git_repair', ?)",
        (SINCE + 250,),
    )
    report = scorecard.build_report(
        _policy(),
        tmp_path,
        NOW,
        [],
        [],
        scorecard.load_lifecycle(connection, tmp_path, SINCE, UNTIL),
    )
    row = next(row for row in report["rows"] if row["id"] == "manual_git_repairs")
    assert (row["eligible"], row["measured"], row["p50"], row["verdict"]) == (2, 1, 1, "fail")


def test_generator_runs_with_current_tables_and_current_run_projection(
    tmp_path: Path, capsys
) -> None:
    policy_dir = tmp_path / "performance"
    policy_dir.mkdir()
    (policy_dir / "budgets.json").write_text(json.dumps(_policy()))
    database = tmp_path / "loopflow.db"
    connection = _connection(tmp_path, str(database))
    now = int(datetime.now(timezone.utc).timestamp())
    _pr(
        connection,
        "merged",
        created_at=now - 100,
        publication_requested_at=now - 40,
        merge_requested_at=now - 30,
        merged_at=now - 10,
    )
    connection.commit()
    connection.close()
    runs = tmp_path / "runs.json"
    runs.write_text(
        json.dumps(
            [
                _run(
                    tmp_path,
                    started=now - 60,
                    ended=now - 1,
                    task_pr_id="merged",
                    first_provider_attempt_at=now - 50,
                )
            ]
        )
    )
    assert (
        scorecard.main(
            [
                "--repo",
                str(tmp_path),
                "--database",
                str(database),
                "--history",
                str(runs),
                "--envelope",
            ]
        )
        == 0
    )
    envelope = json.loads(capsys.readouterr().out)
    rows = {row["id"]: row for row in envelope["report"]["rows"]}
    assert rows["session_elapsed_seconds"]["p50"] == 59
    assert rows["session_total_input_tokens"]["p50"] == 100
    attempt = rows["recorded_attempt_to_merge_seconds"]
    assert (attempt["eligible"], attempt["measured"], attempt["p50"]) == (1, 1, 40)
    assert "Observed lower bound" in envelope["text"]
    assert envelope["metric_observations"][0]["kind"] == "unavailable"
    assert "Task-loop intervals" in envelope["metric_observations"][0]["reason"]
