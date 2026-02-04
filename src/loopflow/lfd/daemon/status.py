"""Shared status computation for lfd daemon."""

import os

from loopflow.lfd.agent import load_agents
from loopflow.lfd.daemon.registration import get_registration_status
from loopflow.lfd.models import WaveStatus
from loopflow.lfd.wave import list_waves


def compute_status() -> dict:
    """Return daemon status dict used by both socket and HTTP servers."""
    waves = list_waves()
    agents = load_agents(active_only=True)
    running_waves = [w for w in waves if w.status == WaveStatus.RUNNING]

    return {
        "pid": os.getpid(),
        "waves_defined": len(waves),
        "waves_running": len(running_waves),
        "agents_active": len(agents),
        "registration": get_registration_status(),
    }
