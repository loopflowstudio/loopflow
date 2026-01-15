"""SQLite database for lfd state."""

import sqlite3
from datetime import datetime
from pathlib import Path

from loopflow.lfd.models import AgentRun, AgentStatus, Session, SessionStatus
from loopflow.lfd.process import is_process_running

DB_PATH = Path.home() / ".lf" / "lfd.db"


def _init_db(db_path: Path) -> None:
    """Initialize lfd.db with schema."""
    db_path.parent.mkdir(parents=True, exist_ok=True)

    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA journal_mode=WAL")

    conn.executescript("""
        CREATE TABLE IF NOT EXISTS agent_runs (
            id TEXT PRIMARY KEY,
            agent_name TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            pid INTEGER,
            worktree TEXT,
            iteration INTEGER DEFAULT 0,
            error TEXT,
            main_sha TEXT,
            emoji TEXT DEFAULT ''
        );

        CREATE INDEX IF NOT EXISTS idx_agent_runs_name ON agent_runs(agent_name);
        CREATE INDEX IF NOT EXISTS idx_agent_runs_status ON agent_runs(status);

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

        CREATE INDEX IF NOT EXISTS idx_sessions_status ON sessions(status);
    """)

    conn.commit()
    conn.close()


def _get_db(db_path: Path | None = None) -> sqlite3.Connection:
    """Get database connection."""
    if db_path is None:
        db_path = DB_PATH

    if not db_path.exists():
        _init_db(db_path)

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    return conn


# Agent runs

def save_run(run: AgentRun, db_path: Path | None = None) -> None:
    """Save an agent run."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO agent_runs
        (id, agent_name, status, started_at, ended_at, pid, worktree, iteration, error, main_sha, emoji)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            run.id,
            run.agent_name,
            run.status.value,
            run.started_at.isoformat(),
            run.ended_at.isoformat() if run.ended_at else None,
            run.pid,
            run.worktree,
            run.iteration,
            run.error,
            run.main_sha,
            run.emoji,
        ),
    )

    conn.commit()
    conn.close()


def load_agent_runs(active_only: bool = False, db_path: Path | None = None) -> list[AgentRun]:
    """Load agent runs."""
    conn = _get_db(db_path)

    if active_only:
        cursor = conn.execute(
            "SELECT * FROM agent_runs WHERE status = 'running'"
        )
    else:
        cursor = conn.execute("SELECT * FROM agent_runs")

    runs = [_run_from_row(dict(row)) for row in cursor]
    conn.close()
    return runs


def get_latest_run(agent_name: str, db_path: Path | None = None) -> AgentRun | None:
    """Get the most recent run for an agent."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM agent_runs WHERE agent_name = ? ORDER BY started_at DESC LIMIT 1",
        (agent_name,),
    )
    row = cursor.fetchone()
    conn.close()

    if not row:
        return None
    return _run_from_row(dict(row))


def update_run_status(run_id: str, status: AgentStatus, error: str | None = None, db_path: Path | None = None) -> bool:
    """Update run status."""
    conn = _get_db(db_path)

    ended_at = None
    if status in (AgentStatus.STOPPED, AgentStatus.ERROR, AgentStatus.IDLE):
        ended_at = datetime.now().isoformat()

    if error:
        cursor = conn.execute(
            "UPDATE agent_runs SET status = ?, ended_at = ?, error = ? WHERE id = ?",
            (status.value, ended_at, error, run_id),
        )
    else:
        cursor = conn.execute(
            "UPDATE agent_runs SET status = ?, ended_at = COALESCE(?, ended_at) WHERE id = ?",
            (status.value, ended_at, run_id),
        )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_dead_runs(db_path: Path | None = None) -> int:
    """Mark runs as stopped if their process is no longer running."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT id, pid FROM agent_runs WHERE status = 'running' AND pid IS NOT NULL"
    )

    count = 0
    for row in cursor.fetchall():
        if not is_process_running(row["pid"]):
            conn.execute(
                "UPDATE agent_runs SET status = 'stopped', ended_at = ? WHERE id = ?",
                (datetime.now().isoformat(), row["id"]),
            )
            count += 1

    conn.commit()
    conn.close()
    return count


def _run_from_row(row: dict) -> AgentRun:
    """Convert database row to AgentRun."""
    return AgentRun(
        id=row["id"],
        agent_name=row["agent_name"],
        status=AgentStatus(row["status"]),
        started_at=datetime.fromisoformat(row["started_at"]),
        ended_at=datetime.fromisoformat(row["ended_at"]) if row.get("ended_at") else None,
        pid=row.get("pid"),
        worktree=row.get("worktree"),
        iteration=row.get("iteration", 0),
        error=row.get("error"),
        main_sha=row.get("main_sha"),
        emoji=row.get("emoji", ""),
    )


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
            session.task,
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


def load_sessions(active_only: bool = False, db_path: Path | None = None) -> list[Session]:
    """Load sessions."""
    conn = _get_db(db_path)

    if active_only:
        cursor = conn.execute(
            "SELECT * FROM sessions WHERE status IN ('running', 'waiting')"
        )
    else:
        cursor = conn.execute("SELECT * FROM sessions")

    sessions = [_session_from_row(dict(row)) for row in cursor]
    conn.close()
    return sessions


def _session_from_row(row: dict) -> Session:
    """Convert database row to Session."""
    return Session(
        id=row["id"],
        task=row["task"],
        repo=row["repo"],
        worktree=row["worktree"],
        status=SessionStatus(row["status"]),
        started_at=datetime.fromisoformat(row["started_at"]),
        ended_at=datetime.fromisoformat(row["ended_at"]) if row.get("ended_at") else None,
        pid=row.get("pid"),
        model=row.get("model", "claude-code"),
        run_mode=row.get("run_mode", "auto"),
    )
