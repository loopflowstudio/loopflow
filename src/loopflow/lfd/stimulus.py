"""Stimulus entity persistence and operations."""

import uuid
from datetime import datetime
from pathlib import Path

from loopflow.lfd.db import _get_db
from loopflow.lfd.models import (
    PendingActivation,
    Stimulus,
    pending_activation_from_row,
    stimulus_from_row,
)

# Stimulus persistence


def save_stimulus(stimulus: Stimulus, db_path: Path | None = None) -> None:
    """Save or update a stimulus."""
    conn = _get_db(db_path)

    created_at = stimulus.created_at or datetime.now()
    last_triggered_at = None
    if stimulus.last_triggered_at:
        last_triggered_at = int(stimulus.last_triggered_at.timestamp())

    conn.execute(
        """
        INSERT OR REPLACE INTO stimuli
        (id, wave_id, kind, cron, last_main_sha, last_triggered_at, enabled, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            stimulus.id,
            stimulus.wave_id,
            stimulus.kind,
            stimulus.cron or "",
            stimulus.last_main_sha,
            last_triggered_at,
            1 if stimulus.enabled else 0,
            created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def get_stimulus(stimulus_id: str, db_path: Path | None = None) -> Stimulus | None:
    """Get a stimulus by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM stimuli WHERE id = ?", (stimulus_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM stimuli WHERE id LIKE ?", (f"{stimulus_id}%",))
        row = cursor.fetchone()

    conn.close()
    return stimulus_from_row(dict(row)) if row else None


def list_stimuli(
    wave_id: str | None = None, kind: str | None = None, db_path: Path | None = None
) -> list[Stimulus]:
    """List stimuli, optionally filtered by wave_id or kind."""
    conn = _get_db(db_path)

    if wave_id and kind:
        cursor = conn.execute(
            "SELECT * FROM stimuli WHERE wave_id = ? AND kind = ? ORDER BY created_at",
            (wave_id, kind),
        )
    elif wave_id:
        cursor = conn.execute(
            "SELECT * FROM stimuli WHERE wave_id = ? ORDER BY created_at", (wave_id,)
        )
    elif kind:
        cursor = conn.execute("SELECT * FROM stimuli WHERE kind = ? ORDER BY created_at", (kind,))
    else:
        cursor = conn.execute("SELECT * FROM stimuli ORDER BY created_at")

    stimuli = [stimulus_from_row(dict(row)) for row in cursor]
    conn.close()
    return stimuli


def list_stimuli_by_kind(kind: str, db_path: Path | None = None) -> list[Stimulus]:
    """List all enabled stimuli of a specific kind."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM stimuli WHERE kind = ? AND enabled = 1 ORDER BY created_at",
        (kind,),
    )
    stimuli = [stimulus_from_row(dict(row)) for row in cursor]
    conn.close()
    return stimuli


def update_stimulus_sha(stimulus_id: str, sha: str | None, db_path: Path | None = None) -> bool:
    """Update a stimulus's last_main_sha (for watch mode)."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE stimuli SET last_main_sha = ? WHERE id = ? OR id LIKE ?",
        (sha, stimulus_id, f"{stimulus_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_stimulus_triggered_at(
    stimulus_id: str, triggered_at: datetime | None, db_path: Path | None = None
) -> bool:
    """Update a stimulus's last_triggered_at (for cron mode)."""
    conn = _get_db(db_path)

    timestamp = int(triggered_at.timestamp()) if triggered_at else None
    cursor = conn.execute(
        "UPDATE stimuli SET last_triggered_at = ? WHERE id = ? OR id LIKE ?",
        (timestamp, stimulus_id, f"{stimulus_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def enable_stimulus(stimulus_id: str, db_path: Path | None = None) -> bool:
    """Enable a stimulus."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE stimuli SET enabled = 1 WHERE id = ? OR id LIKE ?",
        (stimulus_id, f"{stimulus_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def disable_stimulus(stimulus_id: str, db_path: Path | None = None) -> bool:
    """Disable a stimulus."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE stimuli SET enabled = 0 WHERE id = ? OR id LIKE ?",
        (stimulus_id, f"{stimulus_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_stimulus(stimulus_id: str, db_path: Path | None = None) -> bool:
    """Delete a stimulus."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "DELETE FROM stimuli WHERE id = ? OR id LIKE ?",
        (stimulus_id, f"{stimulus_id}%"),
    )
    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def delete_stimuli_for_wave(wave_id: str, db_path: Path | None = None) -> int:
    """Delete all stimuli for a wave. Returns count deleted."""
    conn = _get_db(db_path)

    cursor = conn.execute("DELETE FROM stimuli WHERE wave_id = ?", (wave_id,))
    conn.commit()
    count = cursor.rowcount
    conn.close()
    return count


def create_stimulus(
    wave_id: str,
    kind: str,
    cron: str | None = None,
    db_path: Path | None = None,
) -> Stimulus:
    """Create a new stimulus for a wave."""
    stimulus = Stimulus(
        id=str(uuid.uuid4()),
        wave_id=wave_id,
        kind=kind,
        cron=cron,
        enabled=True,
        created_at=datetime.now(),
    )
    save_stimulus(stimulus, db_path)
    return stimulus


# PendingActivation persistence


def save_pending_activation(activation: PendingActivation, db_path: Path | None = None) -> None:
    """Save a pending activation."""
    conn = _get_db(db_path)

    queued_at = activation.queued_at or datetime.now()
    queued_at_ts = int(queued_at.timestamp())

    conn.execute(
        """
        INSERT OR REPLACE INTO pending_activations
        (id, wave_id, stimulus_id, from_sha, to_sha, queued_at)
        VALUES (?, ?, ?, ?, ?, ?)
        """,
        (
            activation.id,
            activation.wave_id,
            activation.stimulus_id,
            activation.from_sha,
            activation.to_sha,
            queued_at_ts,
        ),
    )
    conn.commit()

    # Update wave's pending_activations count
    conn.execute(
        """
        UPDATE waves SET pending_activations = (
            SELECT COUNT(*) FROM pending_activations WHERE wave_id = ?
        ) WHERE id = ?
        """,
        (activation.wave_id, activation.wave_id),
    )
    conn.commit()
    conn.close()


def list_pending_activations(wave_id: str, db_path: Path | None = None) -> list[PendingActivation]:
    """List pending activations for a wave."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM pending_activations WHERE wave_id = ? ORDER BY queued_at",
        (wave_id,),
    )
    activations = [pending_activation_from_row(dict(row)) for row in cursor]
    conn.close()
    return activations


def get_pending_for_stimulus(
    wave_id: str, stimulus_id: str, db_path: Path | None = None
) -> PendingActivation | None:
    """Get pending activation for a specific stimulus."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM pending_activations WHERE wave_id = ? AND stimulus_id = ?",
        (wave_id, stimulus_id),
    )
    row = cursor.fetchone()
    conn.close()
    return pending_activation_from_row(dict(row)) if row else None


def delete_pending_activations(wave_id: str, db_path: Path | None = None) -> int:
    """Delete all pending activations for a wave. Returns count deleted."""
    conn = _get_db(db_path)

    cursor = conn.execute("DELETE FROM pending_activations WHERE wave_id = ?", (wave_id,))
    conn.commit()
    count = cursor.rowcount

    # Update wave's pending_activations count to 0
    conn.execute("UPDATE waves SET pending_activations = 0 WHERE id = ?", (wave_id,))
    conn.commit()
    conn.close()
    return count


def queue_or_coalesce_activation(
    wave_id: str,
    stimulus_id: str,
    from_sha: str = "",
    to_sha: str = "",
    db_path: Path | None = None,
) -> bool:
    """Queue an activation, coalescing if one already exists for this stimulus.

    For watch stimuli, extends the SHA range if an activation already exists.
    For cron stimuli, just skips if already queued.

    Returns True if a new activation was created, False if coalesced or skipped.
    """
    existing = get_pending_for_stimulus(wave_id, stimulus_id, db_path)

    if existing:
        # Coalesce: keep original from_sha, update to_sha
        if from_sha or to_sha:
            existing.to_sha = to_sha
            save_pending_activation(existing, db_path)
        # For cron, just skip (no SHA range to update)
        return False

    # Create new pending activation
    activation = PendingActivation(
        id=str(uuid.uuid4()),
        wave_id=wave_id,
        stimulus_id=stimulus_id,
        from_sha=from_sha,
        to_sha=to_sha,
        queued_at=datetime.now(),
    )
    save_pending_activation(activation, db_path)
    return True
