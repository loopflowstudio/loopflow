"""StepRun entity persistence.

StepRuns track individual step executions, either:
- Standalone (interactive `lf step` runs)
- As part of a FlowRun (agent-spawned)
"""

import json
import socket
from datetime import datetime
from pathlib import Path
from typing import Any

from loopflow.lfd.db import _get_db
from loopflow.lfd.models import StepRun, StepRunStatus

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"


def save_step_run(step_run: StepRun, db_path: Path | None = None) -> None:
    """Save a step run."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO step_runs
        (id, step, repo, worktree, flow_run_id, wave_id, status,
         started_at, ended_at, pid, model, run_mode)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            step_run.id,
            step_run.step,
            step_run.repo,
            step_run.worktree,
            step_run.flow_run_id,
            step_run.wave_id,
            step_run.status.value,
            step_run.started_at.isoformat(),
            step_run.ended_at.isoformat() if step_run.ended_at else None,
            step_run.pid,
            step_run.model,
            step_run.run_mode,
        ),
    )

    conn.commit()
    conn.close()


def load_step_runs(
    repo: str | None = None,
    active_only: bool = False,
    db_path: Path | None = None,
) -> list[StepRun]:
    """Load step runs, optionally filtered by repo."""
    conn = _get_db(db_path)

    conditions = []
    params: list = []

    if repo:
        conditions.append("repo = ?")
        params.append(repo)

    if active_only:
        conditions.append("status IN ('running', 'waiting')")

    where = f" WHERE {' AND '.join(conditions)}" if conditions else ""
    cursor = conn.execute(f"SELECT * FROM step_runs{where} ORDER BY started_at DESC", params)

    step_runs = [_step_run_from_row(dict(row)) for row in cursor]
    conn.close()
    return step_runs


def load_step_runs_for_worktree(
    worktree: str, limit: int = 20, db_path: Path | None = None
) -> list[StepRun]:
    """Load recent step runs for a worktree path."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM step_runs WHERE worktree = ? ORDER BY started_at DESC LIMIT ?",
        (worktree, limit),
    )

    step_runs = [_step_run_from_row(dict(row)) for row in cursor]
    conn.close()
    return step_runs


def load_step_runs_for_repo(
    repo: str, limit: int = 50, db_path: Path | None = None
) -> list[StepRun]:
    """Load recent step runs across all worktrees in a repo."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM step_runs WHERE repo = ? ORDER BY started_at DESC LIMIT ?",
        (repo, limit),
    )

    step_runs = [_step_run_from_row(dict(row)) for row in cursor]
    conn.close()
    return step_runs


def update_step_run_status(
    step_run_id: str, status: StepRunStatus, db_path: Path | None = None
) -> bool:
    """Update step run status."""
    conn = _get_db(db_path)

    ended_at = None
    if status in (StepRunStatus.COMPLETED, StepRunStatus.FAILED):
        ended_at = datetime.now().isoformat()

    cursor = conn.execute(
        "UPDATE step_runs SET status = ?, ended_at = COALESCE(?, ended_at) WHERE id = ?",
        (status.value, ended_at, step_run_id),
    )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_step_run(step_run_id: str, db_path: Path | None = None) -> bool:
    """Delete a step run from database."""
    conn = _get_db(db_path)

    cursor = conn.execute("DELETE FROM step_runs WHERE id = ?", (step_run_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _step_run_from_row(row: dict) -> StepRun:
    """Convert database row to StepRun."""
    return StepRun(
        id=row["id"],
        step=row["step"],
        repo=row["repo"],
        worktree=row["worktree"],
        flow_run_id=row.get("flow_run_id"),
        wave_id=row.get("wave_id"),
        status=StepRunStatus(row["status"]),
        started_at=datetime.fromisoformat(row["started_at"]),
        ended_at=datetime.fromisoformat(row["ended_at"]) if row.get("ended_at") else None,
        pid=row.get("pid"),
        model=row.get("model", "claude-code"),
        run_mode=row.get("run_mode", "auto"),
    )


def get_waiting_step_run(wave_id: str, db_path: Path | None = None) -> StepRun | None:
    """Find the WAITING StepRun for a wave."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM step_runs WHERE wave_id = ? AND status = ? ORDER BY started_at DESC LIMIT 1",
        (wave_id, StepRunStatus.WAITING.value),
    )

    row = cursor.fetchone()
    conn.close()
    return _step_run_from_row(dict(row)) if row else None


def get_step_run(step_run_id: str, db_path: Path | None = None) -> StepRun | None:
    """Get a step run by ID."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM step_runs WHERE id = ?", (step_run_id,))

    row = cursor.fetchone()
    conn.close()
    return _step_run_from_row(dict(row)) if row else None


# Fire-and-forget logging to lfd daemon


def _send_fire_and_forget(method: str, params: dict[str, Any]) -> None:
    """Send a request to lfd without waiting for response. Fails silently."""
    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(0.5)
        sock.connect(str(SOCKET_PATH))
        request = json.dumps({"method": method, "params": params}) + "\n"
        sock.sendall(request.encode())
        sock.close()
    except Exception:
        pass  # Fire-and-forget: don't block on errors


def log_step_run_start(step_run: StepRun) -> None:
    """Tell lfd a step run started. Fire-and-forget."""
    _send_fire_and_forget("sessions.start", {"session": step_run.to_dict()})


def log_step_run_end(step_run_id: str, status: StepRunStatus) -> None:
    """Tell lfd a step run ended. Fire-and-forget."""
    _send_fire_and_forget("sessions.end", {"session_id": step_run_id, "status": status.value})
