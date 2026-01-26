"""Shared status computation for lfd daemon."""

import os

from loopflow.lfd.models import WaveStatus
from loopflow.lfd.step_run import load_step_runs
from loopflow.lfd.wave import list_waves


def compute_status() -> dict:
    """Return daemon status dict used by both socket and HTTP servers."""
    waves = list_waves()
    step_runs = load_step_runs(active_only=True)
    running_waves = [w for w in waves if w.status == WaveStatus.RUNNING]

    return {
        "pid": os.getpid(),
        "waves_defined": len(waves),
        "waves_running": len(running_waves),
        "step_runs_active": len(step_runs),
    }
