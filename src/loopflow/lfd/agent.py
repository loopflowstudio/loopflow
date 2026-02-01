"""Agent entity persistence.

Agents track individual step executions, either:
- Standalone (interactive `lf step` runs)
- As part of a WaveRun (wave-spawned)
"""

import json
import socket
from datetime import datetime
from pathlib import Path
from typing import Any

from loopflow.lfd.db import _get_db
from loopflow.lfd.models import Agent, AgentStatus

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"


def save_agent(agent: Agent, db_path: Path | None = None) -> None:
    """Save an agent."""
    conn = _get_db(db_path)

    conn.execute(
        """
        INSERT OR REPLACE INTO agents
        (id, step, repo, worktree, wave_run_id, wave_id, status,
         started_at, ended_at, pid, model, run_mode)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            agent.id,
            agent.step,
            agent.repo,
            agent.worktree,
            agent.wave_run_id,
            agent.wave_id,
            agent.status.value,
            agent.started_at.isoformat(),
            agent.ended_at.isoformat() if agent.ended_at else None,
            agent.pid,
            agent.model,
            agent.run_mode,
        ),
    )

    conn.commit()
    conn.close()


def load_agents(
    repo: str | None = None,
    active_only: bool = False,
    db_path: Path | None = None,
) -> list[Agent]:
    """Load agents, optionally filtered by repo."""
    conn = _get_db(db_path)

    conditions = []
    params: list = []

    if repo:
        conditions.append("repo = ?")
        params.append(repo)

    if active_only:
        conditions.append("status IN ('running', 'waiting')")

    where = f" WHERE {' AND '.join(conditions)}" if conditions else ""
    cursor = conn.execute(f"SELECT * FROM agents{where} ORDER BY started_at DESC", params)

    agents = [_agent_from_row(dict(row)) for row in cursor]
    conn.close()
    return agents


def load_agents_for_worktree(
    worktree: str, limit: int = 20, db_path: Path | None = None
) -> list[Agent]:
    """Load recent agents for a worktree path."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM agents WHERE worktree = ? ORDER BY started_at DESC LIMIT ?",
        (worktree, limit),
    )

    agents = [_agent_from_row(dict(row)) for row in cursor]
    conn.close()
    return agents


def load_agents_for_repo(
    repo: str, limit: int = 50, db_path: Path | None = None
) -> list[Agent]:
    """Load recent agents across all worktrees in a repo."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM agents WHERE repo = ? ORDER BY started_at DESC LIMIT ?",
        (repo, limit),
    )

    agents = [_agent_from_row(dict(row)) for row in cursor]
    conn.close()
    return agents


def update_agent_status(
    agent_id: str, status: AgentStatus, db_path: Path | None = None
) -> bool:
    """Update agent status."""
    conn = _get_db(db_path)

    ended_at = None
    if status in (AgentStatus.COMPLETED, AgentStatus.FAILED):
        ended_at = datetime.now().isoformat()

    cursor = conn.execute(
        "UPDATE agents SET status = ?, ended_at = COALESCE(?, ended_at) WHERE id = ?",
        (status.value, ended_at, agent_id),
    )

    conn.commit()
    updated = cursor.rowcount > 0
    conn.close()
    return updated


def delete_agent(agent_id: str, db_path: Path | None = None) -> bool:
    """Delete an agent from database."""
    conn = _get_db(db_path)

    cursor = conn.execute("DELETE FROM agents WHERE id = ?", (agent_id,))

    conn.commit()
    deleted = cursor.rowcount > 0
    conn.close()
    return deleted


def _agent_from_row(row: dict) -> Agent:
    """Convert database row to Agent."""
    return Agent(
        id=row["id"],
        step=row["step"],
        repo=row["repo"],
        worktree=row["worktree"],
        wave_run_id=row.get("wave_run_id"),
        wave_id=row.get("wave_id"),
        status=AgentStatus(row["status"]),
        started_at=datetime.fromisoformat(row["started_at"]),
        ended_at=datetime.fromisoformat(row["ended_at"]) if row.get("ended_at") else None,
        pid=row.get("pid"),
        model=row.get("model", "claude-code"),
        run_mode=row.get("run_mode", "auto"),
    )


def get_waiting_agent(wave_id: str, db_path: Path | None = None) -> Agent | None:
    """Find the WAITING Agent for a wave."""
    conn = _get_db(db_path)

    cursor = conn.execute(
        "SELECT * FROM agents WHERE wave_id = ? AND status = ? ORDER BY started_at DESC LIMIT 1",
        (wave_id, AgentStatus.WAITING.value),
    )

    row = cursor.fetchone()
    conn.close()
    return _agent_from_row(dict(row)) if row else None


def get_agent(agent_id: str, db_path: Path | None = None) -> Agent | None:
    """Get an agent by ID."""
    conn = _get_db(db_path)

    cursor = conn.execute("SELECT * FROM agents WHERE id = ?", (agent_id,))

    row = cursor.fetchone()
    conn.close()
    return _agent_from_row(dict(row)) if row else None


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


def log_agent_start(agent: Agent) -> None:
    """Tell lfd an agent started. Fire-and-forget."""
    _send_fire_and_forget("agents.start", {"agent": agent.to_dict()})


def log_agent_end(agent_id: str, status: AgentStatus) -> None:
    """Tell lfd an agent ended. Fire-and-forget."""
    _send_fire_and_forget("agents.end", {"agent_id": agent_id, "status": status.value})
