"""Session entity persistence.

Sessions track interactive lf runs (not daemon-spawned runs).
Legacy from before the Run model - kept for backwards compatibility.
"""

from datetime import datetime
from pathlib import Path

from loopflow.lfd.db import _get_db
from loopflow.lfd.models import Session, SessionStatus


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
