"""SQLite database for maestro state."""

import json
import sqlite3
from datetime import datetime
from pathlib import Path
from typing import Optional

from loopflow.maestro.session import Session, SessionStatus
from loopflow.maestro.agent import AgentLoopSpec, RegisteredAgent, AgentStatus

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

        CREATE TABLE IF NOT EXISTS agents (
            id TEXT PRIMARY KEY,
            spec TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'idle',
            last_run_at TEXT,
            current_worktree TEXT,
            current_branch TEXT,
            iteration INTEGER NOT NULL DEFAULT 0,
            pid INTEGER
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


# Agent CRUD operations


def save_agent(db_path: Path, agent: RegisteredAgent) -> None:
    """Save or update an agent."""
    conn = get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO agents
        (id, spec, status, last_run_at, current_worktree, current_branch, iteration, pid)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            agent.id,
            json.dumps(agent.spec.to_dict()),
            agent.status.value,
            agent.last_run_at.isoformat() if agent.last_run_at else None,
            str(agent.current_worktree) if agent.current_worktree else None,
            agent.current_branch,
            agent.iteration,
            agent.pid,
        ),
    )

    conn.commit()
    conn.close()


def load_agents(db_path: Path) -> list[RegisteredAgent]:
    """Load all agents from database."""
    if not db_path.exists():
        return []

    conn = get_db(db_path)
    cursor = conn.execute("SELECT * FROM agents")

    agents = []
    for row in cursor:
        agents.append(_agent_from_row(dict(row)))

    conn.close()
    return agents


def load_agent(db_path: Path, agent_id: str) -> RegisteredAgent | None:
    """Load a single agent by ID."""
    if not db_path.exists():
        return None

    conn = get_db(db_path)
    cursor = conn.execute("SELECT * FROM agents WHERE id = ?", (agent_id,))
    row = cursor.fetchone()
    conn.close()

    if not row:
        return None
    return _agent_from_row(dict(row))


def load_agent_by_name(db_path: Path, name: str) -> RegisteredAgent | None:
    """Load an agent by spec name."""
    if not db_path.exists():
        return None

    conn = get_db(db_path)
    cursor = conn.execute("SELECT * FROM agents")

    for row in cursor:
        agent = _agent_from_row(dict(row))
        if agent.spec.name == name:
            conn.close()
            return agent

    conn.close()
    return None


def update_agent_status(
    db_path: Path,
    agent_id: str,
    status: AgentStatus,
    pid: int | None = None,
    worktree: Path | None = None,
    branch: str | None = None,
) -> bool:
    """Update agent status and optional runtime fields."""
    conn = get_db(db_path)

    updates = ["status = ?"]
    values: list = [status.value]

    if status == AgentStatus.RUNNING:
        updates.append("last_run_at = ?")
        values.append(datetime.now().isoformat())

    if pid is not None:
        updates.append("pid = ?")
        values.append(pid)

    if worktree is not None:
        updates.append("current_worktree = ?")
        values.append(str(worktree))

    if branch is not None:
        updates.append("current_branch = ?")
        values.append(branch)

    values.append(agent_id)

    cursor = conn.execute(
        f"UPDATE agents SET {', '.join(updates)} WHERE id = ?",
        tuple(values),
    )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def increment_agent_iteration(db_path: Path, agent_id: str) -> bool:
    """Increment the iteration counter for an agent."""
    conn = get_db(db_path)

    cursor = conn.execute(
        "UPDATE agents SET iteration = iteration + 1 WHERE id = ?",
        (agent_id,),
    )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_agent(db_path: Path, agent_id: str) -> bool:
    """Delete an agent from database."""
    conn = get_db(db_path)

    cursor = conn.execute("DELETE FROM agents WHERE id = ?", (agent_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _agent_from_row(row: dict) -> RegisteredAgent:
    """Convert database row to RegisteredAgent."""
    spec_data = json.loads(row["spec"])
    return RegisteredAgent(
        id=row["id"],
        spec=AgentLoopSpec.from_dict(spec_data),
        status=AgentStatus(row["status"]),
        last_run_at=datetime.fromisoformat(row["last_run_at"]) if row.get("last_run_at") else None,
        current_worktree=Path(row["current_worktree"]) if row.get("current_worktree") else None,
        current_branch=row.get("current_branch"),
        iteration=row.get("iteration", 0),
        pid=row.get("pid"),
    )
