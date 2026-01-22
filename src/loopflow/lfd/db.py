"""SQLite database for lfd state."""

import json
import sqlite3
import uuid
from datetime import datetime
from pathlib import Path

from loopflow.lfd.migrations.registry import MIGRATIONS
from loopflow.lfd.models import (
    Loop,
    LoopRun,
    LoopStatus,
    LoopType,
    MergeMode,
    Session,
    SessionStatus,
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
    """Get database connection."""
    if db_path is None:
        db_path = DB_PATH

    if not db_path.exists():
        _init_db(db_path)

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    _run_migrations(conn)
    _ensure_loop_columns(conn)
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


def _ensure_loop_columns(conn: sqlite3.Connection) -> None:
    """Add missing columns to loops table for forward-compat."""
    # Check if loops table exists first
    cursor = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name='loops'"
    )
    if not cursor.fetchone():
        return  # Table doesn't exist yet; migrations will create it

    cursor = conn.execute("PRAGMA table_info(loops)")
    columns = {row["name"] for row in cursor.fetchall()}

    if "flow" not in columns:
        conn.execute("ALTER TABLE loops ADD COLUMN flow TEXT")
        conn.commit()


# Process status checks


def update_dead_runs(db_path: Path | None = None) -> int:
    """Mark loops as idle if their process is no longer running."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT id, pid FROM loops WHERE status = 'running' AND pid IS NOT NULL")

    count = 0
    for row in cursor.fetchall():
        if not is_process_running(row["pid"]):
            conn.execute(
                "UPDATE loops SET status = 'idle', pid = NULL WHERE id = ?",
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


# Loop functions


def save_loop(loop: Loop, db_path: Path | None = None) -> None:
    """Save or update a loop."""
    conn = _get_db(db_path)
    conn.execute(
        """
        INSERT OR REPLACE INTO loops
        (id, type, area, repo, loop_main, goals, flow, status, iteration, pr_limit, merge_mode,
         project_file, pathset, cron, goal, pid, last_main_sha, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            loop.id,
            loop.type.value,
            loop.area,
            str(loop.repo),
            loop.loop_main,
            json.dumps(loop.goals) if loop.goals else None,
            loop.flow,
            loop.status.value,
            loop.iteration,
            loop.pr_limit,
            loop.merge_mode.value,
            loop.project_file,
            loop.pathset,
            loop.cron,
            loop.goal_name,  # Legacy field
            loop.pid,
            loop.last_main_sha,
            loop.created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def get_loop(loop_id: str, db_path: Path | None = None) -> Loop | None:
    """Get a loop by ID (supports short IDs)."""
    conn = _get_db(db_path)

    # Try exact match first, then prefix match
    cursor = conn.execute("SELECT * FROM loops WHERE id = ?", (loop_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM loops WHERE id LIKE ?", (f"{loop_id}%",))
        row = cursor.fetchone()

    conn.close()
    return _loop_from_row(dict(row)) if row else None


def get_loop_by_area_repo(
    loop_type: LoopType,
    area: str,
    repo: Path,
    *,
    db_path: Path | None = None,
) -> Loop | None:
    """Get a loop by type, area, and repo."""
    conn = _get_db(db_path)
    cursor = conn.execute(
        "SELECT * FROM loops WHERE type = ? AND area = ? AND repo = ?",
        (loop_type.value, area, str(repo)),
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


def update_loop_status(loop_id: str, status: LoopStatus, db_path: Path | None = None) -> bool:
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


def update_loop_last_sha(loop_id: str, sha: str | None, db_path: Path | None = None) -> bool:
    """Update a loop's last_main_sha (for subscribe loops)."""
    conn = _get_db(db_path)
    cursor = conn.execute(
        "UPDATE loops SET last_main_sha = ? WHERE id = ? OR id LIKE ?",
        (sha, loop_id, f"{loop_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_loop(loop_id: str, db_path: Path | None = None) -> bool:
    """Delete a loop and its runs."""
    conn = _get_db(db_path)

    # Get full ID for foreign key deletes
    cursor = conn.execute(
        "SELECT id FROM loops WHERE id = ? OR id LIKE ?", (loop_id, f"{loop_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return False

    full_id = row["id"]

    # Delete runs first
    conn.execute("DELETE FROM loop_runs WHERE loop_id = ?", (full_id,))
    cursor = conn.execute("DELETE FROM loops WHERE id = ?", (full_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _loop_from_row(row: dict) -> Loop:
    """Convert database row to Loop."""
    # Handle legacy "auto" merge mode by mapping to PR
    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    # Parse goals JSON
    goals_str = row.get("goals")
    goals = json.loads(goals_str) if goals_str else []

    # Handle area - could be in 'area' column or legacy 'goal' was used as area marker
    area = row.get("area") or "."

    return Loop(
        id=row["id"],
        type=LoopType(row["type"]),
        area=area,
        repo=Path(row["repo"]),
        loop_main=row["loop_main"],
        goals=goals,
        flow=row.get("flow"),
        status=LoopStatus(row["status"]),
        iteration=row.get("iteration", 0),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(merge_mode_str),
        project_file=row.get("project_file"),
        pathset=row.get("pathset"),
        cron=row.get("cron"),
        goal_name=row.get("goal"),  # Legacy field
        pid=row.get("pid"),
        last_main_sha=row.get("last_main_sha"),
        created_at=datetime.fromisoformat(row["created_at"]),
    )


# Loop run functions


def save_loop_run(run: LoopRun, db_path: Path | None = None) -> None:
    """Save a loop run."""
    conn = _get_db(db_path)
    conn.execute(
        """
        INSERT OR REPLACE INTO loop_runs
        (id, loop_id, iteration, status, started_at, ended_at,
         worktree, current_step, error, pr_url)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            run.id,
            run.loop_id,
            run.iteration,
            run.status.value,
            run.started_at.isoformat(),
            run.ended_at.isoformat() if run.ended_at else None,
            run.worktree,
            run.current_step,
            run.error,
            run.pr_url,
        ),
    )
    conn.commit()
    conn.close()


def get_loop_runs(loop_id: str, limit: int = 10, db_path: Path | None = None) -> list[LoopRun]:
    """Get runs for a loop."""
    conn = _get_db(db_path)

    # Support short IDs
    cursor = conn.execute(
        "SELECT id FROM loops WHERE id = ? OR id LIKE ?", (loop_id, f"{loop_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return []

    full_id = row["id"]

    cursor = conn.execute(
        "SELECT * FROM loop_runs WHERE loop_id = ? ORDER BY started_at DESC LIMIT ?",
        (full_id, limit),
    )
    runs = [_loop_run_from_row(dict(row)) for row in cursor]
    conn.close()
    return runs


def get_latest_loop_run(loop_id: str, db_path: Path | None = None) -> LoopRun | None:
    """Get the most recent run for a loop."""
    runs = get_loop_runs(loop_id, limit=1, db_path=db_path)
    return runs[0] if runs else None


def update_loop_run_status(
    run_id: str,
    status: LoopStatus,
    error: str | None = None,
    db_path: Path | None = None,
) -> bool:
    """Update a loop run's status."""
    conn = _get_db(db_path)

    ended_at = None
    if status in (LoopStatus.IDLE, LoopStatus.ERROR):
        ended_at = datetime.now().isoformat()

    if error:
        cursor = conn.execute(
            "UPDATE loop_runs SET status = ?, ended_at = ?, error = ? WHERE id = ?",
            (status.value, ended_at, error, run_id),
        )
    else:
        cursor = conn.execute(
            "UPDATE loop_runs SET status = ?, ended_at = COALESCE(?, ended_at) WHERE id = ?",
            (status.value, ended_at, run_id),
        )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_loop_run_step(run_id: str, step: str | None, db_path: Path | None = None) -> bool:
    """Update the current step for a loop run."""
    conn = _get_db(db_path)
    cursor = conn.execute(
        "UPDATE loop_runs SET current_step = ? WHERE id = ?",
        (step, run_id),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_loop_run_pr(run_id: str, pr_url: str, db_path: Path | None = None) -> bool:
    """Update the PR URL for a loop run."""
    conn = _get_db(db_path)
    cursor = conn.execute(
        "UPDATE loop_runs SET pr_url = ? WHERE id = ?",
        (pr_url, run_id),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def _loop_run_from_row(row: dict) -> LoopRun:
    """Convert database row to LoopRun."""
    return LoopRun(
        id=row["id"],
        loop_id=row["loop_id"],
        iteration=row["iteration"],
        status=LoopStatus(row["status"]),
        started_at=datetime.fromisoformat(row["started_at"]),
        ended_at=datetime.fromisoformat(row["ended_at"]) if row.get("ended_at") else None,
        worktree=row.get("worktree"),
        current_step=row.get("current_step"),
        error=row.get("error"),
        pr_url=row.get("pr_url"),
    )


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
