"""SQLite database for maestro state."""

import sqlite3
from datetime import datetime
from pathlib import Path
from typing import Optional

from loopflow.maestro.session import Session, SessionStatus

DEFAULT_DB_PATH = Path.home() / ".lf" / "maestro.db"


def init_db(db_path: Path) -> None:
    """Initialize maestro.db with schema."""
    db_path.parent.mkdir(parents=True, exist_ok=True)

    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA journal_mode=WAL")

    conn.executescript("""
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            task TEXT NOT NULL,
            repo TEXT NOT NULL,
            worktree TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            pid INTEGER,
            model TEXT NOT NULL,
            run_mode TEXT NOT NULL DEFAULT 'auto'
        );
    """)

    conn.commit()

    # Migrate older schemas
    cursor = conn.execute("PRAGMA table_info(sessions)")
    columns = {row[1] for row in cursor.fetchall()}
    if "run_mode" not in columns:
        conn.execute("ALTER TABLE sessions ADD COLUMN run_mode TEXT NOT NULL DEFAULT 'auto'")

    conn.close()


def get_db(db_path: Path) -> sqlite3.Connection:
    """Get database connection."""
    if not db_path.exists():
        init_db(db_path)

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    return conn


def save_session(db_path: Path, session: Session) -> None:
    """Save or update a session."""
    conn = get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO sessions
        (id, task, repo, worktree, status, started_at, ended_at, pid, model, run_mode)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            session.id,
            session.task,
            str(session.repo),
            str(session.worktree),
            session.status.value,
            session.started_at.isoformat(),
            session.ended_at.isoformat() if session.ended_at else None,
            session.pid,
            session.backend,
            session.run_mode,
        ),
    )

    conn.commit()
    conn.close()


def load_sessions(
    db_path: Path,
    repo: Optional[Path] = None,
    include_completed: bool = False,
) -> list[Session]:
    """Load sessions from database, optionally filtered by repo."""
    if not db_path.exists():
        return []

    conn = get_db(db_path)

    if include_completed:
        if repo:
            cursor = conn.execute(
                "SELECT * FROM sessions WHERE repo = ?",
                (str(repo),),
            )
        else:
            cursor = conn.execute("SELECT * FROM sessions")
    else:
        if repo:
            cursor = conn.execute(
                "SELECT * FROM sessions WHERE repo = ? AND status IN ('running', 'waiting')",
                (str(repo),),
            )
        else:
            cursor = conn.execute(
                "SELECT * FROM sessions WHERE status IN ('running', 'waiting')"
            )

    sessions = []
    for row in cursor:
        sessions.append(Session.from_dict(dict(row)))

    conn.close()
    return sessions


def load_session(db_path: Path, session_id: str) -> Session | None:
    """Load a single session by ID."""
    if not db_path.exists():
        return None

    conn = get_db(db_path)
    cursor = conn.execute("SELECT * FROM sessions WHERE id = ?", (session_id,))
    row = cursor.fetchone()
    conn.close()

    if not row:
        return None
    return Session.from_dict(dict(row))


def update_session_status(db_path: Path, session_id: str, status: SessionStatus) -> bool:
    """Update session status."""
    ended_at = None
    if status in (SessionStatus.COMPLETED, SessionStatus.ERROR):
        ended_at = datetime.now().isoformat()

    conn = get_db(db_path)

    cursor = conn.execute(
        "UPDATE sessions SET status = ?, ended_at = COALESCE(?, ended_at) WHERE id = ?",
        (status.value, ended_at, session_id),
    )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_session(db_path: Path, session_id: str) -> bool:
    """Delete a session from database."""
    conn = get_db(db_path)

    cursor = conn.execute("DELETE FROM sessions WHERE id = ?", (session_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def delete_session_data(db_path: Path, session_id: str) -> bool:
    """Delete a session from database."""
    return delete_session(db_path, session_id)
