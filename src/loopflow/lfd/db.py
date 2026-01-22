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
    Job,
    JobRun,
    JobStatus,
    JobType,
    MergeMode,
    Session,
    SessionStatus,
)
from loopflow.lfd.process import is_process_running

# Backwards compatibility aliases
Loop = Job
LoopRun = JobRun
LoopStatus = JobStatus
LoopType = JobType

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

    mismatch = _check_schema(conn)
    if mismatch:
        conn.close()
        print(f"[lfd] Schema mismatch: {mismatch}", file=sys.stderr)
        print(f"[lfd] Resetting database: {db_path}", file=sys.stderr)
        db_path.unlink()
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

def _check_schema(conn: sqlite3.Connection) -> str | None:
    """Check schema matches code expectations. Returns error message or None if OK."""
    cursor = conn.execute("SELECT name FROM sqlite_master WHERE type='table'")
    tables = {row[0] for row in cursor.fetchall()}

    required = {"sessions", "loops", "loop_runs", "summaries"}
    missing = required - tables

    if not missing:
        return None

    # Check for common mismatch: jobs/loops rename from branch switching
    if "loops" in missing and "jobs" in tables:
        return "found 'jobs' table but code expects 'loops' (branch switch?)"

    return f"missing tables: {missing}"


# Process status checks


def update_dead_runs(db_path: Path | None = None) -> int:
    """Mark jobs as idle if their process is no longer running."""
    conn = _get_db(db_path)

    # Handle both pre and post migration table names
    cursor = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('jobs', 'loops')"
    )
    tables = {row[0] for row in cursor.fetchall()}
    table_name = "jobs" if "jobs" in tables else "loops"

    cursor = conn.execute(f"SELECT id, pid FROM {table_name} WHERE status = 'running' AND pid IS NOT NULL")

    count = 0
    for row in cursor.fetchall():
        if not is_process_running(row["pid"]):
            conn.execute(
                f"UPDATE {table_name} SET status = 'idle', pid = NULL WHERE id = ?",
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


# Job functions


def _get_table_name(conn: sqlite3.Connection) -> str:
    """Get the current table name (jobs or loops depending on migration status)."""
    cursor = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('jobs', 'loops')"
    )
    tables = {row[0] for row in cursor.fetchall()}
    return "jobs" if "jobs" in tables else "loops"


def _get_column_name(conn: sqlite3.Connection, table: str, new_name: str, old_name: str) -> str:
    """Get the current column name (new or old depending on migration status)."""
    cursor = conn.execute(f"PRAGMA table_info({table})")
    columns = {row["name"] for row in cursor.fetchall()}
    return new_name if new_name in columns else old_name


def save_job(job: Job, db_path: Path | None = None) -> None:
    """Save or update a job."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)
    main_col = _get_column_name(conn, table, "job_main", "loop_main")

    conn.execute(
        f"""
        INSERT OR REPLACE INTO {table}
        (id, type, area, repo, {main_col}, goals, flow, status, iteration, pr_limit, merge_mode,
         project_file, pathset, cron, goal, pid, last_main_sha, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            job.id,
            job.type.value,
            job.area,
            str(job.repo),
            job.job_main,
            json.dumps(job.goals) if job.goals else None,
            job.flow,
            job.status.value,
            job.iteration,
            job.pr_limit,
            job.merge_mode.value,
            job.project_file,
            job.pathset,
            job.cron,
            job.goal_name,  # Legacy field
            job.pid,
            job.last_main_sha,
            job.created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


# Backwards compatibility alias
save_loop = save_job


def get_job(job_id: str, db_path: Path | None = None) -> Job | None:
    """Get a job by ID (supports short IDs)."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)

    # Try exact match first, then prefix match
    cursor = conn.execute(f"SELECT * FROM {table} WHERE id = ?", (job_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute(f"SELECT * FROM {table} WHERE id LIKE ?", (f"{job_id}%",))
        row = cursor.fetchone()

    conn.close()
    return _job_from_row(dict(row), conn) if row else None


# Backwards compatibility alias
get_loop = get_job


def get_job_by_area_repo(
    job_type: JobType,
    area: str,
    repo: Path,
    *,
    db_path: Path | None = None,
) -> Job | None:
    """Get a job by type, area, and repo."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)

    cursor = conn.execute(
        f"SELECT * FROM {table} WHERE type = ? AND area = ? AND repo = ?",
        (job_type.value, area, str(repo)),
    )
    row = cursor.fetchone()
    result = _job_from_row(dict(row), conn) if row else None
    conn.close()
    return result


# Backwards compatibility alias
get_loop_by_area_repo = get_job_by_area_repo


def list_jobs(repo: Path | None = None, db_path: Path | None = None) -> list[Job]:
    """List all jobs, optionally filtered by repo."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)

    if repo:
        cursor = conn.execute(
            f"SELECT * FROM {table} WHERE repo = ? ORDER BY created_at DESC",
            (str(repo),),
        )
    else:
        cursor = conn.execute(f"SELECT * FROM {table} ORDER BY created_at DESC")

    jobs = [_job_from_row(dict(row), conn) for row in cursor]
    conn.close()
    return jobs


# Backwards compatibility alias
list_loops = list_jobs


def update_job_status(job_id: str, status: JobStatus, db_path: Path | None = None) -> bool:
    """Update a job's status."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)

    cursor = conn.execute(
        f"UPDATE {table} SET status = ? WHERE id = ? OR id LIKE ?",
        (status.value, job_id, f"{job_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


# Backwards compatibility alias
update_loop_status = update_job_status


def update_job_iteration(job_id: str, iteration: int, db_path: Path | None = None) -> bool:
    """Update a job's iteration count."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)

    cursor = conn.execute(
        f"UPDATE {table} SET iteration = ? WHERE id = ? OR id LIKE ?",
        (iteration, job_id, f"{job_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


# Backwards compatibility alias
update_loop_iteration = update_job_iteration


def update_job_pid(job_id: str, pid: int | None, db_path: Path | None = None) -> bool:
    """Update a job's process ID."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)

    cursor = conn.execute(
        f"UPDATE {table} SET pid = ? WHERE id = ? OR id LIKE ?",
        (pid, job_id, f"{job_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


# Backwards compatibility alias
update_loop_pid = update_job_pid


def update_job_last_sha(job_id: str, sha: str | None, db_path: Path | None = None) -> bool:
    """Update a job's last_main_sha (for subscribe jobs)."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)

    cursor = conn.execute(
        f"UPDATE {table} SET last_main_sha = ? WHERE id = ? OR id LIKE ?",
        (sha, job_id, f"{job_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


# Backwards compatibility alias
update_loop_last_sha = update_job_last_sha


def delete_job(job_id: str, db_path: Path | None = None) -> bool:
    """Delete a job and its runs."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)
    runs_table = "job_runs" if table == "jobs" else "loop_runs"
    id_col = "job_id" if table == "jobs" else "loop_id"

    # Get full ID for foreign key deletes
    cursor = conn.execute(
        f"SELECT id FROM {table} WHERE id = ? OR id LIKE ?", (job_id, f"{job_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return False

    full_id = row["id"]

    # Delete runs first
    conn.execute(f"DELETE FROM {runs_table} WHERE {id_col} = ?", (full_id,))
    cursor = conn.execute(f"DELETE FROM {table} WHERE id = ?", (full_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


# Backwards compatibility alias
delete_loop = delete_job


def _job_from_row(row: dict, conn: sqlite3.Connection | None = None) -> Job:
    """Convert database row to Job."""
    # Handle legacy "auto" merge mode by mapping to PR
    merge_mode_str = row.get("merge_mode", "pr")
    if merge_mode_str == "auto":
        merge_mode_str = "pr"

    # Parse goals JSON
    goals_str = row.get("goals")
    goals = json.loads(goals_str) if goals_str else []

    # Handle area - could be in 'area' column or legacy 'goal' was used as area marker
    area = row.get("area") or "."

    # Handle job_main vs loop_main column name
    job_main = row.get("job_main") or row.get("loop_main", "")

    return Job(
        id=row["id"],
        type=JobType(row["type"]),
        area=area,
        repo=Path(row["repo"]),
        job_main=job_main,
        goals=goals,
        flow=row.get("flow"),
        status=JobStatus(row["status"]),
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


# Backwards compatibility alias
_loop_from_row = _job_from_row


# Job run functions


def save_job_run(run: JobRun, db_path: Path | None = None) -> None:
    """Save a job run."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)
    runs_table = "job_runs" if table == "jobs" else "loop_runs"
    id_col = "job_id" if table == "jobs" else "loop_id"

    conn.execute(
        f"""
        INSERT OR REPLACE INTO {runs_table}
        (id, {id_col}, iteration, status, started_at, ended_at,
         worktree, current_step, error, pr_url)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            run.id,
            run.job_id,
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


# Backwards compatibility alias
save_loop_run = save_job_run


def get_job_runs(job_id: str, limit: int = 10, db_path: Path | None = None) -> list[JobRun]:
    """Get runs for a job."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)
    runs_table = "job_runs" if table == "jobs" else "loop_runs"
    id_col = "job_id" if table == "jobs" else "loop_id"

    # Support short IDs
    cursor = conn.execute(
        f"SELECT id FROM {table} WHERE id = ? OR id LIKE ?", (job_id, f"{job_id}%")
    )
    row = cursor.fetchone()
    if not row:
        conn.close()
        return []

    full_id = row["id"]

    cursor = conn.execute(
        f"SELECT * FROM {runs_table} WHERE {id_col} = ? ORDER BY started_at DESC LIMIT ?",
        (full_id, limit),
    )
    runs = [_job_run_from_row(dict(row)) for row in cursor]
    conn.close()
    return runs


# Backwards compatibility alias
get_loop_runs = get_job_runs


def get_latest_job_run(job_id: str, db_path: Path | None = None) -> JobRun | None:
    """Get the most recent run for a job."""
    runs = get_job_runs(job_id, limit=1, db_path=db_path)
    return runs[0] if runs else None


# Backwards compatibility alias
get_latest_loop_run = get_latest_job_run


def update_job_run_status(
    run_id: str,
    status: JobStatus,
    error: str | None = None,
    db_path: Path | None = None,
) -> bool:
    """Update a job run's status."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)
    runs_table = "job_runs" if table == "jobs" else "loop_runs"

    ended_at = None
    if status in (JobStatus.IDLE, JobStatus.ERROR):
        ended_at = datetime.now().isoformat()

    if error:
        cursor = conn.execute(
            f"UPDATE {runs_table} SET status = ?, ended_at = ?, error = ? WHERE id = ?",
            (status.value, ended_at, error, run_id),
        )
    else:
        cursor = conn.execute(
            f"UPDATE {runs_table} SET status = ?, ended_at = COALESCE(?, ended_at) WHERE id = ?",
            (status.value, ended_at, run_id),
        )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


# Backwards compatibility alias
update_loop_run_status = update_job_run_status


def update_job_run_step(run_id: str, step: str | None, db_path: Path | None = None) -> bool:
    """Update the current step for a job run."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)
    runs_table = "job_runs" if table == "jobs" else "loop_runs"

    cursor = conn.execute(
        f"UPDATE {runs_table} SET current_step = ? WHERE id = ?",
        (step, run_id),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


# Backwards compatibility alias
update_loop_run_step = update_job_run_step


def update_job_run_pr(run_id: str, pr_url: str, db_path: Path | None = None) -> bool:
    """Update the PR URL for a job run."""
    conn = _get_db(db_path)
    table = _get_table_name(conn)
    runs_table = "job_runs" if table == "jobs" else "loop_runs"

    cursor = conn.execute(
        f"UPDATE {runs_table} SET pr_url = ? WHERE id = ?",
        (pr_url, run_id),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


# Backwards compatibility alias
update_loop_run_pr = update_job_run_pr


def _job_run_from_row(row: dict) -> JobRun:
    """Convert database row to JobRun."""
    # Handle job_id vs loop_id column name
    job_id = row.get("job_id") or row.get("loop_id", "")

    return JobRun(
        id=row["id"],
        job_id=job_id,
        iteration=row["iteration"],
        status=JobStatus(row["status"]),
        started_at=datetime.fromisoformat(row["started_at"]),
        ended_at=datetime.fromisoformat(row["ended_at"]) if row.get("ended_at") else None,
        worktree=row.get("worktree"),
        current_step=row.get("current_step"),
        error=row.get("error"),
        pr_url=row.get("pr_url"),
    )


# Backwards compatibility alias
_loop_run_from_row = _job_run_from_row


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
