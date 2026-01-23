"""Shared status computation for lfd daemon."""

import os

from loopflow.lfd.agent import list_agents
from loopflow.lfd.models import AgentStatus
from loopflow.lfd.step_run import load_step_runs


def compute_status() -> dict:
    """Return daemon status dict used by both socket and HTTP servers."""
    agents = list_agents()
    step_runs = load_step_runs(active_only=True)
    running_agents = [a for a in agents if a.status == AgentStatus.RUNNING]

    return {
        "pid": os.getpid(),
        "agents_defined": len(agents),
        "agents_running": len(running_agents),
        "step_runs_active": len(step_runs),
    }
