"""Agent loop management API.

Public API for registering and managing background agents.
"""

import os
import subprocess
import sys
import time
import uuid
from pathlib import Path

from loopflow.logging import get_log_dir
from loopflow.maestro.agent import (
    AgentLoopSpec,
    AgentStatus,
    RegisteredAgent,
)
from loopflow.maestro.db import (
    DEFAULT_DB_PATH,
    delete_agent,
    load_agent,
    load_agent_by_name,
    load_agents,
    save_agent,
    update_agent_status,
)
from loopflow.maestro.runner import run_agent_iteration, stop_agent as _stop_agent


def _is_process_running(pid: int) -> bool:
    """Check if a process with given PID is still running."""
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def register_agent(spec: AgentLoopSpec) -> RegisteredAgent:
    """Register a new background agent.

    If an agent with the same name exists, it is replaced.
    """
    # Check for existing agent with same name
    existing = load_agent_by_name(DEFAULT_DB_PATH, spec.name)
    if existing:
        # Update existing agent's spec
        existing.spec = spec
        save_agent(DEFAULT_DB_PATH, existing)
        return existing

    # Create new agent
    agent = RegisteredAgent(
        id=str(uuid.uuid4()),
        spec=spec,
        status=AgentStatus.IDLE,
    )
    save_agent(DEFAULT_DB_PATH, agent)
    return agent


def list_agents() -> list[RegisteredAgent]:
    """List all registered agents."""
    return load_agents(DEFAULT_DB_PATH)


def get_agent(agent_id: str) -> RegisteredAgent | None:
    """Get an agent by ID."""
    return load_agent(DEFAULT_DB_PATH, agent_id)


def get_agent_by_name(name: str) -> RegisteredAgent | None:
    """Get an agent by name."""
    return load_agent_by_name(DEFAULT_DB_PATH, name)


def start_agent(
    agent_id: str,
    repo_root: Path,
    background: bool = True,
) -> bool:
    """Start an agent running.

    If background=True, spawns a subprocess. Otherwise runs in foreground.
    Returns True if agent was started.
    """
    agent = load_agent(DEFAULT_DB_PATH, agent_id)
    if not agent:
        return False

    # Check for stale RUNNING status - if the process isn't actually running, reset it
    if agent.status == AgentStatus.RUNNING:
        if agent.pid and _is_process_running(agent.pid):
            return False
        # Process died without updating status; reset to IDLE
        update_agent_status(DEFAULT_DB_PATH, agent_id, AgentStatus.IDLE)

    if background:
        # Create log file for subprocess stderr
        log_dir = get_log_dir(repo_root)
        log_path = log_dir / f"agent-{agent.spec.name}.log"

        with log_path.open("a") as log_file:
            process = subprocess.Popen(
                [
                    sys.executable,
                    "-m",
                    "loopflow.maestro.agent_runner",
                    "--agent-id",
                    agent_id,
                    "--repo-root",
                    str(repo_root),
                ],
                stdout=subprocess.DEVNULL,
                stderr=log_file,
                start_new_session=True,
            )

        # Brief wait to catch immediate startup failures
        time.sleep(0.1)
        if process.poll() is not None:
            # Process exited immediately - startup failed
            update_agent_status(DEFAULT_DB_PATH, agent_id, AgentStatus.ERROR)
            return False

        update_agent_status(
            DEFAULT_DB_PATH,
            agent_id,
            AgentStatus.RUNNING,
            pid=process.pid,
        )
        return True
    else:
        # Run in foreground
        update_agent_status(
            DEFAULT_DB_PATH,
            agent_id,
            AgentStatus.RUNNING,
            pid=os.getpid(),
        )
        try:
            exit_code = run_agent_iteration(agent, repo_root, foreground=True)
            return exit_code == 0
        finally:
            update_agent_status(DEFAULT_DB_PATH, agent_id, AgentStatus.IDLE)


def stop_agent(agent_id: str) -> bool:
    """Stop a running agent."""
    return _stop_agent(agent_id)


def remove_agent(agent_id: str) -> bool:
    """Remove an agent registration."""
    agent = load_agent(DEFAULT_DB_PATH, agent_id)
    if not agent:
        return False

    if agent.status == AgentStatus.RUNNING:
        _stop_agent(agent_id)

    return delete_agent(DEFAULT_DB_PATH, agent_id)
