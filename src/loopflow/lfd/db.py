"""SQLite database infrastructure for lfd.

Connection management, migrations, and shared utilities.
Entity-specific CRUD operations are in runs/*.py modules.
"""

import sqlite3
from datetime import datetime
from pathlib import Path

from loopflow.lfd.migrations.baseline import SCHEMA_VERSION
from loopflow.lfd.migrations.registry import MIGRATIONS

DB_PATH = Path.home() / ".lf" / "lfd.db"


def _init_db(db_path: Path) -> None:
    """Initialize lfd.db with schema."""
    db_path.parent.mkdir(parents=True, exist_ok=True)

    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA journal_mode=WAL")
    _run_migrations(conn)
    conn.close()


def reset_db(db_path: Path | None = None) -> None:
    """Delete and recreate the database."""
    if db_path is None:
        db_path = DB_PATH
    if db_path.exists():
        db_path.unlink()
    # Also remove WAL files
    wal_path = db_path.with_suffix(".db-wal")
    shm_path = db_path.with_suffix(".db-shm")
    if wal_path.exists():
        wal_path.unlink()
    if shm_path.exists():
        shm_path.unlink()
    _init_db(db_path)


def _get_db(db_path: Path | None = None) -> sqlite3.Connection:
    """Get database connection. Resets on schema mismatch."""
    if db_path is None:
        db_path = DB_PATH

    if not db_path.exists():
        _init_db(db_path)

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row

    # Check schema version - reset on any mismatch
    current_version = _get_schema_version(conn)
    if current_version != SCHEMA_VERSION:
        conn.close()
        reset_db(db_path)
        conn = sqlite3.connect(db_path)
        conn.row_factory = sqlite3.Row

    _run_migrations(conn)
    return conn


def _get_schema_version(conn: sqlite3.Connection) -> str | None:
    """Get current schema version from _meta table."""
    try:
        cursor = conn.execute("SELECT value FROM _meta WHERE key = 'schema_version'")
        row = cursor.fetchone()
        return row[0] if row else None
    except sqlite3.OperationalError:
        return None


def _set_schema_version(conn: sqlite3.Connection, version: str) -> None:
    """Set schema version in _meta table."""
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS _meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )
        """
    )
    conn.execute(
        "INSERT OR REPLACE INTO _meta (key, value) VALUES ('schema_version', ?)",
        (version,),
    )
    conn.commit()


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
    # Update schema version after successful migrations
    _set_schema_version(conn, SCHEMA_VERSION)


def update_dead_processes(db_path: Path | None = None) -> int:
    """Mark waves as idle if their process is no longer running."""
    from loopflow.lfd.daemon.process import is_process_running

    conn = _get_db(db_path)
    count = 0

    cursor = conn.execute("SELECT id, pid FROM waves WHERE status = 'running' AND pid IS NOT NULL")
    for row in cursor.fetchall():
        if not is_process_running(row["pid"]):
            conn.execute(
                "UPDATE waves SET status = 'idle', pid = NULL WHERE id = ?",
                (row["id"],),
            )
            count += 1

    conn.commit()
    conn.close()
    return count


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
    """Save a summary to the database. Fails silently on DB errors."""
    import uuid

    try:
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
    except sqlite3.Error:
        pass  # Graceful degradation: summaries are optional


def load_summary_db(
    repo: str,
    path: str,
    token_budget: int,
    db_path: Path | None = None,
) -> dict | None:
    """Load a summary from the database. Returns None on DB errors."""
    try:
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
    except sqlite3.Error:
        return None  # Graceful degradation: treat as cache miss


def delete_summaries_for_repo(repo: str, db_path: Path | None = None) -> int:
    """Delete all summaries for a repo. Returns 0 on DB errors."""
    try:
        conn = _get_db(db_path)
        cursor = conn.execute("DELETE FROM summaries WHERE repo = ?", (repo,))
        conn.commit()
        count = cursor.rowcount
        conn.close()
        return count
    except sqlite3.Error:
        return 0  # Graceful degradation
