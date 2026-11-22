from __future__ import annotations

import importlib.util
import sqlite3
import sys
from contextlib import closing
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[2] / "scripts/benchmarks/desktop-performance"
sys.path.insert(0, str(SCRIPTS))
spec = importlib.util.spec_from_file_location("launch", SCRIPTS / "launch.py")
launch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(launch)


def _rows(path: Path) -> list[tuple]:
    with closing(sqlite3.connect(path)) as db:
        return db.execute("select title from tasks order by title").fetchall()


def test_launch_report_keeps_observed_timings_without_unvalidated_budgets() -> None:
    report = launch.summarize(
        [
            {
                "scenario": "saved",
                "pre_main": 100,
                "restored": 150,
                "first_frame": 200,
                "usable": 250,
                "fresh": None,
                "rss_mb": 80,
                "cpu_s": 0.5,
                "reads": [],
                "reached": False,
                "cache": "saved",
                "usable_source": "saved",
                "lf_calls": {},
                "lf_refused": {},
                "saved_workspace_kept": True,
            }
        ],
        {"source": "test-build", "lf": "test-lf", "host": "test-host", "database_mb": 1},
    )

    rendered = launch.render(report)

    assert "| usable ms | 1 | 250 | — | 250 |" in rendered
    assert "1 launches, 0 reached the endpoint" in rendered
    assert "Milliseconds from kernel process start; p95 needs 20 samples." in rendered
    assert "budgets_ms" not in report
    assert "budget" not in rendered.lower()


def test_snapshot_copies_a_live_home_including_its_write_ahead_log(tmp_path: Path) -> None:
    source = tmp_path / "live home"
    source.mkdir()
    (source / "config.yaml").write_text("team: LOO\n", encoding="utf-8")
    # Held open so the second row stays in the write-ahead log, as in a running Machine.
    with closing(sqlite3.connect(source / "loopflow.db")) as live:
        live.execute("pragma journal_mode=wal")
        live.execute("create table tasks (title text)")
        live.execute("insert into tasks values ('checkpointed')")
        live.commit()
        live.execute("pragma wal_checkpoint(truncate)")
        live.execute("insert into tasks values ('in the log')")
        live.commit()
        before = (source / "loopflow.db").read_bytes()

        launch.snapshot(tmp_path / "copy", source)

        assert (source / "loopflow.db").read_bytes() == before
    assert _rows(tmp_path / "copy/loopflow.db") == [("checkpointed",), ("in the log",)]
    assert (tmp_path / "copy/config.yaml").read_text(encoding="utf-8") == "team: LOO\n"


def test_failed_snapshot_leaves_nothing_a_later_run_would_trust(tmp_path: Path) -> None:
    source, home = tmp_path / "source", tmp_path / "copy"
    source.mkdir()
    (source / "loopflow.db").write_text("not a database, long enough to be read as a header" * 4)

    with pytest.raises(SystemExit, match="could not copy"):
        launch.snapshot(home, source)
    assert not list(home.iterdir())

    (source / "loopflow.db").unlink()
    with closing(sqlite3.connect(source / "loopflow.db")) as db:
        db.execute("create table tasks (title text)")
        db.execute("insert into tasks values ('repaired')")
        db.commit()
    launch.snapshot(home, source)
    assert _rows(home / "loopflow.db") == [("repaired",)]
