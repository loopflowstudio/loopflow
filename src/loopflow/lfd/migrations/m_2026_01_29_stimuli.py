"""Add stimuli and pending_activations tables for many:1 stimulus:wave model.

Stimulus is now an independent trigger entity that can activate a wave.
Multiple stimuli can point to the same wave (many:1).
Wave owns the "what" (area, direction, flow).
Stimulus owns the "when" (kind, trigger config, state).
"""

import sqlite3

VERSION = "2026-01-29T00:00:00Z_stimuli"
DESCRIPTION = "Add stimuli and pending_activations tables for many:1 model"


def apply(conn: sqlite3.Connection) -> None:
    # Create stimuli table
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS stimuli (
            id TEXT PRIMARY KEY,
            wave_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            cron TEXT NOT NULL DEFAULT '',
            last_main_sha TEXT,
            last_triggered_at INTEGER,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE
        )
        """
    )
    conn.execute("CREATE INDEX IF NOT EXISTS idx_stimuli_wave_id ON stimuli(wave_id)")
    conn.execute("CREATE INDEX IF NOT EXISTS idx_stimuli_kind ON stimuli(kind)")

    # Create pending_activations table
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS pending_activations (
            id TEXT PRIMARY KEY,
            wave_id TEXT NOT NULL,
            stimulus_id TEXT NOT NULL,
            from_sha TEXT NOT NULL DEFAULT '',
            to_sha TEXT NOT NULL DEFAULT '',
            queued_at INTEGER NOT NULL,
            FOREIGN KEY (wave_id) REFERENCES waves(id) ON DELETE CASCADE,
            FOREIGN KEY (stimulus_id) REFERENCES stimuli(id) ON DELETE CASCADE
        )
        """
    )
    conn.execute("CREATE INDEX IF NOT EXISTS idx_pending_wave_id ON pending_activations(wave_id)")

    # Migrate existing waves with stimulus_kind to stimuli table
    # Check if stimulus_kind column exists (may not if fresh install)
    cursor = conn.execute("PRAGMA table_info(waves)")
    columns = {row[1] for row in cursor.fetchall()}

    if "stimulus_kind" in columns:
        # Migrate existing wave stimuli
        conn.execute(
            """
            INSERT INTO stimuli (id, wave_id, kind, cron, last_main_sha, enabled, created_at)
            SELECT
                lower(hex(randomblob(16))),
                id,
                stimulus_kind,
                COALESCE(stimulus_cron, ''),
                last_main_sha,
                1,
                created_at
            FROM waves
            WHERE stimulus_kind IN ('loop', 'watch', 'cron')
            AND NOT EXISTS (SELECT 1 FROM stimuli WHERE stimuli.wave_id = waves.id)
            """
        )

    # Add step_index column to waves if not present
    if "step_index" not in columns:
        conn.execute("ALTER TABLE waves ADD COLUMN step_index INTEGER NOT NULL DEFAULT 0")

    conn.commit()
