"""
summaries table for codebase summaries
"""

import sqlite3

VERSION = "2025-01-20T01:00:00"
DESCRIPTION = "add summaries table"


def apply(conn: sqlite3.Connection) -> None:
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS summaries (
            id TEXT PRIMARY KEY,
            repo TEXT NOT NULL,
            path TEXT NOT NULL,
            token_budget INTEGER NOT NULL,
            source_hash TEXT NOT NULL,
            content TEXT NOT NULL,
            model TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_summaries_key
            ON summaries(repo, path, token_budget);
        CREATE INDEX IF NOT EXISTS idx_summaries_repo ON summaries(repo);
        """
    )
