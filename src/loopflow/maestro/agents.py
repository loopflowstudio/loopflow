"""Agent loop management API.

Public API for registering and managing background agents.
"""

import os
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
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
from loopflow.maestro.runner import (
    run_agent_continuous,
    run_agent_iteration,
    stop_agent as _stop_agent,
)


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


@dataclass
class StartResult:
    """Result of starting an agent."""

    success: bool
    log_path: Path | None = None
    error: str | None = None


def start_agent(
    agent_id: str,
    repo_root: Path,
    background: bool = True,
    continuous: bool = False,
    max_iterations: int | None = None,
    check_interval: int = 300,
) -> StartResult:
    """Start an agent running."""
    agent = load_agent(DEFAULT_DB_PATH, agent_id)
    if not agent:
        return StartResult(success=False, error="Agent not found in database")

    # Check for stale RUNNING status - if the process isn't actually running, reset it
    if agent.status == AgentStatus.RUNNING:
        if agent.pid and _is_process_running(agent.pid):
            return StartResult(success=False, error=f"Agent already running (PID {agent.pid})")
        # Process died without updating status; reset to IDLE
        update_agent_status(DEFAULT_DB_PATH, agent_id, AgentStatus.IDLE)

    if background:
        # Create log file for subprocess stderr
        log_dir = get_log_dir(repo_root)
        log_path = log_dir / f"agent-{agent.spec.name}.log"

        cmd = [
            sys.executable,
            "-m",
            "loopflow.maestro.agent_runner",
            "--agent-id",
            agent_id,
            "--repo-root",
            str(repo_root),
        ]
        if continuous:
            cmd.append("--continuous")
        if max_iterations is not None:
            cmd.extend(["--max-iterations", str(max_iterations)])
        if check_interval != 300:
            cmd.extend(["--check-interval", str(check_interval)])

        with log_path.open("a") as log_file:
            process = subprocess.Popen(
                cmd,
                stdout=subprocess.DEVNULL,
                stderr=log_file,
                start_new_session=True,
            )

        # Brief wait to catch immediate startup failures
        time.sleep(0.1)
        if process.poll() is not None:
            # Process exited immediately - read last lines of log for error
            update_agent_status(DEFAULT_DB_PATH, agent_id, AgentStatus.ERROR)
            error_msg = "Process exited immediately"
            if log_path.exists():
                lines = log_path.read_text().strip().split("\n")[-5:]
                if lines:
                    error_msg = "\n".join(lines)
            return StartResult(success=False, log_path=log_path, error=error_msg)

        update_agent_status(
            DEFAULT_DB_PATH,
            agent_id,
            AgentStatus.RUNNING,
            pid=process.pid,
        )
        return StartResult(success=True, log_path=log_path)
    else:
        # Run in foreground
        update_agent_status(
            DEFAULT_DB_PATH,
            agent_id,
            AgentStatus.RUNNING,
            pid=os.getpid(),
        )
        try:
            if continuous:
                exit_code = run_agent_continuous(
                    agent,
                    repo_root,
                    check_interval=check_interval,
                    max_iterations=max_iterations,
                )
            else:
                exit_code = run_agent_iteration(agent, repo_root, foreground=True)
            if exit_code != 0:
                return StartResult(success=False, error=f"Agent exited with code {exit_code}")
            return StartResult(success=True)
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
