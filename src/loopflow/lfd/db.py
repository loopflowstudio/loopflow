"""SQLite database infrastructure for lfd.

Connection management, migrations, and shared utilities.
Entity-specific CRUD operations are in runs/*.py modules.
"""

import sqlite3
from datetime import datetime
from pathlib import Path

from loopflow.lfd.migrations.registry import MIGRATIONS

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


def update_dead_processes(db_path: Path | None = None) -> int:
    """Mark agents as idle if their process is no longer running."""
    from loopflow.lfd.daemon.process import is_process_running

    conn = _get_db(db_path)
    count = 0

    cursor = conn.execute("SELECT id, pid FROM agents WHERE status = 'running' AND pid IS NOT NULL")
    for row in cursor.fetchall():
        if not is_process_running(row["pid"]):
            conn.execute(
                "UPDATE agents SET status = 'idle', pid = NULL WHERE id = ?",
                (row["id"],),
            )
            count += 1

    conn.commit()
    conn.close()
    return count


def list_all_agents(repo: Path | None = None, db_path: Path | None = None) -> list:
    """List all agents for a repo."""
    from loopflow.lfd.agent import list_agents

    return list_agents(repo, db_path)


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
    import uuid

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
    """Load a summary from the database."""
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
