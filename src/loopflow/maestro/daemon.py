"""Agent daemon that monitors triggers and runs agents.

The daemon runs in the background (managed by launchd) and:
1. Loads all agent definitions from ~/.lf/agents/
2. Checks if each agent's trigger condition is met
3. Spawns agent runs as subprocesses when triggered
4. Maintains runtime state in SQLite for the UI to read
"""

import os
import subprocess
import sys
import time
import uuid
from datetime import datetime
from pathlib import Path

from loopflow.maestro.db import DEFAULT_DB_PATH, get_db
from loopflow.maestro.markdown import AgentFile, list_agent_files
from loopflow.maestro.triggers import get_main_sha, should_trigger


def _log(msg: str) -> None:
    """Log with timestamp."""
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    print(f"[{timestamp}] {msg}", flush=True)


def _init_runs_table() -> None:
    """Initialize the agent_runs table for runtime state."""
    conn = get_db(DEFAULT_DB_PATH)
    conn.executescript("""
        CREATE TABLE IF NOT EXISTS agent_runs (
            id TEXT PRIMARY KEY,
            agent_name TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            pid INTEGER,
            worktree TEXT,
            iteration INTEGER DEFAULT 0,
            error TEXT,
            main_sha TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_agent_runs_name ON agent_runs(agent_name);
        CREATE INDEX IF NOT EXISTS idx_agent_runs_status ON agent_runs(status);
    """)
    conn.commit()
    conn.close()


def _get_last_run(agent_name: str) -> tuple[datetime | None, str | None]:
    """Get the last run time and main SHA for an agent."""
    conn = get_db(DEFAULT_DB_PATH)
    cursor = conn.execute(
        """SELECT started_at, main_sha FROM agent_runs
           WHERE agent_name = ?
           ORDER BY started_at DESC LIMIT 1""",
        (agent_name,),
    )
    row = cursor.fetchone()
    conn.close()

    if not row:
        return None, None

    last_run = datetime.fromisoformat(row["started_at"]) if row["started_at"] else None
    return last_run, row["main_sha"]


def _is_agent_running(agent_name: str) -> bool:
    """Check if an agent is currently running."""
    conn = get_db(DEFAULT_DB_PATH)
    cursor = conn.execute(
        """SELECT pid FROM agent_runs
           WHERE agent_name = ? AND status = 'running'""",
        (agent_name,),
    )
    row = cursor.fetchone()
    conn.close()

    if not row or not row["pid"]:
        return False

    # Check if process is actually running
    try:
        os.kill(row["pid"], 0)
        return True
    except OSError:
        return False


def _update_completed_runs() -> None:
    """Check all running agents and mark completed ones."""
    conn = get_db(DEFAULT_DB_PATH)
    cursor = conn.execute(
        "SELECT id, agent_name, pid FROM agent_runs WHERE status = 'running'"
    )
    rows = cursor.fetchall()

    for row in rows:
        pid = row["pid"]
        if not pid:
            continue

        # Check if process is still running
        try:
            os.kill(pid, 0)
            # Process still running
        except OSError:
            # Process exited - mark as completed
            conn.execute(
                """UPDATE agent_runs
                   SET status = 'completed', ended_at = ?
                   WHERE id = ?""",
                (datetime.now().isoformat(), row["id"]),
            )
            _log(f"Agent {row['agent_name']} completed (was PID {pid})")

    conn.commit()
    conn.close()


def _record_run_start(agent: AgentFile, pid: int, main_sha: str | None) -> str:
    """Record that an agent run has started."""
    run_id = str(uuid.uuid4())

    conn = get_db(DEFAULT_DB_PATH)
    conn.execute(
        """INSERT INTO agent_runs (id, agent_name, status, started_at, pid, main_sha)
           VALUES (?, ?, 'running', ?, ?, ?)""",
        (run_id, agent.name, datetime.now().isoformat(), pid, main_sha),
    )
    conn.commit()
    conn.close()

    return run_id


def _spawn_agent_run(agent: AgentFile) -> int | None:
    """Spawn a subprocess to run an agent iteration."""
    log_dir = Path.home() / ".lf" / "logs" / "agents"
    log_dir.mkdir(parents=True, exist_ok=True)
    log_path = log_dir / f"{agent.name}.log"

    cmd = [
        sys.executable,
        "-m",
        "loopflow.cli",
        "agent",
        "run",
        agent.name,
    ]

    try:
        with log_path.open("a") as log_file:
            process = subprocess.Popen(
                cmd,
                stdout=log_file,
                stderr=log_file,
                start_new_session=True,
            )
        return process.pid
    except Exception as e:
        _log(f"Failed to spawn agent {agent.name}: {e}")
        return None


def run_daemon(check_interval: int = 30) -> None:
    """Run the daemon loop."""
    _log("Starting agent daemon")
    _init_runs_table()

    while True:
        try:
            # Update status of any finished runs
            _update_completed_runs()

            agents = list_agent_files()

            for agent in agents:
                # Skip manual-only agents
                if agent.trigger == "manual":
                    continue

                # Skip if already running
                if _is_agent_running(agent.name):
                    continue

                last_run, last_sha = _get_last_run(agent.name)

                if should_trigger(agent, last_run, last_sha):
                    _log(f"Triggering agent: {agent.name} (trigger: {agent.trigger})")

                    # Get current main SHA for tracking
                    current_sha = get_main_sha(agent.repo) if agent.trigger == "main-changed" else None

                    pid = _spawn_agent_run(agent)
                    if pid:
                        _record_run_start(agent, pid, current_sha)
                        _log(f"Started agent {agent.name} (PID {pid})")

        except Exception as e:
            _log(f"Error in daemon loop: {e}")

        time.sleep(check_interval)


def main() -> None:
    """Entry point for daemon."""
    run_daemon()


if __name__ == "__main__":
    main()
