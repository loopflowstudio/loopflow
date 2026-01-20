"""SQLite database for lfd state."""

import sqlite3
import uuid
from datetime import datetime
from pathlib import Path

from loopflow.lfd.models import (
    AgentRun,
    AgentStatus,
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

    conn.executescript("""
        -- New loop-based tables

        CREATE TABLE IF NOT EXISTS loops (
            id TEXT PRIMARY KEY,
            type TEXT NOT NULL,
            goal TEXT NOT NULL,
            repo TEXT NOT NULL,
            personal_main TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'idle',
            iteration INTEGER DEFAULT 0,
            pr_limit INTEGER DEFAULT 5,
            merge_mode TEXT DEFAULT 'auto',
            project_file TEXT,
            pathset TEXT,
            cron TEXT,
            area TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(type, goal, repo)
        );

        CREATE INDEX IF NOT EXISTS idx_loops_repo ON loops(repo);
        CREATE INDEX IF NOT EXISTS idx_loops_status ON loops(status);

        CREATE TABLE IF NOT EXISTS loop_runs (
            id TEXT PRIMARY KEY,
            loop_id TEXT NOT NULL,
            iteration INTEGER NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            worktree TEXT,
            current_step TEXT,
            error TEXT,
            pr_url TEXT,
            FOREIGN KEY (loop_id) REFERENCES loops(id)
        );

        CREATE INDEX IF NOT EXISTS idx_loop_runs_loop ON loop_runs(loop_id);

        -- Legacy tables (kept for migration)

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
            emoji TEXT DEFAULT '',
            current_step TEXT
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

        CREATE TABLE IF NOT EXISTS loop_prs (
            id TEXT PRIMARY KEY,
            loop_name TEXT NOT NULL,
            iteration INTEGER NOT NULL,
            pr_url TEXT NOT NULL,
            pr_number INTEGER,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_loop_prs_name ON loop_prs(loop_name);
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
        (id, agent_name, status, started_at, ended_at, pid, worktree, iteration, error, main_sha, emoji, current_step)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
            run.current_step,
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
        current_step=row.get("current_step"),
    )


def update_current_step(run_id: str, step: str | None, db_path: Path | None = None) -> bool:
    """Update the current step for an agent run."""
    conn = _get_db(db_path)
    cursor = conn.execute(
        "UPDATE agent_runs SET current_step = ? WHERE id = ?",
        (step, run_id),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


# Loop PRs


def save_loop_pr(
    loop_name: str,
    iteration: int,
    pr_url: str,
    pr_number: int | None = None,
    status: str = "open",
    db_path: Path | None = None,
) -> str:
    """Save a PR created by a loop iteration."""
    import uuid

    conn = _get_db(db_path)
    pr_id = str(uuid.uuid4())

    conn.execute(
        """
        INSERT INTO loop_prs (id, loop_name, iteration, pr_url, pr_number, status, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        """,
        (pr_id, loop_name, iteration, pr_url, pr_number, status, datetime.now().isoformat()),
    )

    conn.commit()
    conn.close()
    return pr_id


def get_loop_prs(loop_name: str, limit: int = 10, db_path: Path | None = None) -> list[dict]:
    """Get PRs for a loop."""
    conn = _get_db(db_path)
    cursor = conn.execute(
        """
        SELECT id, loop_name, iteration, pr_url, pr_number, status, created_at
        FROM loop_prs
        WHERE loop_name = ?
        ORDER BY created_at DESC
        LIMIT ?
        """,
        (loop_name, limit),
    )
    prs = [dict(row) for row in cursor]
    conn.close()
    return prs


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


def load_sessions_for_worktree(worktree: str, limit: int = 20, db_path: Path | None = None) -> list[Session]:
    """Load recent sessions for a worktree path."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM sessions WHERE worktree = ? ORDER BY started_at DESC LIMIT ?",
        (worktree, limit),
    )

    sessions = [_session_from_row(dict(row)) for row in cursor]
    conn.close()
    return sessions


def load_sessions_for_repo(repo: str, limit: int = 50, db_path: Path | None = None) -> list[Session]:
    """Load recent sessions across all worktrees in a repo."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM sessions WHERE repo = ? ORDER BY started_at DESC LIMIT ?",
        (repo, limit),
    )

    sessions = [_session_from_row(dict(row)) for row in cursor]
    conn.close()
    return sessions


def update_session_status(session_id: str, status: SessionStatus, db_path: Path | None = None) -> bool:
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


# Loop functions


def save_loop(loop: Loop, db_path: Path | None = None) -> None:
    """Save or update a loop."""
    conn = _get_db(db_path)
    conn.execute(
        """
        INSERT OR REPLACE INTO loops
        (id, type, goal, repo, personal_main, status, iteration, pr_limit, merge_mode,
         project_file, pathset, cron, area, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            loop.id,
            loop.type.value,
            loop.goal,
            str(loop.repo),
            loop.personal_main,
            loop.status.value,
            loop.iteration,
            loop.pr_limit,
            loop.merge_mode.value,
            loop.project_file,
            loop.pathset,
            loop.cron,
            loop.area,
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


def get_loop_by_goal_repo(
    loop_type: LoopType, goal: str, repo: Path, db_path: Path | None = None
) -> Loop | None:
    """Get a loop by type, goal, and repo."""
    conn = _get_db(db_path)
    cursor = conn.execute(
        "SELECT * FROM loops WHERE type = ? AND goal = ? AND repo = ?",
        (loop_type.value, goal, str(repo)),
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


def update_loop_status(
    loop_id: str, status: LoopStatus, db_path: Path | None = None
) -> bool:
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


def update_loop_iteration(
    loop_id: str, iteration: int, db_path: Path | None = None
) -> bool:
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
    return Loop(
        id=row["id"],
        type=LoopType(row["type"]),
        goal=row["goal"],
        repo=Path(row["repo"]),
        personal_main=row["personal_main"],
        status=LoopStatus(row["status"]),
        iteration=row.get("iteration", 0),
        pr_limit=row.get("pr_limit", 5),
        merge_mode=MergeMode(row.get("merge_mode", "auto")),
        project_file=row.get("project_file"),
        pathset=row.get("pathset"),
        cron=row.get("cron"),
        area=row.get("area"),
        created_at=datetime.fromisoformat(row["created_at"]),
    )


# Loop run functions


def save_loop_run(run: LoopRun, db_path: Path | None = None) -> None:
    """Save a loop run."""
    conn = _get_db(db_path)
    conn.execute(
        """
        INSERT OR REPLACE INTO loop_runs
        (id, loop_id, iteration, status, started_at, ended_at, worktree, current_step, error, pr_url)
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
    run_id: str, status: LoopStatus, error: str | None = None, db_path: Path | None = None
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
