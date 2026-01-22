"""Run entity persistence and operations."""

import json
from datetime import datetime
from pathlib import Path

from loopflow.lfd.db import _get_db
from loopflow.lfd.models import Run, RunStatus


def save_run(run: Run, db_path: Path | None = None) -> None:
    """Save or update a run."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO runs
        (id, parent, flow, area, repo, goals, status, iteration,
         worktree, branch, current_step, error, pr_url,
         started_at, ended_at, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            run.id,
            run.parent,
            run.flow,
            run.area,
            str(run.repo),
            json.dumps(run.goals) if run.goals else None,
            run.status.value,
            run.iteration,
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


def get_run(run_id: str, db_path: Path | None = None) -> Run | None:
    """Get a run by ID (supports short IDs)."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM runs WHERE id = ?", (run_id,))
    row = cursor.fetchone()

    if not row:
        cursor = conn.execute("SELECT * FROM runs WHERE id LIKE ?", (f"{run_id}%",))
        row = cursor.fetchone()

    conn.close()
    return _run_from_row(dict(row)) if row else None


def list_runs(
    repo: Path | None = None,
    parent: str | None = None,
    status: RunStatus | None = None,
    limit: int = 50,
    db_path: Path | None = None,
) -> list[Run]:
    """List runs with optional filters."""
    conn = _get_db(db_path)

    conditions = []
    params: list = []

    if repo:
        conditions.append("repo = ?")
        params.append(str(repo))

    if parent:
        conditions.append("parent = ?")
        params.append(parent)

    if status:
        conditions.append("status = ?")
        params.append(status.value)

    where = f" WHERE {' AND '.join(conditions)}" if conditions else ""
    params.append(limit)

    cursor = conn.execute(f"SELECT * FROM runs{where} ORDER BY created_at DESC LIMIT ?", params)

    runs = [_run_from_row(dict(row)) for row in cursor]
    conn.close()
    return runs


def list_runs_for_trigger(
    trigger_type: str,
    trigger_id: str,
    limit: int = 10,
    db_path: Path | None = None,
) -> list[Run]:
    """List runs spawned by a specific trigger."""
    parent = f"{trigger_type}:{trigger_id}"
    return list_runs(parent=parent, limit=limit, db_path=db_path)


def get_latest_run_for_trigger(
    trigger_type: str, trigger_id: str, db_path: Path | None = None
) -> Run | None:
    """Get the most recent run for a trigger."""
    runs = list_runs_for_trigger(trigger_type, trigger_id, limit=1, db_path=db_path)
    return runs[0] if runs else None


def update_run_status(
    run_id: str,
    status: RunStatus,
    error: str | None = None,
    db_path: Path | None = None,
) -> bool:
    """Update a run's status."""
    conn = _get_db(db_path)

    ended_at = None
    if status in (RunStatus.COMPLETED, RunStatus.FAILED, RunStatus.CANCELLED):
        ended_at = datetime.now().isoformat()

    if error:
        cursor = conn.execute(
            "UPDATE runs SET status = ?, ended_at = ?, error = ? WHERE id = ? OR id LIKE ?",
            (status.value, ended_at, error, run_id, f"{run_id}%"),
        )
    else:
        cursor = conn.execute(
            "UPDATE runs SET status = ?, ended_at = COALESCE(?, ended_at) "
            "WHERE id = ? OR id LIKE ?",
            (status.value, ended_at, run_id, f"{run_id}%"),
        )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_run_step(run_id: str, step: str | None, db_path: Path | None = None) -> bool:
    """Update the current step for a run."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE runs SET current_step = ? WHERE id = ? OR id LIKE ?",
        (step, run_id, f"{run_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def update_run_pr(run_id: str, pr_url: str, db_path: Path | None = None) -> bool:
    """Update the PR URL for a run."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "UPDATE runs SET pr_url = ? WHERE id = ? OR id LIKE ?",
        (pr_url, run_id, f"{run_id}%"),
    )
    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_run(run_id: str, db_path: Path | None = None) -> bool:
    """Delete a run."""
    conn = _get_db(db_path)

    cursor = conn.execute("DELETE FROM runs WHERE id = ? OR id LIKE ?", (run_id, f"{run_id}%"))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _run_from_row(row: dict) -> Run:
    """Convert database row to Run."""
    goals_str = row.get("goals")
    goals = json.loads(goals_str) if goals_str else []

    return Run(
        id=row["id"],
        parent=row.get("parent"),
        flow=row["flow"],
        area=row["area"],
        repo=Path(row["repo"]),
        goals=goals,
        status=RunStatus(row["status"]),
        iteration=row.get("iteration", 0),
        worktree=row.get("worktree"),
        branch=row.get("branch"),
        current_step=row.get("current_step"),
        error=row.get("error"),
        pr_url=row.get("pr_url"),
        started_at=datetime.fromisoformat(row["started_at"]) if row.get("started_at") else None,
        ended_at=datetime.fromisoformat(row["ended_at"]) if row.get("ended_at") else None,
        created_at=datetime.fromisoformat(row["created_at"]),
    )
