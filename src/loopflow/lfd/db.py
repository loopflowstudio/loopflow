"""SQLite database for lfd state.

Schema changes: All schema changes must go through migrations in lfd/migrations/.
Never ALTER tables here. See migrations/README.md.
"""

import json
import sqlite3
import sys
import uuid
from datetime import datetime
from pathlib import Path

from loopflow.lfd.migrations.registry import MIGRATIONS
from loopflow.lfd.models import (
    Loop,
    MergeMode,
    Run,
    RunStatus,
    Schedule,
    Session,
    SessionStatus,
    Subscription,
    Trigger,
    TriggerStatus,
)
from loopflow.lfd.process import is_process_running

DB_PATH = Path.home() / ".lf" / "lfd.db"


def _init_db(db_path: Path) -> None:
    """Initialize lfd.db with schema."""
    db_path.parent.mkdir(parents=True, exist_ok=True)

    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA journal_mode=WAL")
    _run_migrations(conn)
    conn.close()


def _get_db(db_path: Path | None = None) -> sqlite3.Connection:
    """Get database connection, auto-resetting on schema mismatch."""
    if db_path is None:
        db_path = DB_PATH

    if not db_path.exists():
        _init_db(db_path)

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    _run_migrations(conn)
    return conn


def _run_migrations(conn: sqlite3.Connection) -> None:
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TEXT NOT NULL
        )
        """
    )
    applied = {row[0] for row in conn.execute("SELECT version FROM schema_migrations").fetchall()}
    for migration in MIGRATIONS:
        if migration.version in applied:
            continue
        migration.apply(conn)
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?, ?)",
            (migration.version, datetime.now().isoformat()),
        )
        conn.commit()


# Process status checks


def update_dead_processes(db_path: Path | None = None) -> int:
    """Mark triggers as idle if their process is no longer running."""
    conn = _get_db(db_path)
    count = 0

    for table in ["loops", "subscriptions", "schedules"]:
        cursor = conn.execute(
            f"SELECT id, pid FROM {table} WHERE status = 'running' AND pid IS NOT NULL"
        )
        for row in cursor.fetchall():
            if not is_process_running(row["pid"]):
                conn.execute(
                    f"UPDATE {table} SET status = 'idle', pid = NULL WHERE id = ?",
                    (row["id"],),
                )
                count += 1

    conn.commit()
    conn.close()
    return count


# Sessions


def save_session(session: Session, db_path: Path | None = None) -> None:
    """Save a session."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO sessions
        (id, task, repo, worktree, status, started_at, ended_at, pid, model, run_mode)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            session.id,
            session.step,  # DB column is 'task' for backward compat
            session.repo,
            session.worktree,
            session.status.value,
            session.started_at.isoformat(),
            session.ended_at.isoformat() if session.ended_at else None,
            session.pid,
            session.model,
            session.run_mode,
        ),
    )

    conn.commit()
    conn.close()


def load_sessions(
    repo: str | None = None,
    active_only: bool = False,
    db_path: Path | None = None,
) -> list[Session]:
    """Load sessions, optionally filtered by repo."""
    conn = _get_db(db_path)

    conditions = []
    params: list = []

    if repo:
        conditions.append("repo = ?")
        params.append(repo)

    if active_only:
        conditions.append("status IN ('running', 'waiting')")

    where = f" WHERE {' AND '.join(conditions)}" if conditions else ""
    cursor = conn.execute(f"SELECT * FROM sessions{where} ORDER BY started_at DESC", params)

    sessions = [_session_from_row(dict(row)) for row in cursor]
    conn.close()
    return sessions


def load_sessions_for_worktree(
    worktree: str, limit: int = 20, db_path: Path | None = None
) -> list[Session]:
    """Load recent sessions for a worktree path."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM sessions WHERE worktree = ? ORDER BY started_at DESC LIMIT ?",
        (worktree, limit),
    )

    sessions = [_session_from_row(dict(row)) for row in cursor]
    conn.close()
    return sessions


def load_sessions_for_repo(
    repo: str, limit: int = 50, db_path: Path | None = None
) -> list[Session]:
    """Load recent sessions across all worktrees in a repo."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM sessions WHERE repo = ? ORDER BY started_at DESC LIMIT ?",
        (repo, limit),
    )

    sessions = [_session_from_row(dict(row)) for row in cursor]
    conn.close()
    return sessions


def update_session_status(
    session_id: str, status: SessionStatus, db_path: Path | None = None
) -> bool:
    """Update session status."""
    conn = _get_db(db_path)

    ended_at = None
    if status in (SessionStatus.COMPLETED, SessionStatus.ERROR):
        ended_at = datetime.now().isoformat()

    cursor = conn.execute(
        "UPDATE sessions SET status = ?, ended_at = COALESCE(?, ended_at) WHERE id = ?",
        (status.value, ended_at, session_id),
    )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_session(session_id: str, db_path: Path | None = None) -> bool:
    """Delete a session from database."""
    conn = _get_db(db_path)

    cursor = conn.execute("DELETE FROM sessions WHERE id = ?", (session_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _session_from_row(row: dict) -> Session:
    """Convert database row to Session."""
    return Session(
        id=row["id"],
        step=row["task"],  # DB column is 'task' for backward compat
        repo=row["repo"],
        worktree=row["worktree"],
        status=SessionStatus(row["status"]),
        started_at=datetime.fromisoformat(row["started_at"]),
        ended_at=datetime.fromisoformat(row["ended_at"]) if row.get("ended_at") else None,
        pid=row.get("pid"),
        model=row.get("model", "claude-code"),
        run_mode=row.get("run_mode", "auto"),
    )


# Runs


def save_run(run: Run, db_path: Path | None = None) -> None:
    """Save or update a run."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO runs
        (id, parent, flow, area, repo, goals, status, iteration,
         worktree, branch, current_step, error, pr_url,
         started_at, ended_at, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            run.id,
            run.parent,
            run.flow,
            run.area,
            str(run.repo),
            json.dumps(run.goals) if run.goals else None,
            run.status.value,
            run.iteration,
            run.worktree,
            run.branch,
            run.current_step,
            run.error,
            run.pr_url,
            run.started_at.isoformat() if run.started_at else None,
            run.ended_at.isoformat() if run.ended_at else None,
            run.created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def get_run(run_id: str, db_path: Path | None = None) -> Run | None:
    """Get a run by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM runs WHERE id = ?", (run_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM runs WHERE id LIKE ?", (f"{run_id}%",))
        row = cursor.fetchone()

    conn.close()
    return _run_from_row(dict(row)) if row else None


def list_runs(
    repo: Path | None = None,
    parent: str | None = None,
    status: RunStatus | None = None,
    limit: int = 50,
    db_path: Path | None = None,
) -> list[Run]:
    """List runs with optional filters."""
    conn = _get_db(db_path)

    conditions = []
    params: list = []

    if repo:
        conditions.append("repo = ?")
        params.append(str(repo))

    if parent:
        conditions.append("parent = ?")
        params.append(parent)

    if status:
        conditions.append("status = ?")
        params.append(status.value)

    where = f" WHERE {' AND '.join(conditions)}" if conditions else ""
    params.append(limit)

    cursor = conn.execute(f"SELECT * FROM runs{where} ORDER BY created_at DESC LIMIT ?", params)

    runs = [_run_from_row(dict(row)) for row in cursor]
    conn.close()
    return runs


def list_runs_for_trigger(
    trigger_type: str,
    trigger_id: str,
    limit: int = 10,
    db_path: Path | None = None,
) -> list[Run]:
    """List runs spawned by a specific trigger."""
    parent = f"{trigger_type}:{trigger_id}"
    return list_runs(parent=parent, limit=limit, db_path=db_path)


def get_latest_run_for_trigger(
    trigger_type: str, trigger_id: str, db_path: Path | None = None
) -> Run | None:
    """Get the most recent run for a trigger."""
    runs = list_runs_for_trigger(trigger_type, trigger_id, limit=1, db_path=db_path)
    return runs[0] if runs else None


# Alias for backwards compatibility
def update_dead_runs(db_path: Path | None = None) -> int:
    """Mark triggers as idle if their process is no longer running."""
    return update_dead_processes(db_path)


def update_run_status(
    run_id: str,
    status: RunStatus,
    error: str | None = None,
    db_path: Path | None = None,
) -> bool:
    """Update a run's status."""
    conn = _get_db(db_path)

    ended_at = None
    if status in (RunStatus.COMPLETED, RunStatus.FAILED, RunStatus.CANCELLED):
        ended_at = datetime.now().isoformat()

    if error:
        cursor = conn.execute(
            "UPDATE runs SET status = ?, ended_at = ?, error = ? WHERE id = ? OR id LIKE ?",
            (status.value, ended_at, error, run_id, f"{run_id}%"),
        )
    else:
        cursor = conn.execute(
            "UPDATE runs SET status = ?, ended_at = COALESCE(?, ended_at) "
            "WHERE id = ? OR id LIKE ?",
            (status.value, ended_at, run_id, f"{run_id}%"),
        )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_run_step(run_id: str, step: str | None, db_path: Path | None = None) -> bool:
    """Update the current step for a run."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE runs SET current_step = ? WHERE id = ? OR id LIKE ?",
        (step, run_id, f"{run_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_run_pr(run_id: str, pr_url: str, db_path: Path | None = None) -> bool:
    """Update the PR URL for a run."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE runs SET pr_url = ? WHERE id = ? OR id LIKE ?",
        (pr_url, run_id, f"{run_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_run(run_id: str, db_path: Path | None = None) -> bool:
    """Delete a run."""
    conn = _get_db(db_path)

    cursor = conn.execute("DELETE FROM runs WHERE id = ? OR id LIKE ?", (run_id, f"{run_id}%"))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _run_from_row(row: dict) -> Run:
    """Convert database row to Run."""
    goals_str = row.get("goals")
    goals = json.loads(goals_str) if goals_str else []

    return Run(
        id=row["id"],
        parent=row.get("parent"),
        flow=row["flow"],
        area=row["area"],
        repo=Path(row["repo"]),
        goals=goals,
        status=RunStatus(row["status"]),
        iteration=row.get("iteration", 0),
        worktree=row.get("worktree"),
        branch=row.get("branch"),
        current_step=row.get("current_step"),
        error=row.get("error"),
        pr_url=row.get("pr_url"),
        started_at=datetime.fromisoformat(row["started_at"]) if row.get("started_at") else None,
        ended_at=datetime.fromisoformat(row["ended_at"]) if row.get("ended_at") else None,
        created_at=datetime.fromisoformat(row["created_at"]),
    )


# Loops


def save_loop(loop: Loop, db_path: Path | None = None) -> None:
    """Save or update a loop."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO loops
        (id, flow, area, repo, goals, status, iteration,
         main_branch, pr_limit, merge_mode, pid, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            loop.id,
            loop.flow,
            loop.area,
            str(loop.repo),
            json.dumps(loop.goals) if loop.goals else None,
            loop.status.value,
            loop.iteration,
            loop.main_branch,
            loop.pr_limit,
            loop.merge_mode.value,
            loop.pid,
            loop.created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def get_loop(loop_id: str, db_path: Path | None = None) -> Loop | None:
    """Get a loop by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM loops WHERE id = ?", (loop_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM loops WHERE id LIKE ?", (f"{loop_id}%",))
        row = cursor.fetchone()

    conn.close()
    return _loop_from_row(dict(row)) if row else None


def get_loop_by_area_repo(area: str, repo: Path, db_path: Path | None = None) -> Loop | None:
    """Get a loop by area and repo."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM loops WHERE area = ? AND repo = ?",
        (area, str(repo)),
    )
    row = cursor.fetchone()
    conn.close()
    return _loop_from_row(dict(row)) if row else None


def list_loops(repo: Path | None = None, db_path: Path | None = None) -> list[Loop]:
    """List all loops, optionally filtered by repo."""
    conn = _get_db(db_path)

    if repo:
        cursor = conn.execute(
            "SELECT * FROM loops WHERE repo = ? ORDER BY created_at DESC",
            (str(repo),),
        )
    else:
        cursor = conn.execute("SELECT * FROM loops ORDER BY created_at DESC")

    loops = [_loop_from_row(dict(row)) for row in cursor]
    conn.close()
    return loops


def update_loop_status(loop_id: str, status: TriggerStatus, db_path: Path | None = None) -> bool:
    """Update a loop's status."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE loops SET status = ? WHERE id = ? OR id LIKE ?",
        (status.value, loop_id, f"{loop_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_loop_iteration(loop_id: str, iteration: int, db_path: Path | None = None) -> bool:
    """Update a loop's iteration count."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE loops SET iteration = ? WHERE id = ? OR id LIKE ?",
        (iteration, loop_id, f"{loop_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_loop_pid(loop_id: str, pid: int | None, db_path: Path | None = None) -> bool:
    """Update a loop's process ID."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE loops SET pid = ? WHERE id = ? OR id LIKE ?",
        (pid, loop_id, f"{loop_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_loop(loop_id: str, db_path: Path | None = None) -> bool:
    """Delete a loop and its runs."""
    conn = _get_db(db_path)

    # Get full ID
    cursor = conn.execute(
        "SELECT id FROM loops WHERE id = ? OR id LIKE ?", (loop_id, f"{loop_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return False

    full_id = row["id"]

    # Delete runs first
    conn.execute("DELETE FROM runs WHERE parent = ?", (f"loop:{full_id}",))
    cursor = conn.execute("DELETE FROM loops WHERE id = ?", (full_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _loop_from_row(row: dict) -> Loop:
    """Convert database row to Loop."""
    goals_str = row.get("goals")
    goals = json.loads(goals_str) if goals_str else []

    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    return Loop(
        id=row["id"],
        flow=row["flow"],
        area=row["area"],
        repo=Path(row["repo"]),
        goals=goals,
        status=TriggerStatus(row["status"]),
        iteration=row.get("iteration", 0),
        main_branch=row.get("main_branch", ""),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(merge_mode_str),
        pid=row.get("pid"),
        created_at=datetime.fromisoformat(row["created_at"]),
    )


# Subscriptions


def save_subscription(sub: Subscription, db_path: Path | None = None) -> None:
    """Save or update a subscription."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO subscriptions
        (id, flow, area, repo, goals, pathset, last_main_sha,
         status, iteration, main_branch, pr_limit, merge_mode, pid, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            sub.id,
            sub.flow,
            sub.area,
            str(sub.repo),
            json.dumps(sub.goals) if sub.goals else None,
            sub.pathset,
            sub.last_main_sha,
            sub.status.value,
            sub.iteration,
            sub.main_branch,
            sub.pr_limit,
            sub.merge_mode.value,
            sub.pid,
            sub.created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def get_subscription(sub_id: str, db_path: Path | None = None) -> Subscription | None:
    """Get a subscription by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM subscriptions WHERE id = ?", (sub_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM subscriptions WHERE id LIKE ?", (f"{sub_id}%",))
        row = cursor.fetchone()

    conn.close()
    return _subscription_from_row(dict(row)) if row else None


def get_subscription_by_area_repo(
    area: str, repo: Path, db_path: Path | None = None
) -> Subscription | None:
    """Get a subscription by area and repo."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM subscriptions WHERE area = ? AND repo = ?",
        (area, str(repo)),
    )
    row = cursor.fetchone()
    conn.close()
    return _subscription_from_row(dict(row)) if row else None


def list_subscriptions(repo: Path | None = None, db_path: Path | None = None) -> list[Subscription]:
    """List all subscriptions, optionally filtered by repo."""
    conn = _get_db(db_path)

    if repo:
        cursor = conn.execute(
            "SELECT * FROM subscriptions WHERE repo = ? ORDER BY created_at DESC",
            (str(repo),),
        )
    else:
        cursor = conn.execute("SELECT * FROM subscriptions ORDER BY created_at DESC")

    subs = [_subscription_from_row(dict(row)) for row in cursor]
    conn.close()
    return subs


def update_subscription_status(
    sub_id: str, status: TriggerStatus, db_path: Path | None = None
) -> bool:
    """Update a subscription's status."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE subscriptions SET status = ? WHERE id = ? OR id LIKE ?",
        (status.value, sub_id, f"{sub_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_subscription_sha(sub_id: str, sha: str | None, db_path: Path | None = None) -> bool:
    """Update a subscription's last_main_sha."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE subscriptions SET last_main_sha = ? WHERE id = ? OR id LIKE ?",
        (sha, sub_id, f"{sub_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_subscription(sub_id: str, db_path: Path | None = None) -> bool:
    """Delete a subscription and its runs."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT id FROM subscriptions WHERE id = ? OR id LIKE ?", (sub_id, f"{sub_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return False

    full_id = row["id"]

    conn.execute("DELETE FROM runs WHERE parent = ?", (f"subscription:{full_id}",))
    cursor = conn.execute("DELETE FROM subscriptions WHERE id = ?", (full_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _subscription_from_row(row: dict) -> Subscription:
    """Convert database row to Subscription."""
    goals_str = row.get("goals")
    goals = json.loads(goals_str) if goals_str else []

    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    return Subscription(
        id=row["id"],
        flow=row["flow"],
        area=row["area"],
        repo=Path(row["repo"]),
        goals=goals,
        pathset=row.get("pathset", ""),
        last_main_sha=row.get("last_main_sha"),
        status=TriggerStatus(row["status"]),
        iteration=row.get("iteration", 0),
        main_branch=row.get("main_branch", ""),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(merge_mode_str),
        pid=row.get("pid"),
        created_at=datetime.fromisoformat(row["created_at"]),
    )


# Schedules


def save_schedule(schedule: Schedule, db_path: Path | None = None) -> None:
    """Save or update a schedule."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO schedules
        (id, flow, area, repo, goals, cron,
         status, iteration, main_branch, pr_limit, merge_mode, pid, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            schedule.id,
            schedule.flow,
            schedule.area,
            str(schedule.repo),
            json.dumps(schedule.goals) if schedule.goals else None,
            schedule.cron,
            schedule.status.value,
            schedule.iteration,
            schedule.main_branch,
            schedule.pr_limit,
            schedule.merge_mode.value,
            schedule.pid,
            schedule.created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def get_schedule(schedule_id: str, db_path: Path | None = None) -> Schedule | None:
    """Get a schedule by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM schedules WHERE id = ?", (schedule_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM schedules WHERE id LIKE ?", (f"{schedule_id}%",))
        row = cursor.fetchone()

    conn.close()
    return _schedule_from_row(dict(row)) if row else None


def get_schedule_by_area_repo(
    area: str, repo: Path, db_path: Path | None = None
) -> Schedule | None:
    """Get a schedule by area and repo."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM schedules WHERE area = ? AND repo = ?",
        (area, str(repo)),
    )
    row = cursor.fetchone()
    conn.close()
    return _schedule_from_row(dict(row)) if row else None


def list_schedules(repo: Path | None = None, db_path: Path | None = None) -> list[Schedule]:
    """List all schedules, optionally filtered by repo."""
    conn = _get_db(db_path)

    if repo:
        cursor = conn.execute(
            "SELECT * FROM schedules WHERE repo = ? ORDER BY created_at DESC",
            (str(repo),),
        )
    else:
        cursor = conn.execute("SELECT * FROM schedules ORDER BY created_at DESC")

    schedules = [_schedule_from_row(dict(row)) for row in cursor]
    conn.close()
    return schedules


def update_schedule_status(
    schedule_id: str, status: TriggerStatus, db_path: Path | None = None
) -> bool:
    """Update a schedule's status."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE schedules SET status = ? WHERE id = ? OR id LIKE ?",
        (status.value, schedule_id, f"{schedule_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_schedule(schedule_id: str, db_path: Path | None = None) -> bool:
    """Delete a schedule and its runs."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT id FROM schedules WHERE id = ? OR id LIKE ?", (schedule_id, f"{schedule_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return False

    full_id = row["id"]

    conn.execute("DELETE FROM runs WHERE parent = ?", (f"schedule:{full_id}",))
    cursor = conn.execute("DELETE FROM schedules WHERE id = ?", (full_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _schedule_from_row(row: dict) -> Schedule:
    """Convert database row to Schedule."""
    goals_str = row.get("goals")
    goals = json.loads(goals_str) if goals_str else []

    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    return Schedule(
        id=row["id"],
        flow=row["flow"],
        area=row["area"],
        repo=Path(row["repo"]),
        goals=goals,
        cron=row.get("cron", ""),
        status=TriggerStatus(row["status"]),
        iteration=row.get("iteration", 0),
        main_branch=row.get("main_branch", ""),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(merge_mode_str),
        pid=row.get("pid"),
        created_at=datetime.fromisoformat(row["created_at"]),
    )


# Convenience functions for all triggers


def list_all_triggers(repo: Path | None = None, db_path: Path | None = None) -> list[Trigger]:
    """List all triggers (loops, subscriptions, schedules) for a repo."""
    triggers: list[Trigger] = []
    triggers.extend(list_loops(repo, db_path))
    triggers.extend(list_subscriptions(repo, db_path))
    triggers.extend(list_schedules(repo, db_path))
    return triggers


def get_trigger(trigger_type: str, trigger_id: str, db_path: Path | None = None) -> Trigger | None:
    """Get a trigger by type and ID."""
    if trigger_type == "loop":
        return get_loop(trigger_id, db_path)
    elif trigger_type == "subscription":
        return get_subscription(trigger_id, db_path)
    elif trigger_type == "schedule":
        return get_schedule(trigger_id, db_path)
    return None


# Summary functions


def save_summary_db(
    repo: str,
    path: str,
    token_budget: int,
    source_hash: str,
    content: str,
    model: str,
    db_path: Path | None = None,
) -> None:
    """Save a summary to the database."""
    conn = _get_db(db_path)
    conn.execute(
        """
        INSERT OR REPLACE INTO summaries
        (id, repo, path, token_budget, source_hash, content, model, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            str(uuid.uuid4()),
            repo,
            path,
            token_budget,
            source_hash,
            content,
            model,
            datetime.now().isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def load_summary_db(
    repo: str,
    path: str,
    token_budget: int,
    db_path: Path | None = None,
) -> dict | None:
    """Load a summary from the database.

    Returns dict with keys: content, source_hash, model, created_at
    """
    conn = _get_db(db_path)
    cursor = conn.execute(
        "SELECT content, source_hash, model, created_at FROM summaries "
        "WHERE repo = ? AND path = ? AND token_budget = ?",
        (repo, path, token_budget),
    )
    row = cursor.fetchone()
    conn.close()

    if not row:
        return None

    return {
        "content": row["content"],
        "source_hash": row["source_hash"],
        "model": row["model"],
        "created_at": row["created_at"],
    }


def delete_summaries_for_repo(repo: str, db_path: Path | None = None) -> int:
    """Delete all summaries for a repo."""
    conn = _get_db(db_path)
    cursor = conn.execute("DELETE FROM summaries WHERE repo = ?", (repo,))
    conn.commit()
    count = cursor.rowcount
    conn.close()
    return count
