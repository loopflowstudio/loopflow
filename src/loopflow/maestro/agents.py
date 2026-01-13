"""Agent loop management API.

Public API for registering and managing background agents.
"""

import os
import subprocess
import sys
import uuid
from pathlib import Path

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

    if agent.status == AgentStatus.RUNNING:
        return False

    if background:
        # Spawn agent runner as background process
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
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
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
