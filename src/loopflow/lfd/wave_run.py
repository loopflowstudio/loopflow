"""WaveRun entity persistence and operations."""

import json
from datetime import datetime
from pathlib import Path

from loopflow.lfd.db import _get_db
from loopflow.lfd.models import WaveRun, WaveRunStatus


def save_wave_run(run: WaveRun, db_path: Path | None = None) -> None:
    """Save or update a wave run."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO wave_runs
        (id, wave_id, flow, direction, area, repo, status, iteration, step_index,
         worktree, branch, current_step, error, pr_url,
         started_at, ended_at, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            run.id,
            run.wave_id,
            run.flow,
            json.dumps(run.direction),
            json.dumps(run.area),
            str(run.repo),
            run.status.value,
            run.iteration,
            run.step_index,
            run.worktree,
            run.branch,
            run.current_step,
            run.error,
            run.pr_url,
            run.started_at.isoformat() if run.started_at else None,
            run.ended_at.isoformat() if run.ended_at else None,
            run.created_at.isoformat(),
        ),
    )
    conn.commit()
    conn.close()


def get_wave_run(run_id: str, db_path: Path | None = None) -> WaveRun | None:
    """Get a wave run by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM wave_runs WHERE id = ?", (run_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM wave_runs WHERE id LIKE ?", (f"{run_id}%",))
        row = cursor.fetchone()

    conn.close()
    return wave_run_from_row(dict(row)) if row else None


def list_wave_runs(
    repo: Path | None = None,
    wave_id: str | None = None,
    status: WaveRunStatus | None = None,
    limit: int = 50,
    db_path: Path | None = None,
) -> list[WaveRun]:
    """List wave runs with optional filters."""
    conn = _get_db(db_path)

    conditions = []
    params: list = []

    if repo:
        conditions.append("repo = ?")
        params.append(str(repo))

    if wave_id:
        conditions.append("wave_id = ?")
        params.append(wave_id)

    if status:
        conditions.append("status = ?")
        params.append(status.value)

    where = f" WHERE {' AND '.join(conditions)}" if conditions else ""
    params.append(limit)

    cursor = conn.execute(
        f"SELECT * FROM wave_runs{where} ORDER BY created_at DESC LIMIT ?", params
    )

    runs = [wave_run_from_row(dict(row)) for row in cursor]
    conn.close()
    return runs


def count_wave_runs(
    repo: Path | None = None,
    wave_id: str | None = None,
    status: WaveRunStatus | None = None,
    db_path: Path | None = None,
) -> int:
    """Count wave runs with optional filters."""
    conn = _get_db(db_path)

    conditions = []
    params: list = []

    if repo:
        conditions.append("repo = ?")
        params.append(str(repo))

    if wave_id:
        conditions.append("wave_id = ?")
        params.append(wave_id)

    if status:
        conditions.append("status = ?")
        params.append(status.value)

    where = f" WHERE {' AND '.join(conditions)}" if conditions else ""
    cursor = conn.execute(f"SELECT COUNT(*) as count FROM wave_runs{where}", params)
    row = cursor.fetchone()
    conn.close()
    return int(row["count"]) if row else 0


def list_wave_runs_for_wave(
    wave_id: str,
    limit: int = 10,
    db_path: Path | None = None,
) -> list[WaveRun]:
    """List wave runs spawned by a specific wave."""
    return list_wave_runs(wave_id=wave_id, limit=limit, db_path=db_path)


def get_latest_wave_run_for_wave(wave_id: str, db_path: Path | None = None) -> WaveRun | None:
    """Get the most recent wave run for a wave."""
    runs = list_wave_runs_for_wave(wave_id, limit=1, db_path=db_path)
    return runs[0] if runs else None


def update_wave_run_status(
    run_id: str,
    status: WaveRunStatus,
    error: str | None = None,
    db_path: Path | None = None,
) -> bool:
    """Update a wave run's status."""
    conn = _get_db(db_path)

    ended_at = None
    if status in (WaveRunStatus.COMPLETED, WaveRunStatus.FAILED, WaveRunStatus.CANCELLED):
        ended_at = datetime.now().isoformat()

    if error:
        cursor = conn.execute(
            "UPDATE wave_runs SET status = ?, ended_at = ?, error = ? WHERE id = ? OR id LIKE ?",
            (status.value, ended_at, error, run_id, f"{run_id}%"),
        )
    else:
        cursor = conn.execute(
            "UPDATE wave_runs SET status = ?, ended_at = COALESCE(?, ended_at) "
            "WHERE id = ? OR id LIKE ?",
            (status.value, ended_at, run_id, f"{run_id}%"),
        )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_wave_run_step(run_id: str, step: str | None, db_path: Path | None = None) -> bool:
    """Update the current step for a wave run."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE wave_runs SET current_step = ? WHERE id = ? OR id LIKE ?",
        (step, run_id, f"{run_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_wave_run_pr(run_id: str, pr_url: str, db_path: Path | None = None) -> bool:
    """Update the PR URL for a wave run."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE wave_runs SET pr_url = ? WHERE id = ? OR id LIKE ?",
        (pr_url, run_id, f"{run_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_wave_run_index(run_id: str, step_index: int, db_path: Path | None = None) -> bool:
    """Update the step_index for a wave run (tick-based execution)."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE wave_runs SET step_index = ? WHERE id = ? OR id LIKE ?",
        (step_index, run_id, f"{run_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_wave_run(run_id: str, db_path: Path | None = None) -> bool:
    """Delete a wave run."""
    conn = _get_db(db_path)

    cursor = conn.execute("DELETE FROM wave_runs WHERE id = ? OR id LIKE ?", (run_id, f"{run_id}%"))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def wave_run_from_row(row: dict) -> WaveRun:
    """Convert database row to WaveRun."""
    direction_str = row.get("direction")
    direction = json.loads(direction_str) if direction_str else ["default"]

    area_str = row.get("area")
    area = json.loads(area_str) if area_str else ["."]

    return WaveRun(
        id=row["id"],
        wave_id=row.get("wave_id"),
        flow=row["flow"],
        direction=direction,
        area=area,
        repo=Path(row["repo"]),
        status=WaveRunStatus(row["status"]),
        iteration=row.get("iteration", 0),
        step_index=row.get("step_index", 0),
        worktree=row.get("worktree"),
        branch=row.get("branch"),
        current_step=row.get("current_step"),
        error=row.get("error"),
        pr_url=row.get("pr_url"),
        started_at=datetime.fromisoformat(row["started_at"]) if row.get("started_at") else None,
        ended_at=datetime.fromisoformat(row["ended_at"]) if row.get("ended_at") else None,
        created_at=datetime.fromisoformat(row["created_at"]),
    )


# Cleanup functions


def mark_wave_run_failed(run_id: str, error: str, db_path: Path | None = None) -> bool:
    """Mark a wave run as failed. Always succeeds if run exists."""
    return update_wave_run_status(run_id, WaveRunStatus.FAILED, error=error, db_path=db_path)


def cleanup_stale_wave_runs(db_path: Path | None = None) -> int:
    """Find RUNNING/PENDING wave runs with dead wave PIDs and mark as FAILED.

    Returns the number of wave runs cleaned up.
    """
    from loopflow.lfd.daemon.process import is_process_running
    from loopflow.lfd.wave import get_wave

    conn = _get_db(db_path)

    # Find wave runs in active states
    cursor = conn.execute(
        "SELECT id, wave_id FROM wave_runs WHERE status IN (?, ?)",
        (WaveRunStatus.RUNNING.value, WaveRunStatus.PENDING.value),
    )
    rows = cursor.fetchall()
    conn.close()

    cleaned = 0
    for row in rows:
        run_id = row["id"]
        wave_id = row["wave_id"]

        # No wave = orphaned run, mark as failed
        if not wave_id:
            mark_wave_run_failed(run_id, "Orphaned run (no wave)", db_path)
            cleaned += 1
            continue

        # Check if wave's process is alive
        wave = get_wave(wave_id, db_path)
        if not wave:
            mark_wave_run_failed(run_id, "Wave no longer exists", db_path)
            cleaned += 1
            continue

        if wave.pid and is_process_running(wave.pid):
            # Process is still running, leave it alone
            continue

        # Wave has no PID or PID is dead
        mark_wave_run_failed(run_id, "Wave process died", db_path)
        cleaned += 1

    return cleaned
